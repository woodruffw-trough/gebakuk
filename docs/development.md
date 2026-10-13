# Architecture and development

The Cargo workspace contains two crates, both forbidding unsafe project code.

| Location | Responsibility |
| --- | --- |
| `crates/presenter-core` | PDF metadata, geometry, attachment extraction, GIF validation, and region rasterization |
| `crates/presenter-web` | Browser interface, navigation, window protocol, render cache, and GIF resource lifecycle |
| `web/` | Static HTML, CSS, icon, and the minimal WASM bootstrap |
| `typst/` | Public guide source, reproducible fixture sources, and optional GIF helper |
| `fixtures/` | Checked-in PDFs and media used by tests |
| `scripts/build.sh` | Cargo and wasm-bindgen static packaging |

## Build and serve locally

Install [Rust with rustup](https://rustup.rs/). The checked-in toolchain file pins
Rust **1.99.0**, rustfmt, Clippy, and the `wasm32-unknown-unknown` target. Run these
commands from the repository root:

```sh
cargo install --locked wasm-bindgen-cli --version 0.2.129
cargo install --locked wasm-tools --version 1.245.1 --no-default-features --features validate
scripts/build.sh
python3 -m http.server 8080 --directory dist
```

Open `http://localhost:8080`. The distribution contains `index.html`, static
assets including the guide PDF, and generated bindings and WebAssembly in
`pkg/`. Serve the whole `dist/` directory through HTTP or HTTPS. Relative asset
paths support deployment beneath a repository subdirectory. Direct `file://`
use is unsupported.

Run the required checks before submitting changes:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo clippy --locked -p presenter-web --target wasm32-unknown-unknown -- -D warnings
scripts/build.sh
```

There is no Node.js, JavaScript package manager, bundler, framework, native PDF
renderer, or backend. Application logic is Rust; the handwritten JavaScript only
initializes the generated WASM module.

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
current slide directly for opaque GIFs. Transparent slides keep separate static
preview and animated canvases; neighboring animated canvases are prefetched.

Transparent GIFs need the PDF's first-frame image removed before playback. The
core matches its direct image draw to the animation marker, then builds a
filtered PDF that retains the background and other artwork. One filtered
renderer is reused across navigation, notes, and resizing; layout changes and
animation failures invalidate it. The original renderer provides static previews
and restores a failed animation's poster. Unsupported or ambiguous matches stay
static. See the [GIF convention](gif-protocol.md) for supported placements.

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

See [browser support](browser-support.md) for desktop version requirements and
their sources.

PDF parsing and cold rasterization still run on the browser's main thread;
complex pages and background preparation can briefly block input. There is no
worker architecture in this release.

## PDF loading

The presenter uses one opener in the Current Slide panel for local files and
URLs. Clicking its file target opens a hidden native file picker; dropping one
file into the panel uses the same loading path, whether a PDF is already open
or not. Empty and text-only drops are ignored, and multiple files produce an
error. Drops elsewhere are prevented from navigating the browser but do not
load a document. File contents are validated without trusting MIME metadata.
The PDF URL field accepts **Open** or Enter. **Open guide** uses
`?source=guide.pdf` to open the public guide in the presenter.

The opener hides after a PDF loads; **Change PDF** reopens it. **Cancel** aborts
an active download and advances the load generation so a late file read cannot
replace the current PDF. The previous document and diagnostics are restored.
The opener is a persistent overlay outside `#current-slide`, which rendering
clears, so the viewport retains its dimensions. While the opener is visible,
the displayed slide is inert and hidden from assistive technology. The opener
is presenter-only. A loaded slide does not open a file picker on a single click;
double-click still toggles fullscreen.

Presenter status messages occupy a fixed-height region in the toolbar, so loading,
errors, and diagnostics do not move the slide panels. Long messages wrap and
scroll. The region enters the Tab order when it has content; navigation shortcuts
leave its scroll keys alone.

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

Typst **0.15.1** regenerates the checked-in PDF fixtures. Attachment tests
verify that original GIF bytes survive export unchanged. The viewer's GIF
convention is independent of Typst versions.

## Regenerate the public guide

The guide source is [`typst/guide/guide.typ`](../typst/guide/guide.typ), and its
PDF is checked in at [`web/guide.pdf`](../web/guide.pdf). Install
[Typst **0.15.1**](https://github.com/typst/typst/releases/tag/v0.15.1) and compile
from the repository root using its bundled fonts and a fixed timestamp:

```sh
typst compile --root . --ignore-system-fonts --creation-timestamp 0 typst/guide/guide.typ web/guide.pdf
```

The application build copies the checked-in guide with the other web assets;
it does not run Typst. See [fixture instructions](../fixtures/README.md) for the
separate PDF regression fixture generator.

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
`wasm-tools` versions listed above. It rejects mismatched tools, performs a
release WASM build with the lockfile, invokes `wasm-bindgen --target web`, and
copies the web assets into `dist/`. Release builds omit debug information;
wasm-bindgen removes the name and producers sections.

The build validates the final WASM artifact with `wasm-tools` **1.245.1**. It also
checks that validation with SIMD disabled fails specifically on a SIMD
instruction, ensuring that the shipped module uses SIMD. CI installs the same
pinned validator. The public guide is copied from `web/guide.pdf` to
`dist/guide.pdf`. Generated assets and Cargo's `target/` are not source files.

[GitHub Actions](../.github/workflows/ci.yml) runs the checks above for pull
requests and `main`, then deploys successful `main` builds to GitHub Pages. Only
the deployment job receives Pages write and OIDC permissions; official actions
are pinned to commit SHAs. The workflow can configure Pages automatically. If
configuration is unavailable, a repository administrator must set **Settings →
Pages → Build and deployment → Source → GitHub Actions**, then rerun the
workflow. The application needs no deployment credential.

CI restores separate caches for pinned WASM tool binaries and Cargo registry,
Git dependencies, and build outputs. Keys include the runner platform, toolchain,
build configuration, lockfile, and Rust sources as appropriate. Missing tools are
compiled and their versions are checked on every run. Pull requests can restore
default-branch caches; successful `main` builds populate them. The distribution
is rebuilt and validated on every run.

To use another static host, upload `dist/` unchanged and serve `.wasm` as
`application/wasm`.

Refer to [verification](verification.md) for fixture regeneration and browser
checks, and to [AGENTS.md](../AGENTS.md) for agent-specific development rules.
