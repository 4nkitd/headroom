#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
BUILD=${BUILD_NUMBER:-$(date +%Y%m%d%H%M)}
APP="$ROOT/dist/Headroom.app"
IDENTITY=${CODESIGN_IDENTITY:--}

if [[ ${1:-} != "--skip-build" ]]; then
  cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/PlugIns"
install -m 755 "$ROOT/target/release/headroom" "$APP/Contents/MacOS/headroom"
sed -e "s/__VERSION__/$VERSION/g" -e "s/__BUILD__/$BUILD/g" \
  "$ROOT/scripts/Info.plist.in" > "$APP/Contents/Info.plist"

# WidgetExtension target compilation & embedding
PLUGINS_DIR="$APP/Contents/PlugIns/HeadroomWidget.appex"
mkdir -p "$PLUGINS_DIR/Contents/MacOS"
swiftc -parse-as-library -target arm64-apple-macosx14.0 -module-cache-path /tmp/swift_cache \
  "$ROOT/widget/HeadroomWidget.swift" \
  -o "$PLUGINS_DIR/Contents/MacOS/HeadroomWidget"
cp "$ROOT/widget/Info.plist" "$PLUGINS_DIR/Contents/Info.plist"

if [[ -f "$ROOT/assets/AppIcon-1024.png" ]]; then
  ICONSET="$ROOT/dist/AppIcon.iconset"
  rm -rf "$ICONSET"
  mkdir -p "$ICONSET"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$ROOT/assets/AppIcon-1024.png" \
      --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" "$ROOT/assets/AppIcon-1024.png" \
      --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
  rm -rf "$ICONSET"
fi

if [[ "$IDENTITY" == "-" ]]; then
  codesign --force --deep --sign - --entitlements "$ROOT/scripts/widget-entitlements.plist" "$PLUGINS_DIR"
  codesign --force --deep --sign - --entitlements "$ROOT/scripts/entitlements.plist" "$APP"
  echo "Packaged with ad-hoc signing; set CODESIGN_IDENTITY for distribution."
else
  codesign --force --deep --options runtime --timestamp --sign "$IDENTITY" --entitlements "$ROOT/scripts/widget-entitlements.plist" "$PLUGINS_DIR"
  codesign --force --deep --options runtime --timestamp --sign "$IDENTITY" --entitlements "$ROOT/scripts/entitlements.plist" "$APP"
fi

codesign --verify --deep --strict --verbose=2 "$APP"
plutil -lint "$APP/Contents/Info.plist"

ARCHIVE="$ROOT/dist/headroom-v${VERSION}-macos-$(uname -m).zip"
rm -f "$ARCHIVE"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ARCHIVE"
echo "$APP"
echo "$ARCHIVE"
