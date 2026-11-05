#!/usr/bin/env bash
# Build Linux distributables from an existing release build.
#   packaging/linux/package.sh [tarball|appimage|deb|rpm|all]
# Requires: tar/sha256sum (tarball), appimagetool (AppImage), cargo-deb (deb), cargo-generate-rpm (rpm).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
BIN="$TARGET_DIR/${CARGO_BUILD_TARGET:+$CARGO_BUILD_TARGET/}release"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)"
ARCH="${ARCH:-$(uname -m)}"
DIST="$ROOT/dist"
mkdir -p "$DIST"
what="${1:-all}"

stage_tree() { # $1 = prefix dir
  install -Dm755 "$BIN/animara" "$1/usr/bin/animara"
  install -Dm755 "$BIN/animara-cli" "$1/usr/bin/animara-cli"
  install -Dm644 "$ROOT/packaging/linux/io.github.animara.Animara.desktop" "$1/usr/share/applications/io.github.animara.Animara.desktop"
  install -Dm644 "$ROOT/packaging/linux/io.github.animara.Animara.metainfo.xml" "$1/usr/share/metainfo/io.github.animara.Animara.metainfo.xml"
  install -Dm644 "$ROOT/packaging/linux/io.github.animara.Animara.mime.xml" "$1/usr/share/mime/packages/io.github.animara.Animara.xml"
  install -Dm644 "$ROOT/assets/animara.svg" "$1/usr/share/icons/hicolor/scalable/apps/io.github.animara.Animara.svg"
}

if [[ $what == tarball || $what == all ]]; then
  name="animara-$VERSION-linux-$ARCH"
  tmp="$(mktemp -d)"; mkdir -p "$tmp/$name"
  install -Dm755 "$BIN/animara" "$tmp/$name/animara"
  install -Dm755 "$BIN/animara-cli" "$tmp/$name/animara-cli"
  cp "$ROOT/README.md" "$ROOT/LICENSES.md" "$ROOT/CHANGELOG.md" "$tmp/$name/"
  cp "$ROOT/packaging/linux/io.github.animara.Animara.desktop" "$ROOT/assets/animara.svg" "$tmp/$name/"
  cat > "$tmp/$name/install.sh" <<'INST'
#!/usr/bin/env bash
# Per-user install (no root): binaries to ~/.local/bin, desktop entry and icon to ~/.local/share.
set -e; d="$(cd "$(dirname "$0")" && pwd)"
install -Dm755 "$d/animara" ~/.local/bin/animara
install -Dm755 "$d/animara-cli" ~/.local/bin/animara-cli
install -Dm644 "$d/io.github.animara.Animara.desktop" ~/.local/share/applications/io.github.animara.Animara.desktop
install -Dm644 "$d/animara.svg" ~/.local/share/icons/hicolor/scalable/apps/io.github.animara.Animara.svg
echo "Installed to ~/.local/bin (make sure it is on your PATH)."
INST
  chmod +x "$tmp/$name/install.sh"
  tar -C "$tmp" -czf "$DIST/$name.tar.gz" "$name"
  rm -rf "$tmp"
  echo "built $DIST/$name.tar.gz"
fi

if [[ $what == appimage || $what == all ]]; then
  if command -v appimagetool >/dev/null; then
    app="$(mktemp -d)/Animara.AppDir"
    stage_tree "$app"
    ln -s usr/share/applications/io.github.animara.Animara.desktop "$app/io.github.animara.Animara.desktop"
    cp "$ROOT/assets/animara.svg" "$app/io.github.animara.Animara.svg"
    printf '#!/bin/sh\nHERE="$(dirname "$(readlink -f "$0")")"\nexec "$HERE/usr/bin/animara" "$@"\n' > "$app/AppRun"; chmod +x "$app/AppRun"
    ARCH="$ARCH" appimagetool "$app" "$DIST/Animara-$VERSION-$ARCH.AppImage"
  else
    echo "appimagetool not found; skipping AppImage" >&2
  fi
fi

if [[ $what == deb || $what == all ]]; then
  if command -v cargo-deb >/dev/null; then
    (cd "$ROOT" && cargo deb -p animara-app --no-build -o "$DIST/")
  else
    echo "cargo-deb not found; skipping .deb" >&2
  fi
fi

if [[ $what == rpm || $what == all ]]; then
  if command -v cargo-generate-rpm >/dev/null; then
    (cd "$ROOT" && cargo generate-rpm -p crates/app -o "$DIST/")
  else
    echo "cargo-generate-rpm not found; skipping .rpm" >&2
  fi
fi

(cd "$DIST" && sha256sum * > SHA256SUMS.txt 2>/dev/null || true)
