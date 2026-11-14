#!/bin/sh
# Stranger test: install, init, one sample end to end, then the workspace tests,
# all under a throwaway HOME and XDG dirs. Synthetic fixtures only; no network beyond cargo's own fetch.
# Prints each command. Exit 0 green, 1 a step failed, 2 bad arguments.
# Usage: scripts/stranger-test.sh [--keep]   (--keep leaves the temp dir for inspection)
set -eu

KEEP=0
case ${1:-} in
  '') ;;
  --keep) KEEP=1 ;;
  -h|--help) sed -n '2,5p' "$0"; exit 0 ;;
  *) printf 'usage: scripts/stranger-test.sh [--keep]\n' >&2; exit 2 ;;
esac

REPO=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
step() { printf '\n== %s\n' "$*"; }
fail() { printf 'FAIL %s\n' "$*" >&2; exit 1; }
run() {
  printf '+ %s\n' "$*"
  "$@" || fail "exit $?: $*"
}
# Require each later argument as a whole line of the first.
expect_line() {
  out=$1
  shift
  for want in "$@"; do
    printf '%s\n' "$out" | grep -qxF -- "$want" || fail "missing line: $want"
  done
}

if [ -n "$(git -C "$REPO" status --porcelain 2>/dev/null)" ]; then
  printf 'warn: working tree is not clean; testing it as-is\n' >&2
fi

# Keep the real toolchain; swap everything else for a throwaway profile.
: "${HOME:?HOME must be set}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO/target}"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/ensembly-stranger.XXXXXX")
WORK=$(CDPATH='' cd -- "$WORK" && pwd)
if [ "$KEEP" = 1 ]; then
  trap 'printf "kept %s\n" "$WORK"' EXIT
else
  trap 'rm -rf "$WORK"' EXIT
fi
export HOME="$WORK/home"
export XDG_CONFIG_HOME="$WORK/config"
export XDG_DATA_HOME="$WORK/data"
export XDG_CACHE_HOME="$WORK/cache"
export XDG_STATE_HOME="$WORK/state"
export PATH="$WORK/prefix/bin:$PATH"
export ENSEMBLY_OFFLINE=1
unset CLAUDE_CONFIG_DIR ENSEMBLY_MEMORY PERAM_MEMORY ENSEMBLY_DB PERAM_DB 2>/dev/null || true
mkdir -p "$HOME"
printf 'work %s\n' "$WORK"

step "install (all harnesses)"
run sh "$REPO/scripts/install.sh" --harness all --prefix "$WORK/prefix"
[ -x "$WORK/prefix/bin/ensembly" ] || fail "ensembly not installed under $WORK/prefix/bin"
run ensembly version
PLUGIN="$REPO/marketplaces/grokbot/ensembly"
cursor_link="$HOME/.cursor/plugins/local/ensembly"
[ -L "$cursor_link" ] || fail "cursor plugin link missing: $cursor_link"
case "$(readlink "$cursor_link")" in
  "$PLUGIN") ;;
  *) fail "cursor plugin link: want $PLUGIN got $(readlink "$cursor_link")" ;;
esac
skills_dir="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/skills"
for skill in ensembly record-layer verified-retrieve; do
  dest="$skills_dir/$skill"
  [ -L "$dest" ] || fail "claude skill link missing: $dest"
  want="$PLUGIN/skills/$skill"
  case "$(readlink "$dest")" in
    "$want") ;;
    *) fail "claude skill $skill: want $want got $(readlink "$dest")" ;;
  esac
done

step "default CLI surface (no opportunity-finder in default build)"
run sh "$REPO/scripts/assert-default-cli-surface.sh" "$WORK/prefix/bin/ensembly"

step "init"
printf '+ ensembly init\n'
out=$(ensembly init) || fail "ensembly init exit $?"
printf '%s\n' "$out"
CONFIG="$XDG_CONFIG_HOME/ensembly/config.toml"
DATA="$XDG_DATA_HOME/ensembly"
[ "$out" = "$CONFIG" ] || fail "init printed $out, want $CONFIG"
run grep -qxF "data_dir = \"$DATA\"" "$CONFIG"
! grep -q opportunity_finder "$CONFIG" || fail "config must not contain packs.opportunity_finder"
[ -d "$DATA" ] || fail "init did not create $DATA"
printf '+ ensembly init   (again: must refuse, exit 7)\n'
set +e
ensembly init >/dev/null 2>&1
code=$?
set -e
[ "$code" = 7 ] || fail "second init exit $code, want 7"

MEM="$DATA/ensembly-memory.json"
FIX="$REPO/fixtures/issue-1-runtime.json"
cd "$WORK"

step "doctor before the sample"
printf '+ ensembly doctor --offline\n'
out=$(ensembly --no-memory doctor --offline) || fail "doctor exit $?"
printf '%s\n' "$out"
expect_line "$out" "harness cursor present" "ops-db absent" "latest skipped"

step "sample: runtime HITL/HOOTL loop on fixtures/issue-1-runtime.json"
run ensembly --memory "$MEM" runtime load --fixture "$FIX"
run ensembly --memory "$MEM" runtime status
run ensembly --memory "$MEM" runtime tick
run ensembly --memory "$MEM" runtime tick
run ensembly --memory "$MEM" runtime tick
run ensembly --memory "$MEM" runtime tick
run ensembly --memory "$MEM" runtime approve pay-rent
run ensembly --memory "$MEM" runtime claim grocery-errand
run ensembly --memory "$MEM" runtime complete grocery-errand
printf '+ ensembly runtime status --json\n'
out=$(ensembly --memory "$MEM" runtime status --json) || fail "status exit $?"
printf '%s\n' "$out"
printf '%s\n' "$out" | grep -qF '"ok": true' || fail "runtime status not ok"
[ -s "$MEM" ] || fail "memory file not written: $MEM"

step "sample: pulse-pack export, then validate it"
run ensembly --memory "$MEM" pulse-pack export --out "$WORK/session.pulse.json"
run ensembly --memory "$MEM" pulse-pack import --pack "$WORK/session.pulse.json" --dry-run

step "doctor after the sample"
out=$(ensembly --no-memory doctor --offline) || fail "doctor exit $?"
printf '%s\n' "$out"
expect_line "$out" "ops-db ok"

step "workspace tests"
cd "$REPO"
run cargo test --workspace --locked

step "green"
