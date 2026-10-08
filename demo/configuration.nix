{
  imports = [
    ./hardware-configuration.nix
    ./modules/options.nix
    ./modules/core.nix
    ./modules/desktop.nix
    ./modules/dev-tools.nix
    ./modules/gaming.nix
    ./modules/virtualisation.nix
    ./modules/maintenance.nix
    ./modules/local.nix
  ];

  # Feature flags (modules/options.nix). printing and virtualisation are off:
  # their modules' definitions show up as mkIf-false (inactive).
  demo.features = {
    laptop = true;
    gaming = true;
    printing = false;
    virtualisation = false;
  };

  system.stateVersion = "25.11";
}
