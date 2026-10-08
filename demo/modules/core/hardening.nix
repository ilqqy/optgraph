{ lib, ... }:
{
  # Demonstrates mkForce beating mkDefault: dev-tools.nix turns the firewall
  # off with mkDefault, this keeps it on whatever other modules say.
  networking.firewall.enable = lib.mkForce true;

  # Demonstrates mkForce beating mkDefault from another flake: the shared
  # input enables sshd with mkDefault, this machine never runs it.
  services.openssh.enable = lib.mkForce false;

  # No editing of the kernel command line at the boot menu (nixpkgs default: true).
  boot.loader.systemd-boot.editor = false;

  security.sudo.execWheelOnly = true;
  security.polkit.enable = true;
}
