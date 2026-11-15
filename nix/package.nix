{
  lib,
  rustPlatform,
  makeWrapper,
  pkg-config,
  libxcb,
  libxkbcommon,
  wayland,
  vulkan-loader,
  fontconfig,
  freetype,
  libX11,
  libXcursor,
  libXi,
  libXrandr,
  libGL,
  libglvnd,
  alsa-lib,
  pulseaudio,
}:
let
  version = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).package.version;
  runtimeLibraries = [
    libxcb
    libxkbcommon
    wayland
    vulkan-loader
    fontconfig
    freetype
    libX11
    libXcursor
    libXi
    libXrandr
    libGL
    libglvnd
    alsa-lib
  ];
in
rustPlatform.buildRustPackage {
  pname = "bluet";
  inherit version;

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      let
        rootEntry = builtins.dirOf (toString path) == toString ../.;
        name = baseNameOf (toString path);
      in
      lib.cleanSourceFilter path type
      && !(rootEntry && (
        builtins.elem name [ "target" ".git" "artifacts" "result" ] || lib.hasPrefix "result" name
      ));
  };

  cargoLock.lockFile = ../Cargo.lock;
  cargoTestFlags = [ "--lib" "--bins" ];

  nativeBuildInputs = [ makeWrapper pkg-config ];
  buildInputs = runtimeLibraries;

  postInstall = ''
    wrapProgram $out/bin/bluet \
      --prefix PATH : ${lib.makeBinPath [ pulseaudio ]} \
      --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath runtimeLibraries}:/run/opengl-driver/lib"

    install -Dm644 packaging/linux/dev.bluet.manager.desktop \
      $out/share/applications/dev.bluet.manager.desktop
    install -Dm644 packaging/linux/dev.bluet.manager.svg \
      $out/share/icons/hicolor/scalable/apps/dev.bluet.manager.svg
    install -Dm644 LICENSE-MIT LICENSE-APACHE -t $out/share/licenses/bluet/
    install -Dm644 README.md $out/share/doc/bluet/README.md
    cp -r assets/licenses $out/share/doc/bluet/licenses
  '';

  meta = {
    description = "Native Bluetooth manager for BlueZ";
    license = with lib.licenses; [ mit asl20 ];
    mainProgram = "bluet";
    platforms = [ "x86_64-linux" ];
  };
}
