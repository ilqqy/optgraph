{ pkgs, ... }:
{
  programs.zsh = {
    enable = true;
    autosuggestions.enable = true;
    syntaxHighlighting.enable = true;
  };
  programs.starship.enable = true;

  # Demonstrates a list set in several modules (also dev-tools.nix,
  # desktop/hyprland.nix, desktop/apps.nix, gaming.nix): several winners, and
  # nixpkgs' own definitions counted as "+N nixpkgs".
  environment.systemPackages = with pkgs; [
    eza
    bat
    fzf
    zoxide
  ];
  environment.shellAliases = {
    ls = "eza";
    ll = "eza -l";
  };
}
