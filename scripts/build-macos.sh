#!/bin/zsh
set -euo pipefail

ROOT="${0:A:h:h}"
DELIVERY_DIR="${AMARA_PACKAGE_DIR:-$ROOT/outputs}"
DELIVERY_DIR="${DELIVERY_DIR:A}"
# Stage a candidate without replacing the currently running, approved app.
# All generated replacements/archives stay in this project's delivery/work tree.
if [[ "$DELIVERY_DIR" != "$ROOT/outputs" && "$DELIVERY_DIR" != "$ROOT/outputs/"* && "$DELIVERY_DIR" != "$ROOT/work/"* ]]; then
  print -u2 "AMARA_PACKAGE_DIR must be inside this project's outputs/ or work/ tree."
  exit 1
fi
APP="$DELIVERY_DIR/amaraMusicVisualiser.app"
SIGNING_IDENTITY="${AMARA_CODESIGN_IDENTITY:--}"
REQUIRE_PERSISTENT_SIGNING="${AMARA_REQUIRE_PERSISTENT_SIGNING:-0}"

# Set AMARA_CODESIGN_IDENTITY to an existing Apple Development / Developer ID
# Application identity (its full name or SHA-1 from security find-identity).
# Never invent a designated requirement or change certificate/TCC trust here.
if [[ "$REQUIRE_PERSISTENT_SIGNING" != 0 && "$REQUIRE_PERSISTENT_SIGNING" != 1 ]]; then
  print -u2 "AMARA_REQUIRE_PERSISTENT_SIGNING must be 0 or 1."
  exit 1
fi
if [[ "$SIGNING_IDENTITY" == - ]]; then
  if [[ "$REQUIRE_PERSISTENT_SIGNING" == 1 ]]; then
    print -u2 "Persistent signing requires AMARA_CODESIGN_IDENTITY for an existing Apple signing identity."
    print -u2 "No app was built or replaced. This script never creates/trusts certificates or edits privacy settings."
    exit 1
  fi
  print -u2 "WARNING: local ad-hoc build. Audio/microphone approvals may need granting again after a rebuild."
  print -u2 "For update-stable signing, configure an existing Apple signing identity via AMARA_CODESIGN_IDENTITY."
else
  VALID_IDENTITIES="$(security find-identity -v -p codesigning)"
  if [[ "$VALID_IDENTITIES" != *"$SIGNING_IDENTITY"* ]]; then
    print -u2 "The configured signing identity is not a valid installed code-signing identity."
    print -u2 "Inspect with: security find-identity -v -p codesigning. No app was built or replaced."
    exit 1
  fi
fi

cd "$ROOT"
cargo build --locked --offline --release

# Rust's optional stripping step can warn yet exit successfully after a disk
# exhaustion error. Reject a truncated/data artifact before signing or replacing
# any bundle; a valid code signature alone does not make it runnable.
MAIN_BINARY_TYPE="$(/usr/bin/file -b "$ROOT/target/release/amaraMusicVisualiser")"
if [[ "$MAIN_BINARY_TYPE" != *"Mach-O"*"executable"* ]]; then
  print -u2 "The release artifact is not a Mach-O executable: $MAIN_BINARY_TYPE"
  print -u2 "No app was replaced. Check free disk space, clean this crate's release artifacts, and rebuild."
  exit 1
fi

mkdir -p "$DELIVERY_DIR" "$ROOT/work"
STAGING="$(mktemp -d "$ROOT/work/macos-bundle.XXXXXX")"
STAGED_APP="$STAGING/amaraMusicVisualiser.app"
CONTENTS="$STAGED_APP/Contents"
trap 'rm -rf "$STAGING"' EXIT

mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources" "$CONTENTS/Helpers"
cp "$ROOT/target/release/amaraMusicVisualiser" "$CONTENTS/MacOS/amaraMusicVisualiser"
zsh "$ROOT/scripts/compile-audio-helper.sh" "$CONTENTS/Helpers/amaraAudioCapture"
if [[ -f "$ROOT/packaging/AppIcon.icns" ]]; then
  # Reuse the checked-in generated app icon. Headless/sandboxed build hosts
  # need not have an AppKit drawing session just to package a visual update.
  cp "$ROOT/packaging/AppIcon.icns" "$CONTENTS/Resources/AppIcon.icns"
