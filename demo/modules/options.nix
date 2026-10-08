# Feature flags of this configuration, set in configuration.nix.
{ lib, ... }:
{
  options.demo.features = {
    laptop = lib.mkEnableOption "laptop power management (TLP instead of power-profiles-daemon)";
    gaming = lib.mkEnableOption "Steam and gamemode";
    printing = lib.mkEnableOption "CUPS printing with network printer discovery";
    virtualisation = lib.mkEnableOption "Podman containers";
  };
}
