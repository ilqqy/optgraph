# Submodule-typed options for the nested attribution cases (nested*.nix);
# fixture.home (like home-manager.users) is declared by the extra input.
{ lib, ... }:
let
  inherit (lib) mkOption types;
  # Like users.users: attrset values are pure config (types.submodule).
  account = types.submodule (
    { name, ... }:
    {
      options = {
        shell = mkOption {
          type = types.str;
          default = "sh";
        };
        greeting = mkOption {
          type = types.str;
          default = "hello ${name}";
        };
        groups = mkOption {
          type = types.listOf types.str;
          default = [ ];
        };
      };
    }
  );
in
{
  options.fixture = {
    accounts = mkOption {
      type = types.attrsOf account;
      default = { };
    };
    lazyAccounts = mkOption {
      type = types.lazyAttrsOf account;
      default = { };
    };
    ordered = mkOption {
      type = types.attrsOf account;
      default = { };
    };
    forced = mkOption {
      type = types.attrsOf account;
      default = { };
    };
    single = mkOption {
      type = account;
      default = { };
    };
    maybe = mkOption {
      type = types.nullOr account;
      default = null;
    };
  };
}
