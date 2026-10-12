# Verification

Run the repository checks with the pinned Rust toolchain:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo clippy --locked -p presenter-web --target wasm32-unknown-unknown -- -D warnings
scripts/build.sh
```

Native tests cover coordinate origins and rotation, page-region splitting,
attachment and marker resolution, GIF integrity and limits, and rendering
fixtures. The WASM build checks the browser crate against actual browser bindings.
Neither compilation nor native tests substitute for browser interaction checks.

## Rebuild fixtures

The committed fixture matrix and exact regeneration procedure are documented in
[`fixtures/README.md`](../fixtures/README.md). Generation uses Typst **0.15.1**,
Python 3, Pillow **12.3.0**, and pypdf **6.10.0**, with no native PDF renderer.

```sh
python3 -m venv .venv
.venv/bin/pip install Pillow==12.3.0 pypdf==6.10.0
.venv/bin/python scripts/generate-fixtures.py --typst /path/to/typst
```

Typst uses its bundled fonts, a fixed creation timestamp, and the repository as
its virtual root. Generated GIFs and their static frames use deterministic
geometric artwork. The generator includes cropped/nonzero-origin and rotated
PDF derivatives, opaque and transparent GIFs, reused attachments, malformed
markers, missing attachments, and a large presentation. Two consecutive builds
were verified to produce byte-identical PDFs, media, and manifests.

## Browser verification checklist

Serve `dist/` through HTTP and exercise the following in a current desktop browser:

1. Open an ordinary PDF and a notes-right PDF through the local file picker.
2. Switch layout; confirm notes are visible only in the presenter.
3. Open the audience window, navigate using every documented key, and confirm
   both windows follow the same slide. Check keyboard focus in controls.
4. Resize both windows and enter/leave fullscreen through a user action; check
   canvas sharpness, clipping, and GIF alignment.
5. Display single, multiple, and reused GIFs. Confirm playback and removal on
   navigation. Open the transparent GIF fixture and confirm its diagnostic and
   static fallback.
6. Close and reopen the audience window. Replace the PDF while it is open, and
   close the presenter to check the audience's disconnected state.
7. Open malformed marker and missing attachment fixtures and confirm the static
   PDF remains usable. Reject invalid and oversized files with useful feedback.
8. Navigate the large fixture repeatedly and resize to check the cache bounds
   and resource cleanup. Record hardware, browser version, and measured render
   times before comparing performance.

Chromium automation used the environment's existing Python Playwright package
for verification, without adding a JavaScript toolchain to this repository.
Firefox and Safari must be exercised independently before claiming coverage.

## Initial release results

On October 12, 2026, headless Chromium **154.0.8037.57** passed the local HTTP
browser checks on a Linux container with five exposed AMD EPYC 9V74 vCPUs.
The presenter viewport was 1440 × 1050 at DPR 1. Checks also covered an 800 × 800
presenter, a 1024 × 768 audience window, and a 375 × 812 mobile viewport.

Verified local file selection, both page layouts, current/notes/next rendering,
single and multiple native GIF playback, marker rectangle alignment, keyboard
navigation and focused-control handling, timer updates, fullscreen, resize,
document replacement, wrong-sender message rejection, window closure/reopening,
and presenter disconnection. The audience received the PDF once per document,
with navigation transferring only state. Instrumented Blob URL accounting
returned to zero on the final slide and document replacement. Transparent GIFs
stayed static with a diagnostic; an invalid PDF left the current document usable.
There were no browser exceptions or document upload requests. Corrupt and empty
GIF LZW streams were rejected while their static PDF images still rendered.
A forced native image decode failure removed the overlay, revoked its URL, and
kept the canvas intact across subsequent redraws.

| Measurement | Result |
| --- | --- |
| WASM startup, local HTTP | 221 ms |
| Open and render the notes-right demo | 290 ms |
| 120-slide fixture, 30 forward and 30 backward key presses | 38 ms median; 85 ms maximum |
| Generated WASM module | Approximately 6.8 MiB |

These measurements describe the initial release, before the navigation update
below.

These elapsed measurements include browser automation and rendering in a
headless container; they are a responsiveness check, not a projector hardware
benchmark. All **23 native Rust tests** pass. Core regression tests separately verify all four page rotations,
nonzero origins and CropBox cropping, exact GIF bytes, nested attachment trees,
malformed markers, missing/duplicate attachments, and bounded raster requests.
The foundation proof also rendered a complete 1280 × 360 page and each 640 × 360
half in the browser, recovering its 22,197-byte GIF attachment exactly.

## Navigation update

The navigation update reuses uploaded canvases, prepares adjacent slides at
their display resolution, and defers notes/preview work until after the current
slide has a paint opportunity. Unchanged state synchronization does not redraw
the slide. Release builds use `opt-level = 3` with the same WASM CPU requirements.

Before/after checks used the same Chromium **154.0.8037.57** container at DPR 2,
a 1440 × 1050 presenter, and a 1280 × 720 audience window. The baseline was commit
`1f8d3ae`. The table shows median time from issuing a keyboard action to mounting
the selected canvas, including input dispatch, for sequential first visits with
180 ms pauses between actions. The update uses those pauses to prepare slides.

| Fixture | Presenter before → after | Audience before → after |
| --- | --- | --- |
| 120-slide text fixture | 74 → 2 ms | 151 → 12 ms |
| Dense 120-slide fixture | 466 → 2 ms | 1,120 → 14 ms |
| Notes-right demo | 274 → 5 ms | 598 → 16 ms |

On the dense fixture, presenter input-to-first-animation-frame opportunity fell
from 468 to 3 ms. Cached navigation required **zero** `ImageData` uploads,
compared with two or three in the presenter and one in the audience before the
update. These are headless browser measurements, not physical display or Safari
timings; audience animation frames can be throttled in a headless popup. Cold
pages and background preparation still use the main thread and can block input.

The dense fixture was generated with pypdf from the checked-in `normal.pdf`:
each of 120 pages is 640 × 360 points and contains four copies of its first page,
scaled by 0.5 and translated to `(0, 0)`, `(320, 0)`, `(0, 180)`, and `(320, 180)`.
Annotations were removed. It combines repeated text, vector shapes, and four
static image regions per page; no native PDF renderer was used.

Browser regression checks verified that the next preview's canvas becomes the
current canvas without an upload, timer resets and unchanged syncs preserve
canvas/GIF identity, and 20 navigation events spaced 100 ms apart finish on the
correct current and next slide. Document replacement cancels stale work, resize
keeps GIF alignment, and replacing the document releases GIF URLs. Retained
canvas memory stays bounded. No browser exceptions or document upload requests
were observed. The updated WASM module is approximately **5.2 MiB**.

## URL loading, Auto layout, and Typst 0.15.1

All fifteen PDF fixtures were regenerated with Typst **0.15.1 (9dfd3a08)**.
Two consecutive generations produced identical hashes. The helper still exports
the exact original GIF bytes as `orbit.gif`, with the expected marker rectangle.
All **27 native Rust tests**, formatting, native and WASM Clippy, and the release
build pass. Layout tests cover ordinary and doubled slide ratios, mixed decks,
and effective inherited CropBoxes after rotation.

Chromium **154.0.8037.57** passed eighteen URL/layout browser cases over local
HTTP. They cover same-origin and CORS-enabled downloads, relative URLs submitted
with Enter, redirects, valid chunked PDFs without Content-Length, automatic
redetection on document replacement, persistent manual overrides, and GIF and
audience synchronization. Requests were GETs without Cookie, Authorization, or
Referer headers.

Invalid schemes and credential-bearing URLs made no requests. HTTP errors,
denied CORS, malformed PDFs, and declared or streamed size-limit failures left
the previous presentation usable. A replacement URL or local file aborted a
pending download and prevented stale completion. Timeout cancellation was
verified by accelerating only the configured 60-second timer to 800 ms in the
test harness. The 375-pixel mobile viewport had no horizontal overflow. There
were no browser exceptions. Existing local-file and navigation regression
checks also passed. Safari and Firefox were not exercised for this update.
