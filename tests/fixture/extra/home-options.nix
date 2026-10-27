# The declaration module of fixture.home instances, from an input like
# home-manager's own modules: its definitions are declaration-side.
{ lib, name, ... }:
let
  inherit (lib) mkOption types;
in
{
  options = {
    editor = mkOption {
      type = types.str;
      default = "nano";
    };
    theme = mkOption {
      type = types.str;
      default = "light";
    };
    owner = mkOption {
      type = types.str;
      default = name;
    };
    # A submodule inside the submodule.
    files = mkOption {
      type = types.attrsOf (
        types.submodule {
          options.text = mkOption { type = types.str; };
          options.enable = mkOption {
            type = types.bool;
            default = true;
          };
        }
      );
      default = { };
    };
  };
  # A declaration-side definition, counted (not listed) where it loses.
  config.theme = lib.mkOptionDefault "from the declaration module";
}
