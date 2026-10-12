// Each ID maps to one root-level PDF attachment named exactly `<id>.gif`.
// Pass read("path/to/original.gif", encoding: none) from the calling document to avoid
// resolving source paths relative to this imported helper.
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

// Wrap an ordinary static image. Use its original aspect ratio and an opaque
// GIF: transparent animations intentionally remain static in the presenter.
#let gif-frame(id, body) = {
  valid-id(id)
  link("https://presenter.invalid/gif/v1/" + id, body)
}
