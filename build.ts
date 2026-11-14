import { createRequire } from "node:module";
import { mkdirSync, readFileSync } from "node:fs";

// NAPI-RS uses dynamic requires; make this platform's addon a static import so
// Bun embeds it in the compiled executable along with the fonts and SVGs.
const require = createRequire(import.meta.url);
const targets: Record<string, string> = {
  "linux-x64": "@gpuix/native-linux-x64-gnu",
  "darwin-arm64": "@gpuix/native-darwin-arm64",
  "win32-x64": "@gpuix/native-win32-x64-msvc",
  "win32-arm64": "@gpuix/native-win32-arm64-msvc",
};
const target = targets[`${process.platform}-${process.arch}`];
if (!target)
  throw new Error(`GPUIX does not ship a native addon for this platform.`);
const addon = require.resolve(target);
const original = readFileSync(
  new URL(import.meta.resolve("@gpuix/native")),
  "utf8",
);
const exports = original
  .slice(original.lastIndexOf("const { AvailableUpdate"))
  .replace("__napiLoadedBindingTarget", "'native'");
mkdirSync("dist", { recursive: true });
const result = await Bun.build({
  entrypoints: ["app.tsx"],
  compile: { outfile: "dist/bluet" },
  // BlueZ uses the system bus's ordinary Unix socket. dbus-next gracefully
  // falls back to node:net without its optional native usocket extension;
  // X11 session-bus discovery is unused by this application.
  external: ["usocket", "x11"],
  plugins: [
    {
      name: "embed-gpuix-native",
      setup(build) {
        build.onLoad({ filter: /@gpuix\/native\/index\.js$/ }, () => ({
          contents: `const nativeBinding = require(${JSON.stringify(addon)});\n${exports}`,
          loader: "js",
        }));
      },
    },
  ],
});
if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
console.log(
  "Built dist/bluet with the native GPUI renderer and bundled assets.",
);
