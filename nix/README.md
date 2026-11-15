# Nix

Bluet supports x86_64 Linux. From a checkout with Nix flakes enabled:

```sh
nix build .#bluet
nix run .#bluet
```

The package builds the Rust/GPUI binary from source with the dependency
versions pinned in `Cargo.lock`. It supplies the native Wayland/X11, Vulkan,
font and audio libraries and makes `pactl` available for audio controls. The
system BlueZ and audio services remain supplied by the operating system.

For NixOS, add this flake as an input, import
`inputs.bluet.nixosModules.default`, and set `programs.bluet.enable = true;`.
This installs Bluet and enables the system Bluetooth service. Audio
configuration remains under your control. Graphics drivers are loaded from the
NixOS system driver path.

No license has been specified for the Bluet project. Third-party notices are
installed alongside the package.
