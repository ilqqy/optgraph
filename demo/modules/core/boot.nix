{
  boot.loader.systemd-boot.enable = true;
  boot.loader.systemd-boot.configurationLimit = 10;
  boot.loader.efi.canTouchEfiVariables = true;
  # Demonstrates a plain definition beating the nixpkgs default (5).
  boot.loader.timeout = 2;

  # Quiet graphical boot. kernelParams is a list: merged with the parameters
  # nixpkgs adds itself.
  boot.initrd.systemd.enable = true;
  boot.initrd.verbose = false;
  boot.consoleLogLevel = 3;
  boot.plymouth.enable = true;
  boot.kernelParams = [
    "quiet"
    "splash"
  ];
}
