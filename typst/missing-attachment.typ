#import "presenter-gif.typ": gif-frame
#import "theme.typ": slide
#set page(width: 640pt, height: 360pt, margin: 0pt)
#slide("Missing attachment", "The static slide must still render.", [
  #gif-frame("absent", image("../fixtures/assets/orbit.png", width: 320pt))
])
