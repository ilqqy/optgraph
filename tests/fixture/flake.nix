{
  description = "optgraph test fixture";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/c59305bab2065cfecc4944690d9eedbb56f3a9fa";
    extra.url = "path:./extra";
  };

  outputs =
    { nixpkgs, extra, ... }:
    {
      nixosConfigurations.test = nixpkgs.lib.nixosSystem {
        modules = [
          ./configuration.nix
          extra.nixosModules.default
          extra.nixosModules.inline
          # modulesIndex 3: inline attrset module, with a nested anonymous import
          {
            fixture.inlineAttrs = "from an inline attrset module";
            imports = [ { fixture.nestedInline = "from an anonymous module inside an inline one"; } ];
          }
          # modulesIndex 4: inline function module
          ({ lib, ... }: { fixture.inlineFunction = lib.mkDefault "from an inline function module"; })
          # A module key with string context
          "${extra}/interpolated.nix"
        ];
      };
    };
}
