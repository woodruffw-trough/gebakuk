# Privacy, input limits, and known limitations

Gebakuk processes PDFs in the browser. It does not send PDFs, extracted media,
or notes to a server, request an account, or use persistent document storage.
The initial application files are fetched from the static host. Audience windows
receive PDF bytes directly from the presenter through same-origin `postMessage`.

The Current Slide opener accepts a file from the native picker or a single-file
drop. Dropped files are validated as PDFs without trusting their filename or
MIME type; the same 64 MiB limit applies. Empty and text-only drops are ignored,
and multiple files are rejected. Drops outside Current Slide do not load a
document and are prevented from navigating the browser. Cancelling an open
operation aborts its download and ignores late file reads, keeping the previous
document and diagnostics.

Loading a PDF URL sends a download request directly to its source server. The
request omits credentials and the referrer, and cross-origin downloads require
CORS permission from that server. Only HTTP and HTTPS URLs are supported;
relative URLs resolve against the application address. HTTPS browsers may block
HTTP sources as mixed content. There is no backend proxy or document upload.

Application links containing `?source=` start that download automatically.
Query parameters are sent to the static host with the page request. The browser
contacts the PDF source server and requires the same CORS permission as a
manually entered URL. Linking to a PDF does not bypass a source server that
blocks cross-origin requests. Audience windows do not fetch these sources, and
generated audience URLs omit query `source`.

Notes are hidden by cropping the audience display. The audience still receives
the complete original PDF, including notes; this is a presentation aid, not a
confidentiality or encryption boundary.

PDF links are not followed, scripts are not executed, and attachments are never
written as local paths. Only the versioned synthetic GIF marker is interpreted.
Message handling verifies exact origin and expected sender, and incoming state
is validated before it changes the interface.

## Resource bounds

| Resource | Limit |
| --- | --- |
| Original PDF | 64 MiB |
| URL download | 64 MiB streamed; 60-second timeout |
| Physical pages | 2,000 |
| PDF objects | 200,000 |
| Non-image lossless stream | 32 MiB decoded |
| Lossless image stream | 128 MiB decoded |
| Combined lossless PDF streams | 256 MiB decoded |
| One PDF image | 32,000,000 pixels; at most 16,384 on either side |
| Combined declared PDF image pixels | 256,000,000 |
| One GIF attachment | 16 MiB |
| Combined GIF attachment bytes | 32 MiB |
| Core raster API | 8,000,000 pixels; at most 8,192 on either side |
| Browser canvas | 4,000,000 pixels; at most 4,096 on either side; DPR at most 2 |
| Uploaded canvas cache per window | Six entries and 64 MiB of pixel data |
| GIF logical canvas | 16,000,000 pixels; at most 8,192 on either side |
| GIF frames | 2,000 |
| Combined decoded GIF frame canvases across unique attachments | 128,000,000 pixels |

Embedded-file tree traversal and annotation counts are bounded. Page inheritance
and attachment trees reject cycles or excessive depth. Encrypted PDFs and
JPEG2000, CCITT, JBIG2, and JPEG filter chains are rejected by the bounded input
validation in this release; ordinary standalone JPEG image streams are supported.
Inline PDF images are unsupported in this release. Image color components and
predictor row parameters are checked before application-managed decoding.

Preparing transparent animation backgrounds bounds decoded page content,
graphics instructions, graphics-state nesting, and the resulting PDF size.
If a poster cannot be safely matched or removed, the animation stays static
with a diagnostic.

Malformed pages, invalid rectangles, unsupported GIF placements, missing
attachments, and duplicate IDs produce controlled errors or diagnostics.
The browser plays GIFs after bounded framing and compressed-index checks. Parser internals,
font handling, compressed PDF streams, and complex drawing operations can still
consume significant memory or CPU before an application check completes. These
bounds limit normal application allocations; they are not a hard CPU or process
memory sandbox. Use trusted presentations when predictable responsiveness matters.

## Current limitations

- Auto selects equal-half notes-right pages when every visible, rotated page is
  at least 2.5:1. Aspect ratio cannot confirm that a PDF contains speaker notes;
  unusual page shapes or mixed decks may require a manual selection. Only normal
  and equal-half notes-right pages are supported; other note arrangements require
  another PDF export.
- Cross-origin PDF URLs require CORS permission. Sources requiring cookies or
  other browser credentials cannot be loaded through the URL control.
- Opaque and transparent GIFs use unrotated rectangles at their original aspect
  ratio. Transparent playback needs one uniquely matched poster image drawn
  directly by the page. Pages containing PDF forms, rotated, skewed or mirrored
  poster images, clipping, custom graphics state, and ambiguous matches keep
  transparent animations static with a diagnostic. Rotated pages remain
  unsupported. See the [GIF convention](gif-protocol.md) for details.
- Parsing and rendering run on the main browser thread. Dense pages can briefly
  block input; raster bounds may reduce resolution on very large displays.
- PDF compatibility follows `hayro` and `lopdf`. Encrypted PDFs and the image
  compression formats listed above are rejected. Damaged, unusually large, or
  unsupported PDFs may fail to load or render. No PDF editing, search,
  arbitrary active content, video, or audio is provided.
- The audience window requires popup permission. Fullscreen requires an explicit
  user gesture; automatic monitor selection is unavailable.
- Serve the application through HTTP or HTTPS. Direct `file://` use is unsupported.
- The WASM module requires WebAssembly SIMD128 support; no scalar fallback is
  provided for browsers without it.
- Desktop Chromium is the initial automated browser verification target. Firefox
  and Safari need separate verification; compatibility with them is not claimed.
