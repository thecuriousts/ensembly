#!/bin/sh
# Install the ensembly CLI from this checkout and wire it into one or more harnesses.
# Local checkout only: no curl|sh, no sudo, no toolchain installs, no marketplace edits.
# Safe to re-run. Exit 0 done, 1 a step failed or a prerequisite is missing, 2 bad arguments.
set -eu

usage() {
  cat <<'EOF'
usage: scripts/install.sh [--harness cursor|claude|grok|cli|all] [--prefix DIR] [--update] [--dry-run]

  --harness H   what to wire after the CLI is installed (default: cli)
                  cli     the `ensembly` binary only
                  cursor  link the ensembly plugin into ~/.cursor/plugins/local
                  claude  link the ensembly skills into ${CLAUDE_CONFIG_DIR:-~/.claude}/skills
                  grok    print how to load the plugin in Grok Bot (writes nothing)
                  all     every one of the above
  --prefix DIR  cargo install root; the binary lands in DIR/bin (default: cargo's own)
  --update      git pull --ff-only in this checkout, then reinstall
  --dry-run     print every command, run none
EOF
}

say() { printf '%s\n' "$*"; }
warn() { printf 'warn: %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# Print each command; run it unless --dry-run.
run() {
  printf '+ %s\n' "$*"
  [ "$DRY_RUN" = 1 ] || "$@"
}

# install.root from cargo config (after CARGO_INSTALL_ROOT); else $CARGO_HOME.
install_root_from_toml() {
  file=$1
  [ -f "$file" ] || return 1
  awk '
    /^\[install\]/ { in_install=1; next }
    /^\[/ { in_install=0 }
    in_install && /^[[:space:]]*root[[:space:]]*=/ {
      sub(/^[^=]*=[[:space:]]*/, "", $0)
      gsub(/"/, "", $0)
      gsub(/\047/, "", $0)
      sub(/[[:space:]]*#.*$/, "", $0)
      gsub(/[[:space:]]+$/, "", $0)
      print $0
      exit 0
    }
  ' "$file"
}

expand_tilde() {
  p=$1
  case $p in
    "~") printf '%s' "$HOME" ;;
    "~/"*) printf '%s' "$HOME/${p#\~/}" ;;
    *) printf '%s' "$p" ;;
  esac
}

cargo_install_root() {
  if [ -n "${CARGO_INSTALL_ROOT:-}" ]; then
    printf '%s' "$CARGO_INSTALL_ROOT"
    return 0
  fi
  cargo_home=${CARGO_HOME:-$HOME/.cargo}
  for file in "$REPO/.cargo/config.toml" "$cargo_home/config.toml" "$cargo_home/config"; do
    root=$(install_root_from_toml "$file" 2>/dev/null || true)
    if [ -n "$root" ]; then
      expand_tilde "$root"
      return 0
    fi
  done
  printf '%s' "$cargo_home"
}

HARNESS=cli
PREFIX=
UPDATE=0
DRY_RUN=0
while [ $# -gt 0 ]; do
  case $1 in
    --harness) [ $# -ge 2 ] || { usage >&2; exit 2; }; HARNESS=$2; shift 2 ;;
    --harness=*) HARNESS=${1#--harness=}; shift ;;
    --prefix) [ $# -ge 2 ] || { usage >&2; exit 2; }; PREFIX=$2; shift 2 ;;
    --prefix=*) PREFIX=${1#--prefix=}; shift ;;
    --update) UPDATE=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'error: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done
case $HARNESS in
  cursor|claude|grok|cli|all) ;;
  *) printf 'error: --harness must be cursor, claude, grok, cli or all (got %s)\n' "$HARNESS" >&2; exit 2 ;;
esac
REPO=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
PLUGIN=$REPO/marketplaces/grokbot/ensembly
[ -f "$REPO/crates/ensembly-kernel/Cargo.toml" ] || die "not an ensembly checkout: $REPO"
[ -d "$PLUGIN/skills" ] || die "plugin folder missing: $PLUGIN"
: "${HOME:?HOME must be set}"

# --- prerequisites: report, never install ---
missing=0
if ! command -v git >/dev/null 2>&1; then
  missing=1
  warn "git not found. Install it from your OS packages (macOS: xcode-select --install; Debian/Ubuntu: apt install git; Fedora: dnf install git)."
fi
if ! command -v cargo >/dev/null 2>&1; then
  missing=1
  warn "cargo not found. Install Rust with rustup: see https://rustup.rs and read the installer before you run it."
  warn "rust-toolchain.toml pins the toolchain; rustup fetches it on the first build."
