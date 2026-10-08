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

  # The viewer is one self-contained file: no external scripts or styles.
  viewer =
    let
      viewer = self.packages.${pkgs.stdenv.hostPlatform.system}.viewer;
    in
    pkgs.runCommand "optgraph-viewer-check" { } ''
      cd ${viewer}
      [ "$(ls -A | tr '\n' ' ')" = "index.html " ] || { echo "expected only index.html, got: $(ls -A)"; exit 1; }
      if grep -nEi '<(script|link|img|iframe)[^>]*(src|href)=["'"'"']?(https?:)?//' index.html; then
        echo "external reference in the viewer"; exit 1
      fi
      if grep -nEi '@import|url\((["'"'"'])?(https?:)?//' index.html; then
        echo "external stylesheet reference in the viewer"; exit 1
      fi
      [ "$(grep -c 'id="optgraph-data">/\*OPTGRAPH_DATA\*/null</script>' index.html)" = 1 ] || { echo "data placeholder missing"; exit 1; }
      echo "viewer: $(wc -c < index.html) bytes, one file, no external references"
      touch $out
    '';

  # optgraph --html: the fixture graph embedded into the viewer round-trips.
  viewer-embed =
    let
      pkg = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
    in
    pkgs.runCommand "optgraph-viewer-embed-check" { nativeBuildInputs = [ pkgs.jq ]; } ''
      ${pkg.embedHtml}/bin/optgraph-embed-html ${
        self.packages.${pkgs.stdenv.hostPlatform.system}.viewer
      }/index.html ${graphJson} page.html
      tr -d '\n' < page.html | sed -n 's#.*<script type="application/json" id="optgraph-data">\([^<]*\)</script>.*#\1#p' > embedded.json
      jq -e . embedded.json > /dev/null
      [ "$(jq -S . embedded.json)" = "$(jq -S . ${graphJson})" ] || { echo "embedded JSON differs from the input"; exit 1; }
      [ "$(grep -c '<script' page.html)" = 3 ] || { echo "unexpected number of script elements"; exit 1; }
      echo "viewer-embed: $(wc -c < page.html) bytes, embedded JSON valid and identical"
      touch $out
    '';

  # Building the package runs shellcheck on the CLI.
  cli = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
}
