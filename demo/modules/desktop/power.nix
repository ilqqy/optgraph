{ config, lib, ... }:
{
  # demo.features.laptop is on: these definitions are active (mkIf true).
  config = lib.mkIf config.demo.features.laptop {
    services.tlp.enable = true;
    services.tlp.settings = {
      CPU_ENERGY_PERF_POLICY_ON_BAT = "power";
      START_CHARGE_THRESH_BAT0 = 75;
      STOP_CHARGE_THRESH_BAT0 = 80;
    };
    # Demonstrates mkForce beating mkDefault: desktop.nix enables
    # power-profiles-daemon with mkDefault, and it conflicts with TLP.
    services.power-profiles-daemon.enable = lib.mkForce false;
    services.upower.enable = true;
    # Demonstrates mkForce beating mkDefault from another flake: the shared
    # input sets protectKernelImage, which adds the `nohibernate` kernel
    # parameter; laptops keep hibernation.
    security.protectKernelImage = lib.mkForce false;
  };
}
