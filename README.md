# Bluet

A native Bluetooth manager built with **Rust and [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui)**, following the connected **BlueT.pen** design. GPUI draws the three-pane interface directly through the GPU, without a browser or Electron.

Bluet connects to the **existing BlueZ system service** through D-Bus using `zbus`. Device names, power, pairing, connection status, battery and trust come from BlueZ. Audio controls use the computer's existing PipeWire/PulseAudio service through `pactl`.

## Run

Requires Rust, a Linux desktop with working Vulkan drivers, and the system's BlueZ `bluetoothd` service. The current desktop user's normal D-Bus/PolicyKit permissions apply. `pactl` enables audio controls when the audio server exposes a Bluetooth card/output.

```sh
cargo run --release
```

Startup only reads service state and subscribes to changes; it does not power on adapters, scan, pair, connect, or change audio.

## Build

```sh
cargo build --release
./target/release/bluet
```

Fonts, icons and dither textures are compiled into the executable. Runtime system services and `pactl` remain supplied by the operating system.

## Verification

```sh
cargo test
dbus-run-session -- sh -c 'DBUS_SYSTEM_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" cargo test --test dbus_integration'
cargo build --release
./scripts/native-smoke.sh
./scripts/native-smoke.sh --capture
./scripts/parity-check.py artifacts/bluet-native.png
```

Unit tests cover device filtering, BlueZ property mapping, audio parsing, the audio command bridge and dither texture generation. The D-Bus integration test runs a fake BlueZ service on a private `dbus-run-session` bus and exercises real marshalling, signals, discovery, connection, trust, removal and pairing-agent confirmation/rejection without touching physical Bluetooth hardware.

The native smoke test launches the real GPUI window against the installed service, verifies it renders and keeps running, compares adapter/device state before and after, and can capture a screenshot under Hyprland with `grim`. It deliberately avoids power, discovery, pairing, connection, trust, removal and audio write controls.

`scripts/parity-check.py` compares a screenshot against the committed reference frame in per-region terms, compensating the small constant window-placement offset of a live capture, so a layout regression fails loudly instead of hiding in the flat backgrounds. Run it on whichever frame `native-smoke.sh --capture` produced.

## Arch Linux package

From a checkout with `base-devel`, `cargo`, `rust`, `pkg-config` and `desktop-file-utils` installed:

```sh
./scripts/package-arch.sh
sudo pacman -U dist/arch/bluet-1.0.0-1-x86_64.pkg.tar.zst
```

The script builds from the current checkout, validates the source archive checksum, runs the unit tests, and produces an installable package. The package includes the desktop launcher and icon, requires BlueZ and the normal graphics libraries, and uses `libpulse` for `pactl`; it does not install a PulseAudio server or change Bluetooth service settings. A Vulkan driver appropriate for the computer is required.

The generated source archive, checksum-pinned `PKGBUILD`, and `.SRCINFO` are also in `dist/arch/`. To prepare those without building, use `./scripts/package-arch.sh --prepare-only`. The tracked recipe template is `packaging/arch/PKGBUILD.in`; no public GitHub release is required.

## Nix and NixOS

The flake supports `x86_64-linux`:

```sh
nix build .#bluet
nix run .#bluet
```

The Nix package builds from source using the dependency versions pinned in `Cargo.lock` and supplies its native Wayland/X11, Vulkan, font and audio libraries. It includes the desktop launcher and icon. System Bluetooth and audio services remain supplied by the operating system.

For a NixOS configuration, add this checkout as a flake input:

```nix
inputs.bluet.url = "path:/absolute/path/to/bluet";
```

Then import the module in a system configuration that receives `inputs`:

```nix
imports = [ inputs.bluet.nixosModules.default ];
programs.bluet.enable = true;
```

The opt-in module installs Bluet and enables `hardware.bluetooth.enable`. It leaves your audio configuration under your control. See [nix/README.md](nix/README.md) for runtime details.

No project license has been specified. Packages include a notice stating that fact and preserve the bundled third-party licenses.

## Controls

- Power and visibility write the selected adapter's actual BlueZ properties.
- Add device starts discovery for this app. Closing the dialog stops this app's discovery session without stopping another client's scan.
- Pairing uses an agent registered by Bluet. PIN, passkey, confirmation and authorization requests appear in the interface; Bluet does not replace the system's default pairing agent.
- Connect, disconnect, forget and trusted-device controls act on the real selected device. Forget removes its pairing keys after confirmation.
- Volume, mute and audio profiles target the selected Bluetooth output/card. Dragging volume commits once on release. Unavailable audio controls show the reason.
- Activity records changes observed during this app session. Unknown battery, signal and connection duration are reported as unavailable rather than estimated.

File transfers are not implemented. The interface shows that limitation and does not claim to send files. The layout, colors, typography and icons follow the Pen reference. Ordered-dither accents add texture to the sidebar, inspector, selected device, device halo, primary buttons, battery meter and pairing panels. Reference exports are in `design/`.

The effects use a Rust adaptation of [dither-kit-rs](https://github.com/DeepanshuMishraa/dither-kit-rs)'s Bayer8 technique and field vocabulary. Textures are generated deterministically, cached as native GPUI images behind text and controls, and use no animation loop. The upstream MIT license is included in `assets/licenses/DitherKit-LICENSE`.

Inter and Geist Mono are bundled under the SIL Open Font License. Lucide icons use the ISC license. Dependency and asset license texts are in `assets/licenses/`.
