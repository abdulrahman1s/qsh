#!/bin/sh
# Any-distro installer for qsh.
#
# Downloads a prebuilt GitHub Release binary and installs it under ~/.local by
# default. The installer never uses sudo/doas; custom install dirs must already
# be writable by the current user.

set -eu

REPO="${QSH_GITHUB_REPO:-abdulrahman1s/qsh}"
VERSION="${QSH_VERSION:-latest}"
TARGET="${QSH_TARGET:-}"
ARCHIVE_URL="${QSH_ARCHIVE_URL:-}"
PREFIX="${PREFIX:-}"
BIN_DIR="${BIN_DIR:-}"
TARGET_SHELL="${QSH_SHELL:-}"

DO_UNINSTALL=0
EMIT_INIT=0
EDIT_ZSHRC=0
EDIT_BASHRC=0
EDIT_FISHRC=0
ASSUME_YES=${QSH_YES:-0}
SKIP_RC=${QSH_NO_MODIFY_RC:-0}
SHELL_SETUP_DONE=0
TMP_DIR=
QSH_SRC=

usage() {
  cat <<'EOF'
Usage: sh install.sh [OPTIONS]

Download a prebuilt qsh release from GitHub and install it for the current user.

Options:
  --version VERSION  Release version to install, with or without v (default: latest)
  --target TARGET    Release target triple (default: detect host)
  --repo OWNER/REPO  GitHub repo that hosts release assets
  --archive-url URL  Full release tarball URL; skips repo/version URL building
  --prefix DIR       Install under DIR/bin (default: $HOME/.local)
  --bin-dir DIR      Install directly into DIR
  --user             Install under $HOME/.local (the default)
  --shell SHELL      Shell for --emit-init: zsh, bash, or fish
  --zshrc            Append qsh integration to ~/.zshrc (skips the prompt)
  --bashrc           Append qsh integration to ~/.bashrc (skips the prompt)
  --fishrc           Append qsh integration to ~/.config/fish/config.fish (skips the prompt)
  -y, --yes          Auto-accept the shell-integration prompt
  --no-modify-rc     Never modify a shell rc file; don't prompt
  --emit-init        Print init code to stdout after install
  --uninstall        Remove the installed qsh binary from the selected prefix
  -h, --help         Show this help

By default the installer detects your shell ($SHELL) and asks before
appending the qsh init line to the matching rc file. Use --yes for
non-interactive installs or --no-modify-rc to opt out entirely.

Environment:
  QSH_VERSION        Same as --version
  QSH_TARGET         Same as --target
  QSH_GITHUB_REPO    Same as --repo
  QSH_ARCHIVE_URL    Same as --archive-url
  PREFIX             Same as --prefix; defaults to $HOME/.local
  BIN_DIR            Same as --bin-dir; defaults to $PREFIX/bin
  QSH_SHELL          Default shell for the prompt and --emit-init
  QSH_YES            Same as --yes
  QSH_NO_MODIFY_RC   Same as --no-modify-rc
EOF
}

if [ -t 2 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-dumb}" != dumb ]; then
  BOLD=$(printf '\033[1m')
  DIM=$(printf '\033[2m')
  RED=$(printf '\033[31m')
  GREEN=$(printf '\033[32m')
  YELLOW=$(printf '\033[33m')
  CYAN=$(printf '\033[36m')
  RESET=$(printf '\033[0m')
else
  BOLD=
  DIM=
  RED=
  GREEN=
  YELLOW=
  CYAN=
  RESET=
fi

say() {
  printf '%s%s==>%s %s\n' "$BOLD" "$CYAN" "$RESET" "$*" >&2
}

ok() {
  printf ' %s✓%s %s\n' "$GREEN" "$RESET" "$*" >&2
}

warn() {
  printf ' %s!%s %s\n' "$YELLOW" "$RESET" "$*" >&2
}

hint() {
  printf '   %s%s%s\n' "$DIM" "$*" "$RESET" >&2
}

die() {
  printf '%s%serror:%s %s\n' "$BOLD" "$RED" "$RESET" "$*" >&2
  exit 1
}

cleanup() {
  if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
    rm -rf "$TMP_DIR"
  fi
}
trap cleanup EXIT HUP INT TERM

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

make_tmp_dir() {
  if command -v mktemp >/dev/null 2>&1; then
    TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/qsh-install.XXXXXX")
  else
    TMP_DIR="${TMPDIR:-/tmp}/qsh-install.$$"
    mkdir -p "$TMP_DIR"
  fi
}

