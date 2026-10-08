{ lib, ... }:
let
  str = lib.mkOption { type = lib.types.str; };
in
{
  options.fixture = {
    prio = lib.mkOption {
      type = lib.types.str;
      default = "from the option default";
    };
    conditional = lib.mkOption {
      type = lib.types.int;
      default = 0;
    };
    conditionalTrue = str;
    conditionalError = str;
    layered = str;
    merged = lib.mkOption { type = lib.types.listOf lib.types.str; };
    viaDefinition = str;
    previews = lib.mkOption { type = lib.types.attrsOf lib.types.anything; };
    fromDisabled = str;
    fromInterpolated = str;
    nestedInline = str;
    lazy = lib.mkOption { type = lib.types.attrsOf lib.types.anything; };
    packageSet = lib.mkOption { type = lib.types.raw; };
    apiToken = str;
    secretEnabled = lib.mkOption { type = lib.types.bool; };
    inlineAttrs = str;
    inlineFunction = str;
    fromExtra = str;
    fromExtraInline = str;
    throws = str;
    aborting = str;
    whnfAbort = str;
  };
}
