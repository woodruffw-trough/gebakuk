#import "../presenter-gif.typ": gif

#set page(width: 1280pt, height: 360pt, margin: 0pt)
#set document(title: "Gebakuk — In-app guide", author: "Gebakuk contributors")
#set text(font: "DejaVu Sans Mono", size: 17pt, fill: rgb("#173047"))
#set par(leading: 0.6em, spacing: 0pt)

#let accent = rgb("#2363a3")
#let muted = rgb("#53677a")
#let lines(items, size: 17pt, gap: 10pt) = stack(dir: ttb, spacing: gap,
  ..items.map(item => block(width: 580pt)[#text(size: size)[#item]]))

#let panel(title, number, body, speaker: false) = block(
  width: 640pt, height: 360pt,
  fill: if speaker { rgb("#e8f0f7") } else { rgb("#f7fafc") },
)[
  #place(dx: 30pt, dy: 24pt,
    text(size: 10pt, fill: accent,
      if speaker { "FOLLOW ALONG" } else { "GEBAKUK GUIDE" }))
  #place(dx: 535pt, dy: 24pt, text(size: 10pt, fill: muted, str(number) + " / 8"))
  #place(dx: 30pt, dy: 52pt,
    block(width: 580pt)[#text(size: 27pt, weight: "bold", title)])
  #place(dx: 30pt, dy: 115pt, block(width: 580pt)[#body])
  #place(dx: 30pt, dy: 338pt,
    text(size: 9pt, fill: muted,
      if speaker { "Right half · speaker notes" } else { "Left half · audience slide" }))
]

#let page(title, number, body, note-title, note-body) = {
  if number > 1 { pagebreak() }
  grid(columns: (640pt, 640pt), gutter: 0pt,
    panel(title, number, body),
    panel(note-title, number, note-body, speaker: true))
}

#page("Use the presenter console", 1, [
  #grid(columns: (305pt, 235pt), gutter: 30pt,
    [#lines((
       [*Current:* audience view],
       [*Notes:* this right half],
       [*Next:* static preview],
       [*Elapsed:* running timer],
     ))],
    [#gif(path("../../fixtures/assets/orbit.gif"), width: 235pt)
     #v(9pt)
     #text(size: 12pt, fill: muted)[Opaque demo GIF]])
  #v(16pt)
  #text(size: 15pt)[Use Next below the slide to follow this guide.]
], "Start with this page", [
  #lines((
    [Find Current slide, Speaker notes and Next slide.],
    [Watch the orange dot move in Current slide.],
    [Elapsed starts when a PDF opens. Reset restarts it.],
    [Click Next to continue. The audience will see only the left half.],
  ), size: 16pt)
])

#page("Navigate, then focus the slide", 2, [
  #lines((
    [→ / Page Down / Space   Next slide],
    [← / Page Up             Previous slide],
    [Home                    First slide],
    [End                     Last slide],
  ), size: 16pt, gap: 12pt)
  #v(20pt)
  #text(size: 16pt)[Click the slide before using these shortcuts.]
], "Try the keys", [
  #lines((
    [Click Current slide. Press → once, then ← to return.],
    [Try Home to return to page 1; use Next to come back.],
    [A focused button, field or menu keeps its normal keyboard behavior.],
    [Click the slide again to navigate. These keys work in the audience window too.],
  ), size: 16pt)
])

#page("Open the audience window", 3, [
  #lines((
    [1. Click *Open audience ↗*.],
    [2. Move that window to your audience display.],
    [3. Click *Fullscreen* in that window.],
    [Click it again, or press Esc, to leave fullscreen.],
    [Only the audience slide appears there.],
  ), size: 16pt, gap: 12pt)
], "Keep the presenter open", [
  #lines((
    [Allow popups for this site if the new window is blocked.],
    [Advance once and check that both windows follow.],
    [Close and reopen the audience window: it returns to your current slide.],
    [Double-click a slide to toggle fullscreen. Move the pointer to reveal audience controls.],
  ), size: 16pt)
])

#page("Choose the PDF page layout", 4, [
  #lines((
    [*Auto:* checks page shape when a PDF loads.],
    [*Normal slides:* displays the whole PDF page.],
    [*Notes on right:* left half to the audience, right half as notes.],
    [A manual choice stays until you select Auto.],
  ), size: 17pt, gap: 15pt)
], "Check the result", [
  #lines((
    [This guide uses equal left and right halves. Auto selects Notes on right.],
    [Auto chooses that mode only when every visible page is at least 2.5 times wider than tall.],
    [It checks geometry, not note content. Unusual wide slides may need Normal slides.],
    [Try Normal slides to see both halves, then restore Auto.],
  ), size: 16pt)
])

