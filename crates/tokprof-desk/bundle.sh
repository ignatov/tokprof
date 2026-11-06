#!/bin/sh
# Builds tokprof.app for a target, signs it ad hoc, checks it and zips it as
# dist/tokprof-desk-<target>.zip, here and in the release workflow alike.
# usage: crates/tokprof-desk/bundle.sh [<target>]   (default: this Mac's)
#        DIST sets the folder of the zip (default: dist in the current folder).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
target=${1:-$(rustc -vV | sed -n 's/^host: //p')}
dist=${DIST:-dist}
# The package's version: the first `version = "…"` line of the manifest is the package's own.
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$here/Cargo.toml" | head -1)
[ -n "$version" ] || { echo "bundle.sh: no version in $here/Cargo.toml" >&2; exit 1; }

cargo build --release --locked --target "$target" --manifest-path "$here/Cargo.toml"

out="$here/target/$target/bundle"
app="$out/tokprof.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$here/target/$target/release/tokprof-desk" "$app/Contents/MacOS/tokprof-desk"
sed "s/__VERSION__/$version/g" "$here/app/Info.plist" > "$app/Contents/Info.plist"
cp "$here/app/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
printf 'APPL????' > "$app/Contents/PkgInfo"

plutil -lint "$app/Contents/Info.plist"
# Ad hoc: the author has no Developer ID yet, so a downloaded app asks to be opened once.
codesign --force --sign - --timestamp=none "$app"
codesign --verify --strict --verbose=1 "$app"

mkdir -p "$dist"
zip="$dist/tokprof-desk-$target.zip"
rm -f "$zip"
# ditto keeps the bundle as Finder's own archives do.
ditto -c -k --keepParent "$app" "$zip"
echo "built $app ($version), zipped as $zip"
