# Verification

Build instructions and required checks are in
[development](development.md#build-and-serve-locally). See the
[fixture instructions](../fixtures/README.md) to regenerate the regression PDFs.

Native tests cover page geometry, cropping and rotation, attachment and marker
resolution, GIF validation, transparent poster removal, static fallback, resource
limits, and navigation protocol values. CI runs these tests and builds the WASM
browser crate. Browser interaction is checked separately.

## Browser checklist

Serve `dist/` over HTTP and exercise these cases:

1. In **Current slide**, open local PDFs through the file picker and drop target.
   Load a PDF URL, including a CORS-enabled source, and open an application link
   with `?source=`. Check **Open guide**, **Change PDF**, and **Cancel**.
2. Check **Auto**, **Normal slides**, and **Notes on right** with the matching
   fixtures. Confirm current, notes, and static next-slide previews.
3. Open the audience window and navigate with every documented key. Check focus
   in controls, the elapsed timer, and **Reset**. Both windows should follow the
   same slide.
4. Resize both windows and toggle fullscreen repeatedly with the button and slide
   double-click. Check the button label, external fullscreen exit, canvas sizing,
   and GIF alignment.
5. Display single, multiple, reused, and transparent GIFs. Check the PDF
   background, partial frames, background/previous disposal, and static previews.
   Simulate one GIF decode failure and confirm its poster returns while other
   animations continue.
6. Close and reopen the audience window, replace the PDF while it is open, and
   close the presenter. Check reconnection and the audience's disconnected state.
7. Reject invalid or oversized PDFs while keeping the previous document usable.
   Open malformed-marker and missing-attachment fixtures; check static content
   and diagnostics. Cancel or replace pending downloads and confirm late
   completions cannot replace the current PDF.
8. Navigate `large.pdf` and the GIF fixtures slowly and rapidly, revisiting slides.
   Check that each panel keeps its canvas and decoded images until a replacement
   is ready. Resize, change layout, or replace the PDF during pending GIF decoding.
   Older completions must not overwrite the latest selected slide or layout.
   Check for blank frames and unreleased animation URLs.

Fixture purposes and transparency sample coordinates are documented in the
[fixture README](../fixtures/README.md).

## Browser coverage

Local acceptance checks have used headless Chromium **154.0.8037.57** on Linux,
including desktop viewports and widths of 320 and 375 pixels. They covered file
and URL loading, CORS and source links, layouts, navigation and focus, timers, fullscreen,
window synchronization, native GIF playback and disposal, independent decode
failure fallback, resizing, cancellation, resource cleanup, and staged rendering
during rapid navigation.

Safari and Firefox remain unverified. The [browser support table](browser-support.md)
records feature-based minimum versions separately from these interaction checks.