#page("Open or replace a local PDF", 5, [
  #lines((
    [Click *Change PDF* to reveal the opener.],
    [Choose *Click or drop a PDF*, or drop one PDF into the Current slide panel.],
    [*Cancel* keeps your current deck and slide.],
    [A new valid PDF starts at slide 1 and resets Elapsed.],
  ), size: 17pt, gap: 14pt)
], "Try it without losing your place", [
  #lines((
    [Click Change PDF, then Cancel. This guide should remain on page 5.],
    [You can also drop a replacement directly into Current slide while presenting.],
    [Choose exactly one PDF, up to 64 MiB. An invalid file leaves the old deck in place.],
    [Finish the guide before opening your own deck.],
  ), size: 16pt)
])

#page("Load a PDF from a URL", 6, [
  #lines((
    [Change PDF → paste a PDF URL → *Open*.],
    [The source must allow cross-origin access (CORS) when it is on another site.],
    [Download limit: *64 MiB*. Timeout: *60 seconds*.],
  ), size: 17pt, gap: 14pt)
  #v(17pt)
  #text(size: 14pt)[An app link can open a PDF automatically:]
  #v(8pt)
  #text(size: 14pt, fill: accent)[?source=guide.pdf]
], "Encode the full source URL", [
  #lines((
    [Relative addresses resolve from the app. Use HTTPS when the app is served over HTTPS.],
    [Put a URL-encoded PDF address in ?source= so its query and fragment stay together.],
  ), size: 16pt, gap: 14pt)
  #v(14pt)
  #text(size: 11pt, fill: accent)[?source=https%3A%2F%2Fexample.org%2Ftalk.pdf%3Fa%3D1%26b%3D2]
  #v(14pt)
  #text(size: 15pt)[Loading contacts that PDF's server. It cannot bypass CORS or use a backend proxy.]
])

#page("Add an animated GIF", 7, [
  #text(size: 16pt)[One call embeds the GIF and its static first frame.]
  #v(14pt)
  #block(fill: rgb("#e4edf5"), inset: 12pt, radius: 6pt)[
    #text(size: 12pt)[#raw(
      "#import \"presenter-gif.typ\": gif\n#gif(path(\"orbit.gif\"), width: 6cm)",
      block: true)]
  ]
  #v(14pt)
  #link("https://github.com/woodruffw-trough/gebakuk/blob/main/docs/gif-protocol.md",
    text(size: 13pt, fill: accent)[Full recipe: docs/gif-protocol.md in the repository])
], "Use the helper and opaque frames", [
  #lines((
    [Use Typst 0.15.1. Copy presenter-gif.typ beside your source.],
    [path("orbit.gif") resolves beside your document. Use the same file on more slides; it attaches once.],
    [Keep the GIF opaque, rectangular, unrotated and at its original aspect ratio.],
    [No separate PNG or ID is needed. Conventional readers show the first frame. See docs/gif-protocol.md for details.],
  ), size: 16pt)
])

#page("Desktop browser support", 8, [
  #set text(size: 16pt)
  #table(
    columns: (240pt, 170pt, 170pt),
    inset: (x: 12pt, y: 7pt),
    stroke: 0.5pt + rgb("#c5d3df"),
    fill: (x, y) => if y == 0 { rgb("#e4edf5") } else { none },
    table.header([*Browser*], [*Minimum*], [*Year*]),
    [Chrome], [105+], [2022],
    [Edge], [105+], [2022],
    [Firefox], [121+], [2023],
    [Safari], [16.4+], [2023],
  )
  #v(12pt)
  #text(size: 12pt, fill: muted)[Feature-based minimums. Use current releases.]
  #v(6pt)
  #text(size: 12pt, fill: muted)[Year = release year of the minimum version.]
], "Before presenting your own deck", [
  #lines((
    [PDFs are processed locally and never uploaded. URL loading contacts the PDF source directly.],
    [Both windows receive the complete PDF. Hiding notes from the audience view does not make them secret.],
    [Keep the presenter open. If it closes, the audience keeps the last slide and reports disconnection.],
    [Use Change PDF to open your deck. Check the layout and audience display before presenting.],
  ), size: 16pt)
])
