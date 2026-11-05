#!/usr/bin/env bash
# Terminal installer for macOS:
#   curl -fsSL https://github.com/nerooo190/animara/releases/latest/download/install-macos.sh | bash
# Downloads the universal .tar.gz, verifies its SHA-256, installs Animara.app into ~/Applications
# and links animara / animara-cli into ~/.local/bin.
set -euo pipefail
REPO="${ANIMARA_REPO:-nerooo190/animara}"
VERSION="${ANIMARA_VERSION:-$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p')}"
NAME="Animara-$VERSION-macos-universal.tar.gz"
URL="https://github.com/$REPO/releases/download/v$VERSION"
tmp="$(mktemp -d)"
curl -fsSL "$URL/$NAME" -o "$tmp/$NAME"
curl -fsSL "$URL/SHA256SUMS.txt" -o "$tmp/SHA256SUMS.txt"
(cd "$tmp" && grep " $NAME\$" SHA256SUMS.txt | shasum -a 256 -c -)
mkdir -p ~/Applications ~/.local/bin
tar -xzf "$tmp/$NAME" -C ~/Applications
ln -sf ~/Applications/Animara.app/Contents/MacOS/animara ~/.local/bin/animara
ln -sf ~/Applications/Animara.app/Contents/MacOS/animara-cli ~/.local/bin/animara-cli
echo "Installed Animara $VERSION to ~/Applications (CLI in ~/.local/bin)."
