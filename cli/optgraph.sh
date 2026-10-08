# optgraph CLI. cli/default.nix prepends OPTGRAPH_SRC, OPTGRAPH_NARHASH and
# OPTGRAPH_VERSION, and writeShellApplication adds errexit/nounset/pipefail.
#
# Why bash: the CLI only orchestrates `nix flake archive`, `nix eval` and a
# retry loop; all JSON is handled by jq. writeShellApplication shellchecks it.
#
# Uncatchable crashes (abort, missing attribute, infinite recursion, ...) kill
# the nix process. The lib wraps each unit of work in an error-context marker
# "optgraph-marker:<stage>:<json subject>"; the innermost marker in the
# --show-trace output names the culprit, which is degraded or excluded on the
# next run. Without a marker the in-scope option set is bisected.

usage() {
  cat <<'EOF'
usage: optgraph <flakeref>#nixosConfigurations.<host> [options]

  -o, --output FILE       write graph.json to FILE (default: stdout, unless --html)
      --html FILE         write a self-contained viewer page with the graph embedded
      --all               no scope filter: every declared option and module
      --include-all-definitions
                          list every definition; by default nixpkgs definitions
                          that don't win are only counted (option.omitted)
      --max-retries N     uncatchable crashes to recover from by re-running
                          (default 10; --all: 100)
      --time-budget SECS  wall-clock budget for all evaluations (default 600;
                          --all: 1800); evaluations are killed when it runs out
  -h, --help              show this help

exit codes: 0 ok, 1 usage, 2 flake or evaluation error, 3 budget exhausted
            (partial output with meta.complete = false is still written)
EOF
}

log() { printf 'optgraph: %s\n' "$*" >&2; }
die() {
  local code=$1
  shift
  printf 'optgraph: error: %s\n' "$*" >&2
  exit "$code"
}
usage_error() {
  usage >&2
  die 1 "$*"
}

installable="" output="" html="" all=false include_all_defs=false max_retries="" time_budget=""
while [ $# -gt 0 ]; do
  case $1 in
  --html)
    [ $# -ge 2 ] || usage_error "$1 needs a file argument"
    html=$2
    shift 2
    ;;
  -o | --output)
    [ $# -ge 2 ] || usage_error "$1 needs a file argument"
    output=$2
    shift 2
    ;;
  --all)
    all=true
    shift
    ;;
  --include-all-definitions)
    include_all_defs=true
    shift
    ;;
  --max-retries)
    [ $# -ge 2 ] || usage_error "$1 needs a number"
    max_retries=$2
    shift 2
    ;;
  --time-budget)
    [ $# -ge 2 ] || usage_error "$1 needs a number of seconds"
    time_budget=$2
    shift 2
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  -*) usage_error "unknown option: $1" ;;
  *)
    [ -z "$installable" ] || usage_error "unexpected argument: $1"
    installable=$1
    shift
    ;;
  esac
done

