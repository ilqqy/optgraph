{
  imports = [
    ./modules/options.nix
    ./modules/prio-default.nix
    ./modules/prio-plain.nix
    ./modules/prio-force.nix
    ./modules/conditional.nix
    ./modules/empty.nix
    ./modules/users.nix
    ./modules/users-force.nix
    ./modules/services.nix
    ./modules/broken.nix
    ./modules/disabled.nix
    ./modules/disabler.nix
    ./modules/layers.nix
    ./modules/wrappers.nix
    ./modules/lazy.nix
    ./modules/secrets.nix
  ];

  boot.loader.grub.enable = false;
  fileSystems."/" = {
    device = "/dev/vda";
    fsType = "ext4";
  };
  networking.hostName = "fixture";
  nixpkgs.hostPlatform = "x86_64-linux";
  system.stateVersion = "25.11";
}
