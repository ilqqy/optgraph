# Base system: only imports.
{
  imports = [
    ./core/boot.nix
    ./core/locale.nix
    ./core/nix-settings.nix
    ./core/networking.nix
    ./core/users.nix
    ./core/shell.nix
    ./core/hardening.nix
  ];
}
