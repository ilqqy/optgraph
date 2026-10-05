# Assertions over the fixture's graph.json. Prints a summary; any failure is an
# error (jq exits 5). Shared by checks.fixture (lib, no CLI) and tests/e2e.sh.
#
#   jq -r -f tests/assertions.jq graph.json
#
# Every check yields exactly one result: an expression that produces no value
# (e.g. a missing option) or an error counts as a failure, never as a pass.

def check($name; f): {name: $name, ok: ([try f catch false] | length > 0 and all(. == true))};

def opt($p): first(.options[] | select(.path == $p));
def hasOpt($p): any(.options[]; .path == $p);
def defsFrom($p; $suffix): [opt($p).definitions[] | select((.file // "") | endswith($suffix))];
def defIn($p; $suffix): defsFrom($p; $suffix) | first;
def idx($p; $suffix): opt($p).definitions | map((.file // "") | endswith($suffix)) | index(true);
def mod($id): first(.modules[] | select(.id == $id));
def modOfDef($p; $i): opt($p).definitions[$i].module as $m | mod($m);
def warning($code; $subject): any(.meta.warnings[]; .code == $code and .subject == $subject);
def codes: [.meta.warnings[].code];
def defaultIdx($p): opt($p).definitions | map(.kind == "default") | index(true);

[
  # --- meta
  check("meta.complete"; .meta.complete == true),
  check("meta.scope is user"; .meta.scope == "user"),
  check("meta.host"; .meta.host == "test"),
  check("nixosSystem injection recognised (attribution aligned)"; .meta.attribution == "aligned"),
  check("no alignment-failed warning"; codes | index("alignment-failed") | not),
  check("no graph-mismatch warning"; codes | index("graph-mismatch") | not),
  check("no reconstruction-mismatch warning"; codes | index("reconstruction-mismatch") | not),
  check("no module-error warning"; codes | index("module-error") | not),
  check("injected nixpkgs.flake.source stays out of scope"; hasOpt("nixpkgs.flake.source") | not),

  # --- priorities: mkDefault vs plain vs mkForce on one option
  check("prio: mkDefault def has priority 1000"; defIn("fixture.prio"; "/modules/prio-default.nix").priority == 1000),
  check("prio: plain def has priority 100"; defIn("fixture.prio"; "/modules/prio-plain.nix").priority == 100),
  check("prio: mkForce def has priority 50"; defIn("fixture.prio"; "/modules/prio-force.nix").priority == 50),
  check("prio: default listed with priority 1500"; opt("fixture.prio").definitions | any(.kind == "default" and .priority == 1500 and .active)),
  check("prio: highestPrio 50"; opt("fixture.prio").highestPrio == 50),
  check("prio: the mkForce def is the only winner"; opt("fixture.prio").winners == [idx("fixture.prio"; "/modules/prio-force.nix")]),
  check("prio: losing defs stay active"; [defIn("fixture.prio"; "/modules/prio-default.nix"), defIn("fixture.prio"; "/modules/prio-plain.nix")] | all(.active)),
  check("prio: winner preview"; defIn("fixture.prio"; "/modules/prio-force.nix").valuePreview == "\"mkForce\""),

  # --- mkIf
  check("mkIf false leaf: inactive, mkIf-false, no priority, no preview"; defIn("fixture.conditional"; "/modules/conditional.nix") | .active == false and .condition == "mkIf-false" and .priority == null and .valuePreview == null),
  check("mkIf false leaf: default wins"; defaultIdx("fixture.conditional") as $d | opt("fixture.conditional") | .winners == [$d] and .highestPrio == 1500),
  check("mkIf false block: kept on fixture.prio with unknown priority"; defIn("fixture.prio"; "/modules/conditional.nix") | .active == false and .condition == "mkIf-false" and .priority == null),
  check("mkIf true: active, no condition, priority 100, wins"; opt("fixture.conditionalTrue") | (.definitions[0] | .active and .condition == null and .priority == 100) and .winners == [0]),
  check("mkIf with throwing condition: mkIf-error, inactive"; defIn("fixture.conditionalError"; "/modules/conditional.nix") | .condition == "mkIf-error" and .active == false),
  check("mkIf with throwing condition: winner unknown, error set"; opt("fixture.conditionalError") | .winners == [] and .highestPrio == null and .error != null),

  # --- other wrappers
  check("mkForce around a block reaches the leaf"; defIn("fixture.layered"; "/modules/layers.nix").priority == 50 and opt("fixture.layered").winners == [idx("fixture.layered"; "/modules/layers.nix")]),
  check("mkMerge at the leaf: three defs from one file"; defsFrom("fixture.merged"; "/modules/wrappers.nix") | map([.priority, .active]) == [[1000, true], [100, true], [null, false]]),
  check("mkMerge at the leaf: the plain one wins"; opt("fixture.merged") | .winners == [1] and .highestPrio == 100),
  check("mkDefinition: its own file and override priority"; defIn("fixture.viaDefinition"; "/virtual/defined-here.nix").priority == 50),

  # --- previews
  check("preview: derivation by name"; defIn("fixture.previews"; "/modules/wrappers.nix").valuePreview | contains("drv = <drv hello")),
  check("preview: function"; defIn("fixture.previews"; "/modules/wrappers.nix").valuePreview | contains("fn = <lambda>;")),
  check("preview: long list truncated"; defIn("fixture.previews"; "/modules/wrappers.nix").valuePreview | contains("...(12 more)")),
  check("preview: total length capped"; defIn("fixture.previews"; "/modules/wrappers.nix").valuePreview | length <= 243),

  # --- origins
  check("empty module listed as user"; [.modules[] | select(.id | endswith("/modules/empty.nix"))] | length == 1 and .[0].origin == "user"),
  check("empty module defines nothing"; ([.modules[] | select(.id | endswith("/modules/empty.nix")) | .id][0]) as $e | [.options[].definitions[] | select(.module == $e)] | length == 0),
  check("configuration.nix imports empty.nix"; first(.modules[] | select(.id | endswith("/configuration.nix"))).imports | any(endswith("/modules/empty.nix"))),
  check("extra input module origin input:extra"; mod(defIn("fixture.fromExtra"; "/extra/module.nix").module).origin == "input:extra"),
  check("extra input inline module origin input:extra"; modOfDef("fixture.fromExtraInline"; 0).origin == "input:extra"),
  check("string-interpolated import: origin input:extra"; modOfDef("fixture.fromInterpolated"; 0) | .origin == "input:extra" and (.id | endswith("/extra/interpolated.nix"))),
  check("networking.hostName: extra's mkDefault and the user's plain def"; defIn("networking.hostName"; "/extra/module.nix").priority == 1000 and defIn("networking.hostName"; "/configuration.nix").priority == 100),

  # --- disabledModules
  check("disabled module listed with disabled = true"; [.modules[] | select(.id | endswith("/modules/disabled.nix"))] | length == 1 and .[0].disabled == true),
  check("disabled module contributes nothing"; hasOpt("fixture.fromDisabled") | not),
  check("other modules are not disabled"; [.modules[] | select(.disabled)] | length == 1),

  # --- inline modules in nixosSystem's modules list
  check("inline attrset module: user-inline, modulesIndex 3"; modOfDef("fixture.inlineAttrs"; 0) | .origin == "user-inline" and .modulesIndex == 3 and (.position | test("/flake.nix:[0-9]+$"))),
  check("inline function module: user-inline, modulesIndex 4"; modOfDef("fixture.inlineFunction"; 0) | .origin == "user-inline" and .modulesIndex == 4 and (.position | test("/flake.nix:[0-9]+$"))),
  check("inline function module: mkDefault priority"; opt("fixture.inlineFunction").definitions[0].priority == 1000),
  check("anonymous module nested in an inline one: user-inline, no modulesIndex"; modOfDef("fixture.nestedInline"; 0) | .origin == "user-inline" and .modulesIndex == null),

  # --- submodule / freeform options are leaves
  check("users.users: def from users.nix"; defsFrom("users.users"; "/modules/users.nix") | length == 1),
  check("users.users: def from users-force.nix (inner mkForce not visible)"; defIn("users.users"; "/modules/users-force.nix").priority == 100),
  check("users.users: both fixture defs win"; opt("users.users") as $o | [idx("users.users"; "/modules/users.nix"), idx("users.users"; "/modules/users-force.nix")] | all(. as $i | $o.winners | index($i) != null)),
  check("no users.users.alice.* option paths"; [.options[].path | select(startswith("users.users."))] | length == 0),
  check("systemd.services: def from services.nix"; defsFrom("systemd.services"; "/modules/services.nix") | length == 1),
  check("freeform: services.openssh.settings def from services.nix"; defsFrom("services.openssh.settings"; "/modules/services.nix") | length == 1),
  check("no services.openssh.settings.* option paths"; [.options[].path | select(startswith("services.openssh.settings."))] | length == 0),

  # --- failures
  check("throwing option: error set"; opt("fixture.throws").error != null),
  check("throwing option: def kept, priority unknown, winner unknown"; opt("fixture.throws") | (.definitions | length == 1 and .[0].priority == null) and .winners == [] and .highestPrio == null),
  check("aborting option: present, excluded, error set"; opt("fixture.aborting") | .error != null and .definitions == []),
  check("aborting option: eval-crash warning"; warning("eval-crash"; "fixture.aborting")),
  check("WHNF-abort option: survives with its mkForce def"; opt("fixture.whnfAbort") | .highestPrio == 50 and (.definitions | length == 1) and .definitions[0].priority == 50),
  check("WHNF-abort option: preview disabled"; opt("fixture.whnfAbort").definitions[0].valuePreview == null),
  check("WHNF-abort option: preview-crash warning"; warning("preview-crash"; "fixture.whnfAbort")),
  check("WHNF-abort option: selfcheck-crash warning"; warning("selfcheck-crash"; "fixture.whnfAbort")),

  # --- scope filter
  check("no _module options"; [.options[].path | select(startswith("_module"))] | length == 0),
  check("nixpkgs-only option filtered out"; hasOpt("services.openssh.enable") | not),
  check("every option has a non-nixpkgs definition"; [.modules[] | select(.origin != "nixpkgs") | .id] as $user | all(.options[]; . as $o | ($o.definitions | any(.module as $m | $user | index($m) != null)) or $o.definitions == [])),
  check("only non-nixpkgs modules, their ancestors and imports listed"; .modules | length < 40),

  # --- referential integrity
  check("imports resolve"; [.modules[].id] as $ids | all(.modules[].imports[]; . as $i | $ids | index($i) != null)),
  check("module ids unique"; [.modules[].id] | length == (unique | length)),
  check("winners index definitions"; all(.options[]; . as $o | all($o.winners[]; . < ($o.definitions | length)))),
  check("winners share highestPrio"; all(.options[]; . as $o | all($o.winners[]; $o.definitions[.].priority == $o.highestPrio))),
  check("options sorted by path"; [.options[].path] | . == sort)
]
| (map(select(.ok != true) | .name)) as $failed
| if ($failed | length) == 0 then
    "fixture assertions: all \(length) passed"
  else
    error("fixture assertions failed (\($failed | length)/\(length)):\n  - " + ($failed | join("\n  - ")))
  end