else
  xcrun swift -module-cache-path "$ROOT/target/swift-module-cache" \
    "$ROOT/tools/make-icon.swift" "$STAGING/AppIcon.iconset"
  iconutil -c icns "$STAGING/AppIcon.iconset" -o "$CONTENTS/Resources/AppIcon.icns"
fi
cp "$ROOT/packaging/Info.plist" "$CONTENTS/Info.plist"
cp "$ROOT/assets/fonts/OFL.txt" "$CONTENTS/Resources/JetBrainsMono-OFL.txt"
chmod 755 "$CONTENTS/MacOS/amaraMusicVisualiser" "$CONTENTS/Helpers/amaraAudioCapture"
# Canonical nested-code location and inside-out signing: Resources is data,
# and --deep signing can silently leave a helper with its linker signature.
codesign --force --sign "$SIGNING_IDENTITY" \
  --identifier com.amara.musicvisualiser.AudioCapture "$CONTENTS/Helpers/amaraAudioCapture"
codesign --verify --strict "$CONTENTS/Helpers/amaraAudioCapture"
codesign --force --sign "$SIGNING_IDENTITY" "$STAGED_APP"
codesign --verify --deep --strict "$STAGED_APP"
if [[ "$SIGNING_IDENTITY" != - ]]; then
  # Verify an Apple-issued chain without overriding the signed code's DR.
  codesign --verify --strict -R 'anchor apple generic' "$CONTENTS/Helpers/amaraAudioCapture"
  codesign --verify --strict -R 'anchor apple generic' "$STAGED_APP"
fi
plutil -lint "$CONTENTS/Info.plist"

# Replace only a completely built and verified generated bundle. Keep the
# previous one locally so a packaging failure never destroys a working build.
if [[ -e "$APP" ]]; then
  mv "$APP" "$STAGING/previous.app"
fi
mv "$STAGED_APP" "$APP"
if [[ -e "$STAGING/previous.app" ]]; then
  if [[ "$DELIVERY_DIR" == "$ROOT/outputs" ]]; then
    PREVIOUS="$ROOT/work/previous-amaraMusicVisualiser.app"
  else
    PREVIOUS="$ROOT/work/previous-${DELIVERY_DIR:t}-amaraMusicVisualiser.app"
  fi
  if [[ -e "$PREVIOUS" ]]; then
    rm -rf "$PREVIOUS"
  fi
  mv "$STAGING/previous.app" "$PREVIOUS"
fi
rm -f "$DELIVERY_DIR/amaraMusicVisualiser-macOS-arm64.zip"
COPYFILE_DISABLE=1 ditto --norsrc --noextattr -c -k --keepParent \
  "$APP" "$DELIVERY_DIR/amaraMusicVisualiser-macOS-arm64.zip"
cp "$ROOT/packaging/START-HERE.txt" "$DELIVERY_DIR/START-HERE.txt"
cp "$ROOT/presets/Velvet-Flight.json" "$DELIVERY_DIR/Velvet-Flight.json"
cp "$ROOT/presets/Liquid-Light.json" "$DELIVERY_DIR/Liquid-Light.json"
cp "$ROOT/presets/Fractal-Mocca.json" "$DELIVERY_DIR/Fractal-Mocca.json"
COPYFILE_DISABLE=1 zip -qj "$DELIVERY_DIR/amaraMusicVisualiser-macOS-arm64.zip" \
  "$ROOT/packaging/START-HERE.txt" \
  "$ROOT/presets/Velvet-Flight.json" "$ROOT/presets/Liquid-Light.json" \
  "$ROOT/presets/Fractal-Mocca.json"
echo "Built: $APP"
echo "Archive: $DELIVERY_DIR/amaraMusicVisualiser-macOS-arm64.zip"