[ -n "$installable" ] || usage_error "missing <flakeref>#nixosConfigurations.<host>"
case $installable in
*'#'?*) ;;
*) usage_error "expected <flakeref>#<attribute path>, got: $installable" ;;
esac
flakeref=${installable%%#*}
attr=${installable#*#}
[ -n "$flakeref" ] || usage_error "empty flakeref in: $installable"
host=${attr#nixosConfigurations.}
host=${host//\"/}

if $all; then
  : "${max_retries:=100}" "${time_budget:=1800}"
else
  : "${max_retries:=10}" "${time_budget:=600}"
fi
case $max_retries in '' | *[!0-9]*) usage_error "--max-retries needs a non-negative integer" ;; esac
case $time_budget in '' | *[!0-9]* | 0) usage_error "--time-budget needs a positive integer" ;; esac

command -v nix >/dev/null || die 2 "nix not found in PATH"

work=$(mktemp -d)
tmp=""
trap 'rm -rf "$work"; [ -z "$tmp" ] || rm -f "$tmp"' EXIT

# Global flags go before the subcommand, lock flags after it. Pure evaluation
# is forced even if the user's nix.conf disables it.
nix_flags=(--extra-experimental-features 'nix-command flakes' --option pure-eval true)
lock_flags=(--no-write-lock-file)
start=$SECONDS
evals=0

# Last error block of a nix stderr log, without colours, at most 20 lines.
errtail() {
  sed 's/\x1b\[[0-9;]*m//g' "$1" |
    awk '/^[[:space:]]*error:/ { buf = ""; on = 1 } on { buf = buf $0 "\n" } END { printf "%s", buf }' |
    sed 's/^[[:space:]]*//' | head -n 20
}

# A JSON document as a Nix string literal: jq quotes it, `$` is escaped.
nix_string() { printf '%s' "$1" | jq -Rs . | sed 's/\$/\\$/g'; }

# ----------------------------------------------------------------- inputs
log "resolving $flakeref"
if ! nix "${nix_flags[@]}" flake archive "${lock_flags[@]}" --dry-run --json "$flakeref" >"$work/archive.json" 2>"$work/archive.err"; then
  die 2 "cannot resolve flake '$flakeref':"$'\n'"$(errtail "$work/archive.err")"
fi
if ! nix "${nix_flags[@]}" flake metadata "${lock_flags[@]}" --json "$flakeref" >"$work/metadata.json" 2>"$work/metadata.err"; then
  die 2 "cannot read metadata of flake '$flakeref':"$'\n'"$(errtail "$work/metadata.err")"
fi
# Input name -> source root. Nested inputs are named parent/child. Relative
# `path:` inputs of the root flake have no store path of their own in the
# archive: they live in the root flake's source at <dir>/<path>.
base_args=$(jq -c -n \
  --slurpfile archive "$work/archive.json" \
  --slurpfile metadata "$work/metadata.json" \
  --arg host "$host" \
  --arg generatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --argjson all "$all" \
  --argjson includeAllDefinitions "$include_all_defs" \
  '
  $archive[0] as $a | $metadata[0] as $m
  | ($a.path + (if ($m.original.dir // "") != "" then "/" + $m.original.dir else "" end)) as $flakeDir
  | def flat($prefix): to_entries[]
      | ($prefix + .key) as $name
      | (if .value.path then {key: $name, value: .value.path} else empty end),
        ((.value.inputs // {}) | flat($name + "/"));
    ($m.locks.nodes[$m.locks.root].inputs // {}) as $rootInputs
    | [ ($a.inputs // {}) | to_entries[] | select(.value.path == null)
        | .key as $name
        | ($rootInputs[$name] | if type == "string" then $m.locks.nodes[.].locked else null end) as $l
        | select($l != null and $l.type == "path" and ($l.path | startswith("/") | not))
        | {key: $name, value: ($flakeDir + "/" + $l.path)} ] as $relative
  | {
      host: $host,
      generatedAt: $generatedAt,
      all: $all,
      includeAllDefinitions: $includeAllDefinitions,
      selfRoot: $a.path,
      inputs: ([($a.inputs // {}) | flat("")] + $relative | from_entries)
    }')

# -------------------------------------------------------------- preflight
log "checking $installable"
if ! nix "${nix_flags[@]}" eval "${lock_flags[@]}" --raw "$installable" --apply 'c: c._type or "(none)"' >"$work/type" 2>"$work/pre.err"; then
  die 2 "cannot evaluate $installable:"$'\n'"$(errtail "$work/pre.err")"
fi
[ "$(cat "$work/type")" = configuration ] ||
  die 2 "$installable is not a NixOS configuration (_type: $(cat "$work/type"))"

# ------------------------------------------------------------- extraction
# Crash records: [{stage, subject: {loc, path} | {slot}, message}]
crashes='[]'

# run_extract EXTRA_JSON [TIMEOUT]: one evaluation; output in $work/out.json,
# log in $work/err. Killed after TIMEOUT seconds (default: what is left of the
# time budget); returns 124 then.
run_extract() {
  local args expr limit=${2:-$((time_budget - (SECONDS - start)))}
  [ "$limit" -gt 0 ] || return 124
  args=$(jq -cn --argjson base "$base_args" --argjson crashes "$crashes" --argjson extra "$1" '
    def locs($s): [$crashes[] | select(.stage == $s) | .subject.loc];
    $base + {
      exclude: locs("reconstruct"),
      noPreview: locs("preview"),
      noSelfcheck: locs("selfcheck"),
      excludeModules: [$crashes[] | select(.stage == "module" or .stage == "walk") | .subject.slot]
    } + $extra')
  expr="cfg: (import ((builtins.fetchTree { type = \"path\"; path = \"$OPTGRAPH_SRC\"; narHash = \"$OPTGRAPH_NARHASH\"; }).outPath + \"/nix\") { toolVersion = \"$OPTGRAPH_VERSION\"; }).extract (builtins.fromJSON $(nix_string "$args") // { config = cfg; })"
  evals=$((evals + 1))
  timeout "$limit" nix "${nix_flags[@]}" eval "${lock_flags[@]}" --json --show-trace "$installable" --apply "$expr" >"$work/out.json" 2>"$work/err"
}

# Innermost marker of the last crash as a crash record (JSON), or nothing.
localize() {
  local marker stage subject
  [ "${OPTGRAPH_LOCALIZE:-marker}" = marker ] || return 0
  marker=$(sed 's/\x1b\[[0-9;]*m//g' "$work/err" | grep -o '… optgraph-marker:[a-z]*:.*$' | tail -n 1 | sed 's/^… optgraph-marker://' || true)
  [ -n "$marker" ] || return 0
  stage=${marker%%:*}
  subject=${marker#*:}
  jq -cn --arg stage "$stage" --argjson subject "$subject" --arg message "$(errtail "$work/err")" \
    '{stage: $stage, subject: $subject, message: $message}'
}

over_budget() { [ $((SECONDS - start)) -ge "$time_budget" ]; }

# Bisect the in-scope options for one that crashes on its own; sets $crash.
# Not run in a subshell, so the evaluation counter stays right.
bisect() {
  local candidates n half found
  crash=""
  log "no crash marker in the trace; bisecting the in-scope options"
  run_extract '{"mode": "scope"}' || return 1
  local status
  candidates=$(jq -c --argjson crashes "$crashes" '
    [$crashes[] | select(.stage == "reconstruct") | .subject.path] as $done
    | [.scope[] | select(.path as $p | $done | index($p) | not)]' "$work/out.json")
  n=$(jq length <<<"$candidates")
  [ "$n" -gt 0 ] || return 1
  while [ "$n" -gt 1 ]; do
    over_budget && return 1
    half=$(jq -c '.[0:(length / 2 | floor)]' <<<"$candidates")
    status=0
    run_extract "$(jq -c '{only: map(.loc)}' <<<"$half")" || status=$?
    [ "$status" -ne 124 ] || return 1
    if [ "$status" -eq 0 ]; then
      candidates=$(jq -c '.[(length / 2 | floor):]' <<<"$candidates")
    else
      found=$(localize)
      if [ -n "$found" ]; then
        crash=$found
        return 0
      fi
      candidates=$half
    fi
    n=$(jq length <<<"$candidates")
  done
  # Confirm the culprit crashes on its own (and get its error message).
  status=0
  run_extract "$(jq -c '{only: map(.loc)}' <<<"$candidates")" || status=$?
  [ "$status" -ne 0 ] && [ "$status" -ne 124 ] || return 1
  crash=$(jq -cn --argjson c "$(jq -c '.[0]' <<<"$candidates")" --arg message "$(errtail "$work/err")" \
    '{stage: "reconstruct", subject: $c, message: $message}')
}

exhausted=""
retries=0
while :; do
  status=0
  run_extract '{}' || status=$?
  [ "$status" -ne 0 ] || break
  if [ "$status" -eq 124 ]; then
    exhausted="time budget of ${time_budget}s exhausted during an evaluation"
    break
  fi
  crash=$(localize)
  [ -n "$crash" ] || bisect || crash=""
  if [ -z "$crash" ]; then
    over_budget && {
      exhausted="time budget of ${time_budget}s exhausted while localizing a crash"
      break
    }
    die 2 "evaluation failed and the crash could not be localized:"$'\n'"$(errtail "$work/err")"
  fi
  # The same culprit crashing again: degrade harder (exclude), or give up.
  if jq -e --argjson c "$crash" 'any(.[]; .stage == $c.stage and .subject == $c.subject)' <<<"$crashes" >/dev/null; then
    case $(jq -r .stage <<<"$crash") in
    preview | selfcheck) crash=$(jq -c '.stage = "reconstruct"' <<<"$crash") ;;
    *) die 2 "excluding $(jq -c .subject <<<"$crash") did not stop the crash:"$'\n'"$(errtail "$work/err")" ;;
    esac
  fi
  # Recorded even when no retry is left, so partial output reports it.
  crashes=$(jq -c --argjson c "$crash" '. + [$c]' <<<"$crashes")
  what=$(jq -r '.stage + " of " + (.subject.path // .subject.slot)' <<<"$crash")
  if [ "$retries" -ge "$max_retries" ]; then
    exhausted="uncatchable crash in $what with all $max_retries crash recoveries used"
    break
  fi
  retries=$((retries + 1))
  log "uncatchable crash in $what; retrying ($retries/$max_retries)"
  if over_budget; then
    exhausted="time budget of ${time_budget}s exhausted"
    break
  fi
done

if [ -n "$exhausted" ]; then
  # Up to two more evaluations, each with its own time limit, for partial output.
  partial_timeout=$((time_budget / 4 > 120 ? time_budget / 4 : 120))
  log "$exhausted; writing partial output"
  if ! run_extract '{"preview": false, "selfcheck": false}' "$partial_timeout" &&
    ! run_extract '{"only": []}' "$partial_timeout"; then
    jq -n --arg host "$host" --arg v "$OPTGRAPH_VERSION" --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" '{
      meta: {host: $host, nixpkgsRev: null, nixpkgsVersion: "unknown", generatedAt: $at, toolVersion: $v,
             scope: "user", attribution: "file-based", complete: false, warnings: []},
      modules: [], options: []}' >"$work/out.json"
  fi
fi

if jq -e 'has("fatal")' "$work/out.json" >/dev/null; then
  die 2 "$(jq -r '.fatal.code + ": " + .fatal.message' "$work/out.json")"
fi

# Attach the trimmed crash messages to the lib's warnings; mark partial output.
enrich() {
  jq --argjson crashes "$crashes" --arg exhausted "$exhausted" '
    def code: {reconstruct: "eval-crash", module: "eval-crash", walk: "eval-crash", preview: "preview-crash", selfcheck: "selfcheck-crash"}[.stage];
    def subj: .subject.path // .subject.slot;
    ($crashes | map({key: (code + " " + subj), value: .message}) | from_entries) as $msg
    | .meta.warnings |= map(
        $msg[.code + " " + (.subject // "")] as $m
        | if $m then .message += ":\n" + $m else . end)
    | if $exhausted != "" then
        .meta.complete = false
        | .meta.warnings += [{code: "budget-exhausted", subject: null, message: $exhausted}]
      else . end' "$work/out.json"
}

enrich >"$work/final.json" || die 2 "cannot post-process the output"
where=""
if [ -n "$output" ]; then
  tmp="$output.tmp.$$"
  cp "$work/final.json" "$tmp" || die 2 "cannot write $output"
  mv -f "$tmp" "$output" || die 2 "cannot write $output"
  tmp=""
  where=$output
elif [ -z "$html" ]; then
  cat "$work/final.json"
  where=stdout
fi
if [ -n "$html" ]; then
  optgraph-embed-html "$OPTGRAPH_VIEWER" "$work/final.json" "$html" || die 2 "cannot write $html"
  where="${where:+$where and }$html"
fi

log "wrote $where: $(jq -r '"\(.modules | length) modules, \(.options | length) options, \(.meta.warnings | length) warnings"' "$work/out.json"); $evals evaluations, $retries crash recoveries, $((SECONDS - start))s"

[ -z "$exhausted" ] || exit 3
