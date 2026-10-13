#![forbid(unsafe_code)]
//! Browser-independent PDF presentation geometry, rendering, and GIF extraction.

mod geometry;
mod gif;
mod pdf;
mod poster_filter;

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
    /// Whether playback may expose the slide underneath the GIF.
    pub has_transparency: bool,
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
    poster_filter: Option<poster_filter::PosterFilter>,
    animated_renderer: Option<(Layout, hayro::hayro_syntax::Pdf)>,
}

impl Presentation {
    /// Loads an untrusted PDF with bounded file, page, attachment, and GIF sizes.
    pub fn load(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(Error::new("PDF exceeds the 64 MiB file limit"));
        }
        let (mut pages, attachments, mut diagnostics, mut poster_filter) = pdf::parse(&bytes)?;
        let mut animated_renderer = None;
        if let Some(filter) = &poster_filter {
            let layout = geometry::detected_layout(&pages);
            match filter.renderer(&pages, layout) {
                Ok(renderer) => animated_renderer = Some((layout, renderer)),
                Err(error) => {
                    for page in &mut pages {
                        page.animations.retain(|overlay| {
                            !attachments[&overlay.attachment_id].has_transparency
                        });
                    }
                    if diagnostics.len() < 100 {
                        diagnostics.push(format!("Transparent animations remain static: {error}."));
                    }
                    poster_filter = None;
                }
            }
        }
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
            poster_filter,
            animated_renderer,
        })
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    /// Guesses a notes-on-right layout when every visible page is at least 2.5:1.
    ///
    /// This includes doubled 4:3 and 16:9 slides, while ordinary widescreen
    /// slides retain their full page. Uses the effective CropBox after rotation.
    /// Aspect ratio cannot prove that a document contains notes; callers should
    /// allow a manual layout override, especially for unusually wide slides.
    pub fn detected_layout(&self) -> Layout {
        geometry::detected_layout(&self.pages)
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

    /// Disable a failed browser animation and restore its original PDF poster.
    pub fn disable_animation(&mut self, id: &str) {
        for page in &mut self.pages {
            page.animations
                .retain(|overlay| overlay.attachment_id != id);
        }
        self.animated_renderer.take();
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
        self.render_using(&self.renderer, index, layout, view, width, height)
    }

    /// Render the slide background without the posters of supported transparent GIFs.
    ///
    /// The original document remains available through [`Self::render`], including
    /// static previews and the fallback after [`Self::disable_animation`]. A single
    /// filtered document is reused across page navigation, notes and resizing.
    pub fn render_with_animations(
        &mut self,
        index: usize,
        layout: Layout,
        view: View,
        width: u32,
        height: u32,
    ) -> Result<Raster, Error> {
        let contains_transparency = self
            .animations(index, layout, view)
            .iter()
            .any(|overlay| self.attachments[&overlay.attachment_id].has_transparency);
        let Some(filter) = self
            .poster_filter
            .as_ref()
            .filter(|_| contains_transparency)
        else {
            return self.render(index, layout, view, width, height);
        };
        if self
            .animated_renderer
            .as_ref()
            .is_none_or(|(cached_layout, _)| *cached_layout != layout)
        {
            match filter.renderer(&self.pages, layout) {
                Ok(renderer) => self.animated_renderer = Some((layout, renderer)),
                Err(error) => {
                    for page in &mut self.pages {
                        page.animations.retain(|overlay| {
                            !self.attachments[&overlay.attachment_id].has_transparency
                        });
                    }
                    self.poster_filter = None;
                    self.animated_renderer = None;
                    if self.diagnostics.len() < 100 {
                        self.diagnostics
                            .push(format!("Transparent animations remain static: {error}."));
                    }
                    return self.render(index, layout, view, width, height);
                }
            }
        }
        let renderer = &self
            .animated_renderer
            .as_ref()
            .expect("Filtered renderer was initialized")
            .1;
        self.render_using(renderer, index, layout, view, width, height)
    }

    fn render_using(
        &self,
        renderer: &hayro::hayro_syntax::Pdf,
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
        let page = &renderer.pages()[index];
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
