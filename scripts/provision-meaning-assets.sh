#!/usr/bin/env bash
set -euo pipefail

# Provision deterministic, redistributable assets for macOS ARM64 builds.
# The application never downloads these assets. The resulting files are bundled
# as Tauri resources and loaded from the app's resource directory.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$ROOT/src-tauri/resources/meaning"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

MODEL_REVISION=a5beb1e3e68b9ab74eb54cfd186867f64f240e1a
ORT_VERSION=1.30.0
ORT_ARCHIVE_SHA256=6ebb5062a934537c352937821f9fe9718e7de1a2db1122a93dd363ffd53a7012

verify() {
  local file="$1" expected="$2" actual
  actual="$(shasum -a 256 "$file" | awk '{print $1}')"
  if [[ "$actual" != "$expected" ]]; then
    echo "SHA-256 mismatch for $file: expected $expected, got $actual" >&2
    return 1
  fi
}

fetch_model_file() {
  local path="$1" sha="$2" output="$3"
  local local_root="${KG_BGE_BASE_MODEL_DIR:-/Users/Nicolas/Documents/agent-runs/knowledge-garden-spec1-20261003/bge-base-model}"
  if [[ -f "$local_root/$path" ]] && verify "$local_root/$path" "$sha" 2>/dev/null; then
    cp "$local_root/$path" "$output"
  else
    curl --fail --location --retry 2 \
      "https://huggingface.co/BAAI/bge-base-en-v1.5/resolve/$MODEL_REVISION/$path" \
      --output "$output"
    verify "$output" "$sha"
  fi
}

mkdir -p "$TMP/meaning"
fetch_model_file onnx/model.onnx 9bc579acdba21c253c62a9bf866891355a63ffa3442b52c8a37d75b2ccb91848 "$TMP/meaning/model.onnx"
fetch_model_file tokenizer.json d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66 "$TMP/meaning/tokenizer.json"
fetch_model_file config.json bc00af31a4a31b74040d73370aa83b62da34c90b75eb77bfa7db039d90abd591 "$TMP/meaning/config.json"
fetch_model_file tokenizer_config.json 9261e7d79b44c8195c1cada2b453e55b00aeb81e907a6664974b4d7776172ab3 "$TMP/meaning/tokenizer_config.json"
fetch_model_file vocab.txt 07eced375cec144d27c900241f3e339478dec958f92fddbc551f295c992038a3 "$TMP/meaning/vocab.txt"

curl --fail --location --retry 2 \
  "https://github.com/microsoft/onnxruntime/releases/download/v$ORT_VERSION/onnxruntime-osx-arm64-$ORT_VERSION.tgz" \
  --output "$TMP/onnxruntime.tgz"
verify "$TMP/onnxruntime.tgz" "$ORT_ARCHIVE_SHA256"
tar -xzf "$TMP/onnxruntime.tgz" -C "$TMP"
cp "$TMP/onnxruntime-osx-arm64-$ORT_VERSION/lib/libonnxruntime.$ORT_VERSION.dylib" "$TMP/meaning/libonnxruntime.$ORT_VERSION.dylib"
cp "$TMP/onnxruntime-osx-arm64-$ORT_VERSION/LICENSE" "$TMP/meaning/ONNXRUNTIME-LICENSE.txt"
cp "$TMP/onnxruntime-osx-arm64-$ORT_VERSION/ThirdPartyNotices.txt" "$TMP/meaning/ONNXRUNTIME-THIRD-PARTY-NOTICES.txt"

if ! lipo -archs "$TMP/meaning/libonnxruntime.$ORT_VERSION.dylib" | tr ' ' '\n' | grep -qx arm64; then
  echo "ONNX Runtime library is not an Apple Silicon arm64 binary." >&2
  exit 1
fi
if ! otool -L "$TMP/meaning/libonnxruntime.$ORT_VERSION.dylib" | tail -n +2 | grep -q '/usr/lib/libSystem'; then
  echo "Unexpected dynamic dependencies in ONNX Runtime library; inspect otool -L output." >&2
  otool -L "$TMP/meaning/libonnxruntime.$ORT_VERSION.dylib" >&2
  exit 1
fi

cat > "$TMP/meaning/MODEL-NOTICE.txt" <<EOF
BAAI/bge-base-en-v1.5, pinned revision $MODEL_REVISION
Publisher: Beijing Academy of Artificial Intelligence
License: MIT (as declared by the pinned upstream model card)
Upstream: https://huggingface.co/BAAI/bge-base-en-v1.5/tree/$MODEL_REVISION

The model weights, tokenizer, and configuration are redistributed under the
upstream model card's MIT license. The upstream repository does not contain a
separate LICENSE file; verify license terms at the pinned upstream URL.
EOF
cat > "$TMP/meaning/manifest.json" <<EOF
{
  "model": "BAAI/bge-base-en-v1.5",
  "model_revision": "$MODEL_REVISION",
  "runtime": "onnxruntime",
  "runtime_version": "$ORT_VERSION",
  "runtime_archive_sha256": "$ORT_ARCHIVE_SHA256",
  "embedding_dimensions": 768,
  "query_prefix": "Represent this sentence for searching relevant passages: ",
  "pooling": "normalized CLS",
  "language": "en",
  "index_format": 1
}
EOF

rm -rf "$DEST.new"
mkdir -p "$(dirname "$DEST")"
mv "$TMP/meaning" "$DEST.new"
rm -rf "$DEST"
mv "$DEST.new" "$DEST"
echo "Provisioned pinned BGE-base ONNX, tokenizer, and ONNX Runtime 1.30.0 into $DEST"
