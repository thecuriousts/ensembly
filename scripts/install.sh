#!/bin/sh
# Install the ensembly CLI from this checkout and wire it into one or more harnesses.
# Local checkout only: no curl|sh, no sudo, no toolchain installs, no marketplace edits.
# Safe to re-run. Exit 0 done, 1 a step failed or a prerequisite is missing, 2 bad arguments.
set -eu

usage() {
  cat <<'EOF'
usage: scripts/install.sh [--harness cursor|claude|grok|cli|all] [--prefix DIR] [--update] [--from-source] [--dry-run]

  --harness H   what to wire after the CLI is installed (default: cli)
                  cli     the `ensembly` binary only
                  cursor  copy the ensembly plugin into ~/.cursor/plugins/local
                  claude  link the ensembly skills into ${CLAUDE_CONFIG_DIR:-~/.claude}/skills
                  grok    print how to load the plugin in Grok Bot (writes nothing)
                  all     every one of the above
  --prefix DIR  install root; the binary lands in DIR/bin (default: cargo's own)
  --update      git pull --ff-only in this checkout, then reinstall
  --from-source skip GitHub release assets; build with cargo only
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
  if [ "$p" = "~" ]; then
    printf '%s' "$HOME"
  elif [ "${p#\~/}" != "$p" ]; then
    printf '%s' "$HOME/${p#\~/}"
  else
    printf '%s' "$p"
  fi
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
FROM_SOURCE=0
DRY_RUN=0
while [ $# -gt 0 ]; do
  case $1 in
    --harness) [ $# -ge 2 ] || { usage >&2; exit 2; }; HARNESS=$2; shift 2 ;;
    --harness=*) HARNESS=${1#--harness=}; shift ;;
    --prefix) [ $# -ge 2 ] || { usage >&2; exit 2; }; PREFIX=$2; shift 2 ;;
    --prefix=*) PREFIX=${1#--prefix=}; shift ;;
    --update) UPDATE=1; shift ;;
    --from-source) FROM_SOURCE=1; shift ;;
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
# shellcheck disable=SC1091
. "$REPO/scripts/resolve_release_location.sh"
PLUGIN=$REPO/marketplaces/grokbot/ensembly
[ -f "$REPO/crates/ensembly-kernel/Cargo.toml" ] || die "not an ensembly checkout: $REPO"
[ -d "$PLUGIN/skills" ] || die "plugin folder missing: $PLUGIN"
: "${HOME:?HOME must be set}"

release_target() {
  os=$(uname -s)
  arch=$(uname -m)
  case $os in
    Darwin)
      case $arch in
        arm64) printf '%s' 'aarch64-apple-darwin' ;;
        x86_64) printf '%s' 'x86_64-apple-darwin' ;;
        *) return 1 ;;
      esac ;;
    Linux)
      case $arch in
        x86_64) printf '%s' 'x86_64-unknown-linux-gnu' ;;
        aarch64|arm64) printf '%s' 'aarch64-unknown-linux-gnu' ;;
        *) return 1 ;;
      esac ;;
    *) return 1 ;;
  esac
}

release_repo_root() {
  printf '%s' "${ENSEMBLY_RELEASE_REPO:-https://github.com/thecuriousts/ensembly}"
}

# Verify one asset using its line from SHA256SUMS. Fail closed on missing file or bad digest.
verify_release_checksum() {
  sums_file=$1
  asset_name=$2
  if [ ! -f "$sums_file" ]; then
    die "release checksum file missing: $sums_file"
  fi
  dir=$(CDPATH='' cd -- "$(dirname -- "$sums_file")" && pwd)
  # shellcheck disable=SC2155
  line=$(grep -F " ${asset_name}" "$sums_file" || true)
  if [ -z "$line" ]; then
    die "no SHA256SUMS entry for ${asset_name}"
  fi
  if [ ! -f "${dir}/${asset_name}" ]; then
    die "release asset missing for checksum: ${asset_name}"
  fi
  if ! (cd "$dir" && printf '%s\n' "$line" | checksum_from_sums -); then
    die "checksum verification failed for ${asset_name}"
  fi
}

checksum_from_sums() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$@"
  else
    shasum -a 256 -c "$@"
  fi
}

release_repo_canonical() {
  printf '%s' "$1" | sed 's#/*$##'
}

resolve_latest_release_tag() {
  root=$(release_repo_canonical "$1")
  loc=""
  loc=$(curl -sS -D - -o /dev/null "${root}/releases/latest" 2>/dev/null | awk '/^[Ll]ocation:/ {sub(/\r$/,"",$2); print $2; exit}') || loc=
  if [ -z "$loc" ]; then
    return 1
  fi
  tag=""
  tag=$(resolve_release_location "$root" "$loc") || return 1
  printf '%s' "$tag"
}

