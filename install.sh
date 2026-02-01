#!/bin/bash
set -e

REPO="noahread/escrituras"
BINARY="scriptures"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/escrituras"
DATA_DIR="$CONFIG_DIR/data"

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

# Get latest release info
echo "Fetching latest release..."
RELEASE_URL="https://api.github.com/repos/$REPO/releases/latest"
RELEASE_JSON=$(curl -sL "$RELEASE_URL")

# Download binary
BINARY_URL=$(echo "$RELEASE_JSON" | grep "browser_download_url.*$OS-$ARCH" | cut -d '"' -f 4)

if [ -z "$BINARY_URL" ]; then
  echo "Could not find release for $OS-$ARCH"
  exit 1
fi

echo "Downloading $BINARY for $OS-$ARCH..."
curl -sL "$BINARY_URL" | tar -xz -C /tmp

echo "Installing binary to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
mv "/tmp/$BINARY" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY"

# Download embeddings for semantic search
EMBEDDINGS_URL=$(echo "$RELEASE_JSON" | grep "browser_download_url.*embeddings.tar.gz" | cut -d '"' -f 4)

if [ -n "$EMBEDDINGS_URL" ]; then
  echo "Downloading semantic search embeddings (~45MB)..."
  mkdir -p "$DATA_DIR"
  curl -sL "$EMBEDDINGS_URL" | tar -xz -C "$DATA_DIR"
  echo "Installed embeddings to $DATA_DIR"
else
  echo "Note: Embeddings not found in release. Semantic search will be disabled."
fi

echo ""
echo "✓ Installed $BINARY to $INSTALL_DIR/$BINARY"
echo ""
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  echo "Add to your PATH by adding this to ~/.bashrc or ~/.zshrc:"
  echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
  echo ""
fi
echo "Run '$BINARY' to start, or '$BINARY --mcp' for MCP server mode."
