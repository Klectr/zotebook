#!/bin/sh
# Installs Zotebook for the current user from this folder: the binary, its icon and its
# launcher entry. Run it from the folder you unpacked.
set -e
cd "$(dirname "$0")"
install -Dm755 zotebook "$HOME/.local/bin/zotebook"
install -Dm644 icons/zotebook.svg "$HOME/.local/share/icons/hicolor/scalable/apps/zotebook.svg"
install -Dm644 icons/zotebook-16.svg "$HOME/.local/share/icons/hicolor/16x16/apps/zotebook.svg"
install -Dm644 zotebook.desktop "$HOME/.local/share/applications/zotebook.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" || true
echo "Installed. Run zotebook, or open Zotebook from the app launcher."
echo "If zotebook is not found, add ~/.local/bin to your PATH."
