# GIF attachment convention, version 1

```typst
#import "presenter-gif.typ": gif
#gif(path("orbit.gif"), width: 6cm)
```

The optional [Typst helper](../typst/presenter-gif.typ) requires Typst **0.15.1**.
Typst displays the GIF's first frame in the PDF, and Gebakuk plays the original
animation over it. The helper embeds the original bytes and assigns IDs
automatically. Reusing the same file path on multiple pages attaches its
GIF once.

Use `path(...)` to resolve the GIF relative to the document calling `gif`, even
when the helper is elsewhere. `width` sizes the image at its original aspect
ratio. Other named arguments are forwarded to Typst's `image`, with the format
fixed to GIF. Ordinary PDF readers show the static first frame.

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

The helper uses absolute virtual attachment paths, `/<id>.gif`, so Typst exports
root-level attachment names regardless of the source directory. The sixteen PDF
fixtures use Typst **0.15.1**; tests verify recovery of the original GIF bytes.
The viewer accepts PDFs that follow this convention regardless of producer or
slide theme.

Duplicate attachment IDs, missing attachments, invalid GIFs, malformed markers,
and unsupported placements produce diagnostics.
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

## Explicit IDs and static frames

The lower-level `attach-gif` and `gif-frame` helpers remain available for existing
presentations and callers that provide their own IDs or static images:

```typst
#import "presenter-gif.typ": attach-gif, gif-frame
#attach-gif("orbit", read("assets/orbit.gif", encoding: none))
#gif-frame("orbit", image("assets/orbit.png", width: 6cm))
```

Call `attach-gif` once per ID, then reuse `gif-frame` wherever that animation is
needed. Supply an opaque static frame with the GIF's original aspect ratio.
