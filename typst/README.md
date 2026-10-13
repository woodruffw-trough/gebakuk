# Authoring animations

Copy [presenter-gif.typ](presenter-gif.typ) beside your presentation source, then
use:

```typst
#import "presenter-gif.typ": gif
#gif(path("orbit.gif"), width: 6cm)
```

The helper requires Typst **0.15.1**. Typst draws the GIF's first frame in the PDF;
Gebakuk plays the original animation in its place. The helper embeds the GIF bytes
and assigns an attachment ID automatically. Ordinary PDF readers show the first
frame.

`path("orbit.gif")` resolves the file relative to the document that calls `gif`,
even when the imported helper is in another directory. Set `width` to size the
image while preserving its aspect ratio. Other named image arguments are passed
to Typst's `image`; the helper fixes the format to GIF. Reuse the same path on
any slide: the file is attached once, and its placements share the animation.

Use opaque or transparent GIFs in an unrotated image at the original aspect ratio.
Unsupported placements retain the static first frame and produce a diagnostic.
See the [GIF convention](../docs/gif-protocol.md) for placement requirements and
how transparent playback preserves the PDF background.

[simple-gif.typ](simple-gif.typ) demonstrates automatic IDs, reuse, and paths
relative to the calling document.
[normal.typ](normal.typ) is a fixture deck with one slide per 640 × 360 pt page.
[notes-right.typ](notes-right.typ) is a fixture deck with 1280 × 360 pt pages whose
left half is the slide and right half is notes. Speaker notes remain readable
in the original PDF.

The [fixture instructions](../fixtures/README.md) describe rebuilding these
examples and the regression PDFs.

## Explicit IDs and static frames

The `attach-gif` and `gif-frame` helpers remain available for existing documents
and applications that supply their own IDs or static frames. Their usage is
documented in the [GIF convention](../docs/gif-protocol.md#explicit-ids-and-static-frames).

## Public guide

[`guide/guide.typ`](guide/guide.typ) contains the application's public guide.
Its compiled PDF is checked in at [`web/guide.pdf`](../web/guide.pdf), and
**Open guide** loads it through `?source=guide.pdf`.

See the [development instructions](../docs/development.md#regenerate-the-public-guide)
to regenerate it.
