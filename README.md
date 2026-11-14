# Bluet

A native Bluetooth manager built with [GPUIX](https://github.com/remorses/gpuix), React and TypeScript, following the connected **BlueT.pen** design. GPUI draws the three-pane interface directly through the GPU, without a browser or Electron.

Bluet connects to the **existing BlueZ system service** through D-Bus. Device names, power, pairing, connection status, battery and trust come from BlueZ. Audio controls use the computer's existing PipeWire/PulseAudio service through `pactl`.

## Run

Requires Bun, a Linux desktop with working Vulkan drivers, and the system's BlueZ `bluetoothd` service. The current desktop user's normal D-Bus/PolicyKit permissions apply. `pactl` enables audio controls when the audio server exposes a Bluetooth card/output.

```sh
bun install
bun start
```

Use `bun dev` for hot reload. Startup only reads service state and subscribes to changes; it does not power on adapters, scan, pair, connect, or change audio.

## Build

```sh
bun run build
./dist/bluet
```

The build embeds GPUI's native addon, fonts, icons and JavaScript in the executable. The built Linux x64 executable has been launched against the installed BlueZ service. Runtime system services and `pactl` remain supplied by the operating system.

## Arch Linux package

From a checkout with `base-devel`, `bun`, and `desktop-file-utils` installed:

```sh
bun run package:arch
sudo pacman -U dist/arch/bluet-1.0.0-1-x86_64.pkg.tar.zst
```

This builds from the current checkout, validates the source archive checksum, runs the unit tests, and produces an installable package. The package includes the desktop launcher and icon, requires BlueZ and the normal graphics libraries, and uses `libpulse` for `pactl`; it does not install a PulseAudio server or change Bluetooth service settings. A Vulkan driver appropriate for the computer is required.

The generated source archive, checksum-pinned `PKGBUILD`, and `.SRCINFO` are also in `dist/arch/`. To prepare those without building, use `bun run package:arch --prepare-only`. The tracked recipe template is `packaging/arch/PKGBUILD.in`; no public GitHub release is required.

## Nix and NixOS

The flake supports `x86_64-linux`:

```sh
nix build .#bluet
nix run .#bluet
```

The Nix package builds from source using the exact dependency hashes in `bun.lock`, patches GPUI's native addon before embedding it, and supplies its runtime libraries and `pactl`. It includes the desktop launcher and icon. System Bluetooth and audio services remain supplied by the operating system.

For a NixOS configuration, add this checkout as a flake input:

```nix
inputs.bluet.url = "path:/absolute/path/to/bluet";
```

Then import the module in a system configuration that receives `inputs`:

```nix
imports = [ inputs.bluet.nixosModules.default ];
programs.bluet.enable = true;
```

The opt-in module installs Bluet and enables `hardware.bluetooth.enable`. It leaves your audio configuration under your control. See [nix/README.md](nix/README.md) for dependency maintenance and runtime details.

No project license has been specified. Packages include a notice stating that fact and preserve the bundled third-party licenses.

## Controls

- Power and visibility write the selected adapter's actual BlueZ properties.
- Add device starts discovery for this app. Closing the dialog stops this app's discovery session without stopping another client's scan.
- Pairing uses an agent registered for Bluet's request. PIN, passkey, confirmation and authorization requests appear in the interface; Bluet does not replace the system's default pairing agent.
- Connect, disconnect, forget and trusted-device controls act on the real selected device. Forget removes its pairing keys after confirmation.
- Volume, mute and audio profiles target the selected Bluetooth output/card. Dragging volume commits once on release. Unavailable audio controls show the reason.
- Activity records changes observed during this app session. Unknown battery, signal and connection duration are reported as unavailable rather than estimated.

File transfers are not implemented. The interface shows that limitation and does not claim to send files. The layout, colors, typography and icons follow the Pen reference. Ordered-dither accents add texture to the sidebar, inspector, selected device, device halo, primary buttons, battery meter and pairing panels. Reference exports are in `design/`.

The effects use a small TypeScript adaptation of [dither-kit-rs](https://github.com/DeepanshuMishraa/dither-kit-rs)'s Bayer8 technique and field vocabulary. Transparent PNG textures are painted by native GPUI image layers behind text and controls. Textures are deterministic and cached, update after a resize settles, and use no animation loop. Button texture density changes on hover. The upstream MIT license is included in `assets/licenses/DitherKit-LICENSE`.

## Verification

```sh
bun run typecheck
bun test
bun run test:dbus
bun run test:native
bun run test:native --binary --capture
```

Unit tests use injected fake BlueZ/audio transports. `test:dbus` runs a fake BlueZ service on a private `dbus-run-session` bus, testing real D-Bus marshalling and pairing-agent callbacks without accessing physical Bluetooth hardware.

The native smoke test opens a real GPUI window against the installed service. It checks actual device data, search, filters and navigation, and verifies that Bluetooth hardware state is unchanged. It deliberately avoids power, discovery, pairing, connection, trust, removal and audio write controls. Add `--binary` to check the packaged executable, and `--capture` to capture screenshots under Hyprland with grim.

Inter and Geist Mono are bundled under the SIL Open Font License. Lucide icons use the ISC license. Dependency and asset license texts are in `assets/licenses/`.
