# Defaults every machine starts from. Mostly mkDefault, so hosts can override
# them without mkForce; the demo host still forces two of them.
{ lib, pkgs, ... }:
{
  # Demonstrates mkForce beating mkDefault: modules/core/hardening.nix turns
  # sshd off with mkForce.
  services.openssh.enable = lib.mkDefault true;
  services.openssh.settings.PasswordAuthentication = false;

  # Demonstrates mkForce beating mkDefault: this adds the `nohibernate` kernel
  # parameter, and modules/desktop/power.nix forces it off on laptops.
  security.protectKernelImage = lib.mkDefault true;

  # modules/core/locale.nix overrides this with a plain definition.
  time.timeZone = lib.mkDefault "UTC";

  # Merged with modules/core/nix-settings.nix and nixpkgs' own settings.
  nix.settings.experimental-features = [
    "nix-command"
    "flakes"
  ];

  # Inside a submodule instance: modules/core/users.nix sets the demo user's
  # shell with a plain definition, which beats this mkDefault.
  users.users.demo.shell = lib.mkDefault pkgs.bash;
}
