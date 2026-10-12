#import "theme.typ": slide
#set page(width: 640pt, height: 360pt, margin: 0pt)
#set document(title: "Ordinary PDF without attachments")
#slide("An ordinary PDF", "No animation markers or embedded attachments.", [
  #rect(width: 210pt, height: 100pt, radius: 12pt, fill: rgb("#c5eaf3"))
  #v(12pt)
  Text and graphics should remain aligned when resized.
])
#pagebreak()
#slide("A second page", "The last page tests End navigation.", [
  #text(size: 44pt)[2]
])
