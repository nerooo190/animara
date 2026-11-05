#!/usr/bin/env bash
# Build macOS distributables on a Mac:
#   packaging/macos/package.sh            # .app, .dmg, .pkg, .tar.gz for the host architecture
#   UNIVERSAL=1 packaging/macos/package.sh # lipo x86_64 + arm64 builds into a universal binary
# Code signing / notarization run only when these variables are set:
#   MACOS_SIGN_IDENTITY="Developer ID Application: …", MACOS_INSTALLER_IDENTITY="Developer ID Installer: …",
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD (for notarytool).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)"
DIST="$ROOT/dist"; mkdir -p "$DIST"
TD="${CARGO_TARGET_DIR:-$ROOT/target}"
if [[ "${UNIVERSAL:-0}" == 1 ]]; then
  for t in x86_64-apple-darwin aarch64-apple-darwin; do cargo build --release --target $t -p animara-app -p animara-cli; done
  mkdir -p "$TD/universal"
  for b in animara animara-cli; do lipo -create -output "$TD/universal/$b" "$TD/x86_64-apple-darwin/release/$b" "$TD/aarch64-apple-darwin/release/$b"; done
  BIN="$TD/universal"; ARCH=universal
else
  BIN="$TD/release"; ARCH="$(uname -m)"
fi
APP="$DIST/Animara.app"
rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN/animara" "$BIN/animara-cli" "$APP/Contents/MacOS/"
sed "s/__VERSION__/$VERSION/g" "$ROOT/packaging/macos/Info.plist" > "$APP/Contents/Info.plist"
# Icon: render the SVG to PNGs and build an .icns (requires rsvg-convert or qlmanage fallback).
ICONSET="$(mktemp -d)/Animara.iconset"; mkdir -p "$ICONSET"
for s in 16 32 64 128 256 512; do
  if command -v rsvg-convert >/dev/null; then rsvg-convert -w $s -h $s "$ROOT/assets/animara.svg" -o "$ICONSET/icon_${s}x${s}.png"; fi
done
command -v iconutil >/dev/null && ls "$ICONSET"/*.png >/dev/null 2>&1 && iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Animara.icns" || true
if [[ -n "${MACOS_SIGN_IDENTITY:-}" ]]; then
  codesign --force --deep --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$APP"
fi
DMG="$DIST/Animara-$VERSION-macos-$ARCH.dmg"
hdiutil create -volname Animara -srcfolder "$APP" -ov -format UDZO "$DMG"
PKG="$DIST/Animara-$VERSION-macos-$ARCH.pkg"
pkgbuild --component "$APP" --install-location /Applications --identifier io.github.animara.Animara --version "$VERSION" \
  ${MACOS_INSTALLER_IDENTITY:+--sign "$MACOS_INSTALLER_IDENTITY"} "$PKG"
tar -C "$DIST" -czf "$DIST/Animara-$VERSION-macos-$ARCH.tar.gz" Animara.app
if [[ -n "${APPLE_ID:-}" && -n "${MACOS_SIGN_IDENTITY:-}" ]]; then
  for f in "$DMG" "$PKG"; do
    xcrun notarytool submit "$f" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
    xcrun stapler staple "$f"
  done
else
  echo "Signing credentials not provided: artifacts are unsigned (Gatekeeper will require right-click ▸ Open)."
fi
(cd "$DIST" && shasum -a 256 Animara-* > SHA256SUMS-macos.txt)
