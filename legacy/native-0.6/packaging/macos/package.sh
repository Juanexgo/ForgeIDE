#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 PATH_TO_FORGEIDE_BINARY OUTPUT_ZIP" >&2
    exit 2
fi

config_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
staging=$(mktemp -d)
trap 'rm -rf -- "$staging"' EXIT HUP INT TERM

bundle="$staging/ForgeIDE.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
install -m 755 "$1" "$bundle/Contents/MacOS/forgeide"
install -m 644 "$config_dir/Info.plist" "$bundle/Contents/Info.plist"
install -m 644 "$config_dir/ForgeIDE.icns" "$bundle/Contents/Resources/ForgeIDE.icns"
assets="$config_dir/../../crates/forge-desktop/assets"
install -m 644 "$assets/Inter-OFL.txt" "$assets/IBMPlexMono-OFL.txt" "$bundle/Contents/Resources/"

plutil -lint "$bundle/Contents/Info.plist"
codesign --force --sign - "$bundle"
codesign --verify --strict "$bundle"
ditto -c -k --norsrc --keepParent "$bundle" "$2"
