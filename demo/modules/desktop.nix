# Graphical desktop: common services, and the desktop pieces as imports.
{ lib, ... }:
{
  imports = [
    ./desktop/hyprland.nix
    ./desktop/audio.nix
    ./desktop/fonts.nix
    ./desktop/bluetooth.nix
    ./desktop/power.nix
    ./desktop/printing.nix
    ./desktop/apps.nix
  ];

  hardware.graphics.enable = true;
  services.udisks2.enable = true;
  services.gvfs.enable = true;
  services.libinput.enable = true;

  # A sane default for desktops. desktop/power.nix replaces it with TLP on
  # laptops (mkForce false), so this mkDefault loses.
  services.power-profiles-daemon.enable = lib.mkDefault true;
}
