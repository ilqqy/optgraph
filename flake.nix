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

      inherit (nixpkgs) lib;

      # Every package's source is limited to the files it reads, so commits
      # that touch only README, docs/, tests/, demo/ or .github/ leave their
      # store paths (and a binary cache's hits) unchanged.
      # The CLI's source: its scripts and the Nix lib it imports at run time.
      cliSrc = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./cli
          ./nix
        ];
      };
      # The extractor's code, not the commit: the first characters of cliSrc's
      # (content-addressed) store hash. A rev would change every package that
      # embeds the version with every commit.
      toolVersion = "0.1.0+${
        builtins.substring 0 7 (baseNameOf (builtins.unsafeDiscardStringContext cliSrc.outPath))
      }";
    in
    {
      lib = import ./nix { inherit toolVersion; };

      packages = forAllSystems (pkgs: rec {
        default = pkgs.callPackage ./cli {
          src = cliSrc;
          inherit toolVersion viewer;
        };
        # The native viewer (app/); runs the CLI above for flake references.
        optgraph-app = pkgs.callPackage ./app/package.nix {
          src = ./.;
          optgraph = default;
          # The crate's version: the vendored dependencies (a fixed-output
          # derivation named after it) then survive commits.
          version = "0.1.0";
        };
        # Single-file viewer: result/index.html (viewer/build.sh). `./viewer`
        # is copied on its own, so only viewer/ affects it.
        viewer = pkgs.runCommand "optgraph-viewer" { } ''
          bash ${./viewer/build.sh} ${./viewer} $out
        '';
      });

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/optgraph";
          meta.description = "Extract the module graph and option definitions of a NixOS configuration";
        };
        app = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.optgraph-app}/bin/optgraph-app";
          meta.description = "Native viewer: who set an option, at what priority, and why the others lost";
        };
      });

      checks = forAllSystems (
        pkgs:
        import ./tests {
          inherit pkgs nixpkgs self;
        }
        // {
          optgraph-core = import ./app/core-tests.nix { inherit pkgs; };
        }
      );

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.gh
            pkgs.jq
            pkgs.nixfmt # nixfmt-rfc-style is a deprecated alias of it
            pkgs.check-jsonschema
            pkgs.ffmpeg-headless # tools/render-tour.sh (ffmpeg without SDL/GUI deps)
            pkgs.python3 # tools/tour-capture.py (standard library only)
          ];
        };
        # Native app (app/): Rust, GPUI's build and runtime libraries, and a
        # headless Wayland compositor with software Vulkan for screenshots
        # (tools/native-shot.sh).
        native = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.clippy
            pkgs.rustfmt
            pkgs.pkg-config
            pkgs.jq
            pkgs.cage
            pkgs.weston
            pkgs.grim
            pkgs.slurp
            pkgs.wlr-randr
            pkgs.wtype
            pkgs.wlrctl
            (pkgs.python3.withPackages (p: [ p.pywayland ])) # tools/native-pointer.py
            pkgs.vulkan-tools
            pkgs.wayland-utils
          ];
          buildInputs = pkgs.callPackage ./app/runtime-libs.nix { };
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (pkgs.callPackage ./app/runtime-libs.nix { });
          # The virtual pointer protocol, for tools/native-pointer.py.
          OPTGRAPH_WLR_PROTOCOLS = "${pkgs.wlr-protocols}/share/wlr-protocols";
          OPTGRAPH_WAYLAND_XML = "${pkgs.wayland-scanner.out}/share/wayland/wayland.xml";
          # Mesa's lavapipe (software Vulkan), used by tools/native-shot.sh.
          OPTGRAPH_LAVAPIPE_ICD = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.${pkgs.stdenv.hostPlatform.uname.processor}.json";
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
