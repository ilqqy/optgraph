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

  # The demo configuration (demo/), built the same way with this flake's nixpkgs.
  shared = (import ../demo/shared/flake.nix).outputs { self = shared; } // {
    outPath = "${self}/demo/shared";
  };
  demo = (import ../demo/flake.nix).outputs {
    self = demo;
    inherit nixpkgs shared;
  };
  demoConfig = demo.nixosConfigurations.demo;

  demoGraphJson = pkgs.writeText "demo-graph.json" (
    builtins.toJSON (
      self.lib.extract {
        config = demoConfig;
        selfRoot = "${self}/demo";
        inputs = {
          nixpkgs = nixpkgs.outPath;
          shared = "${self}/demo/shared";
        };
        host = "demo";
        generatedAt = "1970-01-01T00:00:00Z";
      }
    )
  );

  # The demo is clean, and the options the README links to tell their story.
  demoAssertions = pkgs.writeText "demo-assertions.jq" ''
    def opt($p): first(.options[] | select(.path == $p));
    .meta.complete
    and .meta.warnings == []
    and all(.options[]; .error == null)
    and (opt("networking.firewall.enable") | .highestPrio == 50 and ([.definitions[] | select(.kind == "definition") | .priority] | sort) == [50, 1000])
    and (opt("services.printing.enable") | any(.definitions[]; .condition == "mkIf-false"))
    and (opt("environment.systemPackages") | (.winners | length) > 1 and .omitted.nixpkgsActive > 0)
    and (opt("i18n.defaultLocale") | .definitions[-1].kind == "default" and .winners == [0])
  '';
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

  # The demo system evaluates (its toplevel derivation is instantiated, not
  # built), and its extraction validates, with no warnings and no option
  # errors. pages.yml publishes the same extraction (by the CLI) as demo.json.
  demo =
    pkgs.runCommand "optgraph-demo-check"
      {
        nativeBuildInputs = [
          pkgs.check-jsonschema
          pkgs.jq
        ];
        toplevel = builtins.unsafeDiscardStringContext demoConfig.config.system.build.toplevel.drvPath;
      }
      ''
        echo "demo: system $toplevel"
        check-jsonschema --schemafile ${../schema/graph.schema.json} ${demoGraphJson}
        jq -e -f ${demoAssertions} ${demoGraphJson} > /dev/null || {
          echo "demo graph: warnings, option errors or a broken demo story:"
          jq -c '{warnings: .meta.warnings, errors: [.options[] | select(.error != null) | {path, error}]}' ${demoGraphJson}
          exit 1
        }
        jq '{modules: (.modules | length), options: (.options | length)}' ${demoGraphJson}
        cp ${demoGraphJson} $out
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
