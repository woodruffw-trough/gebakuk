# Architecture and development

The Cargo workspace contains two crates, both forbidding unsafe project code.

| Location | Responsibility |
| --- | --- |
| `crates/presenter-core` | PDF metadata, geometry, attachment extraction, GIF validation, and region rasterization |
| `crates/presenter-web` | Browser interface, navigation, window protocol, render cache, and GIF resource lifecycle |
| `web/` | Static HTML, CSS, icon, and the minimal WASM bootstrap |
| `typst/` | Reproducible sample sources and optional GIF helper |
| `fixtures/` | Checked-in PDFs and media used by tests |
| `scripts/build.sh` | Cargo and wasm-bindgen static packaging |

## Rendering

`lopdf` reads metadata, page boxes, annotations, and attachments. `hayro` rasterizes
the content. Both are pure Rust; no operating-system PDF renderer is required.
Pinned versions and `Cargo.lock` keep native and WASM builds consistent.

The core describes page geometry independently of either parser. It resolves
CropBox/MediaBox and page rotation, converts the PDF's bottom-left coordinates
into top-left display coordinates, and splits notes-right pages after rotation.
Page sizes may differ within a document; each displayed region is fitted using
its own geometry. The raster destination covers only the selected region rather
than allocating a full double-width page bitmap.

Canvas sizing accounts for device pixel ratio and resource limits: the browser
caps DPR at 2, each destination at 4 million pixels, and each side at 4,096 pixels.
Animation rectangles are normalized within the same region, so overlays follow
the fitted canvas during resizing. A least-recently-used raster cache holds at
most six entries and 64 MiB; the current slide, notes, and next preview render
on demand, without rasterizing the entire document. Resize and fullscreen redraws
are debounced by 120 ms. PDF parsing and rasterization run on the browser's main
thread; a complex page can pause navigation while rendering. There is no worker
architecture in this release.

## Window synchronization

An explicit user action opens a same-origin audience window, running the same
WASM module with an audience URL role. The presenter owns the PDF, layout, and
slide state. The audience announces readiness and receives PDF bytes once per
document load; later navigation messages send state rather than the full PDF.
Both windows render locally.

The protocol uses `ready`, `document`, `sync`, `navigate`, `reset`, `command`,
`disconnect`, and `error` messages. Readiness includes a window-instance ID so a
reloaded or reopened audience receives its own initial document transfer. The
presenter handles audience navigation commands authoritatively.

The protocol validates both the sending window and exact origin before accepting
messages. Reopening an audience window transfers the current document and slide.
Document replacement and disconnection are explicit states. The audience checks
whether its opener is closed every second. Fullscreen and window placement remain
governed by browser permissions and user gestures.

## Static packaging

Run `scripts/build.sh` after installing the exact `wasm-bindgen-cli` version in
the README. It rejects a mismatched CLI, performs a release WASM build with the
lockfile, invokes `wasm-bindgen --target web`, and copies the web assets into
`dist/`. The downloadable sample is copied from `fixtures/notes-right.pdf`.
Generated assets and Cargo's `target/` are not source files.

The GitHub workflow performs formatting, native Clippy and tests, browser-target
Clippy, and static packaging. Only the deployment job receives Pages write and
OIDC permissions. Deployment follows successful checks on `main`; pull requests
do not publish a site. To use another static host, upload `dist/` unchanged and
serve `.wasm` as `application/wasm`.

Refer to [verification](verification.md) for fixture regeneration and browser
checks, and to [AGENTS.md](../AGENTS.md) for agent-specific development rules.
