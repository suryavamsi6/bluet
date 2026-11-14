self: { config, lib, pkgs, ... }:
let cfg = config.programs.bluet;
in {
  options.programs.bluet.enable = lib.mkEnableOption "Bluet and the system Bluetooth service";
  config = lib.mkIf cfg.enable {
    hardware.bluetooth.enable = true;
    environment.systemPackages = [ self.packages.${pkgs.stdenv.hostPlatform.system}.default ];
  };
}
