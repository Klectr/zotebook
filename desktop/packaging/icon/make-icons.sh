#!/usr/bin/env bash
# Rebuilds Zotebook's app icons from render.swift. Needs macOS (swiftc, iconutil).
#
#   packaging/macos/zotebook.iconset/  every macOS size, on Apple's icon grid
#   packaging/macos/zotebook-1024.png  a preview of the macOS icon
#   packaging/zotebook-mac-512.png     the Dock icon the app sets on macOS
#   packaging/zotebook-512.png         the window icon on Windows, full bleed
#   packaging/zotebook.ico             the Windows icon, 16 to 256 px, full bleed
#
# Every size is rendered natively, so small sizes get the simplified mark.
# zotebook.svg and zotebook-16.svg, for Linux, are drawn by hand to match.
set -euo pipefail
cd "$(dirname "$0")"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
swiftc -O -o "$work/render" render.swift

set_dir=../macos/zotebook.iconset
rm -rf "$set_dir" && mkdir -p "$set_dir"
# name:pixels — the sizes an .icns needs (each point size at 1x and 2x).
for spec in 16x16:16 16x16@2x:32 32x32:32 32x32@2x:64 128x128:128 128x128@2x:256 \
            256x256:256 256x256@2x:512 512x512:512 512x512@2x:1024; do
  "$work/render" "$set_dir/icon_${spec%%:*}.png" "${spec##*:}" >/dev/null
done
"$work/render" ../macos/zotebook-1024.png 1024 >/dev/null
"$work/render" ../zotebook-mac-512.png 512 >/dev/null
"$work/render" ../zotebook-512.png 512 full >/dev/null

# An .ico holds PNG images directly (Windows Vista and later read them).
sizes=(16 24 32 48 64 128 256)
for s in "${sizes[@]}"; do "$work/render" "$work/ico-$s.png" "$s" full >/dev/null; done
python3 - "$work" ../zotebook.ico "${sizes[@]}" <<'EOF'
import struct, sys
work, out, sizes = sys.argv[1], sys.argv[2], [int(s) for s in sys.argv[3:]]
pngs = [open(f"{work}/ico-{s}.png", "rb").read() for s in sizes]
head = struct.pack("<HHH", 0, 1, len(sizes))
offset = 6 + 16 * len(sizes)
entries = b""
for s, data in zip(sizes, pngs):
    side = 0 if s >= 256 else s  # 0 means 256
    entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
open(out, "wb").write(head + entries + b"".join(pngs))
EOF
echo "wrote the macOS iconset, the 512 px PNGs and zotebook.ico"
