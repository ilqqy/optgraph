{
  services.fwupd.enable = true;
  services.fstrim.enable = true;
  zramSwap.enable = true;
  # Plain definition beating the nixpkgs default (50).
  zramSwap.memoryPercent = 25;
}
