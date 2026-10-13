#!/bin/bash
set -e

# Script to build and package Zero Editor as a native macOS .app bundle
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$DIR"

echo "Building release binary..."
source "$HOME/.cargo/env" 2>/dev/null || true
cargo build --release

APP_NAME="Zero Editor"
BUNDLE_DIR="$DIR/target/$APP_NAME.app"
MACOS_DIR="$BUNDLE_DIR/Contents/MacOS"
RESOURCES_DIR="$BUNDLE_DIR/Contents/Resources"

echo "Creating bundle structure: $BUNDLE_DIR"
rm -rf "$BUNDLE_DIR"
mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

# Copy binary
cp "$DIR/target/release/zero-editor" "$MACOS_DIR/zero-editor"
chmod +x "$MACOS_DIR/zero-editor"

# Copy app icon
if [ -f "$DIR/assets/icon.icns" ]; then
    cp "$DIR/assets/icon.icns" "$RESOURCES_DIR/icon.icns"
fi

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

# Register with macOS LaunchServices
if [ -f "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister" ]; then
    /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$BUNDLE_DIR"
fi

echo "✓ $APP_NAME.app created successfully at: $BUNDLE_DIR"

if [ -d "/Applications/$APP_NAME.app" ]; then
    echo "Updating existing installation in /Applications/$APP_NAME.app..."
    rm -rf "/Applications/$APP_NAME.app"
    cp -R "$BUNDLE_DIR" "/Applications/"
    if [ -f "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister" ]; then
        /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "/Applications/$APP_NAME.app"
    fi
    echo "✓ /Applications/$APP_NAME.app updated successfully"
else
    echo "You can move it to /Applications with: cp -r \"$BUNDLE_DIR\" /Applications/"
fi
