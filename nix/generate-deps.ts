// Regenerate after changing bun.lock: bun nix/generate-deps.ts
import { writeFileSync } from "node:fs";
const lock = Bun.JSONC.parse(await Bun.file("bun.lock").text()) as any;
const packages = lock.packages as Record<string, any[]>;
const selected = new Set<string>();
function visit(key: string) {
  if (selected.has(key)) return;
  const entry = packages[key];
  if (!entry) throw new Error(`Missing locked dependency: ${key}`);
  selected.add(key);
  for (const dependency of Object.keys(entry[2].dependencies || {})) {
    const nested = `${key}/${dependency}`;
    visit(packages[nested] ? nested : dependency);
  }
}
for (const name of [
  "@gpuix/native",
  "@gpuix/native-linux-x64-gnu",
  "@gpuix/react",
  "dbus-next",
  "react",
])
  visit(name);
const output = [...selected].sort().map((key) => {
  const [identity, , , integrity] = packages[key];
  const split = identity.lastIndexOf("@");
  const name = identity.slice(0, split),
    version = identity.slice(split + 1);
  const filename = name.split("/").pop();
  const path = key.startsWith("@")
    ? key
    : key.replaceAll("/", "/node_modules/");
  const url = `https://registry.npmjs.org/${name}/-/${filename}-${version}.tgz`;
  return `  { path = ${JSON.stringify(path)}; url = ${JSON.stringify(url)}; hash = ${JSON.stringify(integrity)}; }`;
});
writeFileSync(
  "nix/deps.nix",
  `# Generated from bun.lock by nix/generate-deps.ts; do not edit.\n[\n${output.join("\n")}\n]\n`,
);
