# Module system notes

Findings about `lib/modules.nix` and friends that optgraph depends on.

- nixpkgs rev: `c59305bab2065cfecc4944690d9eedbb56f3a9fa` (`lib.version` is `26.11.20261001.c59305b` through the flake, `26.11pre-git` when `lib` is imported by path)
- Source: `/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source`
- Nix: 2.34.8
- Re-run date: 2026-10-02; items marked 2026-10-03 were re-run after the review fixes (topics 3, 6, 7, 8)
- Refs are written `nixpkgs@c59305b:path:line`. Refs into this repository (`nix/*.nix`) use line numbers of the same commit and drift with edits.

This file covers module-system internals only. CLI flags and the output schema live in `docs/schema.md` and `README.md`.

## Conventions

- "Re-run" means the snippet was run for this document on 2026-10-02 (or on the date given) and the output below is what it printed. "Research run" means an earlier research session measured it and it was not repeated here. Those items carry `<!-- unverified: ... -->`.
- Store paths in outputs are shortened for readability: `<nixpkgs>` is the source path above, `<fixture>` is the store copy of the fixture flake below. Some outputs were reformatted with `nix run nixpkgs#jq -- -c` (jq is not installed on the host; the plain `jq` commands in topic 3 ran with jq 1.8.2 from nixpkgs).
- Research snippets use `--impure` only because they import a store path by absolute path or call `builtins.getFlake` on an unlocked ref. `pure-eval` is `true` by default in Nix 2.34.8 (`nix config show pure-eval`; this host's `nix.conf` does not set it, and a `pure-eval = false` in `nix.conf` or `NIX_CONFIG` is ignored, topic 8). `nix eval --expr` honours it; `nix eval --file` does not. Snippets that only touch the fixture flake use `--pure-eval ... --apply` and are pure. optgraph itself runs pure (topic 8).

```sh
export NP=/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source
export FLKREF=path:/tmp/docs-verify/flk
```

Fixture flake (`mkdir -p /tmp/docs-verify/flk`). Copy the repo's `flake.lock` next to it so that `nixpkgs` resolves to the pinned rev:

```nix
# flake.nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: {
    nixosConfigurations.t = nixpkgs.lib.nixosSystem {
      modules = [
        {
          nixpkgs.hostPlatform = "x86_64-linux";
          boot.loader.grub.enable = false;
          fileSystems."/" = { device = "none"; fsType = "tmpfs"; };
          system.stateVersion = "26.11";
          networking.hostName = "inline";
        }
        ({ lib, ... }: { services.openssh.enable = lib.mkDefault false; })
        ./mod.nix
      ];
    };
  };
}
```

```nix
# mod.nix
{ lib, ... }: {
  services.openssh.enable = true;
  networking.hostName = lib.mkForce "from-mod";
}
```

Commands that use optgraph's own code (`nix/collect.nix`, `nix/default.nix`) run from the repo root. For research they call the lib on a flake with `--impure`; the CLI itself does not (topic 8).

---

## 1. `.graph` (evalModules result attribute)

**Answer.** `evalModules` returns `graph`, a list of nodes `{ key, file, disabled, imports }` where `imports` is a list of nodes of the same shape. There is no position, class or meta. The internal module (key `lib/modules.nix`) is dropped from the top level.

Key and file rules:

| Module form | `key` | `file` |
|---|---|---|
| path | `toString path` | same as key |
| anything else (attrset, function) | `"${parentKey}:anon-${n}"`, `n` = 1-based index in the parent's `imports` list; top-level parent key is `""`, so `:anon-N` | parent's `_file`; `<unknown-file>` at top level |
| explicit `key = k` | `k` | unchanged |
| explicit `_file = f` | unchanged | `f` |

`disabledModules` match by key. Strings are resolved against `modulesPath` unless they start with `/`; attrsets need a `key` (or convert to string). A disabled node stays in the graph with `disabled = true`; its children are not marked.

The graph is not deduplicated. Only the real module list (`filterModules`) dedups, by key.

`nixosSystem` results also have `.graph`: it is `noUserModules.extendModules { modules = userModules; }`, so the graph is the base modules followed by the user modules.

**Evidence.**

- `nixpkgs@c59305b:lib/modules.nix:406` `inherit (doCollect { }) graph;`; `toGraph` :588-602, internal module filtered at :601; `graph =` :608.
- `nixpkgs@c59305b:doc/module-system/module-system.chapter.md:119-127` documents `key`, `file`, `imports`, `disabled`.
- Keys: `unifyModuleSyntax` :647-720 (`_file`/`key` defaults at :699-701 and :713-715), `loadModule` :421-453, `collectStructuredModules` :526-560 (anon key at :538).
- Disabling: `isDisabled` :473-499; `disabled` collected at :547-555.
- Dedup: `filterModules` :566-586 (`genericClosure` on `key`).
- Internal module: `nixpkgs@c59305b:lib/modules.nix:138-141` (`key = "lib/modules.nix"`), appended last at :259.
- `extendModules` appends: `nixpkgs@c59305b:lib/modules.nix:382-394` (`modules = regularModules ++ modules`); `nixpkgs@c59305b:nixos/lib/eval-config.nix:106`.

**Verified by eval.**

Keys, `_file`, explicit `key`, disabling (re-run):

```sh
nix eval --impure --json --expr '
let
  lib = import (builtins.getEnv "NP" + "/lib");
  strip = n: { inherit (n) key file disabled; imports = map strip n.imports; };
  e = lib.evalModules { modules = [
    { _file = "p.nix"; imports = [ { key = "child"; } { _file = "f.nix"; } ]; }
    { _file = "f.nix"; }
    { key = "k"; }
    { key = "b"; imports = [ { key = "c"; } ]; }
    { disabledModules = [ { key = "b"; } ]; }
  ]; };
in map strip e.graph' | nix run nixpkgs#jq -- -c '.[]'
```

```
{"disabled":false,"file":"p.nix","imports":[{"disabled":false,"file":"p.nix","imports":[],"key":"child"},{"disabled":false,"file":"f.nix","imports":[],"key":":anon-1:anon-2"}],"key":":anon-1"}
{"disabled":false,"file":"f.nix","imports":[],"key":":anon-2"}
{"disabled":false,"file":"<unknown-file>","imports":[],"key":"k"}
{"disabled":true,"file":"<unknown-file>","imports":[{"disabled":false,"file":"<unknown-file>","imports":[],"key":"c"}],"key":"b"}
{"disabled":false,"file":"<unknown-file>","imports":[],"key":":anon-5"}
```

Node `b` is disabled, its child `c` is not.

String form of `disabledModules` against `modulesPath` (re-run):

```sh
nix eval --impure --json --expr '
let lib = import (builtins.getEnv "NP" + "/lib");
  e = lib.evalModules { specialArgs.modulesPath = "/m"; modules = [ { key = "/m/b.nix"; } { disabledModules = [ "b.nix" ]; } ]; };
in map (n: { inherit (n) key disabled; }) e.graph'
```

```
[{"disabled":true,"key":"/m/b.nix"},{"disabled":false,"key":":anon-2"}]
```

NixOS through `nixosSystem`, tail of the top-level list (re-run; fixture has 3 user modules, the 3rd is a path):

```sh
nix eval --pure-eval --json "$FLKREF#nixosConfigurations.t" --apply '
c: let
  g = c.graph;
  n = builtins.length g;
  brief = x: { inherit (x) key file; imports = map (i: i.key) x.imports; };
in {
  top = n;
  last6 = map (i: brief (builtins.elemAt g i)) (builtins.genList (i: n - 6 + i) 6);
}' | nix run nixpkgs#jq -- -c '.last6[], {top}'
```

```
{"file":"<nixpkgs>/nixos/lib/eval-config.nix","imports":[],"key":"<nixpkgs>/nixos/lib/eval-config.nix"}
{"file":"<unknown-file>","imports":[],"key":":anon-2115"}
{"file":"<nixpkgs>/flake.nix","imports":[":anon-2116:anon-1"],"key":":anon-2116"}
{"file":"<nixpkgs>/flake.nix","imports":[":anon-2117:anon-1"],"key":":anon-2117"}
{"file":"<nixpkgs>/flake.nix","imports":["<fixture>/mod.nix"],"key":":anon-2118"}
{"file":"<nixpkgs>/flake.nix","imports":[":anon-2119:anon-1"],"key":":anon-2119"}
{"top":2119}
```

The four wrappers at the end are the three fixture modules plus the module `nixosSystem` appends itself (topic 5). `pkgsModule` (`eval-config.nix` path) and `modulesModule` (`:anon-2115`) sit just before them.

Size and duplication (re-run):

```sh
nix eval --pure-eval --json "$FLKREF#nixosConfigurations.t" --apply '
c: let
  flat = ns: builtins.concatMap (n: [ n ] ++ flat n.imports) ns;
  all = flat c.graph;
in {
  baseModules = builtins.length c._module.args.baseModules;
  userModules = builtins.length c._module.args.modules;
  top = builtins.length c.graph;
  total = builtins.length all;
  uniqueKeys = builtins.length (c.lib.unique (map (n: n.key) all));
  jsonBytes = builtins.stringLength (builtins.toJSON c.graph);
}'
```

```
{"baseModules":2113,"jsonBytes":994818,"top":2119,"total":4099,"uniqueKeys":3979,"userModules":4}
```

`builtins.stringLength (builtins.toJSON c.graph)` alone: 0.51 s wall, 117 MB max RSS (`/usr/bin/env time -v`). Research run on a smaller minimal config: 2116 top-level, 4093 total, 3973 unique keys, ~994 KB, 0.43 s, 112 MB. <!-- unverified: research run, 2026-10-02; the re-run above supersedes it -->

**Caveats.**

- `disabled` must be propagated to descendants by the consumer. A child of a disabled module can show `disabled = false` although it is never loaded (above: `c` under `b`).
- One module appears once per importer: 4099 nodes, 3979 unique keys in the fixture. Dedup by `key` when building a node table.
- `file` is a string and not always a path: `<unknown-file>`, `lib/modules.nix`, or any user `_file` string (`"my-string-file"`).
- Absent on older nixpkgs: no `.graph` on release-25.05, present on 25.11 and unstable. <!-- unverified: secondary source (GitHub raw files), research run, 2026-10-02 -->

**Consequence for optgraph.** Build the import graph from `.graph`, dedup nodes by `key`, propagate `disabled` down, and do not trust `file` to be a path.

---

## 2. Option attributes and what they contain

**Answer.** `evalOptionValue` returns the declared option `//` `{ value, highestPrio, definitions, files, definitionsWithLocations, isDefined, valueMeta, __toString }`. The attribute list of an option is:

```
__toString _type declarationPositions declarations default (only if declared) definitions
definitionsWithLocations files highestPrio isDefined loc options type value valueMeta
```

Every definition-derived attribute (`definitions`, `files`, `definitionsWithLocations`, `isDefined`, `highestPrio`) is computed from `defsFinal`, that is after:

1. `dischargeProperties`: `mkMerge` is flattened, `mkIf` conditions are evaluated, `mkIf false` definitions are dropped.
2. `filterOverrides'`: definitions with a losing priority are dropped and the `mkOverride` wrapper is stripped.
3. `sortProperties`: `mkOrder` sorting.

So losing and `mkIf false` definitions are not visible through any option attribute. What stays:

- `definitionsWithLocations` entries are `{ file, value }`, plus `priority` only when `mkOrder`/`mkBefore`/`mkAfter` was used (it is the order value, not the override priority). There are no per-definition source positions, only the module file.
- The option `default` is a definition with `file = head declarations`, priority 1500 (`mkOptionDefault`), and is present only if nothing beats it.
- Definition order is the reverse of the order of the deduplicated module list (`reverseList`).
- A single definition without `_type` takes a fast path: no discharge/filter/sort, `highestPrio = 100`. <!-- unverified: code reading only (modules.nix:1225-1240); the result is not distinguishable from the slow path from outside, so it was not run -->
- No definitions: `definitionsWithLocations = []`, `highestPrio = 9999`, `isDefined = false`.
- `attrsOf (submodule ...)`: the outer option's `definitionsWithLocations` holds the raw inner values with their wrappers intact. Per-instance sub-options are only reachable through `options.<opt>.valueMeta.attrs.<name>.configuration.options.<sub>`.

**Evidence.**

- `nixpkgs@c59305b:lib/modules.nix:1140-1194` `evalOptionValue` (default injected at :1144-1153, result attrs at :1181-1193).
- `nixpkgs@c59305b:lib/modules.nix:1223-1340` `mergeDefinitions`: fast path :1225-1240, `defsNormalized` :1244-1256, `filterOverrides'` call :1259, `sortProperties` :1262-1267.
- `dischargeProperties` :1410-1422, `filterOverrides'` :1453-1474, `sortProperties` :1487.
- Order reversal: `nixpkgs@c59305b:lib/modules.nix:273` (`mergeModules prefix (reverseList (doCollect { }).modules)`).

**Verified by eval** (re-run):

```sh
nix eval --impure --json --expr '
let
  lib = import (builtins.getEnv "NP" + "/lib");
  inherit (lib) mkOption mkIf mkForce mkDefault mkBefore mkAfter mkOverride types;
  e = lib.evalModules { modules = [
    { _file = "decl.nix"; options = {
        x = mkOption { type = types.int; default = 0; };
        y = mkOption { type = types.int; };
        r = mkOption { type = types.listOf types.int; };
        s = mkOption { type = types.listOf types.str; };
        d = mkOption { type = types.int; default = 9; };
        z = mkOption { type = types.int; default = 9; };
        u = mkOption { default = { }; type = types.attrsOf (types.submodule { options.x = mkOption { type = types.int; default = 0; }; }); };
    }; }
    { _file = "m2.nix"; x = mkDefault 1; y = mkIf false 7; r = [ 2 ]; s = [ "a" ]; u.alice.x = mkIf false 3; }
    { _file = "m3.nix"; x = 2; r = [ 3 ]; s = mkAfter [ "c" ]; u.alice.x = mkForce 4; }
    { _file = "m4.nix"; x = mkForce 3; r = [ 4 ]; s = mkBefore [ "b" ]; }
    { _file = "m5.nix"; x = mkIf false 4; z = mkOverride 10 (mkIf false 5); }
  ]; };
  show = o: {
    inherit (o) highestPrio isDefined;
    dwl = map (d: d // { value = d.value._type or d.value; }) o.definitionsWithLocations;
  };
in {
  attrs = builtins.attrNames e.options.x;
  x = show e.options.x;
  y = show e.options.y;
  r = show e.options.r;
  s = show e.options.s;
  d = show e.options.d;
  z = show e.options.z;
  z_value_ok = (builtins.tryEval e.config.z).success;
  u_outer = show e.options.u;
  u_alice_x = show e.options.u.valueMeta.attrs.alice.configuration.options.x;
}' | nix run nixpkgs#jq -- -c 'to_entries[]'
```

```
{"key":"attrs","value":["__toString","_type","declarationPositions","declarations","default","definitions","definitionsWithLocations","files","highestPrio","isDefined","loc","options","type","value","valueMeta"]}
{"key":"d","value":{"dwl":[{"file":"decl.nix","value":9}],"highestPrio":1500,"isDefined":true}}
{"key":"r","value":{"dwl":[{"file":"m4.nix","value":[4]},{"file":"m3.nix","value":[3]},{"file":"m2.nix","value":[2]}],"highestPrio":100,"isDefined":true}}
{"key":"s","value":{"dwl":[{"file":"m4.nix","priority":500,"value":["b"]},{"file":"m2.nix","value":["a"]},{"file":"m3.nix","priority":1500,"value":["c"]}],"highestPrio":100,"isDefined":true}}
{"key":"u_alice_x","value":{"dwl":[{"file":"m3.nix","value":4}],"highestPrio":50,"isDefined":true}}
{"key":"u_outer","value":{"dwl":[{"file":"m3.nix","value":{"alice":{"x":{"_type":"override","content":4,"priority":50}}}},{"file":"m2.nix","value":{"alice":{"x":{"_type":"if","condition":false,"content":3}}}}],"highestPrio":100,"isDefined":true}}
{"key":"x","value":{"dwl":[{"file":"m4.nix","value":3}],"highestPrio":50,"isDefined":true}}
{"key":"y","value":{"dwl":[],"highestPrio":9999,"isDefined":false}}
{"key":"z","value":{"dwl":[{"file":"m5.nix","value":"if"}],"highestPrio":10,"isDefined":true}}
{"key":"z_value_ok","value":false}
```

Reading: `x` has four definitions (mkDefault 1, plain 2, mkForce 3, `mkIf false` 4) plus the default; only `m4.nix` is visible. `y` has one definition, all dropped. `r` shows reversed module order. `s` shows `mkBefore`/`mkAfter` as `priority` and sorted position. `d`: default is the winner, file is the declaring module. `z`: see caveat. `u_outer`: raw wrappers intact; `u_alice_x`: per-instance result after discharge.

**Caveats.**

- `mkOverride p (mkIf false v)`: `dischargeProperties` stops at the `override` wrapper, so the `mkIf` is not discharged. The definition survives at priority `p` with the `if` value as its content (`z` above: `highestPrio = 10`, value is an `if`). Forcing `config.z` then fails the type check; `tryEval` reports `success = false` (`z_value_ok` above).
- `mkOrder` only works at leaf level; `pushDownProperties` has `# FIXME: handle mkOrder?` (`nixpkgs@c59305b:lib/modules.nix:1387`).

**Consequence for optgraph.** Option attributes show only the surviving definitions, so losing and `mkIf false` definitions cannot come from the option tree: see topic 3.

---

## 3. Getting all definitions, including losing priorities and `mkIf false`

**Answer.** No public attribute exposes the raw definitions; only `defsFinal` is exposed (`defsNormalized` and `defsFiltered` are `let`-bound inside `mergeDefinitions`). The low-level collectors are private and warn. The workable approach, implemented in `nix/collect.nix` and `nix/reconstruct.nix`, re-collects the modules and walks their `config` alongside `eval.options` using only the public API.

`nix/collect.nix` (re-collection and alignment):

1. Take the regular module list from `eval.type.getSubModules` (before import expansion). For NixOS it is `baseModules ++ extraModules ++ [ pkgsModule modulesModule ] ++ map setDefaultModuleLocation modules`; the internal module is not included. Re-implement `unifyModuleSyntax`, `applyModuleArgs`, `collectModules`, `disabledModules` handling and the `genericClosure` dedup (`nix/collect.nix` :57-195, :341-395). The result has one node per tree position, a deduplicated `live` list and the `disabledNodes`.
2. Call module functions with `{ lib, options, config = eval.config // { inherit (eval) _module; }, specialArgs, _class, _prefix } // specialArgs`; arguments the function asks for and that are not in there come from `config._module.args`. `eval.config` has `_module` stripped. Every module load runs under `tryEval` and a `module` marker (topic 6); a module that throws, or that the caller excluded, becomes an empty stub.
3. Align the top-level list with `_module.args.{baseModules,extraModules,modules}` (:196-300): lengths must add up (`base + extra + 2 + user`), user modules are all wrapped or none, and the module `nixosSystem` injects (sets exactly `nixpkgs.flake.source`) must be the last user module. Every top-level node gets a group (`base`, `extra`, `internal`, `user`; `unknown` when not aligned) and, for user modules, an index into `modules`. If anything does not add up, `aligned` is false and the reason is in `alignmentProblem`. `_module.args` is checked with `?` first: a missing `baseModules` is an uncatchable error (topic 6).
4. `graphKeys` are the keys of the module system's own `.graph` (topic 1), walked like `live`; the lib compares them with the re-collected keys (`graph-mismatch` warning).

`nix/reconstruct.nix` (definitions):

1. `walk` goes through one module's `config` next to `eval.options`, pruned by a trie of the wanted option paths (`trieOf`), and carries ancestor `mkIf`/`mkOverride` layers down like `pushDownProperties` does (:54-93). It stops at `lib.isOption`, skips the top-level `_module`, and skips keys that no option declares (freeform keys). `wrap` re-applies the layers around the leaf value, outermost first (:33-37).
2. `classify` resolves the wrappers at the leaf (`if`, `merge`, `override`, `definition`) and keeps definitions whose `mkIf` condition is false or failed (:98-183). Each enclosing condition is tri-state (`true`, `false`, `null` when it threw or is not a bool). The result per definition: `file`, `conditions`, `priority`, `probeFailed`, `content`.
3. `trieOf` groups the locs with `builtins.groupBy` on the first component (:43-51); the obvious `foldl' recursiveUpdate` overflows the stack (topic 6).

Laziness contract (header of `nix/reconstruct.nix`): module configs are forced only along option paths in the trie; conditions are forced under `tryEval`, and only by `classify`; a definition value is forced to WHNF (to read `_type`) only when every condition above it is true, and an `mkIf`'s content only if its condition is; values themselves are never forced. Inactive definitions get `priority = null` (unknown). `mkOverride p (mkIf false v)` is active at priority `p`, like in the module system (topic 2).

Limitations:

- Submodule, `attrsOf` and freeform options are treated as leaves (no per-instance attribution).
- Freeform keys not matched by a declared option are skipped.
- `_module.*` is never walked or listed; `_module.args` is defined by the internal module, which is not in `getSubModules`.
- `mkOrder` above leaf level is unsupported by the module system itself (topic 2).

Not feasible:

- Recording definitions through `extendModules` or a custom type: `type.merge` only receives `defsFinal`, and a module cannot observe other modules' definitions. <!-- unverified: reasoning from code (modules.nix:1286, :1320), not evaluated -->
- Patching `lib/modules.nix`: brittle across nixpkgs revisions.

**Evidence.**

- Private set: `nixpkgs@c59305b:lib/modules.nix:2313-2329`. Members: `applyModuleArgsIfFunction`, `dischargeProperties`, `mergeModules`, `mergeModules'`, `pushDownProperties`, `unifyModuleSyntax`, `collectModules`. Each is wrapped with `warn "External use of `lib.modules.<name>` is deprecated..."`. `warn` is `lib.trivial.warn`, which resolves to `builtins.warn` on Nix >= 2.23 (`nixpkgs@c59305b:lib/trivial.nix:867-871`); under `--abort-on-warn` it is fatal.
- Public without warning: the `inherit` block `nixpkgs@c59305b:lib/modules.nix:2461-2507` (`mergeDefinitions`, `evalOptionValue`, `filterOverrides`, `filterOverrides'`, `sortProperties`, `mkIf`, `mkMerge`, `mkOverride`, `mkOrder`, `mkDefinition`, `defaultOverridePriority`, `defaultOrderPriority`, ...).
- `eval.config` without `_module`: `nixpkgs@c59305b:lib/modules.nix:404`.
- `type.merge loc defsFinal` :1286, `type.merge.v2 { defs = defsFinal; }` :1320.
- `pushDownProperties` :1367-1391.
- NixOS result attributes and `_module.args` (`modules`, `baseModules`, `extraModules`, `noUserModules`): `nixpkgs@c59305b:nixos/lib/eval-config.nix:81-117`; listed below.
- Prior art:
  - `nixpkgs@c59305b:nixos/maintainers/option-usages.nix:27-40` answers a different question (which options read which, by replacing an option with `throw` and re-evaluating); it says it is slow.
  - `nixpkgs@c59305b:pkgs/by-name/ni/nixos-option/nixos-option.nix:133-134` prints declarations and `.files`, winners only.
  - `lib.options.showDefs`, `nixpkgs@c59305b:lib/options.nix:835-865`: previews with `tryEval` + `generators.toPretty` using `withRecursion { depthLimit = 10; }`, first 5 lines.
  - nix-why (`https://github.com/abstracts33d/nix-why`, discourse thread `https://discourse.nixos.org/t/nix-why-why-does-this-option-have-this-value/78284`) uses `definitionsWithLocations` by default and a best-effort raw module walk in `--full`; a fzakaria blog post uses `definitionsWithLocations`/`files` only. <!-- unverified: web sources, research run, 2026-10-02 -->

**Verified by eval.**

Private functions warn, public ones do not (re-run):

```sh
nix eval --impure --abort-on-warn --expr 'let lib = import (builtins.getEnv "NP" + "/lib"); in builtins.typeOf lib.modules.dischargeProperties' 2>&1 | grep -E 'warning|error' | cut -c1-120
```

```
evaluation warning: External use of `lib.modules.dischargeProperties` is deprecated. If your use case isn't covered by n
error:
       error: aborting to reveal stack trace of warning, as abort-on-warn is set
```

```sh
nix eval --impure --abort-on-warn --json --expr 'let lib = import (builtins.getEnv "NP" + "/lib"); in map (n: builtins.typeOf lib.modules.${n}) [ "mergeDefinitions" "evalOptionValue" "filterOverrides" "filterOverrides'"'"'" "sortProperties" "mkIf" "mkMerge" "mkOverride" "mkOrder" "mkDefinition" "defaultOverridePriority" "defaultOrderPriority" ]'
```

```
["lambda","lambda","lambda","lambda","lambda","lambda","lambda","lambda","lambda","lambda","int","int"]
```

Re-collection and alignment on the research fixture (re-run 2026-10-03; from the repo root; `mark` is a no-op here):

```sh
/usr/bin/env time -v nix eval --impure --json --expr '
let
  eval = (builtins.getFlake (builtins.getEnv "FLKREF")).nixosConfigurations.t;
  inherit (eval) lib;
  col = import ./nix/collect.nix { inherit lib; mark = _: _: x: x; } { sys = eval; };
in {
  subModules = builtins.length eval.type.getSubModules;
  live = builtins.length col.live;
  graphKeys = builtins.length col.graphKeys;
  inherit (col) aligned counts injected wrapped;
}' | nix run nixpkgs#jq -- -c .
```

```
{"aligned":true,"counts":{"base":2113,"extra":0,"user":4},"graphKeys":3979,"injected":[3],"live":3979,"subModules":2119,"wrapped":true}
```

0.90 s wall, 223 MB max RSS. `subModules = 2119 = 2113 base + 0 extra + pkgsModule + modulesModule + 4 user modules`, equal to the top-level length of `.graph` (topic 1); `live = 3979` equals the number of unique graph keys. `injected = [3]` is the last of the 4 user modules (the one `nixosSystem` appends).

One option, with the inactive definitions kept (re-run 2026-10-03; `only` restricts the options, `file` and `priority` are the fields of `options[].definitions[]`):

```sh
/usr/bin/env time -v nix eval --impure --json --expr '
let
  eval = (builtins.getFlake (builtins.getEnv "FLKREF")).nixosConfigurations.t;
  g = (import ./nix { }).extract { config = eval; only = [ [ "services" "openssh" "enable" ] ]; };
  o = builtins.head g.options;
in {
  defs = map (d: { inherit (d) file priority active condition; }) o.definitions;
  inherit (o) winners highestPrio;
  warnings = map (w: w.code) g.meta.warnings;
}' | nix run nixpkgs#jq -- -c '.defs[], { winners, highestPrio, warnings }'
```

```
{"active":false,"condition":"mkIf-false","file":"<nixpkgs>/nixos/modules/services/misc/nix-ssh-serve.nix","priority":null}
{"active":false,"condition":"mkIf-false","file":"<nixpkgs>/nixos/modules/services/networking/ssh/sshd.nix","priority":null}
{"active":true,"condition":null,"file":"<nixpkgs>/flake.nix","priority":1000}
{"active":true,"condition":null,"file":"<fixture>/mod.nix","priority":100}
{"active":true,"condition":null,"file":"<nixpkgs>/nixos/modules/services/networking/ssh/sshd.nix","priority":1500}
{"winners":[3],"highestPrio":100,"warnings":[]}
```

0.98 s wall, 241 MB max RSS; `attrNames eval.options` alone is 0.71 s / 196 MB (topic 7). The two `mkIf false` definitions are visible (priority unknown), the `mkDefault false` loses, the option default (1500) loses, the plain definition of `mod.nix` wins. The `flake.nix` file is the misattributed inline module (topic 5).

Winners against the module system: the lib compares them itself. For every option, `nix/extract.nix` reads the module system's `highestPrio` (its 9999 mapped to null) and `files` under `tryEval` and compares them with the reconstructed `highestPrio` and the sorted, unique files of the winning definitions. A difference is a `reconstruction-mismatch` warning; if either accessor throws, or the winner is unknown, the option is skipped (`selfcheck-skipped`). Result on `tests/fixture` (CLI, re-run 2026-10-03):

```sh
nix run . -- ./tests/fixture#nixosConfigurations.test -o /tmp/graph.json
nix run . -- ./tests/fixture#nixosConfigurations.test --all -o /tmp/all.json
jq '[.meta.warnings[] | select(.code == "reconstruction-mismatch")] | length' /tmp/graph.json /tmp/all.json
jq '.options | length' /tmp/graph.json /tmp/all.json
jq -r '[.meta.warnings[].code] | group_by(.) | map("\(.[0]) \(length)") | .[]' /tmp/all.json
```

```
0
0
25
16812
eval-crash 1
preview-crash 3
selfcheck-crash 9
selfcheck-skipped 37
```

0 reconstruction mismatches on the 25 options of the default scope and on all 16812 options of `--all`. In the `--all` run 16765 options were compared and matched; 37 were skipped (the module system's accessor threw, e.g. `services.graylog.package`: `graylog cannot be found in pkgs`, or `fixture.throws`), 9 crashed the self-check (the 8 options of topic 7 and the fixture's `fixture.whnfAbort`) and 1 was excluded (`fixture.aborting`). The three `preview-crash` entries are `fixture.whnfAbort`, `assertions` (`attribute 'cycle' missing`) and `system.build` (`attribute 'optionsDocBook' missing`).

Historical: the first prototype (since removed; `allDefs`/`finalize`/`defsFor` in an earlier `nix/reconstruct.nix`) was checked the same way on the research fixture and matched on 1713 of 1715 options that have definitions: `_module.args` mismatched (its definition from the internal module is not in `getSubModules`) and `services.graylog.package` was skipped (reference threw). 15351 raw definitions, 3.81 s / 645 MB for all of them. <!-- unverified: not repeated against the current code; replaced by the in-tool self-check above --> The two exceptions are module-system facts (re-run 2026-10-02):

```sh
nix eval --pure-eval --json "$FLKREF#nixosConfigurations.t" --apply 'c: c.options._module.args.files'
nix eval --pure-eval "$FLKREF#nixosConfigurations.t" --apply 'c: c.options.services.graylog.package.files' 2>&1 | grep -E 'error: graylog'
```

```
["lib/modules.nix","<unknown-file>","<nixpkgs>/nixos/modules/misc/nixpkgs.nix","<nixpkgs>/nixos/modules/misc/extra-arguments.nix"]
       error: graylog cannot be found in pkgs
```

Result attributes of a `nixosSystem` result, and what `_module` carries (re-run):

```sh
nix eval --pure-eval --json "$FLKREF#nixosConfigurations.t" --apply 'c: { result = builtins.attrNames c; moduleArgs = builtins.attrNames c._module.args; module = builtins.attrNames c._module; }'
```

```
{"module":["args","check","freeformType","specialArgs"],"moduleArgs":["baseModules","extendModules","extraModules","moduleType","modules","noUserModules","pkgs","utils"],"result":["_module","_type","class","config","extendModules","graph","lib","options","pkgs","type"]}
```

**Caveats.**

- The re-collection depends on `eval.type.getSubModules`, on `_module.args` of a `nixosSystem` result, and on re-implemented private logic; a change to `unifyModuleSyntax`, to `setDefaultModuleLocation` or to how `disabledModules` is processed can desync it. Three checks catch this at run time: `alignment-failed` (the top-level lists no longer line up), `graph-mismatch` (re-collected keys differ from `.graph`) and `reconstruction-mismatch` (winners differ from the module system's). `tests/assertions.jq` asserts that none of them fires on the fixture.
- Inactive definitions have unknown priority; the walk never forces the content of a `mkIf false` block to learn it.

**Consequence for optgraph.** Use the re-collection walk, scoped to the options the user asked for; compare winners with the module system's `files`/`highestPrio` on every run, not only in a test.

---

## 4. Priorities (lower number wins)

**Answer.**

| Name | Priority | Source |
|---|---|---|
| `mkOptionDefault` | 1500 | `mkOverride 1500`, `lib/modules.nix:1644` |
| `mkDefault` | 1000 | :1649 |
| plain definition (`defaultOverridePriority`) | 100 | :1651 |
| `mkImageMediaOverride` | 60 | :1657 |
| `mkForce` | 50 | :1662 |
| `mkVMOverride` | 10 | :1667 |
| option `default` | 1500 | wrapped with `mkOptionDefault`, :1144-1153 |
| `mkFixStrictness` | n/a | deprecated, returns its argument and warns, :1669 |

`mkOrder` is a separate axis: ordering inside one priority level, wrapper `_type = "order"` (`mkOrder` :1671). `mkBefore` = 500, `defaultOrderPriority` = 1000, `mkAfter` = 1500 (:1770-1772). Overrides have `_type = "override"`.

**Evidence.** Lines above, `nixpkgs@c59305b:lib/modules.nix`. `highestPrio` of an option with no definitions is 9999 (topic 2).

**Verified by eval** (re-run):

```sh
nix eval --impure --json --expr '
let m = (import (builtins.getEnv "NP" + "/lib")).modules; in {
  mkOptionDefault = (m.mkOptionDefault null).priority;
  mkDefault = (m.mkDefault null).priority;
  plain = m.defaultOverridePriority;
  mkImageMediaOverride = (m.mkImageMediaOverride null).priority;
  mkForce = (m.mkForce null).priority;
  mkVMOverride = (m.mkVMOverride null).priority;
  mkBefore = (m.mkBefore null).priority;
  defaultOrderPriority = m.defaultOrderPriority;
  mkAfter = (m.mkAfter null).priority;
  types = [ (m.mkForce null)._type (m.mkBefore null)._type ];
}'
```

```
{"defaultOrderPriority":1000,"mkAfter":1500,"mkBefore":500,"mkDefault":1000,"mkForce":50,"mkImageMediaOverride":60,"mkOptionDefault":1500,"mkVMOverride":10,"plain":100,"types":["override","order"]}
```

`lib.mkFixStrictness 1` prints `evaluation warning: lib.mkFixStrictness has no effect and will be removed...` and returns `1` (re-run).

**Caveats.** Priority numbers are conventions of the module system; a module can use `mkOverride n` with any `n`. Ties at the winning priority are merged by the option type, so several definitions can "win".

**Consequence for optgraph.** Store the numeric priority per definition; label the standard values by name only as a display aid.

---

## 5. Mapping files to flake inputs

**Answer.** Every file is a `/nix/store/<hash>-source[/sub]` path (or a non-path string), so a longest-prefix match against the input root paths works, with the pitfalls below.

Input names and paths come from `nix flake archive --dry-run --json <ref>`: `{ path, inputs: { <name>: { path, inputs } } }`, recursive. stderr can carry warnings (`Git tree ... is dirty`), so keep it out of stdout. `builtins.getFlake` in pure mode works only on locked refs.

Pitfalls:

- Nested inputs come out of the same JSON (`inputs.<name>.inputs.<child>`); the CLI names them `parent/child`.
- A relative `path:` input of the root flake (`inputs.extra.url = "path:./extra"`) has no `path` in the archive output; its location is in `nix flake metadata --json` (re-run 2026-10-03, on the repo fixture):

  ```sh
  nix flake archive --no-write-lock-file --dry-run --json ./tests/fixture 2>/dev/null
  nix flake metadata --no-write-lock-file --json ./tests/fixture 2>/dev/null | nix run nixpkgs#jq -- -c '{original: .original, extra: .locks.nodes.extra.locked}'
  ```

  ```
  {"inputs":{"extra":{"inputs":{}},"nixpkgs":{"inputs":{},"path":"/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source"}},"path":"/nix/store/wvzynj17a1lyps531kamccifmpp871s0-source"}
  {"original":{"dir":"tests/fixture","type":"git","url":"file:///home/ilyanix/optgraph"},"extra":{"path":"./extra","type":"path"}}
  ```

  The CLI resolves it as `<archive path>/<original.dir>/<locked.path>`, i.e. `…-source/tests/fixture/./extra`. The `/./` stays in strings built from the input (`"${extra}/interpolated.nix"` gives the module key `…/tests/fixture/./extra/interpolated.nix`), so optgraph normalizes `/./` in roots and files before matching. Only the root flake's relative inputs are resolved this way; those of nested flakes are skipped (code reading, `cli/optgraph.sh`).
- Two inputs with the same source share one store path, so a file maps to several input names.
- A subflake (`?dir=sub`) has `self.outPath = <root>/sub`, but its files can live anywhere in the source root. Match against `self.sourceInfo.outPath` / the archive `path`, not `outPath`.
- Git flakes copy tracked files only. A dirty tree gets a different store path, `dirtyRev` (`<rev>-dirty`) and no `rev`. `path:` flakes copy everything.
- `pkgs.path` equals the nixpkgs input path.
- `_file` strings (`"my-string-file"`) and `<unknown-file>` match nothing. `lib/modules.nix` (internal module) is relative and matches nothing. Option `declarations` are plain strings (`file = head opt.declarations`, `nixpkgs@c59305b:lib/modules.nix:1148`).
- Inline modules are misattributed to nixpkgs (below).
- `config._module` does not exist on the result's `config`; use the result's own `_module` (`_module.specialArgs`, `_module.args`). `_module.specialArgs` contains `inputs` only if the user passed them.

**Inline modules and `nixosSystem`.** `eval-config.nix` defaults `modulesLocation` to the file of the `modules` argument's position (`builtins.unsafeGetAttrPos "modules" evalConfigArgs`) and wraps every user module with `lib.setDefaultModuleLocation modulesLocation`. When called through `nixpkgs.lib.nixosSystem`, the `modules` argument is defined in nixpkgs' own `flake.nix`, so:

- Inline attrset/function user modules get `file = <nixpkgs>/flake.nix` and look like nixpkgs code.
- In the graph each user module is a wrapper node `{ key = ":anon-N"; file = <nixpkgs>/flake.nix; imports = [ real ]; }`; the real path module is its child.
- `nixosSystem` appends its own module `{ config.nixpkgs.flake.source = self.outPath; }` as the last element of `modules`, with the same `flake.nix` file.

Other anonymous or non-user modules in the top-level list:

- `pkgsModule` has `_file`/`key` = `<nixpkgs>/nixos/lib/eval-config.nix` (not anonymous).
- `modulesModule` is anonymous (`file = <unknown-file>`) and defines only `_module.args`.
- The last element of `nixos/modules/module-list.nix` is an inline attrset that sets `documentation.nixos.extraModules`; it is anonymous too (`:anon-2113`, `<unknown-file>` in the fixture).
- The internal module has `file = "lib/modules.nix"`.

A "file is not under nixpkgs" scope test therefore wrongly includes `_module.*`, `documentation.nixos.extraModules`, and wrongly excludes everything defined by inline user modules. Classify top-level modules by position instead: index in `baseModules`/`extraModules` versus the user `modules` list (`_module.args` of the result).

**Evidence.**

- `nixpkgs@c59305b:nixos/lib/eval-config.nix:26` (`modulesLocation`), :42-44 (`pkgsModule`), :73-79 (`userModules`), :81-90 (`noUserModules`), :93-104 (`modulesModule`), :106 (`extendModules`).
- `nixpkgs@c59305b:flake.nix:64-95` (`nixosSystem`; `modules = args.modules ++ [ ... ]` at :74-92, the injected module at :81-91).
- `nixpkgs@c59305b:lib/modules.nix:624-627` (`setDefaultModuleLocation` = `{ _file = file; imports = [ m ]; }`).
- `nixpkgs@c59305b:nixos/modules/module-list.nix:2117` (`documentation.nixos.extraModules`).
- `nixpkgs@c59305b:lib/options.nix:911` (`unknownModule = "<unknown-file>"`).

**Verified by eval** (re-run unless stated).

`nix flake archive` on the repo flake (stderr to a file):

```sh
nix flake archive --dry-run --json . 2>/tmp/docs-verify/archive.err | nix run nixpkgs#jq -- -c .; cat /tmp/docs-verify/archive.err
```

```
{"inputs":{"nixpkgs":{"inputs":{},"path":"/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source"}},"path":"/nix/store/l75sl8p4wjdlibs3k60hzfpj8vimw1zj-source"}
warning: Git tree '/home/ilyanix/optgraph' is dirty
```

`getFlake` on an unlocked ref in pure mode:

```sh
nix eval --pure-eval --expr 'builtins.getFlake "path:/tmp/docs-verify/flk"' 2>&1 | grep error:
```

```
error:
       error: cannot call 'getFlake' on unlocked flake reference 'path:/tmp/docs-verify/flk', at «none»:0 (use --impure to override)
```

Inline module misattribution, `pkgs.path`, `_module` (the option `system.stateVersion` is set by the first inline fixture module):

```sh
nix eval --pure-eval --json "$FLKREF#nixosConfigurations.t" --apply 'c: { inherit (c.options.system.stateVersion) files; pkgsPath = toString c.pkgs.path; specialArgs = builtins.attrNames c._module.specialArgs; configHasModule = c.config ? _module; }'
```

```
{"configHasModule":false,"files":["<nixpkgs>/flake.nix"],"pkgsPath":"<nixpkgs>","specialArgs":["modulesPath"]}
```

Naive scope test ("option with a definition file outside the nixpkgs path"), skipping the 8 options that crash `tryEval` (topic 7):

```sh
/usr/bin/env time -v nix eval --impure --json --expr '
let
  eval = (builtins.getFlake (builtins.getEnv "FLKREF")).nixosConfigurations.t;
  inherit (eval) lib;
  skip = [ "hardware.nvidia.gsp.enable" "hardware.nvidia.open" "hardware.nvidia.powerManagement.kernelSuspendNotifier"
           "services.dawarich.redis.host" "services.greenlight.redis.host" "services.immich.redis.host" "services.kener.redis.host"
           "services.nagios.cgiConfigFile" ];
  opts = lib.filter (o: !(lib.elem (lib.showOption o.loc) skip)) (lib.collect lib.isOption eval.options);
  np = toString eval.pkgs.path;
  outside = o: let r = builtins.tryEval (lib.any (f: !(lib.hasPrefix np f)) o.files); in r.success && r.value;
in map (o: lib.showOption o.loc) (lib.filter outside opts)'
```

```
["_module.args","_module.check","_module.freeformType","_module.specialArgs","documentation.nixos.extraModules","networking.hostName","services.openssh.enable"]
```

6.17 s wall, 872 MB max RSS. The four `_module.*` options and `documentation.nixos.extraModules` are false positives; `system.stateVersion`, `nixpkgs.hostPlatform`, `boot.loader.grub.enable`, `fileSystems` (inline user modules) are false negatives.

Subflake (`?dir=`):

```sh
mkdir -p /tmp/docs-verify/mono/sub
cat > /tmp/docs-verify/mono/sub/flake.nix <<'EOF'
{ outputs = { self }: { info = { selfOutPath = toString self.outPath; sourceOutPath = toString self.sourceInfo.outPath; }; }; }
EOF
echo x > /tmp/docs-verify/mono/top.txt
nix eval --json 'path:/tmp/docs-verify/mono?dir=sub#info'
nix flake archive --dry-run --json 'path:/tmp/docs-verify/mono?dir=sub' 2>/dev/null
```

```
{"selfOutPath":"/nix/store/7njqyf9xijs65vw0k3clvklhymf6cnda-source/sub","sourceOutPath":"/nix/store/7njqyf9xijs65vw0k3clvklhymf6cnda-source"}
{"inputs":{},"path":"/nix/store/7njqyf9xijs65vw0k3clvklhymf6cnda-source"}
```

Git flake clean vs dirty, and `path:` flake:

```sh
mkdir /tmp/docs-verify/gitflk && cd /tmp/docs-verify/gitflk && git init -q .
cat > flake.nix <<'EOF'
{ outputs = { self }: { info = { rev = self.rev or null; dirtyRev = self.dirtyRev or null; src = toString self.sourceInfo.outPath; }; }; }
EOF
echo tracked > tracked.txt; echo untracked > untracked.txt
git add flake.nix tracked.txt && git -c user.name=t -c user.email=t@example.invalid commit -qm init
nix eval --json 'git+file:///tmp/docs-verify/gitflk#info'
ls "$(nix eval --raw 'git+file:///tmp/docs-verify/gitflk#info.src')"
echo more >> tracked.txt
nix eval --json 'git+file:///tmp/docs-verify/gitflk#info'
ls "$(nix eval --raw 'path:/tmp/docs-verify/gitflk#info.src')"
```

```
{"dirtyRev":null,"rev":"8d402db0007bda0ed2678fc8472657fa307e41f3","src":"/nix/store/xvwinxh0fd9f99fwmchq49rp4b7f0036-source"}
flake.nix
tracked.txt
warning: Git tree '/tmp/docs-verify/gitflk' is dirty
{"dirtyRev":"8d402db0007bda0ed2678fc8472657fa307e41f3-dirty","rev":null,"src":"/nix/store/6yza7niadqgaj03x5ynyjmpp61ccgcva-source"}
flake.nix
tracked.txt
untracked.txt
```

Two inputs, one store path: `/tmp/docs-verify/two/flake.nix` declares `a = github:NixOS/nixpkgs/nixos-unstable` and `b = github:NixOS/nixpkgs/c59305bab2065cfecc4944690d9eedbb56f3a9fa`; its hand-written `flake.lock` carries the repo lock's `nixpkgs` node under both names:

```sh
nix flake archive --dry-run --json 'path:/tmp/docs-verify/two' --no-write-lock-file | nix run nixpkgs#jq -- -c '.inputs | map_values(.path)'
```

```
{"a":"/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source","b":"/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source"}
```

**Caveats.** `_module.args.modules` is the `modules` list as passed to `eval-config.nix`, which for `nixosSystem` already includes the appended `nixpkgs.flake.source` module (4 entries for the 3 fixture modules).

**Consequence for optgraph.** Resolve input roots with `nix flake archive`, match on the source root, classify user versus nixpkgs modules by position in the top-level module list, and never by file path alone.

---

## 6. `tryEval`

**Answer.** `builtins.tryEval` catches only errors raised by `throw` and failed `assert` (the `AssertionError` family) and evaluates only to WHNF. Every other error aborts the whole `nix eval` with exit 1. The Nix 2.34.8 manual says the same (`:doc builtins.tryEval` in `nix repl`): "tryEval only prevents errors created by throw or assert from being thrown. Errors tryEval doesn't catch are, for example, those created by abort and type errors generated by builtins."

The module system's own "option used but not defined" and "is not of type" errors are `throw`s and are caught. The module system also contains `abort` itself: `mkRenamedOptionModule`/`doRename` ("Renaming error") and `mergeEqualOption` (`This case should never happen.`).

**Evidence.**

- `nixpkgs@c59305b:lib/modules.nix:1298` ("accessed but has no value defined"), :1282 and :1291 ("is not of type").
- `nixpkgs@c59305b:lib/modules.nix:2198` (`abort "Renaming error: ..."`), :2210 (`apply = x: use (toOf config)`): the abort fires when the alias option's `value` is forced and the target path is absent from `config`.
- `nixpkgs@c59305b:lib/options.nix:503` (`abort "This case should never happen."`).
- Failed assertions and warnings are only enforced at `system.build.toplevel`: `nixpkgs@c59305b:nixos/modules/system/activation/top-level.nix:78`, `nixpkgs@c59305b:lib/asserts.nix:195-203` (`checkAssertWarn` throws).
- Which accessor forces what: `nixpkgs@c59305b:lib/modules.nix:1264` (`any (def: def.value._type or "" == "order") defsFiltered.values` forces each surviving definition's content to WHNF).
- Source of the claim that only `AssertionError` is caught (`ThrownError : AssertionError`): NixOS/nix 2.34-maintenance `src/libexpr/primops.cc`, `eval-error.hh`. <!-- unverified: source reading by the research agent, not available locally; the behaviour itself is verified below -->

**Verified by eval** (re-run, Nix 2.34.8). The script below prints the exit code of `nix eval` and its first error line; `exit=0` with `false` means `tryEval` caught the error.

```sh
# usage: run <name> <extra nix flags> <nix expr>
run() {
  out=$(timeout 120 nix eval $2 --expr "(builtins.tryEval ($3)).success" 2>&1); code=$?
  printf '%-30s exit=%s %s\n' "$1" "$code" "$(echo "$out" | grep -m1 -E '^(true|false)$|error: ' | cut -c1-100)"
}
mkdir -p /tmp/docs-verify/t6; echo 'x + 1' > /tmp/docs-verify/t6/undef.nix
run "throw"                    --impure 'throw "x"'
run "assert false"             --impure 'assert false; 1'
run "addErrorContext+throw"    --impure 'builtins.addErrorContext "ctx" (throw "x")'
run "shallow {a.b = throw}"    --impure '{ a.b = throw "x"; }'
run "abort"                    --impure 'abort "x"'
run "missing attribute"        --impure '{ }.a'
run "1 + string"               --impure '1 + "a"'
run "if non-bool"              --impure 'if 1 then 1 else 2'
run "assert non-bool"          --impure 'assert 1; 2'
run "call non-function"        --impure '1 2'
run "undefined variable"       --impure 'import /tmp/docs-verify/t6/undef.nix'
run "missing fn argument"      --impure '({ a }: a) { }'
run "toString {}"              --impure 'toString {}'
run "fromJSON parse"           --impure 'builtins.fromJSON "{"'
run "elemAt out of range"      --impure 'builtins.elemAt [ ] 0'
run "infinite recursion"       --impure 'let x = x; in x'
run "stack overflow"           --impure 'let f = n: 1 + f (n + 1); in f 0'
run "readFile missing"         --impure 'builtins.readFile /nonexistent/file'
run "toJSON function"          --impure 'builtins.toJSON (x: x)'
run "warn, abort-on-warn"      "--impure --abort-on-warn" 'builtins.warn "w" 1'
run "forbidden path (pure)"    --pure-eval 'builtins.readFile /etc/hostname'
run "fetchurl no hash (pure)"  --pure-eval 'builtins.fetchurl "https://example.invalid/x"'
run "fetchurl, DNS failure"    --impure 'builtins.fetchurl { url = "https://example.invalid/x"; sha256 = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="; }'
run "IFD disabled"             "--impure --option allow-import-from-derivation false" 'import (derivation { name = "ifd-probe"; system = "x86_64-linux"; builder = "/bin/sh"; })'
```

```
throw                          exit=0 false
assert false                   exit=0 false
addErrorContext+throw          exit=0 false
shallow {a.b = throw}          exit=0 true
abort                          exit=1        error: evaluation aborted with the following error message: 'x'
missing attribute              exit=1        error: attribute 'a' missing
1 + string                     exit=1        error: cannot add a string to an integer
if non-bool                    exit=1        error: expected a Boolean but found an integer: 1
assert non-bool                exit=1        error: expected a Boolean but found an integer: 1
call non-function              exit=1        error: attempt to call something which is not a function but an integer: 1
undefined variable             exit=1        error: undefined variable 'x'
missing fn argument            exit=1        error: function 'anonymous lambda' called without required argument 'a'
toString {}                    exit=1        error: cannot coerce a set to a string: { }
fromJSON parse                 exit=1        error: [json.exception.parse_error.101] parse error at line 1, column 2: syntax error while p
elemAt out of range            exit=1        error: 'builtins.elemAt' called with index 0 on a list of size 0
infinite recursion             exit=1        error: infinite recursion encountered
stack overflow                 exit=1        error: stack overflow; max-call-depth exceeded
readFile missing               exit=1        error: path '/nonexistent/file' does not exist
toJSON function                exit=1        error: cannot convert a function to JSON
warn, abort-on-warn            exit=1        error: aborting to reveal stack trace of warning, as abort-on-warn is set
forbidden path (pure)          exit=1        error: access to absolute path '/etc/hostname' is forbidden in pure evaluation mode (use '--i
fetchurl no hash (pure)        exit=1        error: in pure evaluation mode, 'fetchurl' requires a 'sha256' argument
fetchurl, DNS failure          exit=1        error: unable to download 'https://example.invalid/x': Could not resolve hostname (6) Could n
IFD disabled                   exit=1        error: cannot build '/nix/store/shdmp0pdlha17hxwc7y3d591vpn9rvdb-ifd-probe.drv^out' during ev
```

Module-system errors are caught (`true` = caught):

```sh
nix eval --impure --json --expr '
let
  lib = import (builtins.getEnv "NP" + "/lib");
  e = lib.evalModules { modules = [
    { options.nodef = lib.mkOption { type = lib.types.int; };
      options.wrong = lib.mkOption { type = lib.types.int; }; }
    { wrong = "str"; }
  ]; };
  caught = x: !(builtins.tryEval x).success;
in { nodef = caught e.config.nodef; wrong = caught e.config.wrong; }'
```

```
{"nodef":true,"wrong":true}
```

The renaming `abort` is not caught:

```sh
nix eval --impure --json --expr '
let
  lib = import (builtins.getEnv "NP" + "/lib");
  e = lib.evalModules { modules = [
    { imports = [ (lib.mkRenamedOptionModule [ "old" ] [ "settings" "foo" ]) ];
      options.settings = lib.mkOption {
        default = { };
        type = lib.types.submodule { freeformType = lib.types.attrsOf lib.types.int; };
      }; }
  ]; };
in (builtins.tryEval e.config.old).success' 2>&1 | grep -E 'error'
```

```
       error: evaluation aborted with the following error message: 'Renaming error: option `settings.foo' does not exist.'
```

A failing assertion does not stop option reads, only `toplevel`; and the accessor asymmetry (`boot.uki.configFile` has one `mkOptionDefault` definition whose content depends on `system.build.toplevel`):

```sh
nix eval --pure-eval --json "$FLKREF#nixosConfigurations.t" --apply 'c:
  let c2 = c.extendModules { modules = [ { assertions = [ { assertion = false; message = "boom"; } ]; } ]; };
  in {
    hostName = c2.config.networking.hostName;
    toplevelOk = (builtins.tryEval c2.config.system.build.toplevel.drvPath).success;
  }'
```

```
{"hostName":"from-mod","toplevelOk":false}
```

```sh
nix eval --pure-eval --json "$FLKREF#nixosConfigurations.t" --apply 'c:
  let
    c2 = c.extendModules { modules = [ { assertions = [ { assertion = false; message = "boom"; } ]; } ]; };
    o = c2.options.boot.uki.configFile;
    ok = x: (builtins.tryEval x).success;
  in { files = ok o.files; highestPrio = ok o.highestPrio; isDefined = ok o.isDefined; baseline = ok c.options.boot.uki.configFile.files; }'
```

```
{"baseline":true,"files":false,"highestPrio":true,"isDefined":false}
```

`toJSON` of a derivation forces `outPath` and writes the `.drv`; `.name` does not:

```sh
nix eval --impure --raw --expr 'builtins.toJSON (derivation { name = "probe-json"; system = "x86_64-linux"; builder = "/bin/sh"; })'; echo; ls /nix/store | grep -c -- '-probe-json.drv$'
nix eval --impure --raw --expr '(derivation { name = "probe-name"; system = "x86_64-linux"; builder = "/bin/sh"; }).name'; echo; ls /nix/store | grep -c -- '-probe-name.drv$'
```

```
"/nix/store/pqyq4dh910pg6ayz3zldsfky90d1l122-probe-json"
1
probe-name
0
```

An uncaught error can be attributed with `addErrorContext` (the marker shows up in the error output with `--show-trace`; here the probed option is `hardware.nvidia.open`, one of the uncatchable ones from topic 7):

```sh
nix eval --impure --show-trace --json --expr 'let eval = (builtins.getFlake (builtins.getEnv "FLKREF")).nixosConfigurations.t; lib = eval.lib; o = eval.options.hardware.nvidia.open; in (builtins.tryEval (builtins.addErrorContext "optgraph-probe: hardware.nvidia.open" (builtins.deepSeq [ o.files o.isDefined ] true))).success' 2>&1 | grep -E '… optgraph-probe|^error:'
```

```
error:
       … optgraph-probe: hardware.nvidia.open
```

Frame order (re-run 2026-10-03, Nix 2.34.8). Nix prints error-context frames outermost first, so with nested markers the innermost one is the last `… optgraph-marker:` line before the final `error:` line:

```sh
nix eval --show-trace --expr 'builtins.addErrorContext "optgraph-marker:outer:1" (builtins.addErrorContext "optgraph-marker:inner:2" (abort "x"))' 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E '… optgraph-marker|^ *error:'
```

```
error:
       … optgraph-marker:outer:1
       … optgraph-marker:inner:2
       error: evaluation aborted with the following error message: 'x'
```

(The full output has blank lines and a `… while calling the 'abort' builtin` frame between the second marker and the final `error:` line; the `grep` drops them.) The CLI relies on this: `grep -o '… optgraph-marker:[a-z]*:.*$' | tail -n 1` after stripping colours (`cli/optgraph.sh`, `localize`). The same pipeline on this command prints `… optgraph-marker:inner:2`.

More errors `tryEval` does not catch, found while building optgraph (re-run 2026-10-03; each exits 1 although it sits inside `tryEval`):

| Case | Error |
|---|---|
| A string with store-path context used as an attribute name. Module keys of `"${input}/module.nix"` imports carry such context. | `the string '/nix/store/…-a' is not allowed to refer to a store path (such as '/nix/store/…-a')` |
| `_module.args.baseModules` read on a result that has none (not built by `eval-config.nix`) | `attribute 'baseModules' missing` |
| `foldl' recursiveUpdate` over thousands of locs under one prefix: the values are a chain of thunks as deep as the list | `stack overflow; max-call-depth exceeded` (with 2500 locs; 2000 still pass) |

```sh
nix eval --impure --expr 'let f = builtins.toFile "a" "b"; in (builtins.tryEval { ${f} = 1; }).success'
nix eval --impure --expr 'let lib = import (builtins.getEnv "NP" + "/lib"); e = lib.evalModules { modules = [ ]; }; in (builtins.tryEval e._module.args.baseModules).success'
nix eval --impure --expr 'let lib = import (builtins.getEnv "NP" + "/lib"); locs = builtins.genList (i: [ "a" (toString i) ]) 2500; in (builtins.tryEval (builtins.deepSeq (builtins.foldl'"'"' (acc: l: lib.recursiveUpdate acc (lib.setAttrByPath l true)) { } locs) true)).success'
```

How optgraph avoids them: attribute names go through `builtins.unsafeDiscardStringContext` (`noCtx` in `nix/collect.nix` and `nix/extract.nix`); `collect.nix` tests `a ? baseModules && a ? extraModules && a ? modules` before reading (`e._module.args ? baseModules` is `false` on the evalModules result above); `trieOf` in `nix/reconstruct.nix` groups with `builtins.groupBy` (20000 locs under one prefix: 0.05 s, 47 MB, no overflow).

**Caveats.**

- `tryEval` is shallow: `tryEval { a.b = throw ""; }` succeeds. Force what you need inside the `tryEval`.
- Accessors differ. `highestPrio` forces each definition's outer value (the wrapper) and the `mkIf` conditions. `files`, `definitions`, `definitionsWithLocations` and `isDefined` additionally force each surviving definition's content to WHNF (the `_type == "order"` test at :1264). Above, the failing assertion breaks `files`/`isDefined` of `boot.uki.configFile` (a caught `throw`) but not `highestPrio`. A bad definition therefore raises through `files`/`definitionsWithLocations` more often than through `highestPrio`.
- Reading option values never checks assertions or warnings; only `system.build.toplevel` does.
- Pure mode adds its own uncatchable errors (forbidden path, `fetchurl` without hash). IFD disabled and network failures are uncatchable too.
- `toJSON` of a derivation instantiates it. Never serialize derivations; read `.name` only.

**Consequence for optgraph.**

- Wrap every per-option access in `tryEval` and read only `typeOf`/`isFunction`/`isAttrs`/`isList` with depth and length limits far below `max-call-depth` (10000); derivations through `.name` only (never `outPath`/`drvPath`); guard `__functor`/`__toString`; `tryEval` each leaf separately.
- Uncatchable errors need a process-level answer: restart with the failing unit degraded or excluded, localized from `addErrorContext` markers in the `--show-trace` output; bisection of the in-scope options as a fallback when there is no marker. The lib wraps loading a module (`module`), walking its `config` (`walk`), reconstructing an option, its self-check and its preview in markers `optgraph-marker:<stage>:<json subject>`; the CLI takes the innermost one (frame order above). Verified by `tests/e2e.sh` (re-run 2026-10-03, all checks passed):
  - marker path: the fixture's `abort` option (`fixture.aborting`, stage `reconstruct`) is excluded; the option that aborts only when its value is forced (`fixture.whnfAbort`) is degraded in two recoveries, first the self-check, then the preview; the warnings carry the Nix error text; 4 evaluations, 3 crash recoveries;
  - bisection (`OPTGRAPH_LOCALIZE=bisect`, markers ignored): both aborting options end up excluded (coarser: `fixture.whnfAbort` loses its definitions too); 17 evaluations, 2 crash recoveries;
  - budget: `--max-retries 1` ends with exit 3, `complete: false` and a `budget-exhausted` warning;
  - exit codes 1 and 2 for usage errors, an unresolvable flake, a missing attribute and a value that is not a configuration, with no output file.
  No fixture case exercises the `module` and `walk` stages; they use the same mechanism but are not covered by a test.

---

## 7. Cost and laziness

**Answer.** Enumerating options is cheap; touching what is under each option is not. Never force `opt.value`.

All numbers: fixture configuration above (`nixosConfigurations.t`), 16798 options, wall time and max RSS from `/usr/bin/env time -v`. The research run's numbers are partial: that agent hit a rate limit, and wall/RSS were logged for the small configs only.

| Operation | Re-run (this document) | Research run (partial) |
|---|---|---|
| `attrNames eval.options` | 0.71 s, 196 MB | 0.67 s, 191 MB |
| `lib.collect lib.isOption eval.options`, count | 16798 options, 0.80 s, 244 MB | 16798 options, 0.64 s CPU, 384 MB GC heap |
| `.highestPrio` on all, under `tryEval` | 3.90 s, 668 MB, 0 failures | ~3.6 s CPU, 432 MB heap, 0 uncatchable, 0-47 caught |
| `.files` + `.isDefined` + `.definitionsWithLocations` on all, no exclusions | exit 1 (uncatchable, first at `hardware.nvidia.open`) | 8 uncatchable crashes |
| same, 8 options excluded | 6.39 s, 882 MB, 34 caught failures | 4.6-6.5 s, 715-865 MB, 35-92 caught |
| `seq opt.value` on all | not re-run | 1395 caught, 60 uncatchable (mostly `abort` "Renaming error", plus missing attributes), ~4.4-5.5 s, 650-800 MB per run once excluded, 18 evaluation warnings |
| naive scope filter (topic 5) | 6.17 s, 872 MB, 7 options | 2.5-3.8 s, 580-650 MB, 6-9 options |
| `extract { only = [ one option ]; }` (topic 3, re-run 2026-10-03) | 0.98 s, 241 MB | n/a |
| first prototype, all definitions (topic 3, historical, since removed) | 3.81 s, 645 MB | ~4.0 s, 643 MB |
| first prototype, single option (historical) | 0.74 s, 212 MB | ~0.7 s, 206 MB |
| CLI on `tests/fixture`, default scope / `--all` (README) | 6.97 s, 564 MB / 84 s, 1.43 GB | n/a |

<!-- unverified: the research-run column, research run 2026-10-02; the two historical rows were not repeated against the current code; the CLI row is the verifier's run of 2026-10-03 (nix 2.34.8, nixpkgs c59305b), my own runs gave 6.66 s / 564 MB and 84.6 s / 1.43 GB -->

The CLI row is a different configuration (the repo fixture, 25 options in scope for the default run, 16812 with `--all`), not `nixosConfigurations.t`.

The 8 uncatchable options, each reproduced individually in the re-run (probe: `deepSeq [ o.files o.isDefined ]` under `tryEval`, exit 1):

| Option | Error |
|---|---|
| `hardware.nvidia.open` | `expected a set but found null: null` |
| `hardware.nvidia.gsp.enable` | same, via `hardware.nvidia.open` |
| `hardware.nvidia.powerManagement.kernelSuspendNotifier` | same, via `hardware.nvidia.open` |
| `services.{dawarich,greenlight,immich,kener}.redis.host` | `attribute '<name>' missing` |
| `services.nagios.cgiConfigFile` | `cannot coerce null to a string: null` |

`.highestPrio` on all options neither crashed nor failed (table above), so these errors come from forcing definition contents, which `files` does and `highestPrio` does not (topic 6).

Commands (re-run; `cost` prints the result and the time/RSS lines):

```sh
cost() { /usr/bin/env time -v nix eval --impure --json --expr "$1" 2>/tmp/docs-verify/tm.err | cut -c1-200; echo; grep -E 'Elapsed|Maximum resident|Exit status' /tmp/docs-verify/tm.err; }
PRE='let eval = (builtins.getFlake (builtins.getEnv "FLKREF")).nixosConfigurations.t; inherit (eval) lib; opts = lib.collect lib.isOption eval.options;'
cost "$PRE in builtins.length (builtins.attrNames eval.options)"
cost "$PRE in builtins.length opts"
cost "$PRE in builtins.length (builtins.filter (o: !(builtins.tryEval o.highestPrio).success) opts)"
cost "$PRE in builtins.length (builtins.filter (o: !(builtins.tryEval (builtins.deepSeq [ o.files o.isDefined (builtins.length o.definitionsWithLocations) ] true)).success) opts)"
cost "$PRE skip = [ \"hardware.nvidia.gsp.enable\" \"hardware.nvidia.open\" \"hardware.nvidia.powerManagement.kernelSuspendNotifier\" \"services.dawarich.redis.host\" \"services.greenlight.redis.host\" \"services.immich.redis.host\" \"services.kener.redis.host\" \"services.nagios.cgiConfigFile\" ]; in builtins.length (builtins.filter (o: !(builtins.elem (lib.showOption o.loc) skip) && !(builtins.tryEval (builtins.deepSeq [ o.files o.isDefined (builtins.length o.definitionsWithLocations) ] true)).success) opts)"
```

Results, in order: `54` (0.71 s, 196228 KB), `16798` (0.80 s, 244216 KB), `0` (3.90 s, 668116 KB), no output and exit 1 (3.88 s), `34` (6.39 s, 882496 KB).

**Caveats.**

- The number of caught failures depends on the configuration (34 here, 35-92 in the research run). A failing assertion in the config adds throws, e.g. `boot.uki.configFile` (topic 6).
- The scope test cost is dominated by touching every option; it does not shrink with a smaller output.

**Consequence for optgraph.**

- Never force `opt.value`; preview definitions instead. For the option default prefer `defaultText`.
- Determine scope by walking only the non-nixpkgs modules (topic 3), not by touching every option.
- Prefer `highestPrio` over `files` where only the priority is needed.
- `--all` will hit uncatchable crashes on stock nixpkgs and needs a larger restart budget. Re-run 2026-10-03, CLI on `tests/fixture` with `--all`: 13 crash recoveries (9 in the self-check, 3 in the preview, 1 in reconstruct), 10 of them in stock options. The self-check reads `files`, so it hits the 8 options of the table above; the preview hit `assertions` (`attribute 'cycle' missing`) and `system.build` (`attribute 'optionsDocBook' missing`). Those two were optgraph's doing: the preview forced values that are lazy by design. An assertion's `message` is meant to be evaluated only when its assertion fails; `nixpkgs@c59305b:nixos/modules/tasks/filesystems.nix:451-452` checks `!(fileSystems' ? cycle)` and its message reads `fileSystems'.cycle`, which exists only when the check fails. Since 2026-10-05 the options `assertions` and `warnings` are never previewed, and attrsets below the top level of a preview show their names only, so such fields are not forced; the fixture covers both (`tests/fixture/modules/lazy.nix`). The CLI's default for `--all` is 100 recoveries.

---

## 8. Purity

**Answer.** optgraph needs no `--impure` anywhere. The extraction is `nix --option pure-eval true eval --json --show-trace --no-write-lock-file <ref>#nixosConfigurations.<host> --apply '<expr>'` (`nix_flags` in `cli/optgraph.sh`); `--apply` is evaluated purely and receives the evaluated configuration. The CLI never passes `--impure`.

The `--option pure-eval true` is a guard. In Nix 2.34.8 flake installables are pure by default, and a `pure-eval = false` in `nix.conf` or `NIX_CONFIG` is ignored (`nix config show pure-eval` still prints `true`); only a command-line `--option pure-eval false` or `--impure` lifts it. Re-run 2026-10-03, `builtins ? currentTime` on a flake installable via `--apply` (`false` = pure; `$FLKREF` is the research flake; all with `--extra-experimental-features 'nix-command flakes'`):

| Settings | Result |
|---|---|
| none | `false` |
| `NIX_CONFIG='pure-eval = false'` | `false` |
| `nix.conf` with `pure-eval = false` (`NIX_CONF_DIR`, `NIX_USER_CONF_FILES=/dev/null`) | `false` |
| `--option pure-eval false` | `true` |
| `--option pure-eval true` (what the CLI passes) | `false` |

```sh
nix eval --no-write-lock-file "$FLKREF#nixosConfigurations.t" --apply 'c: builtins ? currentTime'
```

(`--option pure-eval ...` goes before `eval`; the other settings are set in the environment.) The CLI on the fixture under `NIX_CONFIG='pure-eval = false'` still writes 21 modules and 25 options. The guard therefore changes nothing on this Nix version; it matters only if a version honours the config setting.

What pure `--apply` can and cannot do (re-run; see the table below):

- It cannot `import` an absolute path or store path unless that path belongs to the evaluated flake (its own source or one of its inputs). An unrelated store path is forbidden.
- It cannot use `builtins.storePath` or `builtins.path` on a non-store path, and `fetchTree` on an unlocked input.
- It can use `builtins.fetchTree { type = "path"; path = <store path>; narHash = "sha256-..."; }`. The tool's own flake exposes `self.narHash`, so the CLI can embed the tool's source path and `narHash` and `import` from the result.
- `builtins.currentTime` and `builtins.currentSystem` do not exist; `builtins.getEnv` returns `""`. `generatedAt` is therefore passed in by the CLI.
- `builtins.getFlake` of optgraph's own flake is avoided: it would fetch all of its locked inputs. <!-- unverified: reasoning, not tested -->
- Inlining the extraction code into `--apply` hits the ~128 KiB argument limit. <!-- unverified: not tested, research run, 2026-10-02 -->
- `--override-input` is a no-op for foreign flakes. <!-- unverified: research run, 2026-10-02; my attempt to test it failed on an unrelated error -->
- `nix eval --file` is impure by default and is not used (re-run: with `pure-eval` at its default `true`, `nix eval --file` still allowed `builtins.currentSystem` and an absolute-path `import`, while `--expr` did not).

What goes into `meta` (`nix/extract.nix`): `nixpkgsVersion` is `lib.version` of the configuration's own lib (`config.lib`, the one from the nixpkgs flake; `config.pkgs.lib` if there is none), `nixpkgsRev` is `config.system.nixos.revision` under `tryEval` (`null` when it throws or is not a string). A `lib` imported by path would say `26.11pre-git` and has no revision (caveat below), so the lib is never imported from the nixpkgs store path. A foreign flake without a lock file needs `--no-write-lock-file`. A dirty git flake has `dirtyRev` and no `rev`; nothing about it is impure (topic 5).

**Evidence.**

- `nixpkgs@c59305b:nixos/modules/misc/version.nix:107-112` (`revision` defaults to `trivial.revisionWithDefault null`).
- `nixpkgs@c59305b:lib/flake-version-info.nix:16-19` (`versionSuffix` from `lastModifiedDate`/`shortRev`, `revisionWithDefault = default: self.rev or default`).

**Verified by eval** (re-run). `--pure-eval` explicit; it is the default here anyway.

```sh
SP=/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source          # nixpkgs: an input of the evaluated flake
OTHER=/nix/store/l75sl8p4wjdlibs3k60hzfpj8vimw1zj-source       # a store path unrelated to that flake
NH=sha256-69xHQhAeMAD2wDXO7T2pcOZIF9Sga2W+JkmY2a11Ops=         # narHash of $SP, from flake.lock
t() { printf '%-28s ' "$1"; nix eval --pure-eval --json "$FLKREF#nixosConfigurations.t" --apply "$2" 2>&1 | grep -m1 -E '^[0-9"{\[tf]|error: ' | cut -c1-110; }
t "import input store path"   "c: (import $SP/lib).version"
t "import unrelated store path" "c: import $OTHER/flake.nix"
t "builtins.storePath"        "c: builtins.storePath $OTHER"
t "builtins.path, non-store"  "c: builtins.path { path = /tmp/docs-verify/flk; }"
t "fetchTree path, no hash"   "c: (builtins.fetchTree { type = \"path\"; path = \"$OTHER\"; }).outPath"
t "fetchTree path + narHash"  "c: (builtins.fetchTree { type = \"path\"; path = \"$SP\"; narHash = \"$NH\"; }).outPath"
t "currentTime"               "c: builtins.currentTime"
t "currentSystem"             "c: builtins.currentSystem"
t "getEnv"                    "c: builtins.getEnv \"HOME\""
t "nixos.revision"            "c: c.config.system.nixos.revision"
t "nixos.version"             "c: c.config.system.nixos.version"
t "lib.version"               "c: c.lib.version"
```

```
import input store path      "26.11pre-git"
import unrelated store path         error: access to absolute path '/nix/store/l75sl8p4wjdlibs3k60hzfpj8vimw1zj-source/flake.nix' is forbid
builtins.storePath                  error: 'builtins.storePath' is not allowed in pure evaluation mode
builtins.path, non-store            error: access to absolute path '/tmp/docs-verify/flk' is forbidden in pure evaluation mode (use '--impu
fetchTree path, no hash             error: in pure evaluation mode, 'fetchTree' doesn't fetch unlocked input 'path:/nix/store/l75sl8p4wjdli
fetchTree path + narHash     "/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source"
currentTime                  error: attribute 'currentTime' missing
currentSystem                error: attribute 'currentSystem' missing
getEnv                       ""
nixos.revision               "c59305bab2065cfecc4944690d9eedbb56f3a9fa"
nixos.version                "26.11.20261001.c59305b"
lib.version                  "26.11.20261001.c59305b"
```

The 1st line shows that importing from an input of the evaluated flake works in pure mode; `lib.version` through `import $SP/lib` is `26.11pre-git`, through the flake's `c.lib` it is `26.11.20261001.c59305b`.

`self.narHash` is the NAR hash of the flake's store path (re-run):

```sh
mkdir -p /tmp/docs-verify/selfhash
cat > /tmp/docs-verify/selfhash/flake.nix <<'EOF'
{ outputs = { self }: { info = { narHash = self.narHash; path = toString self.outPath; }; }; }
EOF
nix eval --json 'path:/tmp/docs-verify/selfhash#info'
nix path-info --json "$(nix eval --raw 'path:/tmp/docs-verify/selfhash#info.path')" 2>/dev/null | nix run nixpkgs#jq -- -c 'to_entries[0].value.narHash // .[0].narHash'
```

```
{"narHash":"sha256-a4FvNeDnuBYHCr3LoyQd5GNabo9i8+vJhtn/4wxSdgE=","path":"/nix/store/1s6msq212s3sb6mshvrwnr2k1ipr85m9-source"}
"sha256-a4FvNeDnuBYHCr3LoyQd5GNabo9i8+vJhtn/4wxSdgE="
```

Lock file handling (a `path:` flake with a `path:` input and no `flake.lock`; stderr is shown):

```sh
mkdir -p /tmp/docs-verify/nolock
cat > /tmp/docs-verify/nolock/flake.nix <<'EOF'
{
  inputs.x.url = "path:/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source";
  outputs = { self, x }: { v = x.lib.version; };
}
EOF
nix eval --json 'path:/tmp/docs-verify/nolock#v' 2>&1
ls /tmp/docs-verify/nolock; rm /tmp/docs-verify/nolock/flake.lock
nix eval --no-write-lock-file --json 'path:/tmp/docs-verify/nolock#v' 2>&1
ls /tmp/docs-verify/nolock
```

```
warning: creating lock file "/tmp/docs-verify/nolock/flake.lock": 
• Added input 'x':
    'path:/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source?lastModified=0&narHash=sha256-69xHQhAeMAD2wDXO7T2pcOZIF9Sga2W%2BJkmY2a11Ops%3D' (1970-01-01)
"26.11.19700101.dirty"
flake.lock
flake.nix
warning: not writing modified lock file of flake 'path:/tmp/docs-verify/nolock':
• Added input 'x':
    'path:/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source?lastModified=0&narHash=sha256-69xHQhAeMAD2wDXO7T2pcOZIF9Sga2W%2BJkmY2a11Ops%3D' (1970-01-01)
"26.11.19700101.dirty"
flake.nix
```

The first run writes `flake.lock`; with `--no-write-lock-file` the second does not.

**Caveats.**

- The pure-mode allowance for the evaluated flake's own store paths means a tool path that is not part of the foreign flake is not importable; the `fetchTree` + `narHash` route is the way in.
- For a `lib` that is not the flake's, `trivial.revisionWithDefault` reads `.git` / `.git-revision` next to `lib/` and otherwise returns its default, which `config.system.nixos.revision` sets to `null` (`nixpkgs@c59305b:lib/trivial.nix:534-545`); `trivial.versionSuffix` falls back to `pre-git` (:512-516). <!-- unverified: code reading for the revision part; `26.11pre-git` was run (table above) -->
- `getFlake` on the nixpkgs store path as a `path:` flake has no `rev` and a dirty version (re-run); the pinned flake input has the real rev (table above):

  ```sh
  nix eval --impure --json --expr 'let f = builtins.getFlake "path:/nix/store/fd5qgysvbfx27wj651swlr899w6jnbwd-source"; in { v = f.lib.version; rev = f.rev or null; }'
  ```

  ```
  {"rev":null,"v":"26.11.19700101.dirty"}
  ```
- `nix eval` of a git flake with a dirty tree prints `warning: Git tree ... is dirty` on stderr; this is not an error.

**Consequence for optgraph.** No `--impure`, no `getFlake`: run `nix eval --json <ref> --apply <expr>` (the CLI adds `--option pure-eval true`, `--no-write-lock-file` and `--show-trace`, never `--impure`) with the tool's source supplied through `fetchTree { type = "path"; path; narHash; }`, pass `generatedAt` from the CLI, and take `nixpkgsVersion` from `config.lib.version` and `nixpkgsRev` from `config.system.nixos.revision`.
