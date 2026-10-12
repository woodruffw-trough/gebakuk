# Gebakuk

A local PDF presenter written in Rust and compiled to WebAssembly. Open a PDF,
keep your notes in the presenter window, and show the audience a separate,
synchronized slide window. GIFs attached using the documented convention play
over their static PDF frames.

[Open the application](https://woodruffw-trough.github.io/gebakuk/)
· [Download a notes-right sample](https://woodruffw-trough.github.io/gebakuk/sample.pdf)

## Present a PDF

1. Open the application and choose a PDF from your computer.
2. Choose **Normal** for ordinary slides, or **Notes right** for pages with a
   slide on the left and speaker notes on the right. Each PDF page is one slide.
3. Open the audience window using the presenter control. Allow the popup if
   your browser blocks it, then move the new window to your projector.
4. Use the audience window's fullscreen control. Browsers require a user action
   before entering fullscreen.
5. Navigate from the presenter with the buttons or keyboard. Reopen a closed
   audience window to resume at the current slide.

| Key | Action |
| --- | --- |
| Right arrow, Page Down, Space | Next slide |
| Left arrow, Page Up | Previous slide |
| Home / End | First / final slide |
| Escape | Leave fullscreen through the browser |

Navigation keys are ignored while an interactive control has focus. The presenter
also shows the current slide, notes in notes-right mode, a next-slide preview,
slide counts, and an elapsed timer. Layout selection is manual: a wide page is
not automatically assumed to contain notes.

PDFs are processed in your browser and are never uploaded. Both windows receive
the complete PDF; hiding notes in the audience interface does not make them
confidential. Standard PDF readers still show the original static slides.

## Build and serve locally

Install [Rust with rustup](https://rustup.rs/). The checked-in toolchain file pins
Rust **1.99.0**, rustfmt, Clippy, and the `wasm32-unknown-unknown` target.

```sh
cargo install --locked wasm-bindgen-cli --version 0.2.129
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
