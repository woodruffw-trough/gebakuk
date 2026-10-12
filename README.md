# Gebakuk

A PDF presenter written in Rust and compiled to WebAssembly. Open a PDF,
keep your notes in the presenter window, and show the audience a separate,
synchronized slide window. GIFs attached using the documented convention play
over their static PDF frames.

[Open the application](https://woodruffw-trough.github.io/gebakuk/)
· [Open a notes-right sample](https://woodruffw-trough.github.io/gebakuk/?source=sample.pdf)

## Present a PDF

1. In **Current Slide**, click **Click or drop a PDF** to choose a local file,
   drop one PDF into the panel, or enter a **PDF URL** and choose **Open** or
   press Enter. **Open sample PDF** loads the included notes-right sample.
2. Leave the layout on **Auto**, or choose **Normal** or **Notes right** manually.
   Each PDF page is one slide.
3. Open the audience window using the presenter control. Allow the popup if
   your browser blocks it, then move the new window to your projector.
4. Use the audience window's fullscreen control; click it again to exit.
   Browsers require a user action before entering fullscreen.
5. Navigate from the presenter with the buttons or keyboard. Reopen a closed
   audience window to resume at the current slide.

To replace the document, drop a PDF onto **Current Slide** or choose **Change
PDF** to reopen the file and URL controls. **Cancel** stops a pending load and
returns to the current document. Double-click the displayed slide to toggle
fullscreen.

| Key | Action |
| --- | --- |
| Right arrow, Page Down, Space | Next slide |
| Left arrow, Page Up | Previous slide |
| Home / End | First / final slide |
| Escape | Leave fullscreen through the browser |

Navigation keys are ignored while an interactive control has focus. The presenter
also shows the current slide, notes in notes-right mode, a next-slide preview,
slide counts, and an elapsed timer. **Auto** selects **Notes right** when every
visible page is at least 2.5 times as wide as it is tall, after cropping and
rotation. This detects doubled 4:3 and 16:9 slides while preserving ordinary
widescreen slides. Aspect ratio is a heuristic: use a manual layout for unusual
page shapes or mixed decks. A manual choice stays selected until you choose
**Auto** again; Auto detects the layout whenever a new PDF is loaded.

PDF URLs may be absolute HTTP/HTTPS addresses or relative to the application.
Cross-origin URLs require the source server to allow CORS, and HTTPS browsers
may block HTTP downloads as mixed content. Downloads omit credentials and the
referrer, have a 60-second timeout, and enforce the same 64 MiB limit as local
files. There is no download proxy.

To load a PDF when the application opens, put its URL in `?source=`. This link
loads the included sample:

```text
https://woodruffw-trough.github.io/gebakuk/?source=sample.pdf
```

Encode the entire PDF URL with `encodeURIComponent` or `URLSearchParams` when
it contains `&`, `+`, or `#`. For example, the URL
`https://example.org/slides.pdf?a=1&b=2` can be passed as:

```text
?source=https%3A%2F%2Fexample.org%2Fslides.pdf%3Fa%3D1%26b%3D2
```

These links use the same download limits and CORS requirements as **Open**.
The static host receives query parameters with the page request, and the browser
contacts the PDF source to download the document.

PDFs are processed in your browser and are never uploaded. Both windows receive
the complete PDF; hiding notes in the audience interface does not make them
confidential. Standard PDF readers still show the original static slides.

The application requires a browser with WebAssembly SIMD128 support. The WASM
build uses SIMD for rendering and image resizing; there is no scalar fallback.

## Build and serve locally

Install [Rust with rustup](https://rustup.rs/). The checked-in toolchain file pins
Rust **1.99.0**, rustfmt, Clippy, and the `wasm32-unknown-unknown` target.

```sh
cargo install --locked wasm-bindgen-cli --version 0.2.129
cargo install --locked wasm-tools --version 1.245.1 --no-default-features --features validate
scripts/build.sh
python3 -m http.server 8080 --directory dist
```

Open `http://localhost:8080`. The build creates `dist/index.html`, static assets,
and generated bindings and WebAssembly in `dist/pkg/`. Serve the entire directory
with any ordinary HTTP server. Paths are relative, so deployment beneath a
repository subdirectory works. Opening the HTML with `file://` is unsupported.

There is no Node.js, JavaScript package manager, bundler, framework, native PDF
renderer, or backend. Application logic is Rust; the handwritten JavaScript only
initializes the generated WASM module.

## Add GIFs to a Typst presentation

Use the optional helper in [`typst/presenter-gif.typ`](typst/presenter-gif.typ).
The helper and fixture generation are verified with **Typst 0.15.1**. The viewer
reads the resulting PDF and does not restrict the Typst version used to create it.
Attach an opaque GIF once with an ID and wrap each static frame in a matching
link. IDs map to root-level attachment names `<id>.gif`, and markers use
`https://presenter.invalid/gif/v1/<id>`. The viewer recognizes these links without
following them.

```typst
#import "presenter-gif.typ": attach-gif, gif-frame
#attach-gif("orbit", read("orbit.gif", encoding: none))
#gif-frame("orbit", image("orbit.png", width: 6cm))
```

Use an opaque GIF and a static image with the same aspect ratio. Reuse the same
ID to place the animation on multiple slides without attaching another copy.
Transparent GIFs deliberately remain static and produce a diagnostic, because
transparent animated frames can otherwise expose the earlier PDF frame beneath
them. See [the GIF convention](docs/gif-protocol.md) for the exact rules.

## Development and deployment

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo clippy --locked -p presenter-web --target wasm32-unknown-unknown -- -D warnings
scripts/build.sh
```

[GitHub Actions](.github/workflows/ci.yml) runs these checks for pull requests and
`main`, builds the static site, and deploys successful `main` builds to GitHub
Pages. Official actions are pinned to commit SHAs. If automatic Pages setup is
unavailable for a repository, its administrator must set **Settings → Pages →
Build and deployment → Source → GitHub Actions** before rerunning the workflow.
No deployment credential is needed in the application.

See [architecture and development](docs/development.md),
[security and limits](docs/security.md), and [verification](docs/verification.md).
Project code is available under the MIT or Apache-2.0 license, at your option;
see [LICENSE](LICENSE).
