# PDF fixtures

Open [notes-right.pdf](notes-right.pdf) with the viewer's Notes-right layout or
[normal.pdf](normal.pdf) with Normal layout. Both three-slide decks include one
GIF, then two independent GIFs with one attachment reused, then a static slide.

| File | Purpose |
| --- | --- |
| `plain.pdf` | Ordinary two-page PDF, no attachments or animation links |
| `normal.pdf` | Normal presentation, multiple and reused GIFs |
| `notes-right.pdf` | Slide and speaker-note halves, multiple and reused GIFs |
| `single-gif.pdf` | Known animation rectangle and original attachment bytes |
| `simple-gif.pdf` | Three normal pages, two GIFs with automatic IDs, reuse, and mixed source paths |
| `nonzero-origin.pdf` | Page boxes and content shifted to origin (40, 25) |
| `cropbox.pdf` | CropBox differs from MediaBox, with nonzero origin |
| `rotated.pdf` | Mixed unrotated and 90-degree pages |
| `rotated-gif.pdf` | Static fallback for an animation on a rotated page |
| `inconsistent-pages.pdf` | Different effective page sizes |
| `missing-attachment.pdf` | Marker whose attachment is absent |
| `malformed-marker.pdf` | Empty ID and encoded slash in ID |
| `transparent.pdf` | Transparent GIF must retain the static poster |
| `nested-name-tree.pdf` | Embedded-file name tree uses an indirect child |
| `duplicate-identifiers.pdf` | Duplicate attachment names are diagnosed |
| `large.pdf` | 120 pages for navigation and bounded cache checks |

The original GIFs and their posters are in `assets/`. They are synthetic,
generated from geometric shapes by the checked-in script. `orbit.gif` and
`pulse.gif` contain opaque frames; `transparent.gif` explicitly sets GIF
transparency. Each GIF has 12 frames, a 320 × 180 logical screen and 120 ms frame
duration. The GIFs contain no external media or third-party artwork.

Regeneration requires Typst **0.15.1**, Python 3, Pillow **12.3.0** and pypdf
**6.10.0**. These tools are fixture authoring dependencies only; the viewer and
its build do not use Python. Download the pinned Typst CLI from
[its release](https://github.com/typst/typst/releases/tag/v0.15.1), then run:

```sh
python3 -m pip install Pillow==12.3.0 pypdf==6.10.0
python3 scripts/generate-fixtures.py --typst /path/to/typst
```

The script regenerates the GIFs, compiles the checked-in `typst/*.typ` sources,
and derives the PDF edge cases with pypdf. Compilation uses only Typst's bundled
fonts, an explicit project root and a fixed creation timestamp. Repeated runs
with these versions produce identical files. No native PDF renderer is used.

`manifest.json` records PDF hashes, attachment hashes, page boxes, rotations and
animation rectangles. Regeneration verifies that every embedded GIF matches its
original bytes exactly. It also asserts that `single-gif.pdf` exports the exact
attachment name `orbit.gif` and PDF rectangle **[40, 120, 360, 300]** on a
640 × 360 pt page: a 320 × 180 pt image 40 pt from the left and 60 pt from the top.
