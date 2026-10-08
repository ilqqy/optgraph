{
  # Merged with the shared input's experimental-features.
  nix.settings = {
    auto-optimise-store = true;
    trusted-users = [ "@wheel" ];
  };
  nix.gc = {
    automatic = true;
    dates = "weekly";
    options = "--delete-older-than 14d";
  };
}
