#!/bin/sh
set -e

if [ "$(uname)" = "Darwin" ]; then
  # macOS: Create and install app bundle
  if ! command -v cargo-bundle &> /dev/null; then
    echo "Installing cargo-bundle..."
    cargo install cargo-bundle
  fi
  
  cargo bundle --release
  
  APP_BUNDLE="target/release/bundle/osx/Graphtropy.app"
  if [ -d "$APP_BUNDLE" ]; then
    cp -r "$APP_BUNDLE" "/Applications/"
    echo "Installed Graphtropy.app to /Applications"
  fi
else
  # Linux: Standard cargo install with desktop integration
  cargo install --path .
  
  PREFIX="${PREFIX:-$HOME/.local}"
  BINPATH="$HOME/.cargo/bin/graphtropy"
  ICONPATH="$PREFIX/share/icons/hicolor/256x256/apps/graphtropy.png"
  sed -e "s|@BINPATH@|$BINPATH|" -e "s|@ICONPATH@|$ICONPATH|" graphtropy.desktop > "$PREFIX/share/applications/graphtropy.desktop"
  chmod 644 "$PREFIX/share/applications/graphtropy.desktop"
  install -Dm644 images/icon.png "$ICONPATH"
  echo "Installed to $PREFIX"
fi
