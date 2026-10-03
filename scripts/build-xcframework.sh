#!/bin/bash
# Build EscriturasFFI.xcframework and the generated Swift bindings for the
# EscriturasKit Swift package (apple/EscriturasKit). macOS only.
#
# Usage:
#   scripts/build-xcframework.sh            # universal (arm64 + x86_64), release
#   scripts/build-xcframework.sh --host     # this Mac's architecture only (faster)
#   scripts/build-xcframework.sh --debug    # debug build (combine with --host)
#
# Outputs (both gitignored):
#   apple/EscriturasKit/EscriturasFFI.xcframework
#   apple/EscriturasKit/Sources/EscriturasKit/Generated/escrituras_ffi.swift

set -eu

HOST_ONLY=0
PROFILE=release
for arg in "$@"; do
  case "$arg" in
    --host) HOST_ONLY=1 ;;
    --debug) PROFILE=debug ;;
    *) echo "Unknown option: $arg" >&2; exit 1 ;;
  esac
done

if [ "$(uname -s)" != "Darwin" ]; then
  echo "This script builds Apple frameworks and must run on macOS." >&2
  exit 1
fi

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PACKAGE="$ROOT/apple/EscriturasKit"
BUILD="$ROOT/target/xcframework"
cd "$ROOT"

if [ "$HOST_ONLY" = "1" ]; then
  if [ "$(uname -m)" = "arm64" ]; then
    TARGETS="aarch64-apple-darwin"
  else
    TARGETS="x86_64-apple-darwin"
  fi
else
  TARGETS="aarch64-apple-darwin x86_64-apple-darwin"
fi

CARGO_PROFILE_FLAG=""
if [ "$PROFILE" = "release" ]; then
  CARGO_PROFILE_FLAG="--release"
fi

# Build the static library for each target. `for t in $TARGETS` would not
# split words in zsh, so iterate over a newline-separated list instead.
LIBS=""
for target in $(echo "$TARGETS" | tr ' ' '\n'); do
  echo "==> Building escrituras-ffi for $target ($PROFILE)"
  rustup target add "$target" >/dev/null
  cargo build -p escrituras-ffi --lib --target "$target" $CARGO_PROFILE_FLAG
  LIBS="$LIBS $ROOT/target/$target/$PROFILE/libescrituras_ffi.a"
done

rm -rf "$BUILD"
mkdir -p "$BUILD/headers" "$BUILD/swift"

echo "==> Combining libraries"
# Command substitution splits into words in both bash and zsh
# shellcheck disable=SC2046
lipo -create $(echo "$LIBS") -output "$BUILD/libescrituras_ffi.a"

echo "==> Generating Swift bindings"
FIRST_TARGET=$(echo "$TARGETS" | cut -d' ' -f1)
cargo run -q -p escrituras-ffi --bin uniffi-bindgen -- generate \
  --library "$ROOT/target/$FIRST_TARGET/$PROFILE/libescrituras_ffi.a" \
  --language swift \
  --out-dir "$BUILD/swift"

cp "$BUILD/swift/EscriturasFFI.h" "$BUILD/headers/"
cp "$BUILD/swift/EscriturasFFI.modulemap" "$BUILD/headers/module.modulemap"

echo "==> Creating EscriturasFFI.xcframework"
rm -rf "$PACKAGE/EscriturasFFI.xcframework"
xcodebuild -create-xcframework \
  -library "$BUILD/libescrituras_ffi.a" \
  -headers "$BUILD/headers" \
  -output "$PACKAGE/EscriturasFFI.xcframework" >/dev/null

mkdir -p "$PACKAGE/Sources/EscriturasKit/Generated"
cp "$BUILD/swift/escrituras_ffi.swift" "$PACKAGE/Sources/EscriturasKit/Generated/"

echo "==> Done. Run the Swift tests with: (cd apple/EscriturasKit && swift test)"
