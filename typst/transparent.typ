#import "presenter-gif.typ": attach-gif, gif-frame
#import "theme.typ": slide
#set page(width: 640pt, height: 360pt, margin: 0pt)
#attach-gif("transparent", read("../fixtures/assets/transparent.gif", encoding: none))
#slide("Transparent GIF safety check", "The original static image should stay visible.", [
  #gif-frame("transparent", image("../fixtures/assets/transparent.png", width: 320pt))
  #v(8pt)
  #text(size: 12pt)[The presenter should report that transparency is unsupported.]
])
