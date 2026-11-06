#!/bin/sh
# Renders icon.svg to AppIcon.icns, which is committed: neither a build nor the release workflow
# needs a browser. Run it after changing the SVG, on a Mac with Google Chrome, or set CHROME to
# a Chrome or Chromium binary (Playwright's headless shell works).
# usage: crates/tokprof-desk/app/icon.sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)
chrome=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
if [ ! -x "$chrome" ]; then
  chrome=$(find "$HOME/Library/Caches/ms-playwright" -name chrome-headless-shell -type f 2> /dev/null | head -1)
fi
[ -x "$chrome" ] || { echo "icon.sh: no Chrome found; set CHROME" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
{
  printf '<!doctype html><html><head><style>html,body{margin:0;background:transparent}</style></head><body>'
  cat "$here/icon.svg"
  printf '</body></html>'
} > "$work/icon.html"
"$chrome" --headless --hide-scrollbars --default-background-color=00000000 --window-size=1024,1024 \
  --screenshot="$work/icon.png" "file://$work/icon.html" > /dev/null 2>&1
set=$work/AppIcon.iconset
mkdir "$set"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$work/icon.png" --out "$set/icon_${size}x${size}.png" > /dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$work/icon.png" --out "$set/icon_${size}x${size}@2x.png" > /dev/null
done
iconutil -c icns "$set" -o "$here/AppIcon.icns"
cp "$work/icon.png" "$work/../tokprof-icon-1024.png"
echo "wrote $here/AppIcon.icns ($(wc -c < "$here/AppIcon.icns" | tr -d ' ') bytes); the 1024-point render is $work/../tokprof-icon-1024.png"
