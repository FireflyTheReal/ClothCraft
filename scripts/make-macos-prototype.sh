#!/bin/sh
# Build a double-clickable macOS prototype bundle for the ClothCraft workspace.
#
# This is a hand-built prototype, not packaging: no notarization, no installer,
# no update path, and no Windows equivalent. It exists so a person can open the
# atelier and judge the workspace without a terminal.
#
# The bundle launches `cloth-fit --view`, the interactive authoring atelier
# (draw / sew / arrange / simulate). The older single-panel FEM reference editor
# is bundled alongside as `marvelous-cloth` if you want to compare.
#
# Usage:  scripts/make-macos-prototype.sh [output-directory]
# Default output directory: prototype/mac
#
# Environment:
#   CARGO_HOME   honoured; set it if your cargo cache lives outside $HOME
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT_DIR=${1:-"$ROOT/prototype/mac"}
APP_NAME="ClothCraft"
APP="$OUT_DIR/$APP_NAME.app"
ATELIER_BIN="cloth-fit"
EDITOR_BIN="marvelous-cloth"
ICON_SRC="$ROOT/assets/branding/clothcraft-icon.png"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "This script builds a macOS bundle and only runs on Darwin." >&2
  exit 1
fi

echo "==> Building the desktop feature"
cd "$ROOT"
cargo build --locked --features desktop -j 2 --bin "$ATELIER_BIN" --bin "$EDITOR_BIN"

for name in "$ATELIER_BIN" "$EDITOR_BIN"; do
  if [ ! -x "$ROOT/target/debug/$name" ]; then
    echo "Expected binary not found: $ROOT/target/debug/$name" >&2
    exit 1
  fi
done

echo "==> Assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$ROOT/target/debug/$ATELIER_BIN" "$APP/Contents/MacOS/$ATELIER_BIN"
cp "$ROOT/target/debug/$EDITOR_BIN" "$APP/Contents/MacOS/$EDITOR_BIN"
chmod +x "$APP/Contents/MacOS/$ATELIER_BIN" "$APP/Contents/MacOS/$EDITOR_BIN"

# The atelier resolves `assets/makehuman/mannequin.obj` relative to the working
# directory, so prefer the repository this bundle was built from and fall back to
# a writable scratch directory when the bundle has been moved away from it.
cat > "$APP/Contents/MacOS/$APP_NAME" <<'LAUNCHER'
#!/bin/sh
DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH= cd -- "$DIR/../../../../.." 2>/dev/null && pwd || true)
if [ -n "${ROOT:-}" ] && [ -f "$ROOT/assets/makehuman/mannequin.obj" ]; then
  cd "$ROOT"
else
  WORK=${CLOTHCRAFT_WORKDIR:-$HOME/Documents/ClothCraft}
  mkdir -p "$WORK" 2>/dev/null || true
  cd "$WORK" 2>/dev/null || cd "$HOME" || true
fi
exec "$DIR/cloth-fit" --view "$@"
LAUNCHER
chmod +x "$APP/Contents/MacOS/$APP_NAME"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>ClothCraft</string>
  <key>CFBundleDisplayName</key><string>ClothCraft</string>
  <key>CFBundleIdentifier</key><string>org.clothcraft.prototype</string>
  <key>CFBundleExecutable</key><string>ClothCraft</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
PLIST

if [ -f "$ICON_SRC" ] && command -v sips >/dev/null 2>&1 && command -v iconutil >/dev/null 2>&1; then
  echo "==> Generating AppIcon.icns from assets/branding"
  ICONSET=$(mktemp -d)/AppIcon.iconset
  mkdir -p "$ICONSET"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
  done
  if iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns" 2>/dev/null; then
    echo "    icon embedded"
  else
    echo "    iconutil failed; bundle keeps the default icon" >&2
  fi
  rm -rf "$(dirname "$ICONSET")"
else
  echo "==> No icon source or no sips/iconutil; skipping icon"
fi

if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$APP" >/dev/null 2>&1 &&
    echo "==> Ad-hoc signed" ||
    echo "==> Ad-hoc signing skipped (bundle still runs locally)" >&2
fi

echo
echo "Built: $APP   (launches cloth-fit --view)"
echo "Open it with:  open \"$APP\""
echo "Legacy editor:  \"$APP/Contents/MacOS/$EDITOR_BIN\""
