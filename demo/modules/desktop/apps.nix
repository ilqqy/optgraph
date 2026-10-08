{ pkgs, ... }:
{
  programs.firefox.enable = true;
  environment.systemPackages = with pkgs; [
    kitty
    nautilus
    mpv
    imv
    zathura
  ];
}
