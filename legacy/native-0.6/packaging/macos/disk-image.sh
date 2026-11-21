#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 PATH_TO_FORGEIDE_ZIP OUTPUT_DMG" >&2
    exit 2
fi

staging=$(mktemp -d)
trap 'rm -rf -- "$staging"' EXIT HUP INT TERM

ditto -x -k "$1" "$staging"
codesign --verify --strict "$staging/ForgeIDE.app"
ln -s /Applications "$staging/Applications"
hdiutil create -ov -fs HFS+ -format UDZO -volname ForgeIDE -srcfolder "$staging" "$2"
hdiutil verify "$2"
