#!/bin/bash
set -euo pipefail

PROFILE=release
INSTALL=false
for arg in "$@"; do
    case "$arg" in
        --debug) PROFILE=debug ;;
        --install) INSTALL=true ;;
        *) echo "Usage: $0 [--debug] [--install]" >&2; exit 2 ;;
    esac
done

# Script to build and package Zero Editor as a native macOS .app bundle
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$DIR"

echo "Building $PROFILE binary..."
if ! command -v cargo >/dev/null 2>&1 && [ -r "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi
if [ "$PROFILE" = release ]; then cargo build --release --locked; else cargo build --locked; fi

APP_NAME="Zero Editor"
BUNDLE_DIR="$DIR/target/$APP_NAME.app"
MACOS_DIR="$BUNDLE_DIR/Contents/MacOS"
RESOURCES_DIR="$BUNDLE_DIR/Contents/Resources"

echo "Creating bundle structure: $BUNDLE_DIR"
rm -rf "$BUNDLE_DIR"
mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

# Copy binary
cp "$DIR/target/$PROFILE/zero-editor" "$MACOS_DIR/zero-editor"
chmod +x "$MACOS_DIR/zero-editor"

# Copy app icon
if [ -f "$DIR/assets/icon.icns" ]; then
    cp "$DIR/assets/icon.icns" "$RESOURCES_DIR/icon.icns"
fi

# Bundled font license (font data is embedded in the executable)
mkdir -p "$RESOURCES_DIR/licenses"
cp "$DIR/assets/fonts/OFL.txt" "$RESOURCES_DIR/licenses/JetBrainsMono-OFL.txt"
cp "$DIR/LICENSE" "$RESOURCES_DIR/licenses/ZeroEditor-MIT.txt"

# Create Info.plist
cat <<EOF > "$BUNDLE_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>zero-editor</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundleIdentifier</key>
    <string>dev.zero.editor</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleLocalizations</key>
    <array>
        <string>en</string>
    </array>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>Source Code and Scripts</string>
            <key>CFBundleTypeRole</key>
            <string>Editor</string>
            <key>LSHandlerRank</key>
            <string>Default</string>
            <key>LSItemContentTypes</key>
            <array>
                <string>public.plain-text</string>
                <string>public.text</string>
                <string>public.source-code</string>
                <string>public.script</string>
                <string>public.shell-script</string>
                <string>public.bash-script</string>
                <string>public.zsh-script</string>
                <string>public.c-source</string>
                <string>public.c-header</string>
                <string>public.c-plus-plus-source</string>
                <string>public.c-plus-plus-header</string>
                <string>public.objective-c-source</string>
                <string>public.python-script</string>
                <string>public.perl-script</string>
                <string>public.php-script</string>
                <string>public.ruby-script</string>
                <string>public.yaml</string>
                <string>public.json</string>
                <string>public.xml</string>
                <string>public.html</string>
                <string>public.css</string>
                <string>public.data</string>
                <string>public.content</string>
                <string>public.item</string>
            </array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key>
            <string>All Files</string>
            <key>CFBundleTypeRole</key>
            <string>Editor</string>
            <key>LSHandlerRank</key>
            <string>Alternate</string>
            <key>CFBundleTypeExtensions</key>
            <array>
                <string>*</string>
            </array>
            <key>CFBundleTypeOSTypes</key>
            <array>
                <string>****</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
EOF

# Sign the local bundle before registering it. Distribution signing/notarization
# can be added when publishing a release.
codesign --force --sign - "$BUNDLE_DIR"

echo "✓ $APP_NAME.app created successfully at: $BUNDLE_DIR"

if [ "$INSTALL" = true ]; then
    echo "Updating existing installation in /Applications/$APP_NAME.app..."
    ditto "$BUNDLE_DIR" "/Applications/$APP_NAME.app"
    if [ -f "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister" ]; then
        /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "/Applications/$APP_NAME.app"
    fi
    echo "✓ /Applications/$APP_NAME.app updated successfully"
else
    echo "You can move it to /Applications with: cp -r \"$BUNDLE_DIR\" /Applications/"
fi
