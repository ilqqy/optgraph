{
  config,
  lib,
  pkgs,
  ...
}:
{
  # Demonstrates mkIf false: demo.features.printing is off, so every
  # definition here is listed as inactive (condition mkIf-false).
  config = lib.mkIf config.demo.features.printing {
    services.printing.enable = true;
    services.printing.drivers = [ pkgs.gutenprint ];
    services.avahi = {
      enable = true;
      nssmdns4 = true;
      openFirewall = true;
    };
  };
}
