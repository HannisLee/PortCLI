#!/usr/bin/env sh
set -eu

REPO="${PORTCLI_REPO:-HannisLee/PortCLI}"
VERSION="${PORTCLI_VERSION:-latest}"
INSTALL_DIR="${PORTCLI_INSTALL_DIR:-/usr/local/bin}"
BINARY_NAME="portcli"

info() {
  printf '%s\n' "info: $*"
}

fail() {
  printf '%s\n' "error: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

need_cmd curl
need_cmd find
need_cmd install
need_cmd sed
need_cmd tar
need_cmd mktemp
need_cmd uname

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux) ;;
  *) fail "unsupported OS: $OS. This installer currently supports Linux only." ;;
esac

case "$ARCH" in
  x86_64|amd64) TARGET="x86_64-unknown-linux-musl" ;;
  *) fail "unsupported architecture: $ARCH. This installer currently supports Linux x86_64 only." ;;
esac

if [ "$VERSION" = "latest" ]; then
  VERSION="$(
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
      sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\{0,1\}\([^"]*\)".*/\1/p' |
      sed -n '1p'
  )"
  [ -n "$VERSION" ] || fail "could not determine latest version for $REPO"
fi

VERSION="${VERSION#v}"
ARCHIVE="portcli-v${VERSION}-${TARGET}.tar.gz"
DOWNLOAD_URL="${PORTCLI_DOWNLOAD_URL:-https://github.com/$REPO/releases/download/v$VERSION/$ARCHIVE}"

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

info "downloading $DOWNLOAD_URL"
curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ARCHIVE"

info "extracting $ARCHIVE"
tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR"

BIN_PATH="$(find "$TMP_DIR" -type f -name "$BINARY_NAME" | sed -n '1p')"
[ -n "$BIN_PATH" ] || fail "archive does not contain $BINARY_NAME"

if [ ! -d "$INSTALL_DIR" ]; then
  if mkdir -p "$INSTALL_DIR" 2>/dev/null; then
    :
  elif command -v sudo >/dev/null 2>&1; then
    sudo mkdir -p "$INSTALL_DIR"
  else
    fail "cannot create $INSTALL_DIR. Set PORTCLI_INSTALL_DIR to a writable directory or install sudo."
  fi
fi

DEST="$INSTALL_DIR/$BINARY_NAME"
if install -m 0755 "$BIN_PATH" "$DEST" 2>/dev/null; then
  :
elif command -v sudo >/dev/null 2>&1; then
  sudo install -m 0755 "$BIN_PATH" "$DEST"
else
  fail "cannot write $DEST. Set PORTCLI_INSTALL_DIR to a writable directory or install sudo."
fi

info "installed $("$DEST" --version) to $DEST"
