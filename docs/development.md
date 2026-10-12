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
`Presentation::detected_layout()` selects notes-right only when every effective,
rotated visible page has an aspect ratio of at least 2.5:1. An empty page set or
any page below the threshold selects normal. The browser's Auto option resolves
this choice when a document loads; a manual selection persists until Auto is
selected again.
Page sizes may differ within a document; each displayed region is fitted using
its own geometry. The raster destination covers only the selected region rather
than allocating a full double-width page bitmap.

Canvas sizing accounts for device pixel ratio and resource limits: the browser
caps DPR at 2, each destination at 4 million pixels, and each side at 4,096 pixels.
Animation rectangles are normalized within the same region, so overlays follow
the fitted canvas during resizing. A least-recently-used cache holds at most six
uploaded canvases and 64 MiB of pixel data. A cache hit reuses the canvas without
copying RGBA pixels through `ImageData` again. The next-slide preview uses the
resolution required by the current-slide viewport, so its canvas can become the
current slide directly.

Navigation sends the audience state immediately and displays the current slide
first. Missing notes, the next preview, and neighboring slide/notes canvases are
prepared one task at a time after a paint opportunity. Each task yields through
`requestAnimationFrame` followed by a timer; navigation, document replacement,
layout changes, and resizing cancel obsolete work. An unchanged synchronization
message updates elapsed time and connection state without redrawing the slide or
restarting GIFs. Resize and fullscreen redraws are debounced by 120 ms.

Release builds use `opt-level = 3`, fat LTO, a single codegen unit, and aborting
panics. The WASM target enables `simd128` through `.cargo/config.toml`; a compile
guard rejects builds that disable it, including environment overrides. This
activates the WASM SIMD implementations in VelloCPU rendering through
`fearless_simd` and in PicScale image resizing. Browsers must support WebAssembly
SIMD128; no scalar fallback is built.

PDF parsing and cold rasterization still run on the browser's main thread;
complex pages and background preparation can briefly block input. There is no
worker architecture in this release.

## PDF loading

Local files and URL downloads use the same core parsing and size limits. URL
loading runs in Rust through browser Fetch APIs, accepts HTTP/HTTPS URLs, and
resolves relative URLs against the application address. Requests use CORS mode,
omit credentials, and suppress the referrer. The response body is streamed with
a 64 MiB cap; a 60-second timeout bounds the download. Cross-origin sources must
provide suitable CORS headers. The browser also enforces mixed-content rules.
There is no backend proxy or PDF upload.

Presenter startup accepts `source` in the application query (`?source=`), fills
the PDF URL field, and starts the normal URL-loading path. Values are parsed
once through `URLSearchParams`; callers should encode the entire PDF URL with
`encodeURIComponent` or construct the parameter with `URLSearchParams` so nested
`&`, `+`, and `#` characters are preserved. The download still uses the same CORS,
credential, referrer, size, and timeout rules.

Typst **0.15.1** is used to regenerate the fifteen checked-in PDF fixtures. Two
regeneration passes produced identical fixture bytes, and attachment tests
verify that original GIF bytes survive export unchanged. The previous 0.14.2
pin constrained fixture generation, not which PDFs the viewer could load.
The GIF convention is independent of Typst versions.

## Window synchronization

An explicit user action opens a same-origin audience window, running the same
WASM module with an audience URL role. The presenter owns the PDF, layout, and
slide state. The audience announces readiness and receives PDF bytes once per
document load; later navigation messages send state rather than the full PDF.
Both windows render locally.

Audience windows never fetch a startup `source`, including when opened directly
with `role=audience`. Generated audience URLs remove the query `source`; the
audience receives document bytes through the window protocol instead.

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

Run `scripts/build.sh` after installing the exact `wasm-bindgen-cli` and
`wasm-tools` versions in the README. It rejects mismatched tools, performs a
release WASM build with the lockfile, invokes `wasm-bindgen --target web`, and
copies the web assets into `dist/`. Release builds omit debug information;
wasm-bindgen removes the name and producers sections.

The build validates the final WASM artifact with `wasm-tools` **1.245.1**. It also
checks that validation with SIMD disabled fails specifically on a SIMD
instruction, ensuring that the shipped module uses SIMD. CI installs the same
pinned validator. The downloadable sample is copied from
`fixtures/notes-right.pdf`. Generated assets and Cargo's `target/` are not source
files.

The GitHub workflow performs formatting, native Clippy and tests, browser-target
Clippy, and static packaging. Only the deployment job receives Pages write and
OIDC permissions. Deployment follows successful checks on `main`; pull requests
do not publish a site. To use another static host, upload `dist/` unchanged and
serve `.wasm` as `application/wasm`.

Refer to [verification](verification.md) for fixture regeneration and browser
checks, and to [AGENTS.md](../AGENTS.md) for agent-specific development rules.
