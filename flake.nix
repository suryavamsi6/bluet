{
  description = "Bluet, a native BlueZ Bluetooth manager built with Rust and GPUI";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      package = pkgs.callPackage ./nix/package.nix { };
    in {
      packages.${system} = { default = package; bluet = package; };
      apps.${system}.default = { type = "app"; program = "${package}/bin/bluet"; };
      overlays.default = final: prev: {
        bluet = final.callPackage ./nix/package.nix { };
      };
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          rustc
          rustfmt
          clippy
          pkg-config
          dbus
          pulseaudio
          wayland
          wayland-protocols
          libxkbcommon
          libxcb
          vulkan-loader
          fontconfig
          freetype
          libGL
        ];
      };
      nixosModules.default = import ./nix/module.nix self;
    };
}
