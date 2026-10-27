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
    packages = mkOption {
      type = types.listOf types.str;
      default = [ ];
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
  # Declaration-side definitions, counted (not listed): theme loses where
  # an instance sets it; the two packages definitions win (a list merges
  # them), so an instance whose own packages lose has no listed winner.
  config = {
    theme = lib.mkOptionDefault "from the declaration module";
    packages = lib.mkMerge [
      [ "base" ]
      [ "extra" ]
    ];
  };
}
