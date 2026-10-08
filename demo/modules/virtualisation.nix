{
  config,
  lib,
  pkgs,
  ...
}:
{
  # Demonstrates mkIf false: demo.features.virtualisation is off, so these
  # definitions are listed as inactive, next to the active ones of the same
  # options from other modules (users.users, environment.systemPackages).
  config = lib.mkIf config.demo.features.virtualisation {
    virtualisation.podman = {
      enable = true;
      dockerCompat = true;
      defaultNetwork.settings.dns_enabled = true;
    };
    users.users.demo.extraGroups = [ "podman" ];
    environment.systemPackages = [ pkgs.podman-compose ];
  };
}
