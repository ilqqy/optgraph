{
  config,
  lib,
  pkgs,
  ...
}:
{
  # demo.features.gaming is on: these definitions are active (mkIf true).
  config = lib.mkIf config.demo.features.gaming {
    programs.steam.enable = true;
    programs.gamemode.enable = true;
    hardware.graphics.enable32Bit = true;
    services.pipewire.alsa.support32Bit = true;

    environment.systemPackages = with pkgs; [
      mangohud
      protonup-qt
    ];
    users.users.demo.extraGroups = [ "gamemode" ];

    nixpkgs.config.allowUnfreePredicate =
      pkg:
      builtins.elem (lib.getName pkg) [
        "steam"
        "steam-unwrapped"
      ];
  };
}