try_install_from_release() {
  root=$(release_repo_root)
  tgt=$1
  tag=""
  tag=$(resolve_latest_release_tag "$root") || return 1
  ver=${tag#ensembly-cli--v}
  asset="ensembly-${ver}-${tgt}.tar.gz"
  base="${root}/releases/download/${tag}"
  say "release asset $asset"
  if ! curl -fsSL -o "$2/$asset" "${base}/${asset}"; then
    return 1
  fi
  if ! curl -fsSL -o "$2/SHA256SUMS" "${base}/SHA256SUMS"; then
    die "failed to download SHA256SUMS for ${tag}"
  fi
  verify_release_checksum "$2/SHA256SUMS" "$asset"
  run tar -xzf "$2/$asset" -C "$2"
  unpacked="$2/ensembly-${ver}-${tgt}/ensembly"
  [ -f "$unpacked" ] || die "release archive missing binary: $unpacked"
  if [ -n "$PREFIX" ]; then
    run mkdir -p "$PREFIX/bin"
    run cp "$unpacked" "$PREFIX/bin/ensembly"
    run chmod +x "$PREFIX/bin/ensembly"
    BIN_DIR=$PREFIX/bin
  else
    root_bin=$(cargo_install_root)
    run mkdir -p "${root_bin}/bin"
    run cp "$unpacked" "${root_bin}/bin/ensembly"
    run chmod +x "${root_bin}/bin/ensembly"
    BIN_DIR=$(CDPATH='' cd -- "${root_bin}" && pwd)/bin
  fi
  case $(uname -s) in
    Darwin)
      quarantine_path="${BIN_DIR}/ensembly"
      warn "If macOS blocks the binary, run: xattr -d com.apple.quarantine ${quarantine_path:?}"
      ;;
  esac
  return 0
}

# --- prerequisites: report, never install ---
need_cargo=1
missing=0
if ! command -v git >/dev/null 2>&1; then
  missing=1
  warn "git not found. Install it from your OS packages (macOS: xcode-select --install; Debian/Ubuntu: apt install git; Fedora: dnf install git)."
fi
target=$(release_target 2>/dev/null || true)
if [ "$FROM_SOURCE" = 0 ] && [ -n "$target" ] && command -v curl >/dev/null 2>&1; then
  need_cargo=0
fi
if [ "$need_cargo" = 1 ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    missing=1
    warn "cargo not found. Install Rust with rustup: see https://rustup.rs and read the installer before you run it."
    warn "rust-toolchain.toml pins the toolchain; rustup fetches it on the first build."
  elif ! command -v rustup >/dev/null 2>&1; then
    warn "rustup not found; cargo must already be the version in rust-toolchain.toml."
  fi
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
RELEASE_OK=0
if [ "$FROM_SOURCE" = 0 ] && [ -n "$target" ] && command -v curl >/dev/null 2>&1; then
  if [ "$DRY_RUN" = 1 ]; then
    say "+ try GitHub release asset for target $target ($(release_repo_root)/releases/latest redirect)"
  else
    tmp=$(mktemp -d) || die "mktemp failed"
    trap 'rm -rf "$tmp"' EXIT HUP INT TERM
    if try_install_from_release "$target" "$tmp"; then
      RELEASE_OK=1
    fi
    trap - EXIT HUP INT TERM
    rm -rf "$tmp"
  fi
fi
if [ "$RELEASE_OK" = 0 ]; then
  if [ "$FROM_SOURCE" = 0 ]; then
    warn "no matching release for $target; building from source with cargo"
  fi
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

# Copy src to dest. Cursor skips symlinks in ~/.cursor/plugins/local that point outside it,
# so Cursor gets a real copy. A copy this script made (marker file) or an old link to src is
# replaced; anything else is left alone.
MARKER=.ensembly-installed-from
copy_plugin() {
  src=$1
  dest=$2
  if [ -L "$dest" ]; then
    cur=$(readlink "$dest")
    if [ "$cur" != "$src" ]; then
      warn "skip $dest: links to $cur, not this checkout. Remove it and re-run."
      SKIPPED=1
      return 0
    fi
    run rm -- "$dest"
  elif [ -e "$dest" ]; then
    if [ -f "$dest/$MARKER" ] && [ "$(cat "$dest/$MARKER")" = "$src" ]; then
      run rm -rf -- "$dest"
    else
      warn "skip $dest: exists and was not installed from this checkout. Move it aside and re-run."
      SKIPPED=1
      return 0
    fi
  fi
  # Marker first, then contents: an interrupted copy still carries the marker,
  # so the next run replaces it instead of skipping it.
  run mkdir -p "$dest"
  printf '+ record source in %s\n' "$dest/$MARKER"
  [ "$DRY_RUN" = 1 ] || printf '%s\n' "$src" > "$dest/$MARKER"
  run cp -R "$src/." "$dest/"
}

wire_cursor() {
  say "== cursor"
  copy_plugin "$PLUGIN" "$HOME/.cursor/plugins/local/ensembly"
  say "Restart Cursor (or run Developer: Reload Window), then load the \`ensembly\` skill. \`ensembly doctor --offline\` lists the plugin."
  say "The copy does not follow git pull; re-run this script (add --update to pull first) to refresh it. A re-run replaces the copy, so edits made inside it are lost."
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
[ "$SKIPPED" = 0 ] || die "some links or copies were skipped (see warnings)"
say "done"
