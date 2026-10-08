# Synthetic desktop configuration for the README and the live demo
# (demo/graph.json, published as demo.json). Nothing here comes from a real
# machine: boot and file systems are stubs, the only user is "demo".
#
#   nix run . -- ./demo#nixosConfigurations.demo -o demo/graph.json    # from the repo root
{
  description = "optgraph demo: a synthetic Hyprland desktop";

  inputs = {
    # Same rev as the root flake's nixpkgs.
    nixpkgs.url = "github:NixOS/nixpkgs/c59305bab2065cfecc4944690d9eedbb56f3a9fa";
    # Second local input: defaults shared by all machines (origin input:shared).
    shared.url = "path:./shared";
  };

  outputs =
    { nixpkgs, shared, ... }:
    {
      nixosConfigurations.demo = nixpkgs.lib.nixosSystem {
        modules = [
          ./configuration.nix
          shared.nixosModules.default
          # Inline module (modulesIndex 2). Demonstrates a plain user
          # definition beating the nixpkgs default of networking.hostName.
          { networking.hostName = "demo"; }
        ];
      };
    };
}
