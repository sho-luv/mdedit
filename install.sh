#!/bin/sh
set -e

REPO="sho-luv/mdedit"
INSTALL_DIR="/usr/local/bin"

# Detect OS and architecture
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
  Darwin)
    case "$ARCH" in
      x86_64) TARGET="mdedit-x86_64-apple-darwin" ;;
      arm64)  TARGET="mdedit-aarch64-apple-darwin" ;;
      *)      echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64) TARGET="mdedit-x86_64-unknown-linux-gnu" ;;
      *)      echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS"; exit 1
    ;;
esac

# Get latest release tag
LATEST=$(curl -sI "https://github.com/$REPO/releases/latest" | grep -i '^location:' | sed 's/.*\/tag\///' | tr -d '\r\n')

if [ -z "$LATEST" ]; then
  echo "Error: could not determine latest release"
  exit 1
fi

# The tag comes from a redirect header; only accept a plain version string.
case "$LATEST" in
  *[!A-Za-z0-9._-]*) echo "Error: unexpected release tag: $LATEST"; exit 1 ;;
esac

BASE="https://github.com/$REPO/releases/download/$LATEST"
ARCHIVE="$TARGET.tar.gz"

echo "Installing mdedit $LATEST for $OS ($ARCH)..."
echo "Downloading $BASE/$ARCHIVE"

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

fetch() {
  curl --proto '=https' --tlsv1.2 -fsSL "$1" -o "$2"
}

fetch "$BASE/$ARCHIVE" "$TMPDIR/$ARCHIVE"

# Verify the download against the release's published checksums before
# anything is unpacked or installed.
if fetch "$BASE/SHA256SUMS" "$TMPDIR/SHA256SUMS" 2>/dev/null; then
  EXPECTED=$(awk -v f="$ARCHIVE" '$2 == f || $2 == "*" f { print $1 }' "$TMPDIR/SHA256SUMS")
  if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "$TMPDIR/$ARCHIVE" | awk '{ print $1 }')
  else
    ACTUAL=$(shasum -a 256 "$TMPDIR/$ARCHIVE" | awk '{ print $1 }')
  fi
  if [ -z "$EXPECTED" ] || [ "$EXPECTED" != "$ACTUAL" ]; then
    echo "Error: checksum mismatch for $ARCHIVE"
    echo "  expected: ${EXPECTED:-<not listed>}"
    echo "  actual:   $ACTUAL"
    exit 1
  fi
  echo "Checksum verified."
else
  echo "Warning: this release publishes no SHA256SUMS; skipping checksum verification."
fi

# Unpack only the binary, never arbitrary paths from the archive.
tar xzf "$TMPDIR/$ARCHIVE" -C "$TMPDIR" mdedit

if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPDIR/mdedit" "$INSTALL_DIR/mdedit"
else
  echo "Need sudo to install to $INSTALL_DIR"
  sudo mv "$TMPDIR/mdedit" "$INSTALL_DIR/mdedit"
fi

chmod +x "$INSTALL_DIR/mdedit"

echo ""
echo "mdedit $LATEST installed to $INSTALL_DIR/mdedit"
echo "Run 'mdedit --help' to get started"
