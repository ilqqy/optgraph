# fixture.home emulates home-manager.users, declared by an input like
# home-manager's NixOS module: attrsOf (submoduleWith) without
# shorthandOnlyDefinesConfig, so an instance value is a module (it may have
# `imports`), with an `osConfig` special argument.
{ lib, config, ... }:
{
  options.fixture.home = lib.mkOption {
    type = lib.types.attrsOf (
      lib.types.submoduleWith {
        modules = [ ./home-options.nix ];
        specialArgs.osConfig = config;
      }
    );
    default = { };
  };
}
