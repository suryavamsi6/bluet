import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

// Make a checksum-pinned recipe from the current checkout. This deliberately
// does not depend on a public repository, tag, release, or GitHub credentials.
const project = resolve(import.meta.dir, "..");
const { version } = JSON.parse(
  readFileSync(join(project, "package.json"), "utf8"),
);
if (
  typeof version !== "string" ||
  !/^\d+\.\d+\.\d+(?:[a-zA-Z0-9.+_-]*)$/.test(version)
)
  throw new Error("package.json needs a valid package version");
if (process.platform !== "linux" || process.arch !== "x64")
  throw new Error("The GPUIX Linux package currently supports x86_64 only.");
const work = mkdtempSync(join(tmpdir(), "bluet-arch-"));
const archiveName = `bluet-${version}.tar.gz`;
const archive = join(work, archiveName);
execFileSync(
  "tar",
  [
    "--create",
    "--gzip",
    "--file",
    archive,
    "--directory",
    project,
    "--sort=name",
    "--mtime=@0",
    "--owner=0",
    "--group=0",
    "--numeric-owner",
    "--exclude=./.git",
    "--exclude=./node_modules",
    "--exclude=./dist",
    "--exclude=./artifacts",
    "--exclude=./result",
    "--exclude=./result-*",
    "--exclude=./packaging/arch/src",
    "--exclude=./packaging/arch/pkg",
    "--transform",
    `s,^\\.,bluet-${version},`,
    ".",
  ],
  { stdio: "inherit" },
);
const sha256 = createHash("sha256").update(readFileSync(archive)).digest("hex");
const template = readFileSync(
  join(project, "packaging/arch/PKGBUILD.in"),
  "utf8",
);
const recipe = template
  .replaceAll("@VERSION@", version)
  .replaceAll("@SOURCE_SHA256@", sha256);
writeFileSync(join(work, "PKGBUILD"), recipe);
const output = join(project, "dist", "arch");
mkdirSync(output, { recursive: true });
copyFileSync(archive, join(output, archiveName));
writeFileSync(join(output, "PKGBUILD"), recipe);
const sourceInfo = execFileSync("makepkg", ["--printsrcinfo"], {
  cwd: work,
  encoding: "utf8",
});
writeFileSync(join(output, ".SRCINFO"), sourceInfo);
console.log(`Source archive and checksum-pinned recipe: ${output}`);
console.log(`Build directory: ${work}`);
if (!process.argv.includes("--prepare-only")) {
  execFileSync("makepkg", ["--cleanbuild", "--force", "--noconfirm"], {
    cwd: work,
    stdio: "inherit",
  });
  for (const file of readdirSync(work)) {
    if (file.endsWith(".pkg.tar.zst") || file.endsWith(".pkg.tar.xz")) {
      copyFileSync(join(work, file), join(output, file));
      console.log(`Built ${join(output, file)}`);
    }
  }
}
