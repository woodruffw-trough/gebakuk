#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

bindgen_version="0.2.129"
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "Install the matching bindings generator:" >&2
  echo "  cargo install --locked wasm-bindgen-cli --version $bindgen_version" >&2
  exit 1
fi
if [[ "$(wasm-bindgen --version)" != "wasm-bindgen $bindgen_version" ]]; then
  echo "wasm-bindgen-cli must be exactly $bindgen_version to match Cargo.lock." >&2
  exit 1
fi

validator_version="1.245.1"
if ! command -v wasm-tools >/dev/null 2>&1; then
  echo "Install the matching WASM validator:" >&2
  echo "  cargo install --locked wasm-tools --version $validator_version --no-default-features --features validate" >&2
  exit 1
fi
if [[ "$(wasm-tools --version)" != "wasm-tools $validator_version" ]]; then
  echo "wasm-tools must be exactly $validator_version." >&2
  exit 1
fi

cargo build --locked -p presenter-web --release --target wasm32-unknown-unknown
rm -rf -- "$project_root/dist"
mkdir -p dist/pkg
cp -R web/. dist/
wasm-bindgen \
  --target web \
  --remove-name-section \
  --remove-producers-section \
  --out-dir dist/pkg \
  --out-name presenter_web \
  target/wasm32-unknown-unknown/release/presenter_web.wasm

wasm_path="dist/pkg/presenter_web_bg.wasm"
wasm-tools validate "$wasm_path"
# Check the packaged module, not just compiler flags or feature metadata.
if scalar_validation="$(wasm-tools validate --features=-simd,-relaxed-simd "$wasm_path" 2>&1)"; then
  echo "The packaged WASM contains no required SIMD instructions." >&2
  exit 1
fi
if [[ "$scalar_validation" != *"SIMD support is not enabled"* ]]; then
  echo "Unexpected SIMD validation failure: $scalar_validation" >&2
  exit 1
fi

touch dist/.nojekyll
echo "Validated SIMD128 application built in $project_root/dist"