elif ! command -v rustup >/dev/null 2>&1; then
  warn "rustup not found; cargo must already be the version in rust-toolchain.toml."
fi
[ "$missing" = 0 ] || die "missing prerequisites above; re-run when they are on PATH"
if [ -n "$PREFIX" ] && [ "$DRY_RUN" = 0 ]; then
  mkdir -p -- "$PREFIX"
  PREFIX=$(CDPATH='' cd -- "$PREFIX" && pwd)
fi
say "repo $REPO"
say "harness $HARNESS"

# --- update ---
if [ "$UPDATE" = 1 ]; then
  run git -C "$REPO" pull --ff-only
fi

# --- CLI ---
# --force: a path install keeps its version string across commits, so reinstall every time.
if [ -n "$PREFIX" ]; then
  run cargo install --path "$REPO/crates/ensembly-kernel" --locked --force --bin ensembly --root "$PREFIX"
  BIN_DIR=$PREFIX/bin
elif [ "$DRY_RUN" = 1 ]; then
  run cargo install --path "$REPO/crates/ensembly-kernel" --locked --force --bin ensembly
  BIN_DIR=$(CDPATH='' cd -- "$(cargo_install_root)" && pwd)/bin
else
  run cargo install --path "$REPO/crates/ensembly-kernel" --locked --force --bin ensembly
  BIN_DIR=$(CDPATH='' cd -- "$(cargo_install_root)" && pwd)/bin
fi
case :${PATH}: in
  *:"$BIN_DIR":*) ;;
  *) warn "$BIN_DIR is not on PATH. Add it, e.g.: export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac
if [ "$DRY_RUN" = 0 ]; then
  [ -x "$BIN_DIR/ensembly" ] || die "installed binary missing: $BIN_DIR/ensembly"
  "$BIN_DIR/ensembly" version >/dev/null || die "installed binary does not run: $BIN_DIR/ensembly"
fi

# Link src at dest. An existing link to src is fine; anything else is left alone.
SKIPPED=0
link() {
  src=$1
  dest=$2
  if [ -L "$dest" ]; then
    cur=$(readlink "$dest")
    if [ "$cur" = "$src" ]; then
      say "ok $dest"
      return 0
    fi
    warn "skip $dest: links to $cur, not this checkout. Remove it and re-run to relink."
    SKIPPED=1
    return 0
  fi
  if [ -e "$dest" ]; then
    warn "skip $dest: exists and is not a link. Move it aside and re-run."
    SKIPPED=1
    return 0
  fi
  run mkdir -p "$(dirname -- "$dest")"
  run ln -s "$src" "$dest"
}

wire_cursor() {
  say "== cursor"
  link "$PLUGIN" "$HOME/.cursor/plugins/local/ensembly"
  say "Restart Cursor, then load the \`ensembly\` skill. \`ensembly doctor\` lists the plugin."
}

wire_claude() {
  say "== claude"
  skills_dir=${CLAUDE_CONFIG_DIR:-$HOME/.claude}/skills
  for skill in "$PLUGIN"/skills/*/; do
    skill=${skill%/}
    link "$skill" "$skills_dir/${skill##*/}"
  done
  say "Start a new Claude Code session; the skills show as ensembly, record-layer, verified-retrieve."
  say "Per-session alternative (no links): claude --plugin-dir \"$PLUGIN\""
}

wire_grok() {
  say "== grok (manual; nothing written)"
  say "1. In Grok Bot, install a local plugin from this folder: $PLUGIN"
  say "2. In any bot, load the \`ensembly\` skill, then \`record-layer\` or \`verified-retrieve\`."
  say "3. Optional, Grok Build MCP (read-only memory and record view):"
  say "     cargo install --path \"$REPO/crates/ensembly-agents\" --locked --bin ensembly-mcp"
  say "     grok mcp add --scope project ensembly -- ensembly-mcp"
}

case $HARNESS in
  cursor) wire_cursor ;;
  claude) wire_claude ;;
  grok) wire_grok ;;
  all) wire_cursor; wire_claude; wire_grok ;;
  cli) ;;
esac

say "== next"
say "ensembly init            # writes ${XDG_CONFIG_HOME:-$HOME/.config}/ensembly/config.toml"
say "ensembly doctor --offline"
[ "$DRY_RUN" = 0 ] || say "dry run: nothing was changed"
[ "$SKIPPED" = 0 ] || die "some links were skipped (see warnings)"
say "done"
