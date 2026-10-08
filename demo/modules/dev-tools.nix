{ lib, pkgs, ... }:
{
  programs.git.enable = true;
  programs.direnv.enable = true;
  documentation.dev.enable = true;

  environment.systemPackages = with pkgs; [
    ripgrep
    fd
    jq
    neovim
    gnumake
  ];

  # Handy for local dev servers, but core/hardening.nix overrides it with
  # mkForce: this mkDefault loses.
  networking.firewall.enable = lib.mkDefault false;
}
