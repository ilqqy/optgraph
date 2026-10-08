{
  description = "optgraph demo: defaults shared by all machines (second local input)";

  outputs =
    { ... }:
    {
      nixosModules.default = ./base.nix;
    };
}
