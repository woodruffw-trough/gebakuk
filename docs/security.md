# Privacy, input limits, and known limitations

Gebakuk processes PDFs in the browser. It does not send PDFs, extracted media,
or notes to a server, request an account, or use persistent document storage.
The initial application files are fetched from the static host. Audience windows
receive PDF bytes directly from the presenter through same-origin `postMessage`.

Loading a PDF URL sends a download request directly to its source server. The
request omits credentials and the referrer, and cross-origin downloads require
CORS permission from that server. Only HTTP and HTTPS URLs are supported;
relative URLs resolve against the application address. HTTPS browsers may block
HTTP sources as mixed content. There is no backend proxy or document upload.

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
- Animation requires opaque GIFs and rectangular placements with matching aspect
  ratios. The first frame must cover the complete canvas; a frame following a
  clear/restore disposal operation must also cover the canvas. GIFs that expose
  transparent canvas pixels remain static with a diagnostic. Animation placements
  on rotated pages, skewed images, and arbitrary clipping are unsupported.
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
