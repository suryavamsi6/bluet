# Nix

Bluet supports x86_64 Linux, matching the available GPUIX native addon. From a checkout with Nix flakes enabled:

```sh
nix build .#bluet
nix run .#bluet
```

The build fetches the exact runtime dependency tarballs listed in `bun.lock` using their integrity hashes, patches the GPUIX native addon for Nix, and compiles from source. After changing `bun.lock`, run `bun nix/generate-deps.ts` and commit `nix/deps.nix` with it. A stale generated manifest fails the build.

For NixOS, add this flake as an input, import `inputs.bluet.nixosModules.default`, and set `programs.bluet.enable = true;`. This installs Bluet and enables the system Bluetooth service. Audio configuration remains under your control. The app talks to the system BlueZ daemon and uses `pactl` if available for audio volume controls. Graphics drivers are loaded from the NixOS system driver path.

No license has been specified for the Bluet project. Third-party notices are installed alongside the package.
