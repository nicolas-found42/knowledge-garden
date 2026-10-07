#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
TAURI_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
REPO_DIR=$(CDPATH= cd -- "$TAURI_DIR/.." && pwd)
SOURCE_DIR="$TAURI_DIR/vendor/whisper.cpp"
ASSET_DIR="$TAURI_DIR/resources/audio"
BUILD_DIR="$TAURI_DIR/target/whisper-static-metal"
MODEL_NAME=ggml-large-v3-turbo.bin
MODEL_SHA256=1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69
MODEL_URL=https://huggingface.co/ggerganov/whisper.cpp/resolve/98aa99a0a9db05ae2342309f5096248665f7cba3/ggml-large-v3-turbo.bin
SOURCE_REVISION=371b5a7561823ab2bb32142d2751e35e7534727b
BUILD_SIGNATURE="$SOURCE_REVISION|arm64|static|metal|embedded-metal|release"

git -C "$REPO_DIR" submodule update --init --recursive src-tauri/vendor/whisper.cpp
mkdir -p "$ASSET_DIR"
if ! git -C "$SOURCE_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1 || [ "$(git -C "$SOURCE_DIR" rev-parse HEAD)" != "$SOURCE_REVISION" ]; then
  echo "The whisper.cpp source submodule is missing or not pinned at $SOURCE_REVISION." >&2
  echo "Initialize submodules before building the macOS app." >&2
  exit 1
fi

MODEL_PATH="$ASSET_DIR/$MODEL_NAME"
if [ ! -f "$MODEL_PATH" ] || [ "$(shasum -a 256 "$MODEL_PATH" | awk '{print $1}')" != "$MODEL_SHA256" ]; then
  temporary="$MODEL_PATH.download"
  rm -f "$temporary"
  curl --fail --location --retry 3 "$MODEL_URL" --output "$temporary"
  actual=$(shasum -a 256 "$temporary" | awk '{print $1}')
  if [ "$actual" != "$MODEL_SHA256" ]; then
    rm -f "$temporary"
    echo "The downloaded Whisper model failed SHA-256 verification: $actual" >&2
    exit 1
  fi
  mv "$temporary" "$MODEL_PATH"
fi

if [ ! -x "$ASSET_DIR/whisper-cli" ] || [ "$(cat "$ASSET_DIR/whisper-cli.build-signature" 2>/dev/null || true)" != "$BUILD_SIGNATURE" ]; then
  rm -f "$ASSET_DIR/whisper-cli"
  cmake -S "$SOURCE_DIR" -B "$BUILD_DIR" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_ARCHITECTURES=arm64 \
    -DBUILD_SHARED_LIBS=OFF \
    -DGGML_METAL=ON \
    -DGGML_METAL_EMBED_LIBRARY=ON \
    -DGGML_BLAS=OFF \
    -DWHISPER_BUILD_TESTS=OFF \
    -DWHISPER_BUILD_EXAMPLES=ON \
    -DWHISPER_BUILD_SERVER=OFF
  cmake --build "$BUILD_DIR" --target whisper-cli --config Release --parallel 6
  install -m 755 "$BUILD_DIR/bin/whisper-cli" "$ASSET_DIR/whisper-cli"
  printf '%s\n' "$BUILD_SIGNATURE" > "$ASSET_DIR/whisper-cli.build-signature"
fi
install -m 644 "$SOURCE_DIR/LICENSE" "$ASSET_DIR/WHISPER-CPP-LICENSE.txt"

if otool -L "$ASSET_DIR/whisper-cli" | awk 'NR > 1 { print $1 }' | grep -Ev '^(/usr/lib/|/System/Library/Frameworks/)' >/dev/null; then
  echo "The bundled Whisper processor has a non-system dynamic library dependency." >&2
  exit 1
fi

cat > "$ASSET_DIR/ASSET-MANIFEST.txt" <<EOF
Whisper ASR bundle
Source project: https://github.com/ggml-org/whisper.cpp
Source revision: $SOURCE_REVISION (v1.9.3)
Source license: MIT (upstream whisper.cpp repository)
Model project: https://huggingface.co/ggerganov/whisper.cpp
Model revision: 98aa99a0a9db05ae2342309f5096248665f7cba3
Model file: $MODEL_NAME
Model SHA-256: $MODEL_SHA256
Model license: MIT (upstream model repository)
Processor build: static arm64, Metal enabled with embedded Metal library
Inference downloads: none
EOF
