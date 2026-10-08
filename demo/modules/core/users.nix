{ pkgs, ... }:
{
  # gaming.nix and virtualisation.nix add groups to the same user.
  users.users.demo = {
    isNormalUser = true;
    description = "Demo User";
    extraGroups = [
      "wheel"
      "networkmanager"
      "video"
    ];
    shell = pkgs.zsh;
  };
}
