# optgraph

Shows how a NixOS flake configuration is put together: the module import graph, and for every option who sets it, at what priority, and why one definition won over the others.

`optgraph` evaluates a `nixosConfigurations.<host>` of your flake and writes `graph.json`: the modules and where they come from (your files, inline modules, flake inputs, nixpkgs), and all definitions of each option you touch, including the ones that lost and the ones switched off by `mkIf`. An interactive viewer for this file is planned; today you get the JSON.

<!-- demo.gif -->

Live demo (synthetic graph of `tests/fixture`): https://ilqqy.github.io/optgraph/ (for example https://ilqqy.github.io/optgraph/?opt=fixture.prio)

Status: phase 2 of 4 (extractor and viewer). Diff and polish are not started.

## Install and run

Requires Nix with flakes enabled. Developed and tested with Nix 2.34.8 on x86_64-linux; the package is also exposed for aarch64-linux, which was not run.

From a checkout (first run downloads the fixture's nixpkgs):

```sh
nix run . -- ./tests/fixture#nixosConfigurations.test -o graph.json
```

From GitHub: <!-- unverified: the repository is not published yet -->

```sh
nix run github:ilqqy/optgraph -- <flakeref>#nixosConfigurations.<host> -o graph.json
```

Or build once and call the binary:

```sh
nix build . && ./result/bin/optgraph --help
```

The argument is `<flakeref>#nixosConfigurations.<host>`. The attribute must be a configuration (a `nixosSystem` result). The CLI never passes `--impure` and forces `--option pure-eval true`, so evaluation stays pure whatever your Nix config says. The target flake's `flake.lock` is never written (`--no-write-lock-file`).

Logs go to stderr. Without `-o` the JSON goes to stdout. With `-o` it is written to a temporary file next to the target and renamed, so a failed run leaves no partial file (except the deliberate partial document of exit code 3, below). A write error exits 2.

### Flags

| Flag | Meaning |
|---|---|
| `-o`, `--output FILE` | Write `graph.json` to `FILE`. Default: stdout (unless `--html` is given). |
| `--html FILE` | Write a self-contained viewer page with the graph embedded. Can be combined with `-o`. |
| `--all` | No scope filter: every declared option and every loaded module. Slow and big, see [Measured](#measured). |
| `--include-all-definitions` | List every definition. By default, nixpkgs definitions that neither win nor come from a non-nixpkgs module are only counted (`options[].omitted`). Independent of `--all`. |
| `--max-retries N` | Number of uncatchable crashes to recover from by re-running. Default 10, with `--all` 100. The (N+1)th crash ends the run with partial output and exit 3. The fixture crashes 3 times on purpose: `--max-retries 3` completes, `--max-retries 2` ends partial. |
| `--time-budget SECS` | Wall-clock budget for all evaluations. Default 600, with `--all` 1800. Each evaluation runs under `timeout` with what is left of the budget; an evaluation that is killed (status 124), or a budget that runs out between evaluations, ends the run as exhausted (exit 3). The partial-output evaluations that follow have their own limit of `max(120, SECS/4)` seconds each, so the total can exceed `SECS`. |
| `-h`, `--help` | Usage. |

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Done. |
| 1 | Usage error. |
| 2 | Flake or evaluation error. No output is written. The flake or attribute cannot be resolved; the value is not a NixOS configuration (`not-a-configuration`); nixpkgs is older than 25.11 or has no `.graph` (`nixpkgs-unsupported`); the configuration does not evaluate (`config-broken`); a crash could not be localized; or the output cannot be written. Coded errors print as `optgraph: error: <code>: <message>`. |
| 3 | Crash recoveries or time budget exhausted. A partial document with `meta.complete = false` and a `budget-exhausted` warning is still written: up to two more evaluations, first with previews and the self-check off for all options, then with no options (modules only). |

## What you get

`graph.json` has three parts: `meta`, `modules`, `options`. The full field reference is in [docs/schema.md](docs/schema.md); the JSON Schema is `schema/graph.schema.json`.

- `modules`: every module in the scope with its imports and an origin: `user`, `user-inline` (an anonymous module whose code is yours; the ones that are entries of your `nixosSystem { modules = [ ... ]; }` list also get `modulesIndex`), `input:<name>`, `nixpkgs` or `unknown`. Anonymous modules are attributed by the file they live in, or, when their `_file` is only a fallback, by the position of their code. `position` is the `file:line` of a module's first attribute. Modules disabled through `disabledModules` are listed as disabled.
- `options`: for each option that one of your modules defines, every definition from every module: priority (1500 default, 1000 `mkDefault`, 100 plain, 50 `mkForce`, 10 `mkVMOverride`), whether it is active (`mkIf` conditions), a bounded value preview (never the raw value), which definitions won, and the resulting `highestPrio`. When a value or condition throws, the winner is unknown: `highestPrio` is `null`, `winners` is empty and `error` says so.
- `meta.warnings`: everything that went wrong or was degraded, with a machine-readable code.

On the fixture (the commands were run on the output of the call above):

```sh
jq -r '.options[] | select(.path == "fixture.prio") | .definitions[] | [(.priority // "?"), .active, (.valuePreview // "-")] | @tsv' graph.json
```

```
1000	true	"mkDefault"
100	true	"plain"
50	true	"mkForce"
?	false	-
1500	true	"from the option default"
```

`mkForce` (50) wins; the fourth line is a definition under `mkIf false` with unknown priority. The inline modules:

```sh
jq -r '.modules[] | select(.origin == "user-inline") | [.id, .modulesIndex // "null", (.position | sub("^.*/"; ""))] | @tsv' graph.json
```

```
:anon-2119:anon-1	3	flake.nix:19
:anon-2120:anon-1	4	flake.nix:23
:anon-2119:anon-1:anon-1	null	flake.nix:20
```

The third is an anonymous module imported by the first; it has no index of its own. The warnings of the same run:

```sh
jq -r '.meta.warnings[] | [.code, .subject] | @tsv' graph.json
```

```
eval-crash	fixture.aborting
selfcheck-skipped	fixture.conditionalError
selfcheck-skipped	fixture.throws
preview-crash	fixture.whnfAbort
selfcheck-crash	fixture.whnfAbort
```

All five come from options that the fixture breaks on purpose.

## Viewer

`optgraph … --html graph.html` writes one HTML file that opens offline in any browser: the module graph on the left, option search and details on the right. The same viewer without embedded data is `nix build .#viewer` (`result/index.html`); open a `graph.json` in it with *Open graph.json* or by dropping the file onto the page, or serve it next to a graph and open `index.html?src=graph.json`. Served over http(s) with no embedded data and no `?src=`, the page loads `./demo.json` if the site has one (that is how the live demo works); opened as a `file://` page it shows the drop zone.

- Graph: modules coloured by origin (user, user-inline, input, nixpkgs, unknown), edges are imports, disabled modules dashed; inline user modules are labelled by their index in nixosSystem's `modules`. Pan by dragging, zoom with the wheel; click a module to see the options it sets. Above 1500 modules (`--all`) only the non-nixpkgs modules are drawn.
- Options: instant fuzzy search over paths (`/` focuses it). The detail panel shows the definitions as a priority ladder (lowest number first, with names: 50 mkForce, 100 normal, 1000 mkDefault, 1500 default), winners highlighted, losers dimmed, `mkIf` false/error marked, each with its module, origin and value preview; `+N nixpkgs` counts definitions that were omitted. Selecting an option highlights its modules in the graph.
- Top bar: host, nixpkgs version, attribution, `complete`, and the warnings (click to list them). Light and dark follow the system setting.
- Deep links, for embedded data and `?src=` alike: `?opt=<option path>` opens that option's ladder and highlights its modules, `?module=<index or id>` opens a module (index into `modules`, or its `id`), `?warnings` opens the warnings panel. Example: `graph.html?opt=networking.hostName`, `https://ilqqy.github.io/optgraph/?module=0`.
- Local only: no analytics and no external requests. The page's Content-Security-Policy allows inline code only and limits `fetch` (`?src=`) to the page's own origin. d3-force and its dependencies are vendored in `viewer/vendor/` (ISC licence, pinned in `viewer/vendor/VERSIONS`).

## Scope and support

- Default scope: options with at least one definition from a module that is not nixpkgs, plus the non-nixpkgs modules, their ancestors and their direct imports. `--all` lifts the filter.
- nixpkgs 25.11 to 26.11 is supported. The fixture check passes with nixpkgs `c59305b` (26.11) and with nixos-25.11 (`b6018f87da91d19d0ab4cf979885689b469cdd41`). <!-- unverified by the docs writer: the 25.11 run was reported by the verifier, 2026-10-03 --> Older releases are refused (exit 2, `nixpkgs-unsupported`; run with nixos-25.05). A newer release works with a `nixpkgs-untested` warning.
- Configurations built with `nixpkgs.lib.nixosSystem` are supported. When a configuration is not built that way (tested: `nixos/lib/eval-config.nix` called directly) optgraph falls back to file-based attribution and adds an `alignment-failed` warning; `modulesIndex` is always `null`, and the wrapper nodes around inline modules show up as modules of their own. Other builders were not tried.
- Flake inputs are resolved with `nix flake archive --dry-run` and `nix flake metadata`, so evaluating a flake needs its inputs to be fetchable. Nested inputs are named `parent/child`. A relative `path:` input of the root flake is resolved as `<source root>/<dir>/<path>`.

## How crashes are handled

`builtins.tryEval` catches only `throw` and failed `assert`. Other errors (`abort`, missing attributes, ...) kill the whole `nix eval`, and nixpkgs has options that do this. optgraph handles them at the process level:

- Each unit of work (loading a module, walking its `config`, reconstructing an option, the self-check, the preview) runs inside an error-context marker. After a crash the CLI reads the innermost marker (the last one in the `--show-trace` output) and re-runs with that unit degraded: previews or self-check switched off for the option, or the option or module excluded. A crash while loading a module or while walking its `config` excludes the module. Each recovery shows up as a warning (`preview-crash`, `selfcheck-crash`, `eval-crash`) that carries the Nix error text.
- Without a marker the CLI bisects the in-scope options. `OPTGRAPH_LOCALIZE=bisect` forces this, for testing. It is coarser: the culprit option is excluded entirely.
- When the recovery limit or the time budget runs out, the CLI writes partial output (exit 3, see above).
- A built-in self-check compares the reconstructed winners (`highestPrio`, files) with what the module system reports. A difference becomes a `reconstruction-mismatch` warning; where the module system's own accessors throw or the winner is unknown, the check is skipped with a `selfcheck-skipped` warning. There were no mismatches on the fixture, with or without `--all`.

## Measured

Nix 2.34.8, nixpkgs `c59305b`, x86_64-linux, `/usr/bin/env time -v`, fixture `./tests/fixture#nixosConfigurations.test`:

| Run | Wall | Max RSS | JSON | Modules | Options | Evaluations / crash recoveries | Mismatches |
|---|---|---|---|---|---|---|---|
| default (2026-10-05) | 6.29 s | 295 MB | 38 KB | 24 | 30 | 4 / 3 (all deliberate) | 0 |
| `--all` (2026-10-03, before definitions were omitted by default) | 84 s | 1.43 GB | 20 MB | 3993 | 16812 | 14 / 13 (3 deliberate, 10 in stock nixpkgs options; by stage: 9 self-check, 3 preview, 1 reconstruct) | 0 |

With `OPTGRAPH_LOCALIZE=bisect` the default run takes 17 evaluations and 2 crash recoveries. Timings vary by about a second between runs.

## Limitations

- Submodule-typed options (`users.users`, `systemd.services`) and freeform options (`services.openssh.settings`) are leaves: definitions are listed per module at the option's own path, with no attribution to the entries inside.
- The priority of a definition under a false `mkIf` is unknown (`null`): the module system never forces it, and neither does optgraph.
- An option whose active definition has an unknown priority, or whose `mkIf` condition throws, gets no winner (`highestPrio: null`, `winners: []`).
- Messages of `throw`/`assert` cannot be read (`tryEval` does not return them); `error` fields name the failing stage only. Crash warnings do carry the Nix error text.
- `--all` is slow and big (table above).
- For options with several winners (lists, attrsets) only non-nixpkgs definitions are listed; nixpkgs winners are counted in `omitted`, so the listed `winners` of such an option are not the whole merged value. Use `--include-all-definitions` to see them.
- `graph.json` and `--html` pages contain previews of option values. Previews are bounded but not scrubbed: only options whose path looks like a secret (`password`, `token`, `secret`, `credential`, `api_key`, ...) are `<redacted>`. A preview can still contain sensitive values; don't share `graph.json` or an `--html` page blindly.
- Relative `path:` inputs are resolved only for the root flake.
- Inline user modules carry nixpkgs' own `flake.nix` as `file` (a nixosSystem quirk); use `origin`, `modulesIndex` and `position` for them. An anonymous module nested in an inline one has no `modulesIndex`.
- A module key built from a string keeps what the string says: with a relative `path:` input, `"${extra}/interpolated.nix"` gives an id containing `/./`.
- The internal `_module.args` definitions are not visible, and `_module.*` options are never listed.
- No fixture case exercises a crash while loading or walking a module; that path is not covered by a test.
- Only NixOS configurations are supported; other module-system roots are untested.

## Development

```sh
nix develop            # shell with gh, jq, nixfmt, check-jsonschema
nix flake check -L     # lib checks on the fixture, schema validation, CLI build (a few minutes)
nix fmt                # nixfmt (RFC style); CI runs: nix fmt -- --check .
nix develop -c tests/e2e.sh [OUTDIR]   # CLI end to end: crash recovery, bisection, budget, exit codes, --html
nix build .#viewer     # viewer/build.sh: one result/index.html
```

`nix flake check -L`, `nix fmt -- --check .` and `tests/e2e.sh` pass on this checkout (2026-10-08). `tests/assertions.jq` has 84 checks on the fixture's output. `nix flake check` skips aarch64-linux unless `--all-systems` is given. The e2e test needs network access for the fixture's nixpkgs.

Layout: `nix/` extraction library, `cli/` the `nix eval` wrapper, `viewer/` the viewer sources (`template.html`, `style.css`, `src/*.js`, `vendor/`), `demo/graph.json` the synthetic demo graph, `schema/graph.schema.json` output schema, `tests/fixture/` test flake, `docs/schema.md` field reference, `docs/module-system-notes.md` verified findings about `lib/modules.nix` with source references.

## License

MIT, see [LICENSE](LICENSE).
