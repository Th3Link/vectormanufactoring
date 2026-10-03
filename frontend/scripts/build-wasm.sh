#!/usr/bin/env bash
# Builds vecmanf-editor-wasm for the browser and runs wasm-bindgen over it,
# producing the ES module frontend/src/wasm-bindings/ imports
# (specs/path-node-editing/adrs.md, ADR 0001 §3's wasm facade). Generated
# output, not committed — see frontend/.gitignore.
#
# Needs: a Rust toolchain with the wasm32-unknown-unknown target
# (`rustup target add wasm32-unknown-unknown`) and the `wasm-bindgen` CLI
# at the same version as the `wasm-bindgen` crate in Cargo.lock
# (`cargo install wasm-bindgen-cli --version <that version>`) — the two
# must match exactly or wasm-bindgen refuses to run.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
out_dir="${script_dir}/../src/wasm-bindings"

cargo build \
  --manifest-path "${repo_root}/Cargo.toml" \
  --target wasm32-unknown-unknown \
  --release \
  -p vecmanf-editor-wasm

wasm-bindgen \
  "${repo_root}/target/wasm32-unknown-unknown/release/vecmanf_editor_wasm.wasm" \
  --target web \
  --out-dir "${out_dir}" \
  --out-name vecmanf_editor_wasm
