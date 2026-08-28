#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
ARCHIVE=${1:-"$ROOT/dist/headroom-v${VERSION}-macos-$(uname -m).zip"}
PROFILE=${APPLE_NOTARY_PROFILE:?Set APPLE_NOTARY_PROFILE to an xcrun notarytool keychain profile}

xcrun notarytool submit "$ARCHIVE" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$ROOT/dist/Headroom.app"
xcrun stapler validate "$ROOT/dist/Headroom.app"
spctl --assess --type execute --verbose=2 "$ROOT/dist/Headroom.app"
