# graph.json schema

`schema/graph.schema.json` (JSON Schema 2020-12) is the source of truth. This file explains what the fields mean and which invariants hold. Statements here were checked against `nix/*.nix`, `cli/optgraph.sh` and a run on `tests/fixture` (nixpkgs `c59305b`, Nix 2.34.8, 2026-10-03) unless marked otherwise.

All objects set `additionalProperties: false` and list every field below as required, except `definitions[].file`, which is absent when it equals its module's file; other nullable fields are present with value `null`. There is no separate schema version; `meta.toolVersion` identifies the producer.

`optgraph --html FILE` embeds the same document, unchanged, in the viewer page's `<script type="application/json" id="optgraph-data">` element; every `<` in it is written as `\u003c`, so `JSON.parse` of the element's text gives the document back.

```
graph.json = { meta, modules[], options[] }
```

## `meta`

| Field | Type | Meaning |
|---|---|---|
| `host` | string or null | Host name: the attribute after `nixosConfigurations.` in the CLI argument (quotes removed). `null` only when the lib is called without it. |
| `nixpkgsRev` | string or null | `config.system.nixos.revision`. `null` when it cannot be read; seen with a configuration built by importing `nixos/lib/eval-config.nix` directly instead of through `nixosSystem`. |
| `nixpkgsVersion` | string | `lib.version` of the evaluated configuration (`config.lib`), e.g. `26.11.20261001.c59305b`. `"unknown"` only in the CLI's last-resort document (see `complete`). |
| `generatedAt` | string or null | UTC timestamp `YYYY-MM-DDTHH:MM:SSZ`, set by the CLI (pure evaluation has no clock). `null` when the lib is called directly without it. |
| `toolVersion` | string | `0.1.0+<short rev>`, `-dirty` suffix for a dirty tree, `+unknown` without git info. |
| `scope` | `"user"` or `"all"` | `"user"`: default scope filter. `"all"`: CLI flag `--all`. See [Scope](#scope). |
| `attribution` | `"aligned"` or `"file-based"` | How top-level modules were classified. `"aligned"`: the evaluation's `getSubModules` list was lined up with `_module.args.baseModules`, `extraModules` and `modules` of `nixos/lib/eval-config.nix`, and the module `nixosSystem` injects (it sets only `nixpkgs.flake.source`) was found by content as the last user module. `"file-based"`: fallback; origins come from file paths and positions only, `modulesIndex` is always `null`, the `setDefaultModuleLocation` wrapper nodes are not collapsed (they show up as extra anonymous modules), and an `alignment-failed` warning is present. |
| `complete` | boolean | `false` only when the CLI ran out of crash recoveries or time and wrote a partial document (CLI exit code 3). The lib itself always reports `true`. |
| `warnings` | array of warning | Order: lib-level warnings (alignment, version, inputs, modules, graph), then per-option warnings in option-path order, then the CLI's `budget-exhausted`. |

Partial documents (`complete: false`): the CLI keeps the crash exclusions it already found, then runs up to two more evaluations, each limited to `max(120, time budget / 4)` seconds. First with previews and self-check disabled for every option (so every `valuePreview` is `null` and there are no `reconstruction-mismatch` warnings). If that run fails too (crash or timeout), once more with an empty option list (modules only). If that fails as well, it writes a document with empty `modules` and `options`, `nixpkgsVersion: "unknown"`, `attribution: "file-based"` and no warnings other than `budget-exhausted`. The last step is from code reading. The first was run (`--max-retries 2` on the fixture: 25 options, no previews, exit 3), and so was the second (`--time-budget 1`: the first evaluation is killed, the partial run without previews still hits the fixture's deliberate `abort`, the modules-only run gives 21 modules, 0 options, exit 3).

### `warnings[]`

`{ code, message, subject }`, all required. `subject` is a string or `null`:

- an option path (`fixture.aborting`), exactly as in `options[].path`;
- a module slot: the position-based name `<parent key>:anon-<n>` (`n` = 1-based index in the parent's `imports`; top-level parent key is empty, giving `:anon-3`). It equals `modules[].id` for anonymous modules only; a module loaded from a path has the path as its id but its slot is still `<parent key>:anon-<n>`;
- an input source root (store path), for `input-ambiguous`;
- `null`.

For `eval-crash`, `preview-crash` and `selfcheck-crash` the CLI appends `":\n"` and the trimmed Nix error (last error block of stderr, at most 20 lines) to `message`.

| Code | Emitted by | Subject | When |
|---|---|---|---|
| `alignment-failed` | lib | `null` | Top-level modules could not be lined up with eval-config's lists. `attribution` becomes `"file-based"`. The message names the reason: `_module.args` lacks `baseModules`/`extraModules`/`modules`; `getSubModules` has a different length than expected; only some user modules carry a location wrapper, or the wrappers disagree; the module `nixosSystem` injects was not found, or is not the last user module. Run with a configuration built by `eval-config.nix` directly: `the module lib.nixosSystem injects (nixpkgs.flake.source) was not found; ...`. |
| `nixpkgs-untested` | lib | `null` | `major.minor` of `lib.version` is newer than the newest tested release (`26.11`, `nix/supported.nix`). Older than `25.11` is not a warning: the run fails with exit code 2 (see [Fatal errors](#fatal-errors)). The newer-than-tested branch is from the code; it was not run. |
| `input-ambiguous` | lib | source root | Several flake inputs resolve to the same source root. Files below it are attributed to the first name in alphabetical order: `inputs ["extra","extra2"] share one source; attributed to input:extra` (run with two `path:` inputs pointing at one directory). |
| `module-error` | lib | module slot | Loading the module threw (`throw`/`assert`): it is treated as empty. Or walking its `config` threw: its definitions are missing from `options`. |
| `graph-mismatch` | lib | `null` | The module keys re-collected by optgraph differ from the module system's own `.graph` (checked only when no module failed to load). The message gives both counts and up to three keys unique to each side. Indicates a bug in the re-collection or a nixpkgs change. |
| `eval-crash` | lib, message extended by CLI | option path or module slot | An uncatchable error (e.g. `abort`) was localized to this option's reconstruction, to loading this module (stage `module`) or to walking its `config` (stage `walk`), and the run was repeated without it. The option is still listed, with `definitions: []`, `type: null`, `declaredIn: []` and `error` set. The module is excluded: it is replaced by an empty stub (no imports, no definitions). |
| `preview-crash` | lib, message extended by CLI | option path | Uncatchable error while previewing this option's values. All `valuePreview` of the option are `null`. |
| `selfcheck-crash` | lib, message extended by CLI | option path | Uncatchable error in the self-check of this option. The self-check is skipped for it. |
| `selfcheck-skipped` | lib | option path | The self-check could not run: the module system's `highestPrio`/`files` threw (`throw`/`assert`), or the winner is unknown (an active definition has an unknown priority, or an `mkIf` condition failed). |
| `reconstruction-mismatch` | lib | option path | The reconstructed `highestPrio` or the files of the winning definitions differ from the module system's `highestPrio` and `files`. Should not happen; it means the reconstruction is wrong for this option. The message shows both sides. |
| `budget-exhausted` | CLI | `null` | The crash-recovery limit (`--max-retries`: the N+1th crash) or the time budget (`--time-budget`; an evaluation killed by `timeout` exits 124) was reached. Always accompanied by `meta.complete: false`. |

## `modules[]`

One record per module of the configuration's module tree, deduplicated by key. Record order is the order in which the module system loads modules (top-level modules first, then their imports); disabled modules come last. Do not rely on it.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The module's key; unique across `modules`. A path module: its absolute path. An anonymous module (attrset or function): `<parent key>:anon-<n>` (see slot above), e.g. `:anon-2120:anon-1`. A module with an explicit `key` uses it. A key derived from a string is kept as is: `"${extra}/interpolated.nix"` with a relative `path:` input `extra` gives `…/tests/fixture/./extra/interpolated.nix`, the `/./` included. The number in a top-level anonymous id is the index in the whole top-level module list (about 2100 base modules come first), so it changes with nixpkgs; do not treat it as stable across revisions. |
| `file` | string | The module's `_file`. For path modules the path itself. For anonymous modules the parent's `_file`, or `<unknown-file>` at top level. For inline modules in nixosSystem's `modules` list this is nixpkgs' own `flake.nix` (eval-config's `modulesLocation`), not your file; use `origin` and `position`. |
| `imports` | array of string | Ids of the modules this one imports, sorted, unique, restricted to ids that appear in `modules` (disabled modules count: they stay listed). Empty for a disabled module. |
| `origin` | string | `user`, `user-inline`, `nixpkgs`, `input:<name>` or `unknown`. See below. |
| `disabled` | boolean | `true` if some module's `disabledModules` excludes this key. The record is listed (always, whatever the scope) so that import edges resolve; the module is not part of the evaluation, so `imports` is empty and it defines nothing. |
| `modulesIndex` | integer or null | Index in the `modules` list given to `nixosSystem`. Set only for the module that is itself an entry of that list, has origin `user-inline`, and only in `"aligned"` attribution. `null` otherwise: for anonymous modules nested in such an entry (they have no index of their own), for path modules, for entries whose origin is not `user-inline` (an inline module exported by an input, an entry with a real `_file`), and in `"file-based"` attribution. |
| `position` | string or null | `<file>:<line>` of the module's alphabetically first attribute (`attrNames` order: for `{ options = ...; imports = ...; }` it is `imports`, not `options`). `null` for path modules (key is their path), for a top-level module that is an `mkIf`/`mkOverride` value, for modules without attributes, and for modules that failed to load or were excluded. |

### `origin`

Pattern: `^(user|user-inline|nixpkgs|unknown|input:.+)$`. The rules apply in this order; the first match decides. They are the same for `"aligned"` and `"file-based"` attribution, except where the group is mentioned.

1. Path modules (key equals an absolute path): longest-prefix match of the path against the nixpkgs source root (the configuration's `modulesPath`), the root flake's source (`user`), and the source root of each flake input (`input:<name>`; nested inputs are named `input:parent/child`; `/./` in roots and files is normalized). No match: `unknown`. (`pkgsModule` is keyed by its file path and falls here.)
2. Anonymous modules in `baseModules` or the internal `modulesModule`, anything nested below them, and the module `nixosSystem` injects: `nixpkgs`. (Aligned attribution only: only then are the groups known.)
3. Any other anonymous module (in `extraModules`, in the user's `modules` list, nested anywhere): if its `_file` is real, the file wins. An anonymous module inside a file belongs to that file, so the origin is the one of `_file` by rule 1 (`user`, `input:extra`, ...; `unknown` if `_file` is under no known root, e.g. `_file = "my-string-file"`).
4. If `_file` is only a location fallback, the position of the code decides. A fallback is `<unknown-file>`, the location wrapper's file (nixpkgs' `flake.nix` with `nixosSystem`, aligned attribution), or a nixpkgs file while the code sits outside nixpkgs. The `position` (see above) is matched against the roots: user code gives `user-inline`; nixpkgs and inputs give their own origin (`nixpkgs`, `input:<name>`, for an inline module exported by an input); a position under no known root gives `unknown`.
5. No position at all: `user-inline` for modules in the user's `modules` list or `extraModules` (aligned attribution), else `unknown`.

`unknown` counts as non-nixpkgs for the scope filter.

Examples on the fixture (aligned):

- `modules/options.nix` of the root flake is `user`; `extra/module.nix` from the `extra` input is `input:extra`.
- `{ fixture.inlineAttrs = "..."; }` at index 3 of `modules` has the wrapper's `flake.nix` as `_file` (a fallback) and its code in the fixture's `flake.nix`: `user-inline`, `modulesIndex: 3`. The anonymous module it imports (`fixture.nestedInline`) is `user-inline` as well, with `modulesIndex: null`.
- `extra.nixosModules.inline` (index 2) is an inline module of the `extra` input: code in `extra/flake.nix`, so `input:extra`, `modulesIndex: null`.
- `"${extra}/interpolated.nix"` (index 5, a string key with store-path context) is a path module in `extra`: `input:extra`.

With a configuration built by `nixos/lib/eval-config.nix` directly (file-based attribution; one run with two inline modules): `modulesLocation` is the position of your `modules` argument, so `_file` of the inline modules is your own `flake.nix`, a real file by rule 3. They come out as `user`, not `user-inline`, with `modulesIndex: null`. The location wrapper nodes are not collapsed: each is listed as a module of its own, origin `user`, with a `position` in nixpkgs' `lib/modules.nix`.

## `options[]`

Sorted by `path` (bytewise). `_module.*` is never listed, in either scope.

| Field | Type | Meaning |
|---|---|---|
| `path` | string | `lib.showOption loc`. Unique. |
| `loc` | array of string, at least 1 | Option path components. |
| `declaredIn` | array of string | The option's `declarations`: files that declare it, usually one. `[]` for excluded options, or when reading them threw. |
| `type` | string or null | `type.description` of the option type, e.g. `string`, `signed integer`, `attribute set of (submodule)`. `null` for excluded options, or when it cannot be read. |
| `definitions` | array of definition | The listed definitions of the option, plus the option default as the last entry if the option has a `default` (always listed, whether it wins or loses). By default a definition is listed when it comes from a module whose origin is not `nixpkgs` (`user`, `user-inline`, `input:*` or `unknown`; active or not), or when it is the option's only winner. Every other nixpkgs definition, including nixpkgs winners of options with several winners (lists, attrsets), is only counted in `omitted`; `winners` then indexes only the listed ones. With `--include-all-definitions` every definition found in the loaded modules is listed. Order: modules in the order of `modules`, each module's definitions in walk order. This is not the module system's merge order. |
| `winners` | array of integer | Ascending indices into the listed `definitions` of the definitions that are active and have priority `highestPrio`. One entry for single-value types; several for mergeable types (lists, attrsets, submodules, ...). Empty when `highestPrio` is `null`. |
| `highestPrio` | integer or null | Lowest priority number among active definitions (lower wins). `null` if there is no active definition (the module system reports `9999` then), or if the winner is unknown: some active definition has an unknown priority (reading its value threw) or some `mkIf` condition is `"mkIf-error"`. In the unknown case `winners` is `[]` and `error` ends with `winner unknown: ...`; the module system itself would throw for such an option. |
| `omitted` | object | `{ nixpkgsActive, nixpkgsInactive }`: nixpkgs definitions that were counted instead of listed, split by `active`. Both `0` with `--include-all-definitions`. The option default is never counted here (it is always listed). A single winner is never omitted; with several winners, nixpkgs ones are counted here (as active). Computed after the self-check, which always sees every definition. |
| `error` | string or null | `null`, or messages joined by `"; "`, in this order: per definition `N`, `definition N: throw/assert while reading its value` (the outer value threw; its priority is unknown) and `definition N: an mkIf condition threw or is not a bool` (`condition` is `"mkIf-error"`); then `winner unknown: a definition could not be classified` if `highestPrio` is `null` for that reason; then `definition N: preview hit throw/assert` (a throw/assert was hit while previewing it). The last one is set by an explicit failure flag, not by the text: a string value that contains `<error>` does not count. An excluded option has only `excluded after an uncatchable crash during reconstruct (see meta.warnings)`. `N` indexes `definitions`. `tryEval` cannot return the message of a `throw`, so the text names the stage, not the cause. |

Submodule-typed options (`users.users`, `systemd.services`) and freeform options (`services.openssh.settings`) are leaves: definitions are listed per module at the declared option path, as one value each. There are no entries for `users.users.alice.shell` or `services.openssh.settings.PermitRootLogin`, and priorities inside the submodule value are not visible.

### `definitions[]`

| Field | Type | Meaning |
|---|---|---|
| `kind` | `"definition"` or `"default"` | `"default"`: the option's own `default`, treated as `mkOptionDefault`. At most one per option, always the last entry. |
| `file` | string or null, optional | Only present when it differs from the file of `module`, i.e. for `"default"` entries (the first declaration file, `null` if there is none) and for an explicit `mkDefinition` with its own `file`. Otherwise absent: take `modules[]` record of `module` and use its `file`. For inline user modules that file is nixpkgs' `flake.nix`; use the module's `origin` and `position` instead. |
| `module` | string or null | `modules[].id` of the defining module, always listed in `modules`. `null` for `"default"`. |
| `priority` | integer or null | Override priority; lower wins. `1500` `mkOptionDefault` and the option default, `1000` `mkDefault`, `100` plain, `50` `mkForce`, `10` `mkVMOverride`; `mkOverride n` gives `n`. `null` when unknown: the definition is under a false (or failed) `mkIf`, which the module system does not force and neither does optgraph; or reading its outer value threw. |
| `active` | boolean | `true` when every enclosing `mkIf` condition is `true`. Always `true` for `"default"`. A definition under `mkOverride p (mkIf false v)` stays active at priority `p`, mirroring the module system, which does not discharge a `mkIf` below an override (docs/module-system-notes.md topic 2; from code, not run through optgraph). |
| `condition` | `"mkIf-false"`, `"mkIf-error"` or `null` | `null` iff `active`. `"mkIf-false"`: some enclosing condition is `false`. `"mkIf-error"`: a condition threw or is not a boolean (and none is `false`). |
| `valuePreview` | string or null | Bounded textual preview, never the raw value. `null` for inactive definitions, for definitions whose value could not be read, for the options `assertions` and `warnings` (never previewed), and for all definitions of an option whose previews were disabled (`preview-crash`, partial document). `"<redacted>"` when the option path looks like a secret (see below). Format below. |

#### `valuePreview` format

- Only the top-level attrset shows values (`name = value;`). A nested attrset shows its attribute names only, `{ a, b }`, so its values are never forced: many values are lazy by design and throw when forced out of context (assertion messages are only meant to be evaluated when the assertion fails). A fixpoint package set (an attrset with `__unfix__`, made by `lib.makeExtensible` or `makeScope`, e.g. a `boot.kernelPackages` value) shows its names only even at the top level: its attributes include aliases of removed packages, which throw when forced. Lists are expanded down to three levels; a non-empty list at the fourth level collapses to `[ ...(N) ]`. Empty containers are `{ }` and `[ ]`.
- At most 8 items per list or attrset, then `...(N more)`.
- Strings are cut at 80 characters (`...` inside the quotes). The whole preview is cut at 240 characters plus `...`, so at most 243.
- Strings and numbers are written as JSON; attribute names that are not identifiers are quoted; paths are printed as paths.
- `<drv name>` for a derivation (`.name` only; `<error>` if the name cannot be read), `<lambda>`, `<functor>` for attrsets with `__functor`, `<error>` where reading threw (`throw`/`assert`). A failed read sets an explicit flag, which is what `error` reports (see above); the text `<error>` alone does not.
- `<_type>` for module-system wrappers that were not unwrapped: `<if>` (`mkIf`), `<override>` (`mkOverride`, `mkForce`, ...), `<order>` (`mkOrder`; `mkBefore`/`mkAfter` values appear as `<order>`).
- Redaction: if the option path matches `password|passwd|secret|token|private|credential|api[_-]?key` (case-insensitive) and the value is not a bool, null or number, the preview is `"<redacted>"`. Values of other options can still be sensitive (e.g. a secret inside `environment.etc`): don't share a `graph.json` blindly.
- `literalExpression`/`literalMD` values show their text. For `"default"` entries the option's `defaultText` is used when present (`networking.hostName`: `config.system.nixos.distroId`), otherwise the default itself is previewed.

## Fatal errors

Not part of `graph.json`. When the lib cannot produce a document it returns `{ fatal: { code, message } }`, and the CLI prints `optgraph: error: <code>: <message>`, exits 2 and writes no file. Before that the CLI checks that the attribute's `_type` is `configuration` (message: `<installable> is not a NixOS configuration (_type: ...)`).

| Code | When | Run |
|---|---|---|
| `not-a-configuration` | The value has no `options`/`type`/`_module`, or neither `.lib` nor `.pkgs.lib`. | Code reading only; the CLI's `_type` check refuses such values first (run: `...#nixosConfigurations.test.config.networking`). |
| `nixpkgs-unsupported` | `lib.version` is older than `25.11` (`nix/supported.nix`), or the configuration has no `.graph`. Own message for each. | Version case run: `nixos-25.05` gives `nixpkgs 25.05.20260102.ac62194 is older than the minimum supported 25.11`. The `.graph` case is from the code. |
| `config-broken` | Reading the names of `_module.specialArgs` or `_module.args` threw (`throw`/`assert`), e.g. a configuration without `nixpkgs.hostPlatform`. | Run: `evaluating _module.specialArgs / _module.args of the configuration threw (throw/assert); does the configuration evaluate at all?` |

## Scope

| | `meta.scope: "user"` (default) | `"all"` (`--all`) |
|---|---|---|
| Options | Options with at least one definition entry from a module whose origin is not `nixpkgs`, whether it loses, is under a false `mkIf` or wins. Listed definitions: see `definitions` and `omitted` (independent of `--all`). | Every declared option (except `_module.*`), also those without definitions (`definitions` may be `[]` and `winners` empty). |
| Modules | Modules whose origin is not `nixpkgs`, all their ancestors in the import tree, and their direct imports; the modules of all listed definitions; plus disabled modules. | Every loaded module, plus disabled modules. |

The `setDefaultModuleLocation` wrapper nodes that `nixosSystem` puts around each user module are collapsed in aligned attribution: only the wrapped module is listed.

Options that a user module touches can have hundreds of nixpkgs definitions (in the fixture `systemd.services` has 1666, almost all under a false `mkIf`). By default those that don't win are counted in `omitted` instead of listed; the option default is the exception, it is always listed. For options with several winners (every active definition of a list option such as `assertions` or `environment.systemPackages` is a winner), nixpkgs winners are counted too, so only non-nixpkgs definitions are listed.

## Invariants

Enforced by the JSON Schema: field presence and types, enums, `origin` pattern, unique `imports` and `winners`, `loc` non-empty, indices and `modulesIndex` non-negative.

Not expressible in the schema; asserted by `tests/assertions.jq` on the fixture (88 checks; a check whose target is missing fails, it is not skipped):

- every id in `imports` and every definition's `module` is the `id` of a record in `modules`, and module ids are unique;
- a definition's `file` is absent when it equals its module's file;
- in the default scope a listed nixpkgs definition is the option's only winner (the option default is always listed and never counted in `omitted`);
- every index in `winners` is below `definitions.length`, and each winner's `priority` equals `highestPrio`;
- `options` is sorted by `path`;
- no `_module.*` option; in the default scope every option has a definition from a non-nixpkgs module (or none at all);
- an option with a throwing definition or a throwing `mkIf` condition has `winners: []`, `highestPrio: null` and `error` set;
- none of `alignment-failed`, `graph-mismatch`, `reconstruction-mismatch`, `module-error` is among the fixture's warnings.

The remaining checks cover the fixture's cases one by one (priorities, `mkIf` true/false/throwing, `mkForce` around a block, `mkMerge` at the leaf, `mkDefinition` with its own file, preview edge cases, origins, `disabledModules`, inline modules, string-interpolated imports, submodule and freeform leaves, crashing options).

Hold by construction (code reading and the runs above, not asserted by tests):

- `options[].path` is unique;
- `active` is `true` exactly when `condition` is `null`;
- a `"default"` definition has `module: null`, `priority: 1500`, `active: true` and is the last entry;
- every definition at index `i` in `winners` is active and has a non-null `priority`; `highestPrio` is `null` exactly when `winners` is empty.

## Example

Run: `nix run . -- ./tests/fixture#nixosConfigurations.test -o /tmp/graph.json` (5 warnings, 24 modules, 30 options). Store paths are shortened to `/nix/store/…-source/...`. In the module record below, `file` is nixpkgs' `flake.nix`, `position` is the fixture's own.

`meta`, warnings reduced to code and subject:

```json
{
  "host": "test",
  "nixpkgsRev": "c59305bab2065cfecc4944690d9eedbb56f3a9fa",
  "nixpkgsVersion": "26.11.20261001.c59305b",
  "generatedAt": "2026-10-02T23:48:29Z",
  "toolVersion": "0.1.0+02eeb32-dirty",
  "scope": "user",
  "attribution": "aligned",
  "complete": true,
  "warnings": [
    { "code": "eval-crash", "subject": "fixture.aborting" },
    { "code": "selfcheck-skipped", "subject": "fixture.conditionalError" },
    { "code": "selfcheck-skipped", "subject": "fixture.throws" },
    { "code": "preview-crash", "subject": "fixture.whnfAbort" },
    { "code": "selfcheck-crash", "subject": "fixture.whnfAbort" }
  ]
}
```

The first warning in full (`message` is the lib's text, then the CLI's appended error):

```json
{
  "code": "eval-crash",
  "message": "uncatchable crash while reconstructing definitions; option excluded:\nerror: evaluation aborted with the following error message: 'fixture: this definition aborts'",
  "subject": "fixture.aborting"
}
```

A user-inline module (the function module at index 4 of the fixture's `modules` list, `lib.mkDefault` on `fixture.inlineFunction`), and the anonymous module nested in the attrset module at index 3 (no index of its own):

```json
{
  "id": ":anon-2120:anon-1",
  "file": "/nix/store/…-source/flake.nix",
  "imports": [],
  "origin": "user-inline",
  "disabled": false,
  "modulesIndex": 4,
  "position": "/nix/store/…-source/tests/fixture/flake.nix:23"
}
```

```json
{
  "id": ":anon-2119:anon-1:anon-1",
  "file": "/nix/store/…-source/flake.nix",
  "imports": [],
  "origin": "user-inline",
  "disabled": false,
  "modulesIndex": null,
  "position": "/nix/store/…-source/tests/fixture/flake.nix:20"
}
```

`fixture.prio`: `mkDefault`, plain and `mkForce` definitions in three modules, a `mkIf false` block that sets `mkOverride 10` (priority unknown, never forced), and the option default. The `mkForce` definition (index 2) wins:

```json
{
  "declaredIn": [
    "/nix/store/…-source/tests/fixture/modules/options.nix"
  ],
  "definitions": [
    {
      "active": true,
      "condition": null,
      "kind": "definition",
      "module": "/nix/store/…-source/tests/fixture/modules/prio-default.nix",
      "priority": 1000,
      "valuePreview": "\"mkDefault\""
    },
    {
      "active": true,
      "condition": null,
      "kind": "definition",
      "module": "/nix/store/…-source/tests/fixture/modules/prio-plain.nix",
      "priority": 100,
      "valuePreview": "\"plain\""
    },
    {
      "active": true,
      "condition": null,
      "kind": "definition",
      "module": "/nix/store/…-source/tests/fixture/modules/prio-force.nix",
      "priority": 50,
      "valuePreview": "\"mkForce\""
    },
    {
      "active": false,
      "condition": "mkIf-false",
      "kind": "definition",
      "module": "/nix/store/…-source/tests/fixture/modules/conditional.nix",
      "priority": null,
      "valuePreview": null
    },
    {
      "active": true,
      "condition": null,
      "file": "/nix/store/…-source/tests/fixture/modules/options.nix",
      "kind": "default",
      "module": null,
      "priority": 1500,
      "valuePreview": "\"from the option default\""
    }
  ],
  "error": null,
  "highestPrio": 50,
  "loc": [
    "fixture",
    "prio"
  ],
  "omitted": {
    "nixpkgsActive": 0,
    "nixpkgsInactive": 0
  },
  "path": "fixture.prio",
  "type": "string",
  "winners": [
    2
  ]
}
```

An option with a definition that throws (`fixture.throws = throw "..."`): the definition is kept, its priority is unknown, so the winner is unknown: `highestPrio` is `null`, `winners` is empty and `error` says which definition and that the winner is unknown:

```json
{
  "declaredIn": [
    "/nix/store/…-source/tests/fixture/modules/options.nix"
  ],
  "definitions": [
    {
      "active": true,
      "condition": null,
      "kind": "definition",
      "module": "/nix/store/…-source/tests/fixture/modules/broken.nix",
      "priority": null,
      "valuePreview": null
    }
  ],
  "error": "definition 0: throw/assert while reading its value; winner unknown: a definition could not be classified",
  "highestPrio": null,
  "loc": [
    "fixture",
    "throws"
  ],
  "omitted": {
    "nixpkgsActive": 0,
    "nixpkgsInactive": 0
  },
  "path": "fixture.throws",
  "type": "string",
  "winners": []
}
```

`fixture.conditionalError = lib.mkIf (throw "...") "never"` ends the same way. The definition has `active: false`, `condition: "mkIf-error"`, `priority: null`, and the option has `highestPrio: null`, `winners: []` and `error` `"definition 0: an mkIf condition threw or is not a bool; winner unknown: a definition could not be classified"`.