download_file() {
  url=$1
  output=$2

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$output"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$output" "$url"
  else
    die "missing required command: curl or wget"
  fi
}

download_required() {
  url=$1
  output=$2

  download_file "$url" "$output" || die "failed to download $url"
}

detect_target() {
  os=$(uname -s 2>/dev/null || true)
  arch=$(uname -m 2>/dev/null || true)

  case "$os:$arch" in
    Linux:x86_64|Linux:amd64)
      printf '%s\n' x86_64-unknown-linux-gnu
      ;;
    Darwin:arm64|Darwin:aarch64)
      printf '%s\n' aarch64-apple-darwin
      ;;
    Darwin:x86_64|Darwin:amd64)
      printf '%s\n' x86_64-apple-darwin
      ;;
    *)
      die "no prebuilt release target for $os/$arch; set QSH_TARGET or use --target"
      ;;
  esac
}

normalise_tag() {
  case "$1" in
    v*) printf '%s\n' "$1" ;;
    *) printf 'v%s\n' "$1" ;;
  esac
}

resolve_tag() {
  if [ "$VERSION" != latest ]; then
    normalise_tag "$VERSION"
    return
  fi

  latest_json="$TMP_DIR/latest.json"
  say "resolving latest GitHub release for $REPO"
  download_required "https://api.github.com/repos/$REPO/releases/latest" "$latest_json"

  tag=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$latest_json" | sed -n '1p')
  [ -n "$tag" ] || die "could not find tag_name in GitHub latest-release response"
  printf '%s\n' "$tag"
}

sha256_of() {
  file=$1

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | sed 's/[[:space:]].*//'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | sed 's/[[:space:]].*//'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$file" | sed 's/^.*= //'
  else
    return 1
  fi
}

verify_checksum() {
  archive=$1
  checksum_file=$2

  expected=$(sed 's/[[:space:]].*//' "$checksum_file" | sed -n '1p')
  [ -n "$expected" ] || die "checksum file is empty"

  actual=$(sha256_of "$archive" || true)
  if [ -z "$actual" ]; then
    warn "no sha256 tool found; skipping checksum verification"
    return
  fi

  [ "$actual" = "$expected" ] || die "checksum mismatch for downloaded archive"
  ok "verified sha256 checksum"
}

