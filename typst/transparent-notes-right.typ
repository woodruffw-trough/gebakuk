#import "presenter-gif.typ": gif
#import "transparency/pattern.typ": transparency-background
#set page(width: 1280pt, height: 360pt, margin: 0pt)
#set text(font: "DejaVu Sans Mono", size: 16pt)
#set document(title: "Transparent GIFs in both notes-right regions")
#place(dx: 40pt, dy: 20pt, [Audience half: moving transparent dot])
#transparency-background(40pt, 60pt)
#place(dx: 40pt, dy: 60pt,
  gif(path("../fixtures/assets/transparent.gif"), width: 320pt))
#place(dx: 40pt, dy: 270pt, [Only this half belongs on the audience display.])

#place(dx: 680pt, dy: 20pt, [Notes half: partial frames, previous disposal])
#transparency-background(680pt, 60pt)
#place(dx: 680pt, dy: 60pt,
  gif(path("../fixtures/assets/transparent-previous.gif"), width: 320pt))
#place(dx: 680pt, dy: 270pt, [Both regions retain their original PDF artwork.])
