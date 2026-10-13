#import "presenter-gif.typ": gif
#import "transparency/pattern.typ": transparency-background
#set page(width: 640pt, height: 360pt, margin: 0pt)
#set text(font: "DejaVu Sans Mono", size: 16pt)
#set document(title: "Transparent animation over patterned PDF artwork")
#place(dx: 40pt, dy: 20pt, [Transparent GIF over PDF artwork])
#transparency-background(40pt, 60pt)
#place(dx: 40pt, dy: 60pt,
  gif(path("../fixtures/assets/transparent.gif"), width: 320pt))
#place(dx: 40pt, dy: 270pt, [The orange dot must move without leaving a ghost.])
#place(dx: 40pt, dy: 300pt, [The blue and green checkerboard must remain visible.])
