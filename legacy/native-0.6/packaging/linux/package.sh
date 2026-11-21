#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 PATH_TO_FORGEIDE_BINARY OUTPUT_DEB" >&2
    exit 2
fi

config_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
staging=$(mktemp -d)
trap 'rm -rf -- "$staging"' EXIT HUP INT TERM
arch=$(dpkg --print-architecture)
case "$arch" in amd64|arm64) ;; *) echo "Unsupported Debian architecture: $arch" >&2; exit 1;; esac
mkdir -p "$staging/DEBIAN" "$staging/usr/bin" "$staging/usr/share/applications" "$staging/usr/share/doc/forgeide"
install -m 755 "$1" "$staging/usr/bin/forgeide"
install -m 644 "$config_dir/forgeide.desktop" "$staging/usr/share/applications/dev.forgeide.desktop"
assets="$config_dir/../../crates/forge-desktop/assets"
install -m 644 "$assets/Inter-OFL.txt" "$assets/IBMPlexMono-OFL.txt" "$staging/usr/share/doc/forgeide/"
cat > "$staging/DEBIAN/control" <<PKG
Package: forgeide
Version: 0.6.0
Section: devel
Priority: optional
Architecture: $arch
Maintainer: ForgeIDE
Depends: libc6 (>= 2.39), libgcc-s1, libegl1, libgl1, libwayland-client0, libxkbcommon0, libxkbcommon-x11-0, libx11-6, libxcursor1, libxi6, libxrandr2, libxfixes3
Recommends: git, xdg-desktop-portal
Description: ForgeIDE native development environment
 Local code editor with Git, terminal, language servers, and a debugger.
PKG
dpkg-deb --root-owner-group --build "$staging" "$2"
