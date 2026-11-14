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

check_help "ensembly --help" --help
for cmd in record pulse runtime doctor turn digital-flow backup ops-bundle pulse-pack channel-pulse migrate-local-paths version init; do
  check_help "ensembly $cmd --help" "$cmd" --help
done
printf 'ok: default CLI surface has no apps/decision leaks\n'