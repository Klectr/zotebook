#!/bin/sh
# Installs Zotebook for the current user: the binary, its icon and its launcher entry.
set -e
cd "$(dirname "$0")"
cargo build --release
install -Dm755 target/release/zotebook "$HOME/.local/bin/zotebook"
install -Dm644 packaging/zotebook.svg "$HOME/.local/share/icons/hicolor/scalable/apps/zotebook.svg"
# Below 24 px the icon drops its back sheet, so small sizes get their own drawing.
install -Dm644 packaging/zotebook-16.svg "$HOME/.local/share/icons/hicolor/16x16/apps/zotebook.svg"
for size in 32 48 64 128 256 512; do
  magick -background none -density 600 packaging/zotebook.svg -resize ${size}x${size} "$HOME/.local/share/icons/hicolor/${size}x${size}/apps/zotebook.png" 2>/dev/null \
    || install -Dm644 packaging/zotebook.svg "$HOME/.local/share/icons/hicolor/${size}x${size}/apps/zotebook.svg"
done
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" || true
install -Dm644 packaging/zotebook.desktop "$HOME/.local/share/applications/zotebook.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
echo "Installed. Run zotebook, or open Zotebook from the app launcher."
