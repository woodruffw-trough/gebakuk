#import "theme.typ": slide
#set page(width: 640pt, height: 360pt, margin: 0pt)
#slide("Malformed marker", "No navigation and no crash.", [
  #link("https://presenter.invalid/gif/v1/bad%2Fid",
    image("../fixtures/assets/orbit.png", width: 160pt))
  #link("https://presenter.invalid/gif/v1/",
    image("../fixtures/assets/pulse.png", width: 160pt))
])
