#!/bin/bash
set -e

REPO="noahread/escrituras"
BINARY="scriptures"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin) OS="macos" ;;
  linux) OS="linux" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Get latest release
RELEASE_URL="https://api.github.com/repos/$REPO/releases/latest"
DOWNLOAD_URL=$(curl -sL "$RELEASE_URL" | grep "browser_download_url.*$OS-$ARCH" | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
  echo "Could not find release for $OS-$ARCH"
  exit 1
fi

echo "Downloading $BINARY for $OS-$ARCH..."
curl -sL "$DOWNLOAD_URL" | tar -xz -C /tmp

echo "Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
mv "/tmp/$BINARY" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY"

echo ""
echo "Installed $BINARY to $INSTALL_DIR/$BINARY"
echo ""
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  echo "Add to your PATH by adding this to ~/.bashrc or ~/.zshrc:"
  echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
fi
