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
[`fixtures/README.md`](../fixtures/README.md). Generation uses Typst **0.14.2**,
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

## Recorded results

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

These elapsed measurements include browser automation and rendering in a
headless container; they are a responsiveness check, not a projector hardware
benchmark. All **23 native Rust tests** pass. Core regression tests separately verify all four page rotations,
nonzero origins and CropBox cropping, exact GIF bytes, nested attachment trees,
malformed markers, missing/duplicate attachments, and bounded raster requests.
The foundation proof also rendered a complete 1280 × 360 page and each 640 × 360
half in the browser, recovering its 22,197-byte GIF attachment exactly.
