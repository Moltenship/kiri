#!/bin/sh
# Builds Kiri.app, signs and notarizes it, and with --publish uploads it as a GitHub release.
# Needs a Developer ID identity in the keychain and an App Store Connect API key in
# ASC_KEY_ID, ASC_ISSUER_ID and ASC_KEY_PATH. See docs/operations/release.md.
set -eu
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
IDENTITY=${KIRI_IDENTITY:-"Developer ID Application: Timofei Maksimov (Q9SDRFKBC6)"}
APP=dist/Kiri.app
ZIP=dist/Kiri.zip
: "${ASC_KEY_ID:?}" "${ASC_ISSUER_ID:?}" "${ASC_KEY_PATH:?}"

if [ "${1:-}" = "--publish" ]; then
    git diff --quiet HEAD || { echo "Commit first: the release is tagged at HEAD." >&2; exit 1; }
    NOTES=$(awk -v v="$VERSION" '$0 ~ "^## \\[?" v "[] ]" {on=1; next} /^## / {on=0} on' CHANGELOG.md)
    [ -n "$NOTES" ] || { echo "CHANGELOG.md has no section for $VERSION." >&2; exit 1; }
fi

cargo build --release
rm -rf "$APP" "$ZIP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/kiri "$APP/Contents/MacOS/kiri"
cp macos/Kiri.icns "$APP/Contents/Resources/Kiri.icns"
sed -e "s/@VERSION@/$VERSION/" -e "s/@BUILD@/${VERSION%%-*}/" macos/Info.plist > "$APP/Contents/Info.plist"

codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
ditto -c -k --keepParent "$APP" "$ZIP"
xcrun notarytool submit "$ZIP" --key "$ASC_KEY_PATH" --key-id "$ASC_KEY_ID" --issuer "$ASC_ISSUER_ID" --wait
xcrun stapler staple "$APP"
# The stapled ticket lets Gatekeeper pass offline, so the zip is rebuilt with it.
rm "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"
spctl --assess --type execute "$APP"
echo "Built $ZIP ($VERSION)"

[ "${1:-}" = "--publish" ] || exit 0
case $VERSION in *-*) PRE=--prerelease ;; *) PRE= ;; esac
printf '%s\n' "$NOTES" | gh release create "v$VERSION" "$ZIP" --target "$(git rev-parse HEAD)" --title "Kiri $VERSION" --notes-file - $PRE
