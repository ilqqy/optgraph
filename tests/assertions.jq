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
# A definition's file: its own `file`, else its module's file.
def defFile($g): .file // (.module as $m | first($g.modules[] | select(.id == $m)) | .file);
def defsFrom($p; $suffix): . as $g | [opt($p).definitions[] | select((defFile($g) // "") | endswith($suffix))];
def defIn($p; $suffix): defsFrom($p; $suffix) | first;
def idx($p; $suffix): . as $g | opt($p).definitions | map((defFile($g) // "") | endswith($suffix)) | index(true);
def mod($id): first(.modules[] | select(.id == $id));
def modOfDef($p; $i): opt($p).definitions[$i].module as $m | mod($m);
def warning($code; $subject): any(.meta.warnings[]; .code == $code and .subject == $subject);
def codes: [.meta.warnings[].code];
def defaultIdx($p): opt($p).definitions | map(.kind == "default") | index(true);
# Nested (submodule instance) helpers.
def under($p): .options[] | select(.path | startswith($p + "."));
def noNested($p): ([under($p)] | length == 0) and ([.modules[] | select((.scope // "") | startswith($p))] | length == 0);
def ownerFile($g): .owner as $o | first($g.modules[] | select(.id == $o)) | .file;
def defOwnerFile($g): .module as $m | first($g.modules[] | select(.id == $m)) | (ownerFile($g) // .file);
def nestedDefFrom($p; $suffix): . as $g | [opt($p).definitions[] | select(.kind == "definition" and ((defOwnerFile($g) // "") | endswith($suffix)))];
def nestedIdx($p; $suffix): . as $g | opt($p).definitions | map(.kind == "definition" and ((defOwnerFile($g) // "") | endswith($suffix))) | index(true);
def warningLike($code; $subject; $text): any(.meta.warnings[]; .code == $code and .subject == $subject and (.message | contains($text)));
def unstable: (.meta.nixpkgsVersion | startswith("25.11")) | not;

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
  check("users.users stays listed as a leaf next to its instances"; opt("users.users") | .definitions | length >= 2),
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
  check("default scope: a nixpkgs definition is listed only as the single winner"; ([.modules[] | {key: .id, value: .origin}] | from_entries) as $o | all(.options[]; . as $opt | all(range(0; $opt.definitions | length); . as $i | $opt.definitions[$i].module == null or $o[$opt.definitions[$i].module] != "nixpkgs" or $opt.winners == [$i]))),
  check("list option: only the user definition listed, nixpkgs winners counted"; opt("environment.pathsToLink") | ([.definitions[] | select(.kind == "definition")] | length) == 1 and .winners == [0] and .omitted.nixpkgsActive >= 1 and .omitted.nixpkgsWinners >= 1),
  check("list option lost to nixpkgs: winners empty, nixpkgsWinners win at highestPrio"; opt("environment.systemPackages") | .winners == [] and .highestPrio == 100 and .omitted.nixpkgsWinners > 0),
  check("list option lost to nixpkgs: the user's mkDefault is listed, losing"; defsFrom("environment.systemPackages"; "/modules/lists.nix") | length == 1 and .[0].priority == 1000 and .[0].active),
  check("list option: the losing option default is listed"; opt("environment.pathsToLink") | .definitions[-1] | .kind == "default" and .priority == 1500 and .active),
  check("losing nixpkgs option default listed, not counted"; opt("networking.hostName") | (.definitions | length - 1) as $d | .definitions[$d].kind == "default" and (.winners | index($d) | not) and .omitted == {nixpkgsActive: 0, nixpkgsInactive: 0, nixpkgsWinners: 0}),
  check("list option: the listed winner is the user's"; defsFrom("environment.pathsToLink"; "/modules/lists.nix") | length == 1),
  check("omitted counts on every option"; all(.options[]; .omitted.nixpkgsActive >= 0 and .omitted.nixpkgsInactive >= 0 and .omitted.nixpkgsWinners >= 0)),
  check("omitted winners are among the omitted active ones"; all(.options[]; .omitted.nixpkgsWinners <= .omitted.nixpkgsActive)),
  check("omitted winners only with several winners"; all(.options[]; .omitted.nixpkgsWinners == 0 or (.winners | length) + .omitted.nixpkgsWinners >= 2)),
  check("highestPrio set: a winner is listed or omitted"; all(.options[]; .highestPrio == null or (.winners | length) + .omitted.nixpkgsWinners + (.omitted.declarationWinners // 0) > 0)),
  check("systemd.services: inactive nixpkgs definitions counted, not listed"; opt("systemd.services") | .omitted.nixpkgsInactive > 1000 and (.definitions | length) < 100),
  check("user definitions are never omitted"; defsFrom("fixture.prio"; "/modules/conditional.nix") | length == 1),

  # --- redaction and lazy values
  check("secret-looking option: string preview redacted"; opt("fixture.apiToken").definitions[0].valuePreview == "<redacted>"),
  check("secret-looking option: bool preview kept"; opt("fixture.secretEnabled").definitions[0].valuePreview == "true"),
  check("nested attrsets preview names only (lazy field not forced)"; defIn("fixture.lazy"; "/modules/lazy.nix").valuePreview == "{ nested = { boom, fine }; }"),
  check("fixpoint package set previewed as names only (throwing alias not forced)"; defIn("fixture.packageSet"; "/modules/lazy.nix").valuePreview == "{ __unfix__, extend, fine, removed }"),
  check("fixpoint package set: no error"; opt("fixture.packageSet").error == null),
  check("lazy field: no crash, no recovery"; [.meta.warnings[] | select(.subject == "fixture.lazy" or .subject == "assertions")] | length == 0),
  check("assertions option: never previewed"; opt("assertions") | all(.definitions[]; .valuePreview == null)),
  check("assertions option: the fixture's entry is listed"; defsFrom("assertions"; "/modules/lazy.nix") | length == 1),
  check("only the four deliberate crash warnings"; [.meta.warnings[] | select(.code | endswith("crash"))] | length == 4),

  # --- submodule instances (nested attribution)
  check("meta.submoduleDepth 3"; .meta.submoduleDepth == 3),
  check("users.users.alice: the mkForce definition from users-force.nix wins"; nestedIdx("users.users.alice.description"; "/modules/users-force.nix") as $i | opt("users.users.alice.description") | .highestPrio == 50 and .winners == [$i] and .instance == "users.users.alice" and .parent == "users.users"),
  check("users.users.alice: users.nix's plain definition loses"; nestedDefFrom("users.users.alice.description"; "/modules/users.nix") | length == 1 and .[0].priority == 100 and .[0].active),
  check("users.users.alice: mkIf false inside the instance"; nestedDefFrom("users.users.alice.home"; "/modules/users-force.nix") | length == 1 and .[0].condition == "mkIf-false" and .[0].priority == null),
  check("users.users.alice: mkIf false around the instance definition, attributed to its module"; defsFrom("users.users.alice.extraGroups"; "/modules/nested-plain.nix") | length == 1 and .[0].condition == "mkIf-false" and (.[0].module | startswith("/"))),
  check("users.users.alice: the sub-option default is listed last"; opt("users.users.alice.isNormalUser").definitions[-1] | .kind == "default" and .priority == 1500),
  check("mkDefault vs plain vs mkForce inside an instance"; nestedDefFrom("fixture.accounts.dave.shell"; "/modules/nested-force.nix")[0].priority == 50 and nestedDefFrom("fixture.accounts.dave.shell"; "/modules/nested-plain.nix")[0].priority == 100 and nestedDefFrom("fixture.accounts.dave.shell"; "/modules/nested.nix")[0].priority == 1000),
  check("mkForce wins inside an instance"; nestedIdx("fixture.accounts.dave.shell"; "/modules/nested-force.nix") as $i | opt("fixture.accounts.dave.shell") | .highestPrio == 50 and .winners == [$i]),
  check("sub-option mkIf false inside an instance"; nestedDefFrom("fixture.accounts.dave.greeting"; "/modules/nested.nix") | length == 1 and .[0].condition == "mkIf-false"),
  check("instance-level mkIf false: listed inactive on the sub-option"; defsFrom("fixture.accounts.erin.shell"; "/modules/nested.nix") | length == 1 and .[0].active == false and .[0].condition == "mkIf-false" and .[0].priority == null),
  check("function-style instance: name and osConfig from the instance"; nestedDefFrom("fixture.home.alice.editor"; "/modules/nested.nix")[0].valuePreview == "\"vim for alice on fixture\""),
  check("function-style instance: a definition module owned by nested.nix"; . as $g | nestedDefFrom("fixture.home.alice.editor"; "/modules/nested.nix")[0].module as $m | mod($m) | .scope == "fixture.home.alice" and (ownerFile($g) | endswith("/modules/nested.nix")) and (.id | startswith("«fixture.home.alice»"))),
  check("import inside an instance: attributed to the imported file"; defIn("fixture.home.alice.theme"; "/modules/nested/alice-extra.nix") | .priority == 1000 and (.module | startswith("«fixture.home.alice»/"))),
  check("import inside an instance: an import edge from the definition module"; . as $g | nestedDefFrom("fixture.home.alice.editor"; "/modules/nested.nix")[0].module as $m | mod($m).imports | any(endswith("/modules/nested/alice-extra.nix"))),
  check("instance theme: mkForce wins over plain and mkDefault"; nestedIdx("fixture.home.alice.theme"; "/modules/nested-force.nix") as $i | opt("fixture.home.alice.theme") | .highestPrio == 50 and .winners == [$i]),
  check("declaration-side definitions counted, not listed"; opt("fixture.home.alice.theme") | .omitted.declarationActive == 1 and ([.definitions[] | select(.kind == "definition")] | length) == 3),
  check("options touched only by declaration modules are out of scope"; hasOpt("fixture.home.bob.theme") | not),
  check("two instance names"; hasOpt("fixture.home.alice.editor") and hasOpt("fixture.home.bob.editor")),
  check("submodule inside a submodule"; opt("fixture.home.alice.files.todo.text") | .instance == "fixture.home.alice.files.todo" and .parent == "fixture.home.alice.files" and .winners == [0]),
  check("submodule inside a submodule: owner is the enclosing instance's module"; . as $g | modOfDef("fixture.home.alice.files.todo.text"; 0) | .scope == "fixture.home.alice.files.todo" and (.owner | startswith("«fixture.home.alice»"))),
  check("submodule inside a submodule: from the imported file"; . as $g | modOfDef("fixture.home.alice.files.notes.text"; 0) | (.owner | endswith("/modules/nested/alice-extra.nix"))),
  check("instance-level mkForce: the other definition is shadowed"; nestedDefFrom("fixture.home.carol.theme"; "/modules/nested-plain.nix") | length == 1 and .[0].shadowedAt == "fixture.home.carol"),
  check("shadowed definitions never win"; opt("fixture.home.carol.theme") as $o | ($o.definitions | to_entries | map(select(.value.shadowedAt != null) | .key)) as $s | all($s[]; . as $i | $o.winners | index($i) | not) and ($s | length) == 1),
  check("shadowed definition on an option no winner defines"; opt("fixture.home.carol.editor") | (.definitions[0].shadowedAt == "fixture.home.carol") and .winners == [1] and .highestPrio == 1500),
  check("single submodule: instance is the option"; opt("fixture.single.shell") | .instance == "fixture.single" and .parent == "fixture.single" and .winners == [0]),
  check("single submodule: definitions from two modules"; hasOpt("fixture.single.greeting") and (nestedDefFrom("fixture.single.greeting"; "/modules/nested-plain.nix") | length == 1)),
  check("nullOr submodule: expanded on unstable, a leaf on 25.11"; if unstable then opt("fixture.maybe.shell").winners == [0] else noNested("fixture.maybe") end),
  check("lazyAttrsOf: the good instance is expanded"; opt("fixture.lazyAccounts.fine.shell").winners == [0]),
  check("lazyAttrsOf: the throwing instance degrades alone"; warningLike("nested-degraded"; "fixture.lazyAccounts.bad"; "could not be classified") and noNested("fixture.lazyAccounts.bad")),
  check("throwing instance: degraded to a leaf"; warningLike("nested-degraded"; "fixture.home.broken"; "does not evaluate") and noNested("fixture.home.broken")),
  check("aborting instance: excluded alone, siblings expanded"; warning("nested-crash"; "fixture.home.aborts") and noNested("fixture.home.aborts") and hasOpt("fixture.home.alice.editor")),
  check("instance-level mkBefore: degraded to a leaf"; warningLike("nested-degraded"; "fixture.ordered.frank"; "mkBefore/mkAfter/mkOrder") and noNested("fixture.ordered") and (opt("fixture.ordered").definitions | length >= 2)),
  check("winning non-attrset definition: degraded to a leaf"; warningLike("nested-degraded"; "fixture.forced"; "not a plain attribute set") and noNested("fixture.forced") and (opt("fixture.forced").definitions | length >= 1)),
  check("degraded instances: nothing attributed inside"; [.meta.warnings[] | select(.code == "nested-degraded" or .code == "nested-crash") | .subject] as $d | all(.options[]; (.instance // "") as $i | all($d[]; . as $s | ($i == $s or ($i | startswith($s + "."))) | not))),
  check("no nested-unchecked"; codes | index("nested-unchecked") | not),
  check("nested options carry instance and parent"; all(under("fixture.home.alice"); .instance != null and .parent != null)),
  check("declaration counters only when not zero"; all(.options[]; .omitted | [.declarationActive, .declarationInactive, .declarationWinners] | all(. == null or . > 0))),
  check("all winners omitted declaration definitions: winners empty, highestPrio set, counted"; opt("fixture.home.bob.packages") | .winners == [] and .highestPrio == 100 and .omitted.declarationWinners == 2 and .omitted.declarationActive == 2 and .omitted.nixpkgsWinners == 0 and .omitted.declarationOrigin == "input:extra"),
  check("all winners omitted: the user's mkDefault is listed, losing"; nestedDefFrom("fixture.home.bob.packages"; "/modules/nested.nix") | length == 1 and .[0].priority == 1000 and .[0].active),
  check("declarationOrigin names the declaration input"; opt("fixture.home.alice.theme").omitted.declarationOrigin == "input:extra"),
  check("top-level options carry no nested fields"; all(.options[] | select(.instance == null); (has("parent") | not) and (.omitted | has("declarationActive") or has("declarationOrigin") | not))),
  check("nested paths extend their instance, instances extend their parent"; all(.options[] | select(.instance != null); . as $o | ($o.path | startswith($o.instance + ".")) and ($o.instance | startswith($o.parent)))),
  check("declaration winners among declaration active"; all(.options[] | select(.instance != null); (.omitted.declarationWinners // 0) <= (.omitted.declarationActive // 0))),
  check("every owner is a listed module"; [.modules[].id] as $ids | all(.modules[] | select(.owner != null); .owner as $o | $ids | index($o) != null)),
  check("nested modules carry their scope in the id"; all(.modules[] | select(.scope != null); . as $m | $m.id | startswith("«" + $m.scope + "»"))),
  check("option paths unique"; [.options[].path] | length == (unique | length)),

  # --- referential integrity
  check("every definition's module is listed"; [.modules[].id] as $ids | all(.options[].definitions[]; .module == null or (.module as $m | $ids | index($m) != null))),
  check("file dropped when it is the module's file"; . as $g | all(.options[].definitions[]; .module == null or .file == null or (.module as $m | first($g.modules[] | select(.id == $m)).file != .file))),
  check("mkDefinition keeps its own file"; any(opt("fixture.viaDefinition").definitions[]; .file == "/virtual/defined-here.nix")),
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
