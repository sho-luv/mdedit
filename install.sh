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

URL="https://github.com/$REPO/releases/download/$LATEST/$TARGET.tar.gz"

echo "Installing mdedit $LATEST for $OS ($ARCH)..."
echo "Downloading $URL"

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL "$URL" | tar xz -C "$TMPDIR"

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
