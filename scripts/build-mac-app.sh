#!/bin/bash
# Build Escrituras.app, the native macOS app, into target/Escrituras.app.
# Run scripts/build-xcframework.sh first (it builds the Rust library the app uses).
#
# Usage:
#   scripts/build-mac-app.sh             # release build for this Mac
#   scripts/build-mac-app.sh --debug     # debug build
#   scripts/build-mac-app.sh --universal # arm64 + x86_64 (needs a universal
#                                        # XCFramework: build-xcframework.sh without --host)
#
# VERSION=1.2.3 sets the app's version (default: the version in Cargo.toml).
# The app is signed ad hoc; scripts/package-mac-app.sh signs it for distribution.

set -eu

CONFIG=release
UNIVERSAL=0
for arg in "$@"; do
  case "$arg" in
    --debug) CONFIG=debug ;;
    --universal) UNIVERSAL=1 ;;
    *) echo "Unknown option: $arg" >&2; exit 1 ;;
  esac
done

if [ "$(uname -s)" != "Darwin" ]; then
  echo "This script builds a macOS app and must run on macOS." >&2
  exit 1
fi

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PACKAGE="$ROOT/apple/EscriturasKit"
APP="$ROOT/target/Escrituras.app"

if [ ! -d "$PACKAGE/EscriturasFFI.xcframework" ]; then
  echo "EscriturasFFI.xcframework not found. Run scripts/build-xcframework.sh first." >&2
  exit 1
fi

ARCH_FLAGS=""
if [ "$UNIVERSAL" = "1" ]; then
  ARCH_FLAGS="--arch arm64 --arch x86_64"
fi

echo "==> Building Escrituras ($CONFIG)"
cd "$PACKAGE"
# shellcheck disable=SC2086 # ARCH_FLAGS is empty or a fixed list of flags
swift build -c "$CONFIG" --product Escrituras $ARCH_FLAGS
# shellcheck disable=SC2086
BIN_DIR=$(swift build -c "$CONFIG" --show-bin-path $ARCH_FLAGS)
cd "$ROOT"

# The release workflow passes the tag's version; otherwise use the crate version
VERSION=${VERSION:-$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)}

echo "==> Assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/lds-scriptures-2020.12.08/json" "$APP/Contents/Resources/data"
cp "$BIN_DIR/Escrituras" "$APP/Contents/MacOS/Escrituras"

# Scripture text and the embeddings used for search by meaning
cp lds-scriptures-2020.12.08/json/lds-scriptures-json.txt "$APP/Contents/Resources/lds-scriptures-2020.12.08/json/"
cp data/scripture_embeddings.npy data/scripture_metadata.json "$APP/Contents/Resources/data/"

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>Escrituras</string>
  <key>CFBundleExecutable</key><string>Escrituras</string>
  <key>CFBundleIdentifier</key><string>com.escrituras.app</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Escrituras</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.reference</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
EOF

# Ad-hoc signature so the app runs on Apple Silicon. Distribution needs a
# Developer ID signature and notarization instead.
codesign --force --sign - "$APP" >/dev/null

echo "==> Done: open $APP"
