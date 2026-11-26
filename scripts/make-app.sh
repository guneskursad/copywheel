#!/bin/sh
# Build the CopyWheel macOS app bundle.
set -eu
export LC_ALL=C
CDPATH='' cd "$(dirname "$0")/.."

if ! command -v cargo >/dev/null 2>&1; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust is required. Install it from https://rustup.rs." >&2
    exit 1
fi

export MACOSX_DEPLOYMENT_TARGET=11.0
metadata=$(cargo metadata --format-version 1 --no-deps --locked)
target_dir=$(printf '%s\n' "$metadata" | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
version=$(printf '%s\n' "$metadata" | python3 -c '
import json
import sys

packages = json.load(sys.stdin)["packages"]
print(next(package["version"] for package in packages if package["name"] == "copywheel"))
')
host=$(rustc -vV | sed -n 's/^host: //p')
arm_target=aarch64-apple-darwin
intel_target=x86_64-apple-darwin
installed_targets=
if command -v rustup >/dev/null 2>&1; then
    installed_targets=$(rustup target list --installed)
fi

set -- "$host"
if printf '%s\n' "$installed_targets" | grep -qx "$arm_target" &&
   printf '%s\n' "$installed_targets" | grep -qx "$intel_target"; then
    set -- "$arm_target" "$intel_target"
fi

for target do
    cargo build --release --locked --bin copywheel --target "$target"
done

# The example generates the PNGs for iconutil on the build host.
iconset="$target_dir/appicon/AppIcon.iconset"
icon="$target_dir/appicon/AppIcon.icns"
cargo run --release --locked --quiet --target "$host" --example app_icon -- "$iconset"
iconutil -c icns "$iconset" -o "$icon"

app=CopyWheel.app
executable="$app/Contents/MacOS/CopyWheel"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
if [ "$#" -eq 2 ]; then
    lipo -create "$target_dir/$1/release/copywheel" "$target_dir/$2/release/copywheel" -output "$executable"
else
    cp "$target_dir/$1/release/copywheel" "$executable"
fi
cp "$icon" "$app/Contents/Resources/AppIcon.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>CopyWheel</string>
    <key>CFBundleDisplayName</key><string>CopyWheel</string>
    <key>CFBundleIdentifier</key><string>local.clipwheel</string>
    <key>CFBundleExecutable</key><string>CopyWheel</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>Copyright © 2026 Kürşad Güneş. Licensed under AGPL-3.0.</string>
</dict>
</plist>
PLIST

codesign --force --deep --options runtime --sign - "$app"
codesign --verify --deep --strict "$app"
echo "Ready: $PWD/$app"
