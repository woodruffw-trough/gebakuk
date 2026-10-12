#let ink = rgb("#152a40")
#let accent = rgb("#0e7490")
#let paper = rgb("#f4f8fb")

#let slide(title, subtitle, body) = block(width: 640pt, height: 360pt, fill: paper)[
  #set text(font: "Libertinus Serif", fill: ink, size: 18pt)
  #place(dx: 30pt, dy: 30pt,
    text(font: "DejaVu Sans Mono", size: 10pt, fill: accent)[PDF PRESENTER / DEMO])
  #place(dx: 30pt, dy: 54pt, text(size: 30pt, weight: "bold", title))
  #place(dx: 30pt, dy: 98pt, text(size: 15pt, subtitle))
  #place(dx: 30pt, dy: 136pt, block(width: 580pt)[#body])
]

#let notes(title, body) = block(width: 640pt, height: 360pt, fill: rgb("#e2eaf1"))[
  #set text(font: "Libertinus Serif", fill: ink, size: 20pt)
  #place(dx: 30pt, dy: 30pt,
    text(font: "DejaVu Sans Mono", size: 10pt, fill: accent)[SPEAKER NOTES])
  #place(dx: 30pt, dy: 58pt, text(size: 28pt, weight: "bold", title))
  #place(dx: 30pt, dy: 118pt, block(width: 580pt)[#body])
]
