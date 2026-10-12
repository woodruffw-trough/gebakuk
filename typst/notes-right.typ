#import "presenter-gif.typ": attach-gif, gif-frame
#import "theme.typ": slide, notes
#set page(width: 1280pt, height: 360pt, margin: 0pt)
#set document(title: "PDF Presenter — Notes on the right")
#attach-gif("orbit", read("../fixtures/assets/orbit.gif", encoding: none))
#attach-gif("pulse", read("../fixtures/assets/pulse.gif", encoding: none))

#grid(columns: (640pt, 640pt), gutter: 0pt,
  slide("Keep your notes beside your slides", "Auto detects this notes-right layout.", [
    #gif-frame("orbit", image("../fixtures/assets/orbit.png", width: 320pt))
    #v(8pt)
    #text(size: 12pt)[The audience sees only this left half.]
  ]),
  notes("Introduce the demo", [
    - Explain that the PDF stays on this device.
    - Point out the animated orbit on the audience screen.
    - Notes are in the original PDF; they are not secret.
  ]))
#pagebreak()
#grid(columns: (640pt, 640pt), gutter: 0pt,
  slide("Independent animations", "One attachment can appear on several slides.", [
    #grid(columns: (240pt, 240pt), gutter: 30pt,
      gif-frame("orbit", image("../fixtures/assets/orbit.png", width: 240pt)),
      gif-frame("pulse", image("../fixtures/assets/pulse.png", width: 240pt)))
  ]),
  notes("Try the presenter controls", [
    - Advance and return with the keyboard.
    - Resize either window to check alignment.
    - Close and reopen the audience window.
  ]))
#pagebreak()
#grid(columns: (640pt, 640pt), gutter: 0pt,
  slide("The whole presentation is a PDF", "It still works in conventional PDF readers.", [
    - Static images remain visible everywhere.
    - The viewer recognizes versioned animation links.
    - No service receives the document.
  ]),
  notes("Finish", [
    Thank the audience. The final slide has no animation, so navigation also
    checks that earlier animations stop and their resources are released.
  ]))
