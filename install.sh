#!/bin/sh
set -e

PREFIX="${PREFIX:-$HOME/.local}"

cargo install --path .

BINPATH="$HOME/.cargo/bin/graphtropy"
ICONPATH="$PREFIX/share/icons/hicolor/256x256/apps/graphtropy.png"
sed -e "s|@BINPATH@|$BINPATH|" -e "s|@ICONPATH@|$ICONPATH|" graphtropy.desktop > "$PREFIX/share/applications/graphtropy.desktop"
chmod 644 "$PREFIX/share/applications/graphtropy.desktop"
install -Dm644 images/icon.png "$PREFIX/share/icons/hicolor/256x256/apps/graphtropy.png"

echo "Installed to $PREFIX"
