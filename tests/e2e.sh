#!/usr/bin/env bash
# End-to-end test of the optgraph CLI on the fixture (a CI step; needs network
# access to fetch the fixture's nixpkgs). Requires nix, jq and check-jsonschema
# (all in `nix develop`).
#
#   tests/e2e.sh [OUTDIR]     # OUTDIR keeps graph.json and logs
set -euo pipefail
cd "$(dirname "$0")/.."

out=${1:-$(mktemp -d)}
mkdir -p "$out"
fixture=./tests/fixture#nixosConfigurations.test
failures=0

pass() { printf 'ok   %s\n' "$*"; }
fail() {
  printf 'FAIL %s\n' "$*"
  failures=$((failures + 1))
}
expect_exit() { # expect_exit WANT NAME CMD...
  local want=$1 name=$2 got=0
  shift 2
  "$@" >"$out/$name.stdout" 2>"$out/$name.log" || got=$?
  if [ "$got" -eq "$want" ]; then pass "$name: exit $got"; else fail "$name: exit $got, want $want (see $out/$name.log)"; fi
}
evaluations() { sed -n 's/.* \([0-9][0-9]*\) evaluations, \([0-9][0-9]*\) crash recoveries.*/\1 evaluations, \2 crash recoveries/p' "$out/$1.log"; }
jq_check() { # jq_check NAME FILE FILTER
  if jq -e "$3" "$2" >/dev/null; then pass "$1"; else fail "$1"; fi
}

nix build .#default -o "$out/result"
optgraph=$out/result/bin/optgraph

# 1. Marker-based localization: abort excluded, WHNF abort degraded.
expect_exit 0 full "$optgraph" "$fixture" -o "$out/graph.json"
check-jsonschema --schemafile schema/graph.schema.json "$out/graph.json" && pass "full: schema" || fail "full: schema"
jq -r -f tests/assertions.jq "$out/graph.json" && pass "full: fixture assertions" || fail "full: fixture assertions"
jq_check "full: eval-crash message carries the abort" "$out/graph.json" \
  'any(.meta.warnings[]; .code == "eval-crash" and .subject == "fixture.aborting" and (.message | contains("fixture: this definition aborts")))'
jq_check "full: preview-crash message carries the abort" "$out/graph.json" \
  'any(.meta.warnings[]; .code == "preview-crash" and .subject == "fixture.whnfAbort" and (.message | contains("aborts when forced to WHNF")))'
jq_check "full: selfcheck-crash message carries the abort" "$out/graph.json" \
  'any(.meta.warnings[]; .code == "selfcheck-crash" and .subject == "fixture.whnfAbort" and (.message | contains("aborts when forced to WHNF")))'
echo "     full: $(evaluations full)"

# 2. Bisection fallback (markers ignored): both aborting options get excluded.
OPTGRAPH_LOCALIZE=bisect expect_exit 0 bisect "$optgraph" "$fixture" -o "$out/bisect.json"
check-jsonschema --schemafile schema/graph.schema.json "$out/bisect.json" && pass "bisect: schema" || fail "bisect: schema"
jq_check "bisect: both aborting options excluded" "$out/bisect.json" \
  '[.meta.warnings[] | select(.code == "eval-crash") | .subject] | sort == ["fixture.aborting", "fixture.whnfAbort"]'
jq_check "bisect: complete" "$out/bisect.json" '.meta.complete == true'
echo "     bisect: $(evaluations bisect)"

# 3. Budget exhaustion: partial output, exit 3.
expect_exit 3 budget "$optgraph" "$fixture" -o "$out/partial.json" --max-retries 1
check-jsonschema --schemafile schema/graph.schema.json "$out/partial.json" && pass "budget: schema" || fail "budget: schema"
jq_check "budget: complete = false, budget-exhausted warning" "$out/partial.json" \
  '.meta.complete == false and any(.meta.warnings[]; .code == "budget-exhausted")'
jq_check "budget: the recovered crash is reported" "$out/partial.json" \
  'any(.meta.warnings[]; .code == "eval-crash" or .code == "preview-crash" or .code == "selfcheck-crash")'

# 4. Errors and usage.
expect_exit 2 bad-flakeref "$optgraph" ./tests/does-not-exist#nixosConfigurations.x -o "$out/x.json"
expect_exit 2 missing-host "$optgraph" ./tests/fixture#nixosConfigurations.nope -o "$out/x.json"
expect_exit 2 not-a-config "$optgraph" ./tests/fixture#nixosConfigurations.test.config.networking -o "$out/x.json"
expect_exit 1 no-args "$optgraph"
expect_exit 1 no-attr "$optgraph" ./tests/fixture
expect_exit 1 bad-flag "$optgraph" "$fixture" --frobnicate
[ ! -e "$out/x.json" ] && pass "errors: no output file written" || fail "errors: output file written"

if [ "$failures" -gt 0 ]; then
  echo "e2e: $failures failure(s)"
  exit 1
fi
echo "e2e: all passed"