download_qsh() {
  need_cmd tar
  make_tmp_dir

  if [ -n "$ARCHIVE_URL" ]; then
    url=$ARCHIVE_URL
    archive_name=qsh.tar.gz
  else
    [ -n "$TARGET" ] || TARGET=$(detect_target)
    tag=$(resolve_tag)
    archive_name="qsh-$tag-$TARGET.tar.gz"
    url="https://github.com/$REPO/releases/download/$tag/$archive_name"
  fi

  archive="$TMP_DIR/$archive_name"
  checksum_file="$archive.sha256"

  say "downloading $url"
  download_required "$url" "$archive"

  if download_file "$url.sha256" "$checksum_file"; then
    verify_checksum "$archive" "$checksum_file"
  else
    warn "checksum asset not found; skipping checksum verification"
  fi

  tar -xzf "$archive" -C "$TMP_DIR"

  for candidate in "$TMP_DIR/qsh" "$TMP_DIR"/*/qsh; do
    if [ -f "$candidate" ]; then
      QSH_SRC=$candidate
      break
    fi
  done

  [ -n "$QSH_SRC" ] || die "archive did not contain a qsh binary"
}

detect_shell() {
  shell_name=$(basename "${SHELL:-}" 2>/dev/null || true)
  case "$shell_name" in
    zsh|bash|fish) printf '%s\n' "$shell_name" ;;
    *) printf '%s\n' zsh ;;
  esac
}

append_once() {
  rc_file=$1
  marker=$2
  line=$3
  label=$4
  rc_dir=$(dirname "$rc_file")

  [ -d "$rc_dir" ] || mkdir -p "$rc_dir"
  touch "$rc_file"

  if grep -qxF "$line" "$rc_file"; then
    say "$label integration already present in $rc_file"
  else
    {
      printf '\n%s\n' "$marker"
      printf '%s\n' "$line"
    } >> "$rc_file"
    ok "added $label integration to $rc_file"
  fi
}

bin_dir_in_rc() {
  rc_file=$1
  [ -f "$rc_file" ] || return 1

  grep -qF "$BIN_DIR" "$rc_file" && return 0

  if [ -n "${HOME:-}" ]; then
    case "$BIN_DIR" in
      "$HOME"|"$HOME/"*)
        rest=${BIN_DIR#$HOME}
        grep -qF "\$HOME$rest" "$rc_file" && return 0
        grep -qF "~$rest" "$rc_file" && return 0
        ;;
    esac
  fi

  return 1
}

append_path_once() {
  rc_file=$1
  marker=$2
  line=$3
  label=$4

  if bin_dir_in_rc "$rc_file"; then
    say "$label already references $BIN_DIR in $rc_file"
    return
  fi
  append_once "$rc_file" "$marker" "$line" "$label"
}

append_integration_once() {
  rc_file=$1
  shell=$2
  marker=$3
  line=$4
  label=$5

  if [ -f "$rc_file" ] && grep -qE "qsh[[:space:]]+init[[:space:]]+$shell" "$rc_file"; then
    say "$label integration already present in $rc_file"
    return
  fi
  append_once "$rc_file" "$marker" "$line" "$label"
}

prompt_yes_no() {
  question=$1
  default=$2

  case "$default" in
    y) suffix=" [Y/n] " ;;
    *) suffix=" [y/N] " ;;
  esac

  answer=
  printf '%s%s==>%s %s%s' "$BOLD" "$CYAN" "$RESET" "$question" "$suffix" > /dev/tty
  if ! IFS= read -r answer < /dev/tty; then
    printf '\n' > /dev/tty
    return 1
  fi

  case "$answer" in
    [Yy]|[Yy][Ee][Ss]) return 0 ;;
    [Nn]|[Nn][Oo]) return 1 ;;
    '')
      case "$default" in y) return 0 ;; *) return 1 ;; esac
      ;;
    *) return 1 ;;
  esac
}

setup_shell_rc() {
  shell=$1
  [ -n "${HOME:-}" ] || die "shell integration setup needs HOME to be set"

  case "$shell" in
    zsh)
      append_path_once "$HOME/.zshrc" '# qsh PATH' "export PATH=\"$BIN_DIR:\$PATH\"" "zsh PATH"
      append_integration_once "$HOME/.zshrc" zsh '# qsh zsh integration' "eval \"\$($QSH_BIN init zsh)\"" zsh
      hint "restart your shell or run: exec zsh"
      ;;
    bash)
      append_path_once "$HOME/.bashrc" '# qsh PATH' "export PATH=\"$BIN_DIR:\$PATH\"" "bash PATH"
      append_integration_once "$HOME/.bashrc" bash '# qsh bash integration' "eval \"\$($QSH_BIN init bash)\"" bash
      hint "restart your shell or run: exec bash"
      ;;
    fish)
      append_path_once "$HOME/.config/fish/config.fish" '# qsh PATH' "fish_add_path $BIN_DIR" "fish PATH"
      append_integration_once "$HOME/.config/fish/config.fish" fish '# qsh fish integration' "$QSH_BIN init fish | source" fish
      hint "restart your shell or run: exec fish"
      ;;
    *)
      die "unsupported shell: $shell"
      ;;
  esac
  SHELL_SETUP_DONE=1
}

auto_shell_setup() {
  if [ "$SKIP_RC" = 1 ]; then
    return
  fi
  if [ "$EDIT_ZSHRC" = 1 ] || [ "$EDIT_BASHRC" = 1 ] || [ "$EDIT_FISHRC" = 1 ]; then
    return
  fi

  shell=$TARGET_SHELL
  [ -n "$shell" ] || shell=$(detect_shell)

  case "$shell" in
    zsh) rc_path="$HOME/.zshrc" ;;
    bash) rc_path="$HOME/.bashrc" ;;
    fish) rc_path="$HOME/.config/fish/config.fish" ;;
    *) return ;;
  esac

  [ -n "${HOME:-}" ] || return

  if [ "$ASSUME_YES" = 1 ]; then
    setup_shell_rc "$shell"
    return
  fi

  if [ ! -r /dev/tty ] || [ ! -w /dev/tty ]; then
    warn "non-interactive run; skipping shell-integration setup (use --yes to auto-accept)"
    return
  fi

  if prompt_yes_no "add qsh shell integration to $rc_path?" y; then
    setup_shell_rc "$shell"
  else
    say "skipping rc edit; add integration later with: qsh init $shell"
  fi
}

install_qsh() {
  dest="$BIN_DIR/qsh"
  tmp="$BIN_DIR/.qsh.$$"

  need_cmd install
  install -d -m 0755 "$BIN_DIR"
  install -m 0755 "$QSH_SRC" "$tmp"
  mv -f "$tmp" "$dest"

  version=$("$QSH_SRC" --version 2>/dev/null || printf 'qsh unknown')
  ok "installed $version to $dest"

  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) warn "$BIN_DIR is not on PATH" ;;
  esac
}

uninstall_qsh() {
  dest="$BIN_DIR/qsh"
  if [ ! -e "$dest" ]; then
    say "no qsh binary at $dest"
    return
  fi

  rm -f "$dest"
  ok "removed $dest"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      shift
      [ "$#" -gt 0 ] || die "--version needs a value"
      VERSION="$1"
      ;;
    --version=*)
      VERSION="${1#--version=}"
      ;;
    --target)
      shift
      [ "$#" -gt 0 ] || die "--target needs a target triple"
      TARGET="$1"
      ;;
    --target=*)
      TARGET="${1#--target=}"
      ;;
    --repo)
      shift
      [ "$#" -gt 0 ] || die "--repo needs OWNER/REPO"
      REPO="$1"
      ;;
    --repo=*)
      REPO="${1#--repo=}"
      ;;
    --archive-url)
      shift
      [ "$#" -gt 0 ] || die "--archive-url needs a URL"
      ARCHIVE_URL="$1"
      ;;
    --archive-url=*)
      ARCHIVE_URL="${1#--archive-url=}"
      ;;
    --prefix)
      shift
      [ "$#" -gt 0 ] || die "--prefix needs a directory"
      PREFIX="$1"
      ;;
    --prefix=*)
      PREFIX="${1#--prefix=}"
      ;;
    --bin-dir)
      shift
      [ "$#" -gt 0 ] || die "--bin-dir needs a directory"
      BIN_DIR="$1"
      ;;
    --bin-dir=*)
      BIN_DIR="${1#--bin-dir=}"
      ;;
    --user)
      [ -n "${HOME:-}" ] || die "--user needs HOME to be set"
      PREFIX="$HOME/.local"
      ;;
    --shell)
      shift
      [ "$#" -gt 0 ] || die "--shell needs zsh, bash, or fish"
      TARGET_SHELL="$1"
      ;;
    --shell=*)
      TARGET_SHELL="${1#--shell=}"
      ;;
    --zshrc)
      EDIT_ZSHRC=1
      ;;
    --bashrc)
      EDIT_BASHRC=1
      ;;
    --fishrc)
      EDIT_FISHRC=1
      ;;
    --emit-init)
      EMIT_INIT=1
      ;;
    -y|--yes)
      ASSUME_YES=1
      ;;
    --no-modify-rc)
      SKIP_RC=1
      ;;
    --uninstall)
      DO_UNINSTALL=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
  shift
done

if [ -z "$BIN_DIR" ]; then
  if [ -z "$PREFIX" ]; then
    [ -n "${HOME:-}" ] || die "HOME is not set; use --prefix or --bin-dir"
    PREFIX="$HOME/.local"
  fi
  BIN_DIR="$PREFIX/bin"
fi

case "$TARGET_SHELL" in
  ''|zsh|bash|fish) ;;
  *) die "unsupported shell: $TARGET_SHELL" ;;
esac

if [ "$(id -u)" -eq 0 ]; then
  die "do not run this installer as root; run it as your normal user"
fi

if [ "$DO_UNINSTALL" = 1 ]; then
  uninstall_qsh
  exit 0
fi

download_qsh
install_qsh

QSH_BIN="$BIN_DIR/qsh"

if [ "$EDIT_ZSHRC" = 1 ]; then setup_shell_rc zsh; fi
if [ "$EDIT_BASHRC" = 1 ]; then setup_shell_rc bash; fi
if [ "$EDIT_FISHRC" = 1 ]; then setup_shell_rc fish; fi

auto_shell_setup

if [ "$EMIT_INIT" = 1 ]; then
  [ -n "$TARGET_SHELL" ] || TARGET_SHELL=$(detect_shell)
  "$QSH_BIN" init "$TARGET_SHELL"
elif [ "$SHELL_SETUP_DONE" != 1 ]; then
  say "add shell integration with one of:"
  hint "eval \"\$($QSH_BIN init zsh)\""
  hint "eval \"\$($QSH_BIN init bash)\""
  hint "$QSH_BIN init fish | source"
fi
