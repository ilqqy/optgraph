{ lib, pkgs, ... }:
{
  # A list option with a user definition next to nixpkgs ones: with several
  # winners only the user's is listed, nixpkgs winners are counted.
  environment.pathsToLink = [ "/share/fixture" ];

  # A list option where the user's mkDefault loses to nixpkgs' normal-priority
  # definitions: every winner is an omitted nixpkgs definition, so `winners`
  # is empty and `omitted.nixpkgsWinners` says how many win.
  environment.systemPackages = lib.mkDefault [ pkgs.hello ];
}
