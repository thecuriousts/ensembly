#!/bin/sh
# Fail if default (no opportunity-finder) ensembly help leaks pack commands or job-hunt prose.
# Usage: scripts/assert-default-cli-surface.sh /path/to/ensembly
# Exit 0 clean, 1 leak or binary error, 2 usage.
set -eu

fail() {
  printf 'FAIL %s\n' "$*" >&2
  exit 1
}

if [ $# -ne 1 ] || [ ! -x "$1" ]; then
  printf 'usage: scripts/assert-default-cli-surface.sh /path/to/ensembly\n' >&2
  exit 2
fi
BIN=$1

assert_no_leak() {
  label=$1
  help=$2
  if printf '%s\n' "$help" | grep -qE '^\s+(apps|decision)\b'; then
    fail "$label: help lists apps or decision"
  fi
  if printf '%s\n' "$help" | grep -qiE '(job hunt|recruiter|verdict.*triage)'; then
    fail "$label: job-hunt prose in help"
  fi
}

check_help() {
  label=$1
  shift
  printf '+ %s %s\n' "$BIN" "$*"
  help=$("$BIN" "$@" 2>&1) || fail "$BIN $* exited $?"
  assert_no_leak "$label" "$help"
}

top=$("$BIN" --help 2>&1) || fail "$BIN --help exited $?"
assert_no_leak "ensembly --help" "$top"
sub_cmds=$(printf '%s\n' "$top" | sed -n 's/^  \([a-z0-9][-a-z0-9]*\).*/\1/p' | grep -v '^help' | grep . || true)
[ -n "$sub_cmds" ] || fail "no subcommands discovered from --help"
n_cmds=$(printf '%s\n' "$sub_cmds" | wc -l | tr -d ' ')
[ "$n_cmds" -ge 3 ] || fail "too few subcommands discovered ($n_cmds)"
while read -r cmd; do
  [ -n "$cmd" ] || continue
  check_help "ensembly $cmd --help" "$cmd" --help
done <<EOF
$sub_cmds
EOF
printf 'ok: default CLI surface has no apps/decision leaks\n'