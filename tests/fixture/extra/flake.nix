{
  description = "optgraph test fixture: second local input";

  outputs =
    { ... }:
    {
      nixosModules.default = ./module.nix;
      # An anonymous module defined in this input's flake.nix.
      nixosModules.inline = { lib, ... }: {
        fixture.fromExtraInline = lib.mkDefault "from an inline module of the extra input";
      };
    };
}
