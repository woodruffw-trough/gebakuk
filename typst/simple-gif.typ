#import "presenter-gif.typ": gif
#set page(width: 640pt, height: 360pt, margin: 0pt)
#set document(title: "One-call GIF helper and canonical attachment reuse")

// Match the low-level single-gif fixture's first-frame geometry exactly.
#place(dx: 40pt, dy: 60pt,
  gif(path("../fixtures/assets/orbit.gif"), width: 320pt))

#pagebreak()
#place(dx: 40pt, dy: 60pt,
  gif(path("../fixtures/assets/orbit.gif"), width: 240pt))
#place(dx: 340pt, dy: 60pt,
  gif(path("../fixtures/assets/pulse.gif"), width: 240pt))

#pagebreak()
// Root-absolute aliases must reuse the same two canonical attachments.
#place(dx: 40pt, dy: 60pt,
  gif(path("/fixtures/assets/orbit.gif"), width: 240pt))
#place(dx: 340pt, dy: 60pt,
  gif(path("/fixtures/assets/pulse.gif"), width: 240pt))
