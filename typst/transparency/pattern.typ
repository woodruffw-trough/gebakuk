// Shared vector artwork: colors remain visible through transparent GIF pixels.
#let transparency-background(x, y) = {
  for row in range(6) {
    for column in range(8) {
      place(dx: x + column * 40pt, dy: y + row * 30pt,
        rect(width: 40pt, height: 30pt,
          fill: if calc.rem(row + column, 2) == 0 {
            rgb("#123c69")
          } else {
            rgb("#38b6a5")
          }, stroke: none))
    }
  }
}
