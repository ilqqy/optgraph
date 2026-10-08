{
  hardware.bluetooth = {
    enable = true;
    # Plain definition beating the nixpkgs default (true).
    powerOnBoot = false;
    settings.General.Experimental = true;
  };
  services.blueman.enable = true;
}
