#import "presenter-gif.typ": attach-gif, gif-frame
#set page(width: 640pt, height: 360pt, margin: 0pt)
#set document(title: "One opaque animation at a known rectangle")
#attach-gif("orbit", read("../fixtures/assets/orbit.gif", encoding: none))
#place(dx: 40pt, dy: 60pt,
  gif-frame("orbit", image("../fixtures/assets/orbit.png", width: 320pt, height: 180pt)))
