{ lib, pkgs, ... }:
{
  programs.hyprland.enable = true;
  programs.hyprlock.enable = true;
  services.hypridle.enable = true;

  # Text-mode login that starts Hyprland.
  services.greetd = {
    enable = true;
    settings.default_session.command = "${lib.getExe pkgs.tuigreet} --time --cmd Hyprland";
  };

  environment.sessionVariables.NIXOS_OZONE_WL = "1";
  environment.systemPackages = with pkgs; [
    waybar
    fuzzel
    mako
    grim
    slurp
    wl-clipboard
    brightnessctl
    hyprpolkitagent
  ];
}
