#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
# Native macOS build only; does not sign or publish.
cargo build --release --locked --target aarch64-apple-darwin
bundle="$PWD/artifacts/Rotor GPUI P0.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp target/aarch64-apple-darwin/release/rotor-gpui-probe "$bundle/Contents/MacOS/rotor-gpui-probe"
cp packaging/Info.plist "$bundle/Contents/Info.plist"
cp README.md "$bundle/Contents/Resources/P0-README.md"
/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist"
file "$bundle/Contents/MacOS/rotor-gpui-probe"
otool -l "$bundle/Contents/MacOS/rotor-gpui-probe" > artifacts/macho-load-commands.txt
tar -czf artifacts/Rotor-GPUI-P0-app.tar.gz -C artifacts 'Rotor GPUI P0.app'
printf '%s\n' "Unsigned test bundle: $bundle"
