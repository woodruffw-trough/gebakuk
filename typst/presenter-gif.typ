// Low-level helpers retained for existing presentations and explicit IDs.
#let valid-id(id) = {
  assert(type(id) == str, message: "GIF ID must be a string")
  assert(id.len() >= 1 and id.len() <= 128, message: "GIF ID must have 1–128 characters")
  assert(id.match(regex("^[A-Za-z0-9._-]+$")) != none,
    message: "GIF ID may contain only ASCII letters, digits, '.', '_' and '-'")
}

#let attach-gif(id, data) = {
  valid-id(id)
  // Typst exports the project-root-relative path as the attachment
  // name. An absolute virtual path fixes the name even from a subdirectory.
  pdf.attach("/" + id + ".gif", data, mime-type: "image/gif",
    description: "Presenter animation: " + id)
}

// Wrap one static image at the GIF's original aspect ratio. Transparent GIFs
// animate when the viewer can match this image to its marker.
#let gif-frame(id, body) = {
  valid-id(id)
  link("https://presenter.invalid/gif/v1/" + id, body)
}

// path(...) resolves the file beside the caller, even when this helper is elsewhere.
// Typst draws the GIF's first frame; Gebakuk displays its original animation.
#let gif-files = state("gebakuk/gif-files/v1", ())
#let gif(source, ..args) = context {
  assert(type(source) == path,
    message: "Pass path(\"animation.gif\") to gif")
  let files = gif-files.get()
  let index = files.position(file => file == source)
  if index == none {
    index = files.len()
    attach-gif("gebakuk-gif-" + str(index), read(source, encoding: none))
    gif-files.update(files => files + (source,))
  }
  gif-frame("gebakuk-gif-" + str(index), image(source, format: "gif", ..args))
}
