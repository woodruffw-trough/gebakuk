#import "theme.typ": slide
#set page(width: 640pt, height: 360pt, margin: 0pt)
#set document(title: "120-slide resource fixture")
#for i in range(120) {
  if i > 0 { pagebreak() }
  slide("Slide " + str(i + 1) + " / 120", "Navigate without retaining every rasterized page.", [
    #text(size: 64pt)[#(i + 1)]
    #v(12pt)
    #rect(width: 420pt, height: 20pt,
      fill: rgb(i * 2, 120, 180))
  ])
}
