#![forbid(unsafe_code)]
//! Browser-independent PDF presentation geometry, rendering, and GIF extraction.

mod geometry;
mod gif;
mod pdf;

pub use geometry::{PageInfo, Rect};
use std::{collections::BTreeMap, fmt};

/// Maximum accepted original PDF size (64 MiB).
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_RASTER_PIXELS: u64 = 8_000_000;
pub const MAX_RASTER_SIDE: u32 = 8192;
pub(crate) const MAX_ATTACHMENT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_TOTAL_ATTACHMENT_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_PAGES: usize = 2_000;
pub(crate) const MAX_GIF_DECODED_PIXELS: u64 = 128_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Normal,
    NotesRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Audience,
    Notes,
}

#[derive(Debug, Clone)]
pub struct Error(String);

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug)]
pub struct GifAttachment {
    pub bytes: Vec<u8>,
    pub width: u16,
    pub height: u16,
    pub(crate) decoded_frame_pixels: u64,
}

/// A supported GIF placement. `animations()` returns normalized region coordinates.
#[derive(Clone, Debug)]
pub struct Overlay {
    pub attachment_id: String,
    pub rect: Rect,
}

pub struct Raster {
    pub width: u32,
    pub height: u32,
    /// Straight-alpha RGBA8 pixels, row-major, with a white background.
    pub rgba: Vec<u8>,
}

/// A PDF parsed once for reuse across navigation and resizing.
pub struct Presentation {
    renderer: hayro::hayro_syntax::Pdf,
    pages: Vec<PageInfo>,
    attachments: BTreeMap<String, GifAttachment>,
    diagnostics: Vec<String>,
}

impl Presentation {
    /// Loads an untrusted PDF with bounded file, page, attachment, and GIF sizes.
    pub fn load(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(Error::new("PDF exceeds the 64 MiB file limit"));
        }
        let (pages, attachments, diagnostics) = pdf::parse(&bytes)?;
        let renderer = hayro::hayro_syntax::Pdf::new(bytes)
            .map_err(|error| Error::new(format!("Cannot open PDF renderer: {error:?}")))?;
        if renderer.pages().len() != pages.len() {
            return Err(Error::new("PDF parsers disagree on page count"));
        }
        Ok(Self {
            renderer,
            pages,
            attachments,
            diagnostics,
        })
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
    pub fn page(&self, index: usize) -> Result<&PageInfo, Error> {
        self.pages
            .get(index)
            .ok_or_else(|| Error::new("Slide number is outside this document"))
    }
    pub fn region(&self, index: usize, layout: Layout, view: View) -> Result<Rect, Error> {
        self.page(index)?.region(layout, view)
    }
    pub fn attachment(&self, id: &str) -> Option<&GifAttachment> {
        self.attachments.get(id)
    }

    /// Returns only fully contained, aspect-preserving overlays in the selected region.
    pub fn animations(&self, index: usize, layout: Layout, view: View) -> Vec<Overlay> {
        let Ok(page) = self.page(index) else {
            return Vec::new();
        };
        let Ok(region) = page.region(layout, view) else {
            return Vec::new();
        };
        page.animations
            .iter()
            .filter(|overlay| region.contains(overlay.rect))
            .map(|overlay| Overlay {
                attachment_id: overlay.attachment_id.clone(),
                rect: overlay.rect.normalized_in(region),
            })
            .collect()
    }

    /// Renders directly into the requested region-sized destination, without a full-page bitmap.
    pub fn render(
        &self,
        index: usize,
        layout: Layout,
        view: View,
        width: u32,
        height: u32,
    ) -> Result<Raster, Error> {
        let region = self.region(index, layout, view)?;
        if width == 0
            || height == 0
            || width > MAX_RASTER_SIDE
            || height > MAX_RASTER_SIDE
            || u64::from(width) * u64::from(height) > MAX_RASTER_PIXELS
        {
            return Err(Error::new(
                "Requested raster exceeds the 8 million pixel / 8192 pixel side limit",
            ));
        }
        use hayro::hayro_interpret::{InterpreterSettings, util::TransformExt};
        use hayro::kurbo::Affine;
        use hayro::vello_cpu::{
            Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit,
            color::palette::css::WHITE,
        };
        let page = &self.renderer.pages()[index];
        let (page_width, page_height) = page.render_dimensions();
        let info = self.page(index)?;
        if (f64::from(page_width) - info.width).abs() > 0.1
            || (f64::from(page_height) - info.height).abs() > 0.1
        {
            return Err(Error::new(
                "PDF render geometry does not match the visible page box",
            ));
        }
        let mut context = RenderContext::new(width as u16, height as u16);
        let transform = Affine::scale_non_uniform(
            f64::from(width) / region.width,
            f64::from(height) / region.height,
        ) * Affine::translate((-region.x, -region.y))
            * page.initial_transform(true).to_kurbo();
        let settings = InterpreterSettings {
            render_annotations: false,
            ..Default::default()
        };
        hayro::render_into(
            page,
            &hayro::RenderCache::new(),
            &settings,
            &hayro::RenderSettings::default(),
            &mut context,
            transform,
        );
        context.flush();
        let mut pixmap = Pixmap::new(width as u16, height as u16);
        context.render_with(
            &mut pixmap,
            &mut Resources::default(),
            RasterizerSettings {
                target_init: TargetInit::Clear(WHITE),
                ..Default::default()
            },
        );
        Ok(Raster {
            width,
            height,
            rgba: pixmap.data_as_u8_slice().to_vec(),
        })
    }
}
