#!/bin/bash
# Package a release build of the terminal app with its data, for GitHub
# Releases and install.sh.
#
# Usage: scripts/package-tui.sh <rust target> <archive name>
#   e.g. scripts/package-tui.sh aarch64-apple-darwin scriptures-macos-aarch64
#
# Expects target/<rust target>/release/scriptures(.exe). Writes
# target/dist/<archive name>.tar.gz, or .zip for Windows targets. The archive
# holds the program, the scripture text and the search embeddings side by
# side; the program finds the data next to itself, so it runs once unpacked.

set -eu

if [ $# -ne 2 ]; then
  echo "Usage: $0 <rust target> <archive name>" >&2
  exit 1
fi
TARGET="$1"
NAME="$2"

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DIST="$ROOT/target/dist"
STAGE="$DIST/$NAME"

BINARY=scriptures
if echo "$TARGET" | grep -qF windows; then
  BINARY=scriptures.exe
fi

rm -rf "$STAGE"
mkdir -p "$STAGE/data" "$STAGE/lds-scriptures-2020.12.08/json"
cp "$ROOT/target/$TARGET/release/$BINARY" "$STAGE/"
cp "$ROOT/lds-scriptures-2020.12.08/README.txt" "$STAGE/lds-scriptures-2020.12.08/"
cp "$ROOT/lds-scriptures-2020.12.08/json/lds-scriptures-json.txt" "$STAGE/lds-scriptures-2020.12.08/json/"
cp "$ROOT/data/scripture_embeddings.npy" "$ROOT/data/scripture_metadata.json" "$STAGE/data/"
cp "$ROOT/README.md" "$STAGE/"
if [ -f "$ROOT/LICENSE" ]; then
  cp "$ROOT/LICENSE" "$STAGE/"
fi

cd "$STAGE"
if [ "$BINARY" = "scriptures.exe" ]; then
  rm -f "$DIST/$NAME.zip"
  # Windows runners have 7-Zip; elsewhere use zip
  if command -v 7z >/dev/null 2>&1; then
    7z a -tzip "$DIST/$NAME.zip" . >/dev/null
  else
    zip -qr "$DIST/$NAME.zip" .
  fi
  echo "$DIST/$NAME.zip"
else
  # Files at the top level, as install.sh expects
  tar -czf "$DIST/$NAME.tar.gz" .
  echo "$DIST/$NAME.tar.gz"
fi
