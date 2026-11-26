#!/bin/sh
# Build and package CopyWheel in a compressed macOS disk image.
set -eu
export LC_ALL=C
CDPATH='' cd "$(dirname "$0")/.."

sh scripts/make-app.sh

app=CopyWheel.app
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")
architectures=$(lipo -archs "$app/Contents/MacOS/CopyWheel")
case "$architectures" in
    'arm64 x86_64'|'x86_64 arm64') arch=universal ;;
    arm64|x86_64) arch=$architectures ;;
    *)
        echo "Unexpected CopyWheel architectures: $architectures" >&2
        exit 1
        ;;
esac

staging_dir=$(mktemp -d)
trap 'rm -rf "$staging_dir"' EXIT
trap 'exit 1' HUP INT TERM
ditto "$app" "$staging_dir/$app"
ln -s /Applications "$staging_dir/Applications"

mkdir -p dist
dmg="CopyWheel-$version-$arch.dmg"
hdiutil create -ov -format UDZO -volname CopyWheel -srcfolder "$staging_dir" "dist/$dmg"
# Stable name for the "latest release" download link.
cp "dist/$dmg" dist/CopyWheel.dmg
(
    cd dist
    shasum -a 256 "$dmg" CopyWheel.dmg > SHA256SUMS
)
echo "Ready: $PWD/dist/$dmg"
