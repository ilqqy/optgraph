# Checks run by `nix flake check`. The extraction lib is exercised directly on
# the fixture (no CLI): the fixture's outputs are called with this flake's
# nixpkgs, so `--override-input nixpkgs ...` tests other nixpkgs releases.
# The CLI is covered by tests/e2e.sh (a CI step).
{
  pkgs,
  nixpkgs,
  self,
}:
let
  extra = (import ./fixture/extra/flake.nix).outputs { self = extra; } // {
    outPath = "${self}/tests/fixture/extra";
  };
  fixture = (import ./fixture/flake.nix).outputs {
    self = fixture;
    inherit nixpkgs extra;
  };

  graph = self.lib.extract {
    config = fixture.nixosConfigurations.test;
    selfRoot = "${self}/tests/fixture";
    inputs = {
      nixpkgs = nixpkgs.outPath;
      extra = "${self}/tests/fixture/extra";
    };
    # What the CLI ends up passing after the fixture's uncatchable crashes.
    exclude = [
      [
        "fixture"
        "aborting"
      ]
    ];
    noPreview = [
      [
        "fixture"
        "whnfAbort"
      ]
    ];
    noSelfcheck = [
      [
        "fixture"
        "whnfAbort"
      ]
    ];
    host = "test";
    generatedAt = "1970-01-01T00:00:00Z";
  };

  graphJson = pkgs.writeText "graph.json" (builtins.toJSON graph);
in
{
  fixture =
    pkgs.runCommand "optgraph-fixture-check"
      {
        nativeBuildInputs = [
          pkgs.check-jsonschema
          pkgs.jq
        ];
      }
      ''
        check-jsonschema --schemafile ${../schema/graph.schema.json} ${graphJson}
        jq -r -f ${./assertions.jq} ${graphJson}
        jq '{modules: (.modules | length), options: (.options | length), warnings: [.meta.warnings[].code]}' ${graphJson}
        cp ${graphJson} $out
      '';

  # Building the package runs shellcheck on the CLI.
  cli = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
}
