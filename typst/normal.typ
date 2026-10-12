#import "presenter-gif.typ": attach-gif, gif-frame
#import "theme.typ": slide
#set page(width: 640pt, height: 360pt, margin: 0pt)
#set document(title: "PDF Presenter — Normal layout")
#attach-gif("orbit", read("../fixtures/assets/orbit.gif", encoding: none))
#attach-gif("pulse", read("../fixtures/assets/pulse.gif", encoding: none))

#slide("Motion inside a PDF", "Original GIF bytes travel with the document.", [
  #gif-frame("orbit", image("../fixtures/assets/orbit.png", width: 320pt))
  #v(8pt)
  #text(size: 12pt)[A conventional PDF reader displays this static first frame.]
])
#pagebreak()
#slide("Two animations, one attachment reused", "Navigate back to see the same orbit attachment again.", [
  #grid(columns: (240pt, 240pt), gutter: 30pt,
    gif-frame("orbit", image("../fixtures/assets/orbit.png", width: 240pt)),
    gif-frame("pulse", image("../fixtures/assets/pulse.png", width: 240pt)))
])
#pagebreak()
#slide("Ready to present", "This slide contains no animation.", [
  - Open an audience window.
  - Use arrow keys, Page Up / Down, Home and End.
  - Enter fullscreen from the audience controls.
])
