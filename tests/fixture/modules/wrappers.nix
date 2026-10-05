{ lib, pkgs, ... }:
{
  # mkMerge at the leaf: three definitions from one file.
  fixture.merged = lib.mkMerge [
    (lib.mkDefault [ "a" ])
    [ "b" ]
    (lib.mkIf false [ "c" ])
  ];

  # mkDefinition: its own file, and an override inside.
  fixture.viaDefinition = lib.mkDefinition {
    file = "/virtual/defined-here.nix";
    value = lib.mkForce "via mkDefinition";
  };

  # Preview edge cases: derivation, function, long string, long list.
  fixture.previews = {
    drv = pkgs.hello;
    fn = x: x;
    list = lib.range 1 20;
    long = lib.concatStrings (lib.genList (_: "a") 200);
  };
}
