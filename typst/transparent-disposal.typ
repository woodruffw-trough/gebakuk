#import "presenter-gif.typ": gif
#import "transparency/pattern.typ": transparency-background
#set page(width: 640pt, height: 360pt, margin: 0pt)
#set text(font: "DejaVu Sans Mono", size: 16pt)
#set document(title: "Transparent partial GIF frames and local palettes")
#place(dx: 40pt, dy: 20pt, [Disposal 2: restore transparent background])
#transparency-background(40pt, 60pt)
#place(dx: 40pt, dy: 60pt,
  gif(path("../fixtures/assets/transparent-background.gif"), width: 320pt))
#place(dx: 40pt, dy: 270pt, [Orange → cyan → magenta, without old circles.])
#place(dx: 40pt, dy: 300pt, [Later frames use offset rectangles and local palettes.])

#pagebreak()
#place(dx: 40pt, dy: 20pt, [Disposal 3: restore previous transparent canvas])
#transparency-background(40pt, 60pt)
#place(dx: 40pt, dy: 60pt,
  gif(path("../fixtures/assets/transparent-previous.gif"), width: 320pt))
#place(dx: 40pt, dy: 270pt, [Cyan → magenta → cyan restored, plus purple.])
#place(dx: 40pt, dy: 300pt, [Previous disposal restores the covered cyan circle.])
