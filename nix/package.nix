{ lib, stdenv, fetchurl, bun, autoPatchelfHook, makeWrapper, libxcb
, libxkbcommon, wayland, vulkan-loader, fontconfig, freetype, libX11
, libXcursor, libXi, libXrandr, libGL, libglvnd, alsa-lib, pulseaudio }:
let
  locked = import ./deps.nix;
  packages = map (dep: dep // { src = fetchurl { inherit (dep) url hash; }; }) locked;
  unpackPackage = dep: ''
    mkdir -p "node_modules/${dep.path}"
    tar -xzf ${dep.src} -C "node_modules/${dep.path}" --strip-components=1
  '';
  runtimeLibraries = [ stdenv.cc.cc.lib stdenv.cc.libc libxcb libxkbcommon wayland vulkan-loader fontconfig freetype libX11 libXcursor libXi libXrandr libGL libglvnd alsa-lib ];
  sourceRoot = toString ../.;
  sourceFilter = path: type:
    let
      rootEntry = builtins.dirOf (toString path) == sourceRoot;
      name = baseNameOf (toString path);
    in lib.cleanSourceFilter path type && !(rootEntry && (
      builtins.elem name [ "node_modules" "dist" ".git" "artifacts" ] || lib.hasPrefix "result" name
    ));
in stdenv.mkDerivation {
  pname = "bluet";
  version = "1.0.0";
  src = lib.cleanSourceWith { src = ../.; filter = sourceFilter; };
  # Bun executables contain embedded assets that strip/patchelf can corrupt.
  dontStrip = true;
  dontPatchELF = true;
  dontAutoPatchelf = true;
  nativeBuildInputs = [ bun autoPatchelfHook makeWrapper ];
  buildInputs = runtimeLibraries;
  buildPhase = ''
    runHook preBuild
    export HOME="$TMPDIR/home"
    mkdir -p "$HOME" node_modules
    # Fail rather than silently use a stale dependency manifest.
    cp nix/deps.nix "$TMPDIR/deps.expected"
    bun nix/generate-deps.ts
    cmp "$TMPDIR/deps.expected" nix/deps.nix
    ${lib.concatMapStringsSep "\n" unpackPackage packages}
    # build.ts embeds this addon into the executable: patch it before compiling.
    autoPatchelf node_modules/@gpuix/native-linux-x64-gnu
    bun build.ts
    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall
    install -Dm755 dist/bluet "$out/libexec/bluet"
    makeWrapper "$out/libexec/bluet" "$out/bin/bluet" \
      --prefix PATH : ${lib.makeBinPath [ pulseaudio ]} \
      --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath runtimeLibraries}:/run/opengl-driver/lib"
    install -Dm644 packaging/linux/dev.bluet.manager.desktop "$out/share/applications/dev.bluet.manager.desktop"
    install -Dm644 packaging/linux/dev.bluet.manager.svg "$out/share/icons/hicolor/scalable/apps/dev.bluet.manager.svg"
    install -Dm644 packaging/linux/PROJECT-LICENSE-NOTICE "$out/share/doc/bluet/PROJECT-LICENSE-NOTICE"
    install -Dm644 README.md "$out/share/doc/bluet/README.md"
    cp -r assets/licenses "$out/share/doc/bluet/licenses"
    runHook postInstall
  '';
  meta = {
    description = "Native Bluetooth manager for BlueZ";
    mainProgram = "bluet";
    platforms = [ "x86_64-linux" ];
  };
}
