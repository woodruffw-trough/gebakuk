#![forbid(unsafe_code)]

//! Minimal browser feasibility proof for the PDF core and WASM renderer.

use presenter_core::{Layout, Presentation, Raster, View};
use wasm_bindgen::{Clamped, JsCast, prelude::*};
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement, ImageData};

/// Render a complete fixture page and both notes-right regions in the browser.
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let dom = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("No browser document"))?;
    let mount = dom
        .get_element_by_id("app")
        .ok_or_else(|| JsValue::from_str("Missing app mount"))?;
    mount.set_inner_html(
        "<h1>Gebakuk · WASM feasibility proof</h1>\
         <p>Pure-Rust PDF rendering: complete page, audience half, and notes half.</p>\
         <h2>Complete double-width page</h2><canvas id=\"complete\"></canvas>\
         <div class=\"regions\"><section><h2>Audience half</h2>\
         <canvas id=\"audience\"></canvas></section><section><h2>Notes half</h2>\
         <canvas id=\"notes\"></canvas></section></div><p id=\"extraction\"></p>",
    );
    let pdf = Presentation::load(include_bytes!("../../../fixtures/notes-right.pdf").to_vec())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    for (id, layout, view, width) in [
        ("complete", Layout::Normal, View::Audience, 1280),
        ("audience", Layout::NotesRight, View::Audience, 640),
        ("notes", Layout::NotesRight, View::Notes, 640),
    ] {
        let raster = pdf
            .render(0, layout, view, width, 360)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        draw(&dom, id, &raster)?;
    }
    let gif = pdf
        .attachment("orbit")
        .ok_or_else(|| JsValue::from_str("Fixture GIF attachment not found"))?;
    if gif.bytes != include_bytes!("../../../fixtures/assets/orbit.gif") {
        return Err(JsValue::from_str("Extracted GIF differs from the original"));
    }
    let overlays = pdf.animations(0, Layout::NotesRight, View::Audience);
    let overlay = overlays
        .first()
        .ok_or_else(|| JsValue::from_str("Fixture GIF rectangle not found"))?;
    let region = pdf
        .region(0, Layout::NotesRight, View::Audience)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let status = format!(
        "Recovered GIF byte-for-byte ({} bytes). Audience rectangle: x {:.0}, y {:.0}, width {:.0}, height {:.0} PDF points.",
        gif.bytes.len(),
        overlay.rect.x * region.width,
        overlay.rect.y * region.height,
        overlay.rect.width * region.width,
        overlay.rect.height * region.height,
    );
    dom.get_element_by_id("extraction")
        .ok_or_else(|| JsValue::from_str("Missing extraction status"))?
        .set_text_content(Some(&status));
    Ok(())
}

fn draw(dom: &Document, id: &str, raster: &Raster) -> Result<(), JsValue> {
    let canvas = dom
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str("Missing proof canvas"))?
        .dyn_into::<HtmlCanvasElement>()?;
    canvas.set_width(raster.width);
    canvas.set_height(raster.height);
    let context = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("Canvas rendering is unavailable"))?
        .dyn_into::<CanvasRenderingContext2d>()?;
    let pixels = ImageData::new_with_u8_clamped_array_and_sh(
        Clamped(&raster.rgba),
        raster.width,
        raster.height,
    )?;
    context.put_image_data(&pixels, 0.0, 0.0)
}
