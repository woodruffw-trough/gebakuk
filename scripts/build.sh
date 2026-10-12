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

cargo build --locked -p presenter-web --release --target wasm32-unknown-unknown
rm -rf -- "$project_root/dist"
mkdir -p dist/pkg
cp -R web/. dist/
wasm-bindgen \
  --target web \
  --out-dir dist/pkg \
  --out-name presenter_web \
  target/wasm32-unknown-unknown/release/presenter_web.wasm
touch dist/.nojekyll
if [[ -f fixtures/notes-right.pdf ]]; then
  cp fixtures/notes-right.pdf dist/sample.pdf
fi
echo "Static application built in $project_root/dist"
