#!/bin/zsh
set -euo pipefail

ROOT="${0:A:h:h}"
DELIVERY_DIR="${AMARA_WEB_PACKAGE_DIR:-$ROOT/outputs/web-macos}"
DELIVERY_DIR="${DELIVERY_DIR:A}"
if [[ "$DELIVERY_DIR" != "$ROOT/outputs" && "$DELIVERY_DIR" != "$ROOT/outputs/"* \
    && "$DELIVERY_DIR" != "$ROOT/work" && "$DELIVERY_DIR" != "$ROOT/work/"* ]]; then
  print -u2 "AMARA_WEB_PACKAGE_DIR must be inside this project's outputs/ or work/ tree."
  exit 1
fi

APP_NAME="amara music visualiser"
EXECUTABLE_NAME="amaraWebShell"
APP_BUNDLE_ID="com.amara.musicvisualiser.web"
HELPER_BUNDLE_ID="com.amara.musicvisualiser.web.AudioCapture"
APP="$DELIVERY_DIR/$APP_NAME.app"
ARCHIVE="$DELIVERY_DIR/amara-music-visualiser-macOS-arm64.zip"
SIGNING_IDENTITY="${AMARA_WEB_CODESIGN_IDENTITY:-${AMARA_CODESIGN_IDENTITY:--}}"
REQUIRE_PERSISTENT_SIGNING="${AMARA_WEB_REQUIRE_PERSISTENT_SIGNING:-0}"

if [[ "$REQUIRE_PERSISTENT_SIGNING" != 0 && "$REQUIRE_PERSISTENT_SIGNING" != 1 ]]; then
  print -u2 "AMARA_WEB_REQUIRE_PERSISTENT_SIGNING must be 0 or 1."
  exit 1
fi
if [[ "$SIGNING_IDENTITY" == - ]]; then
  if [[ "$REQUIRE_PERSISTENT_SIGNING" == 1 ]]; then
    print -u2 "Persistent signing requires AMARA_WEB_CODESIGN_IDENTITY (or AMARA_CODESIGN_IDENTITY)."
    print -u2 "No app was built or replaced. This script never creates or trusts certificates."
    exit 1
  fi
  print -u2 "WARNING: local ad-hoc build. Audio and microphone approvals may need granting again after a rebuild."
else
  VALID_IDENTITIES="$(/usr/bin/security find-identity -v -p codesigning)"
  if [[ "$VALID_IDENTITIES" != *"$SIGNING_IDENTITY"* ]]; then
    print -u2 "The configured signing identity is not a valid installed code-signing identity."
    print -u2 "Inspect with: security find-identity -v -p codesigning. No app was built or replaced."
    exit 1
  fi
fi

SHELL_SOURCE="$ROOT/tools/amaraWebShell/main.swift"
ORIGIN_SOURCE="$ROOT/tools/amaraWebShell/StableOrigin.swift"
AUDIO_SOURCE="$ROOT/tools/amaraAudioCapture/main.swift"
INFO_PLIST="$ROOT/packaging/WebInfo.plist"
ICON_SOURCE="$ROOT/packaging/WebAppIcon.icns"
RUNTIME_WEB=(
  index.html
  style.css
  audio-intent.js
  audio.js
  choreography.js
  dot-pass.js
  equaliser-meter.js
  geometry-pass.js
  journeys.js
  look-names.js
  main.js
  music-insight.js
  profile-adapter.js
  profiles.js
  recipes.js
  system-audio-worklet.js
  variation-mode.js
  world-transition.js
)
RUNTIME_SHADERS=(
  params.wgsl
  shader.wgsl
  raster.wgsl
  raster/attractor.wgsl
  raster/constellation.wgsl
  raster/disco_silk.wgsl
  raster/dot_equaliser.wgsl
  raster/dot_morph.wgsl
  worlds/aquarelle_world.wgsl
  worlds/aurora.wgsl
  worlds/chromatic_space.wgsl
  worlds/feedback.wgsl
  worlds/flight.wgsl
  worlds/liquid_stained_glass.wgsl
  worlds/mandelbrot.wgsl
  worlds/mathematics.wgsl
  worlds/mocha_world.wgsl
  worlds/neon_coast.wgsl
  worlds/neon_memory.wgsl
  worlds/tide.wgsl
  worlds/water.wgsl
)

