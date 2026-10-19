#!/bin/bash
# Sign target/Escrituras.app, put it in a disk image and notarize it.
# Run scripts/build-mac-app.sh first (with --universal for a release).
#
# Usage: scripts/package-mac-app.sh <version>
#   Writes target/dist/Escrituras-<version>.dmg
#
# Signing and notarization use these environment variables when they are set
# (the release workflow maps them from repository secrets):
#   MACOS_SIGNING_IDENTITY   e.g. "Developer ID Application: Jane Doe (TEAMID1234)"
#   MACOS_CERTIFICATE        the Developer ID certificate, a base64-encoded .p12
#   MACOS_CERTIFICATE_PASSWORD
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD   for notarization (an
#                            app-specific password from account.apple.com)
# Without MACOS_SIGNING_IDENTITY the app keeps its ad-hoc signature: it works,
# but Gatekeeper blocks it on first open until it is allowed in System Settings.
# If MACOS_CERTIFICATE is unset, the identity must already be in a keychain.

set -eu

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>" >&2
  exit 1
fi
VERSION="$1"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "This script packages a macOS app and must run on macOS." >&2
  exit 1
fi

ROOT=$(cd "$(dirname "$0")/.." && pwd)
APP="$ROOT/target/Escrituras.app"
DIST="$ROOT/target/dist"
DMG="$DIST/Escrituras-$VERSION.dmg"
IDENTITY="${MACOS_SIGNING_IDENTITY:-}"

if [ ! -d "$APP" ]; then
  echo "$APP not found. Run scripts/build-mac-app.sh first." >&2
  exit 1
fi
mkdir -p "$DIST"

KEYCHAIN=""
cleanup() {
  if [ -n "$KEYCHAIN" ]; then
    security delete-keychain "$KEYCHAIN" >/dev/null 2>&1 || true
  fi
  rm -rf "${STAGE:-}"
}
trap cleanup EXIT

# Import the certificate into a throwaway keychain
if [ -n "$IDENTITY" ] && [ -n "${MACOS_CERTIFICATE:-}" ]; then
  echo "==> Importing signing certificate"
  KEYCHAIN="${RUNNER_TEMP:-$(mktemp -d)}/escrituras-signing.keychain-db"
  KEYCHAIN_PASSWORD=$(uuidgen)
  CERT="$(mktemp -d)/certificate.p12"
  echo "$MACOS_CERTIFICATE" | base64 --decode > "$CERT"
  security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
  security set-keychain-settings -lut 21600 "$KEYCHAIN"
  security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
  security import "$CERT" -k "$KEYCHAIN" -P "${MACOS_CERTIFICATE_PASSWORD:-}" -T /usr/bin/codesign
  security set-key-partition-list -S apple-tool:,apple: -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN" >/dev/null
  # Search the new keychain first, then the existing ones
  # shellcheck disable=SC2046 # the list is newline-separated quoted paths
  security list-keychains -d user -s "$KEYCHAIN" $(security list-keychains -d user | tr -d '"')
  rm -f "$CERT"
fi

if [ -n "$IDENTITY" ]; then
  echo "==> Signing with $IDENTITY"
  # Hardened runtime and a secure timestamp are required for notarization
  codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
  codesign --verify --strict --verbose=2 "$APP"
else
  echo "==> MACOS_SIGNING_IDENTITY is not set; keeping the ad-hoc signature"
fi

echo "==> Creating $DMG"
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "Escrituras $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null

if [ -n "$IDENTITY" ]; then
  codesign --force --timestamp --sign "$IDENTITY" "$DMG"
fi

if [ -n "$IDENTITY" ] && [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_APP_PASSWORD:-}" ]; then
  echo "==> Notarizing (this usually takes a few minutes)"
  xcrun notarytool submit "$DMG" \
    --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" \
    --wait
  xcrun stapler staple "$DMG"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
elif [ -n "$IDENTITY" ]; then
  echo "==> Notarization credentials are not set; the disk image is signed but not notarized"
fi

echo "$DMG"
