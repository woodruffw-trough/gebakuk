# Development

Keep application logic in Rust. Do not add JavaScript package managers, bundlers,
frontend frameworks, native PDF renderers, or a backend. The handwritten
JavaScript in `web/index.html` only starts the generated WASM module.

Project crates forbid unsafe Rust. Keep PDF processing and geometry in
`presenter-core`, and browser APIs and presentation state in `presenter-web`.
Use bounded allocations and validate inter-window message origin and source.

Before submitting changes, run `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
and `scripts/build.sh`. Add meaningful regression coverage when changing PDF
geometry, attachment extraction, or window synchronization. Never claim browser
coverage for browsers that were not exercised.