for required in "$SHELL_SOURCE" "$ORIGIN_SOURCE" "$AUDIO_SOURCE" "$INFO_PLIST" "$ICON_SOURCE" \
    "$ROOT/LICENSE" "$ROOT/packaging/NOTICES.txt" "$ROOT/web/assets/amara-logo.svg"; do
  if [[ ! -f "$required" ]]; then
    print -u2 "Required packaging input is missing: $required"
    exit 1
  fi
done
for relative in $RUNTIME_WEB; do
  if [[ ! -f "$ROOT/web/$relative" ]]; then
    print -u2 "Required browser runtime file is missing: web/$relative"
    exit 1
  fi
done
for relative in $RUNTIME_SHADERS; do
  if [[ ! -f "$ROOT/src/$relative" ]]; then
    print -u2 "Required shader is missing: src/$relative"
    exit 1
  fi
done
/usr/bin/plutil -lint "$INFO_PLIST" >/dev/null

mkdir -p "$DELIVERY_DIR" "$ROOT/work"
STAGING="$(mktemp -d "$ROOT/work/web-macos-bundle.XXXXXX")"
STAGED_APP="$STAGING/$APP_NAME.app"
CONTENTS="$STAGED_APP/Contents"
RESOURCES="$CONTENTS/Resources"
RUNTIME="$RESOURCES/runtime"
STAGED_ARCHIVE="$STAGING/${ARCHIVE:t}"
trap 'rm -rf -- "$STAGING"' EXIT

mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Helpers" "$RUNTIME/web/assets" \
  "$RUNTIME/src/raster" "$RUNTIME/src/worlds" "$STAGING/swift-module-cache"

/usr/bin/xcrun swiftc -parse-as-library \
  -target arm64-apple-macosx13.0 \
  -module-cache-path "$STAGING/swift-module-cache" \
  "$SHELL_SOURCE" "$ORIGIN_SOURCE" -O \
  -framework AppKit -framework WebKit -framework Network -framework Foundation \
  -o "$CONTENTS/MacOS/$EXECUTABLE_NAME"

# Embed a helper-specific plist so both the binary metadata and its signature
# remain distinct from the established native app and helper identities.
HELPER_PLIST="$STAGING/AudioCapture-Info.plist"
cp "$INFO_PLIST" "$HELPER_PLIST"
/usr/bin/plutil -replace CFBundleDisplayName -string "amara Web Audio Capture" "$HELPER_PLIST"
/usr/bin/plutil -replace CFBundleExecutable -string "amaraAudioCapture" "$HELPER_PLIST"
/usr/bin/plutil -replace CFBundleIdentifier -string "$HELPER_BUNDLE_ID" "$HELPER_PLIST"
/usr/bin/plutil -replace CFBundleName -string "amaraAudioCapture" "$HELPER_PLIST"
/usr/bin/plutil -replace CFBundlePackageType -string "BNDL" "$HELPER_PLIST"
/usr/bin/plutil -lint "$HELPER_PLIST" >/dev/null
/usr/bin/xcrun swiftc -parse-as-library \
  -target arm64-apple-macosx13.0 \
  -module-cache-path "$STAGING/swift-module-cache" \
  "$AUDIO_SOURCE" \
  -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist -Xlinker "$HELPER_PLIST" \
  -O -framework Foundation -framework ScreenCaptureKit -framework CoreMedia \
  -framework CoreAudio -framework CoreGraphics \
  -o "$CONTENTS/Helpers/amaraAudioCapture"

for relative in $RUNTIME_WEB; do
  cp "$ROOT/web/$relative" "$RUNTIME/web/$relative"
