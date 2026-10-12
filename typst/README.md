# Authoring animations

Examples and fixtures use Typst **0.15.1**. The helper was also verified with
0.14.2 and works with ordinary Typst documents without a presentation package.

```typst
#import "presenter-gif.typ": attach-gif, gif-frame

// Attach the original once, even if several slides use it.
#attach-gif("orbit", read("assets/orbit.gif", encoding: none))

// Supply a static poster with the GIF's original aspect ratio.
#gif-frame("orbit", image("assets/orbit.png", width: 320pt))
```

Each ID contains 1–128 ASCII letters, digits, periods, underscores or hyphens.
`attach-gif` embeds the original bytes under the exact PDF name `orbit.gif`;
`gif-frame` writes a link annotation for
`https://presenter.invalid/gif/v1/orbit`. The viewer recognizes the marker as
animation metadata. Conventional PDF readers continue to display the poster.

Typst exports the project-root-relative attachment path, including directories.
There is no `name:` option in `pdf.attach` in Typst 0.15.1. The helper supplies
`/orbit.gif` with caller-provided bytes so that the exported name stays exact,
regardless of the helper's directory. Compile with an explicit project root:

```sh
typst compile --root . typst/notes-right.typ demo.pdf
```

Use opaque GIF frames and an opaque poster. Transparent GIFs retain the static
poster and produce a diagnostic: overlaying transparent frames over their own
poster would leave old pixels visible. Initially, placements must be rectangular,
unrotated and preserve the original GIF aspect ratio. GIFs on rotated PDF pages
also retain their static representation.

[normal.typ](normal.typ) is a fixture deck with one slide per 640 × 360 pt page.
[notes-right.typ](notes-right.typ) is a fixture deck with 1280 × 360 pt pages whose
left half is the slide and right half is notes. Attach each ID once and reuse `gif-frame`
wherever needed. Speaker notes remain readable in the original PDF.

The [fixture instructions](../fixtures/README.md) describe rebuilding these
examples and the regression PDFs.

## Public guide

[`guide/guide.typ`](guide/guide.typ) contains the application's public guide.
Its compiled PDF is checked in at [`web/guide.pdf`](../web/guide.pdf), and
**Open guide** loads it through `?source=guide.pdf`.

Regenerate it with Typst **0.15.1** from the repository root, using bundled fonts
and a fixed creation timestamp:

```sh
typst compile --root . --ignore-system-fonts --creation-timestamp 0 typst/guide/guide.typ web/guide.pdf
```

The application build copies the checked-in PDF to `dist/guide.pdf`; Typst is
needed only when editing the guide or regenerating fixtures.
