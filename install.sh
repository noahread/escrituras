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

# Set config directory to match Rust dirs::config_dir()
if [ "$OS" = "macos" ]; then
  CONFIG_DIR="${HOME}/Library/Application Support/escrituras"
else
  CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/escrituras"
fi
DATA_DIR="$CONFIG_DIR/data"

case "$ARCH" in
  x86_64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Get latest release info
echo "Fetching latest release..."
RELEASE_URL="https://api.github.com/repos/$REPO/releases/latest"
RELEASE_JSON=$(curl -sfL "$RELEASE_URL") || {
  echo "Error: Failed to fetch release info from GitHub"
  exit 1
}

# Download binary
BINARY_URL=$(echo "$RELEASE_JSON" | grep "browser_download_url.*$OS-$ARCH" | cut -d '"' -f 4)

if [ -z "$BINARY_URL" ]; then
  echo "Could not find release for $OS-$ARCH"
  exit 1
fi

echo "Downloading $BINARY for $OS-$ARCH..."
TEMP_DIR=$(mktemp -d)
curl -sfL "$BINARY_URL" | tar -xz -C "$TEMP_DIR" || {
  echo "Error: Failed to download or extract binary"
  rm -rf "$TEMP_DIR"
  exit 1
}

echo "Installing binary to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
mv "$TEMP_DIR/$BINARY" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY"

# Install scripture data
if [ -d "$TEMP_DIR/lds-scriptures-2020.12.08" ]; then
  echo "Installing scripture data to $CONFIG_DIR..."
  mkdir -p "$CONFIG_DIR"
  rm -rf "$CONFIG_DIR/lds-scriptures-2020.12.08"
  mv "$TEMP_DIR/lds-scriptures-2020.12.08" "$CONFIG_DIR/"
fi

# Install embeddings for semantic search
if [ -d "$TEMP_DIR/data" ]; then
  echo "Installing semantic search data to $DATA_DIR..."
  mkdir -p "$DATA_DIR"
  mv "$TEMP_DIR/data"/* "$DATA_DIR/"
fi

rm -rf "$TEMP_DIR"

echo ""
echo "✓ Installed $BINARY to $INSTALL_DIR/$BINARY"
echo ""
if ! echo ":$PATH:" | grep -qF ":$INSTALL_DIR:"; then
  echo "Add to your PATH by adding this to ~/.bashrc or ~/.zshrc:"
  echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
  echo ""
fi
echo "Run '$BINARY' to start, or '$BINARY --mcp' for MCP server mode."

# Offer to install Claude Code skills
echo ""
printf "Would you like to install scripture study skills for Claude Code? [y/N] "
read REPLY < /dev/tty
if [ "$REPLY" = "y" ] || [ "$REPLY" = "Y" ]; then
  SKILLS_DIR="${HOME}/.claude/skills"
  SKILLS=(
    "scriptures-scripture"
    "scriptures-topical"
    "scriptures-cross-ref"
    "scriptures-compare"
    "scriptures-ponder"
    "scriptures-journal"
    "scriptures-memorize"
    "scriptures-daily"
  )

  # Extract version from SKILL.md file
  get_version() {
    grep "^version:" "$1" 2>/dev/null | sed 's/version: *//' | tr -d ' '
  }

  # Compare semver: returns 0 if $1 > $2, 1 otherwise
  version_gt() {
    local v1="${1:-0.0.0}" v2="${2:-0.0.0}"
    [ "$v1" = "$v2" ] && return 1
    local IFS=.
    local i v1_parts=($v1) v2_parts=($v2)
    for ((i=0; i<3; i++)); do
      local n1="${v1_parts[i]:-0}" n2="${v2_parts[i]:-0}"
      [ "$n1" -gt "$n2" ] && return 0
      [ "$n1" -lt "$n2" ] && return 1
    done
    return 1
  }

  echo "Checking scripture study skills..."
  installed=0
  updated=0
  skipped=0
  updates_available=()

  for skill in "${SKILLS[@]}"; do
    SKILL_URL="https://raw.githubusercontent.com/$REPO/main/skills/${skill}/SKILL.md"
    LOCAL_FILE="${SKILLS_DIR}/${skill}/SKILL.md"
    TEMP_FILE="/tmp/${skill}-SKILL.md"

    # Download remote version
    if ! curl -sfL "$SKILL_URL" -o "$TEMP_FILE"; then
      echo "  ⚠ Failed to download /${skill}"
      continue
    fi
    remote_version=$(get_version "$TEMP_FILE")

    if [ -f "$LOCAL_FILE" ]; then
      local_version=$(get_version "$LOCAL_FILE")
      if version_gt "$remote_version" "$local_version"; then
        updates_available+=("$skill:$local_version:$remote_version")
      else
        echo "  ✓ /${skill} (v${local_version}, up to date)"
        ((skipped++)) || true
      fi
    else
      # New install
      mkdir -p "${SKILLS_DIR}/${skill}"
      mv "$TEMP_FILE" "$LOCAL_FILE"
      echo "  ✓ /${skill} (v${remote_version}, installed)"
      ((installed++)) || true
    fi
  done

  # Offer to update outdated skills
  if [ ${#updates_available[@]} -gt 0 ]; then
    echo ""
    echo "Updates available:"
    for update in "${updates_available[@]}"; do
      IFS=':' read -r skill local_ver remote_ver <<< "$update"
      echo "  /${skill}: v${local_ver} → v${remote_ver}"
    done
    echo ""
    printf "Would you like to update these skills? [y/N] "
    read REPLY < /dev/tty
    if [ "$REPLY" = "y" ] || [ "$REPLY" = "Y" ]; then
      for update in "${updates_available[@]}"; do
        IFS=':' read -r skill local_ver remote_ver <<< "$update"
        TEMP_FILE="/tmp/${skill}-SKILL.md"
        LOCAL_FILE="${SKILLS_DIR}/${skill}/SKILL.md"
        mv "$TEMP_FILE" "$LOCAL_FILE"
        echo "  ✓ /${skill} updated to v${remote_ver}"
        ((updated++)) || true
      done
    else
      echo "  Skipped updates"
      # Clean up temp files
      for update in "${updates_available[@]}"; do
        IFS=':' read -r skill _ _ <<< "$update"
        rm -f "/tmp/${skill}-SKILL.md"
      done
    fi
  fi

  echo ""
  [ $installed -gt 0 ] && echo "✓ Installed $installed new skills"
  [ $updated -gt 0 ] && echo "✓ Updated $updated skills"
  [ $skipped -gt 0 ] && echo "  ($skipped already up to date)"
  echo ""
  echo "Available commands in Claude Code:"
  for skill in "${SKILLS[@]}"; do
    echo "  /${skill}"
  done
fi
