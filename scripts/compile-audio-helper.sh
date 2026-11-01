#!/bin/zsh
set -euo pipefail
PROJECT_ROOT="${0:A:h:h}"
DESTINATION="${1:?Pass the output helper filename}"
mkdir -p "${DESTINATION:h}" "$PROJECT_ROOT/target/swift-module-cache"
xcrun swiftc -parse-as-library \
  -target arm64-apple-macosx13.0 \
  -module-cache-path "$PROJECT_ROOT/target/swift-module-cache" \
  "$PROJECT_ROOT/tools/amaraAudioCapture/main.swift" \
  -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist \
  -Xlinker "$PROJECT_ROOT/packaging/Info.plist" \
  -O -framework Foundation -framework ScreenCaptureKit \
  -framework CoreMedia -framework CoreAudio -framework CoreGraphics -o "$DESTINATION"
