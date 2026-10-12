# GIF attachment convention, version 1

The presentation remains an ordinary PDF. Its visible content contains a static
frame, and the original GIF is stored as an embedded-file attachment. A link
annotation around the frame joins the two.

| Item | Required value |
| --- | --- |
| Marker URI | `https://presenter.invalid/gif/v1/<id>` |
| Attachment filename | `<id>.gif` at the PDF attachment root |
| ID | 1–128 ASCII letters, digits, `.`, `_`, or `-` |
| Placement | Unrotated rectangle, contained in the displayed page region |
| GIF background | Opaque in every frame |

The marker URI is matched strictly. Queries, fragments, percent-encoded IDs,
unsupported versions, and different hosts are not part of the convention.
Attachment names are identifiers, never local filesystem paths. A GIF may be
referenced by multiple annotations on one page or across several pages.

The optional [Typst helper](../typst/presenter-gif.typ) uses an absolute virtual
attachment path, `/<id>.gif`, so Typst exports the same attachment name even when
the importing source file lives in a subdirectory. This mapping is verified with
Typst 0.14.2. It does not require a slide theme or presentation package.

```typst
#import "presenter-gif.typ": attach-gif, gif-frame
#attach-gif("orbit", read("assets/orbit.gif", encoding: none))

// Use a PNG made from a frame of the GIF, at its original aspect ratio.
#gif-frame("orbit", image("assets/orbit.png", width: 6cm))
#pagebreak()
#gif-frame("orbit", image("assets/orbit.png", width: 4cm))
```

Only call `attach-gif` once per ID. Duplicate attachment IDs, missing attachments,
invalid GIFs, malformed markers, and unsupported placements produce diagnostics.
The PDF's static image stays available when an overlay is skipped. An animation
crossing the audience/notes boundary is not partially overlaid.

The viewer enumerates indirect file specifications and nested embedded-file name
trees, extracts supported GIF streams, and converts annotation rectangles from
PDF coordinates into the same displayed region used for rendering. Browser-native
images decode and animate the GIF. Blob URLs are released when the slide changes
or the document is replaced.

The GIF must define an opaque background and cover its complete canvas in the
first frame. Frames following background-clear or previous-frame-restore
disposal must redraw the whole canvas. Optimized partial updates are accepted
when the previous frame is retained. Unsupported opacity or disposal patterns
remain static with a diagnostic.

Transparent GIFs are intentionally not animated. Covering a static PDF frame with
a transparent animation can leave stale pixels visible underneath; an opaque
background prevents that artifact. Rotation, skew, arbitrary clipping, distorted
aspect ratios, and media formats other than GIF are outside this first protocol.
