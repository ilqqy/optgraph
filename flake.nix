{
  description = "optgraph: NixOS module graph and option provenance extractor";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      toolVersion = "0.1.0+${self.shortRev or self.dirtyShortRev or "unknown"}";
    in
    {
      lib = import ./nix { inherit toolVersion; };

      packages = forAllSystems (pkgs: {
        default = pkgs.callPackage ./cli {
          src = self;
          inherit (self) narHash;
          inherit toolVersion;
        };
      });

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/optgraph";
          meta.description = "Extract the module graph and option definitions of a NixOS configuration";
        };
      });

      checks = forAllSystems (
        pkgs:
        import ./tests {
          inherit pkgs nixpkgs self;
        }
      );

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.gh
            pkgs.jq
            pkgs.nixfmt # nixfmt-rfc-style is a deprecated alias of it
            pkgs.check-jsonschema
          ];
        };
      });

      # nixfmt (RFC style) on .nix files. `nix fmt` passes no arguments, and
      # nixfmt no longer takes directories: both expand to the .nix files below.
      formatter = forAllSystems (
        pkgs:
        pkgs.writeShellApplication {
          name = "optgraph-fmt";
          runtimeInputs = [
            pkgs.findutils
            pkgs.nixfmt
          ];
          text = ''
            flags=() files=() dirs=()
            for a in "$@"; do
              if [ -d "$a" ]; then dirs+=("$a")
              elif [[ $a == -* ]]; then flags+=("$a")
              else files+=("$a"); fi
            done
            [ ''${#files[@]} -gt 0 ] || [ ''${#dirs[@]} -gt 0 ] || dirs=(.)
            if [ ''${#dirs[@]} -gt 0 ]; then
              mapfile -t -O "''${#files[@]}" files < <(find "''${dirs[@]}" -name '*.nix' -not -path '*/.git/*' -type f)
            fi
            exec nixfmt "''${flags[@]}" "''${files[@]}"
          '';
        }
      );
    };
}