done
for relative in $RUNTIME_SHADERS; do
  cp "$ROOT/src/$relative" "$RUNTIME/src/$relative"
done
cp "$ROOT/web/assets/amara-logo.svg" "$RUNTIME/web/assets/amara-logo.svg"
cp "$ICON_SOURCE" "$RESOURCES/WebAppIcon.icns"
cp "$INFO_PLIST" "$CONTENTS/Info.plist"
cp "$ROOT/LICENSE" "$RESOURCES/LICENSE.txt"
cp "$ROOT/packaging/NOTICES.txt" "$RESOURCES/NOTICES.txt"
chmod 755 "$CONTENTS/MacOS/$EXECUTABLE_NAME" "$CONTENTS/Helpers/amaraAudioCapture"

for binary in "$CONTENTS/MacOS/$EXECUTABLE_NAME" "$CONTENTS/Helpers/amaraAudioCapture"; do
  BINARY_TYPE="$(/usr/bin/file -b "$binary")"
  if [[ "$BINARY_TYPE" != *"Mach-O"*"executable"* || "$(/usr/bin/lipo -archs "$binary")" != arm64 ]]; then
    print -u2 "Expected an arm64 Mach-O executable, got: $BINARY_TYPE"
    exit 1
  fi
done

# Sign nested code first. Resources contain data only, so no --deep signing is
# used to manufacture or conceal an invalid helper signature.
/usr/bin/codesign --force --sign "$SIGNING_IDENTITY" \
  --identifier "$HELPER_BUNDLE_ID" "$CONTENTS/Helpers/amaraAudioCapture"
/usr/bin/codesign --verify --strict "$CONTENTS/Helpers/amaraAudioCapture"
/usr/bin/codesign --force --sign "$SIGNING_IDENTITY" \
  --identifier "$APP_BUNDLE_ID" "$STAGED_APP"
/usr/bin/codesign --verify --deep --strict "$STAGED_APP"
if [[ "$SIGNING_IDENTITY" != - ]]; then
  /usr/bin/codesign --verify --strict -R 'anchor apple generic' "$CONTENTS/Helpers/amaraAudioCapture"
  /usr/bin/codesign --verify --strict -R 'anchor apple generic' "$STAGED_APP"
fi

/usr/bin/plutil -lint "$CONTENTS/Info.plist" >/dev/null
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$CONTENTS/Info.plist")" == "$APP_BUNDLE_ID" ]]
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$CONTENTS/Info.plist")" == "$EXECUTABLE_NAME" ]]
[[ "$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$CONTENTS/Info.plist")" == 13.0 ]]
[[ "$(/usr/bin/codesign -d --verbose=4 "$CONTENTS/Helpers/amaraAudioCapture" 2>&1 | sed -n 's/^Identifier=//p')" == "$HELPER_BUNDLE_ID" ]]
[[ "$(/usr/bin/codesign -d --verbose=4 "$STAGED_APP" 2>&1 | sed -n 's/^Identifier=//p')" == "$APP_BUNDLE_ID" ]]

COPYFILE_DISABLE=1 /usr/bin/ditto --norsrc --noextattr -c -k --keepParent \
  "$STAGED_APP" "$STAGED_ARCHIVE"
/usr/bin/unzip -tq "$STAGED_ARCHIVE" >/dev/null

# Publish only generated outputs in the caller-selected project delivery tree.
# No installed or running app is located, stopped, opened or replaced.
if [[ -e "$APP" ]]; then
  if [[ ! -d "$APP" || -L "$APP" ]]; then
    print -u2 "Refusing to replace a non-directory or symbolic-link output: $APP"
    exit 1
  fi
  mv "$APP" "$STAGING/previous.app"
fi
mv "$STAGED_APP" "$APP"
mv -f "$STAGED_ARCHIVE" "$ARCHIVE"

/usr/bin/codesign --verify --deep --strict "$APP"
/usr/bin/unzip -tq "$ARCHIVE" >/dev/null
echo "Built: $APP"
echo "Archive: $ARCHIVE"
