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
            <string>Plain Text</string>
            <key>CFBundleTypeRole</key>
            <string>Editor</string>
            <key>LSItemContentTypes</key>
            <array>
                <string>public.plain-text</string>
                <string>public.text</string>
                <string>public.source-code</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
EOF

echo "✓ $APP_NAME.app created successfully at: $BUNDLE_DIR"
echo "You can move it to /Applications with: cp -r \"$BUNDLE_DIR\" /Applications/"
