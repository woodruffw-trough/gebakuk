//! Remove identified PDF poster draws, preserving the artwork behind transparent GIFs.
//!
//! This supports the ordinary image draws emitted by the Typst helper. It deliberately
//! retains static posters when forms, clipping, transforms or duplicate images make
//! the association ambiguous. The attachment marker alone is not sufficient evidence
//! that every image underneath its rectangle belongs to the animation.

use crate::{Error, GifAttachment, Layout, Overlay, PageInfo, Rect, View, pdf};
use hayro::{
    hayro_syntax::Pdf,
    kurbo::{Affine, Point},
};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, content::Content};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
};

const MAX_PAGE_CONTENT: usize = 32 * 1024 * 1024;
const MAX_PLANNED_CONTENT: usize = 32 * 1024 * 1024;
const MAX_FILTERED_PDF: usize = 64 * 1024 * 1024;
const MAX_OPERATIONS: usize = 250_000;
const MAX_CONTENT_TOKENS: usize = 1_000_000;
const MAX_GRAPHICS_DEPTH: usize = 128;

struct PagePlan {
    page: usize,
    page_id: ObjectId,
    content: Content,
    posters: Vec<(usize, Overlay)>,
}

pub(crate) struct PosterFilter {
    document: Document,
    plans: Vec<PagePlan>,
}

impl PosterFilter {
    pub(crate) fn prepare(
        document: Document,
        pages: &mut [PageInfo],
        attachments: &BTreeMap<String, GifAttachment>,
        diagnostics: &mut Vec<String>,
    ) -> Option<Self> {
        let mut plans = Vec::new();
        let page_ids: Vec<_> = document.page_iter().collect();
        let mut remaining_content = MAX_PLANNED_CONTENT;
        let mut remaining_operations = MAX_OPERATIONS;
        let mut remaining_tokens = MAX_CONTENT_TOKENS;
        for (index, page) in pages.iter_mut().enumerate() {
            let transparent: Vec<_> = page
                .animations
                .iter()
                .filter(|overlay| attachments[&overlay.attachment_id].has_transparency)
                .cloned()
                .collect();
            if transparent.is_empty() {
                continue;
            }
            let result = scan_page(
                &document,
                page_ids[index],
                page,
                &mut remaining_content,
                &mut remaining_operations,
                &mut remaining_tokens,
            );
            let mut posters = Vec::new();
            let mut disabled = Vec::new();
            for overlay in transparent {
                let matched = result.as_ref().ok().and_then(|(_, draws)| {
                    let mut matches = draws
                        .iter()
                        .filter(|draw| same_rect(draw.rect, overlay.rect));
                    let draw = matches.next()?;
                    if matches.next().is_some() || !draw.supported {
                        return None;
                    }
                    // A single image must not serve two overlapping animation markers.
                    if page
                        .animations
                        .iter()
                        .filter(|other| same_rect(other.rect, overlay.rect))
                        .count()
                        != 1
                    {
                        return None;
                    }
                    Some(draw.operation)
                });
                if let Some(operation) = matched {
                    posters.push((operation, overlay));
                } else {
                    if diagnostics.len() < 99 {
                        let detail = result
                            .as_ref()
                            .err()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| {
                                "the linked image is missing, transformed, clipped or ambiguous"
                                    .into()
                            });
                        diagnostics.push(format!(
                            "Slide {}: Transparent animation '{}' remains static: {detail}.",
                            index + 1,
                            overlay.attachment_id
                        ));
                    }
                    disabled.push(overlay);
                }
            }
            page.animations.retain(|overlay| {
                !disabled.iter().any(|other| {
                    other.attachment_id == overlay.attachment_id && other.rect == overlay.rect
                })
            });
            if !posters.is_empty() {
                let (content, _) = result.expect("Matched posters require a parsed page");
                plans.push(PagePlan {
                    page: index,
                    page_id: page_ids[index],
                    content,
                    posters,
                });
            }
        }
        if plans.is_empty() {
            None
        } else {
            Some(Self { document, plans })
        }
    }

    pub(crate) fn renderer(&self, pages: &[PageInfo], layout: Layout) -> Result<Pdf, Error> {
        let mut document = self.document.clone();
        for plan in &self.plans {
            let page = &pages[plan.page];
            let excluded: BTreeSet<_> = plan
                .posters
                .iter()
                .filter(|(_, overlay)| {
                    page.animations.iter().any(|active| {
                        active.attachment_id == overlay.attachment_id && active.rect == overlay.rect
                    })
                })
                .filter(|(_, overlay)| {
                    layout == Layout::Normal
                        || [View::Audience, View::Notes].into_iter().any(|view| {
                            page.region(layout, view)
                                .is_ok_and(|region| region.contains(overlay.rect))
                        })
                })
                .map(|(operation, _)| *operation)
                .collect();
            if excluded.is_empty() {
                continue;
            }
            let operations: Vec<_> = plan
                .content
                .operations
                .iter()
                .enumerate()
                .filter(|(index, _)| !excluded.contains(index))
                .map(|(_, operation)| operation.clone())
                .collect();
            let content = Content { operations }.encode().map_err(|error| {
                Error::new(format!("Cannot prepare animation background: {error}"))
            })?;
            let mut stream = Stream::new(Dictionary::new(), content);
            stream.compress().map_err(|error| {
                Error::new(format!("Cannot compress animation background: {error}"))
            })?;
            let stream_id = document.add_object(stream);
            // Page-local replacement avoids altering any shared original content stream.
            document
                .get_dictionary_mut(plan.page_id)
                .map_err(|_| Error::new("Cannot update animation page"))?
                .set("Contents", stream_id);
        }
        let mut output = BoundedPdf { bytes: Vec::new() };
        document
            .save_to(&mut output)
            .map_err(|error| Error::new(format!("Cannot prepare animation PDF: {error}")))?;
        let renderer = Pdf::new(output.bytes)
            .map_err(|error| Error::new(format!("Cannot open animation renderer: {error:?}")))?;
        if renderer.pages().len() != pages.len() {
            return Err(Error::new(
                "Animation renderer has an inconsistent page count",
            ));
        }
        Ok(renderer)
    }
}

struct BoundedPdf {
    bytes: Vec<u8>,
}
impl Write for BoundedPdf {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_FILTERED_PDF.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("Filtered PDF exceeds the 64 MiB limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ImageDraw {
    operation: usize,
    rect: Rect,
    supported: bool,
}
#[derive(Clone, Copy)]
struct GraphicsState {
    transform: Affine,
    clipped: bool,
    custom_graphics: bool,
}

fn scan_page(
    document: &Document,
    page_id: ObjectId,
    page: &PageInfo,
    remaining_content: &mut usize,
    remaining_operations: &mut usize,
    remaining_tokens: &mut usize,
) -> Result<(Content, Vec<ImageDraw>), Error> {
    let bytes = document
        .get_page_content_with_limit(page_id, MAX_PAGE_CONTENT.min(*remaining_content))
        .map_err(|error| {
            Error::new(format!(
                "the page content cannot be safely decoded ({error})"
            ))
        })?;
    *remaining_content = remaining_content.saturating_sub(bytes.len());
    // Count a conservative upper bound on tokens before lopdf materializes any
    // operations/operands. This catches millions of tiny instructions or arrays
    // in an otherwise small decoded stream without allocating their objects.
    let mut in_token = false;
    for byte in &bytes {
        let delimiter = b"()<>[]{}/%".contains(byte);
        let whitespace = byte.is_ascii_whitespace() || *byte == 0;
        if delimiter || (!whitespace && !in_token) {
            if *remaining_tokens == 0 {
                return Err(Error::new(
                    "the document exceeds the animation content-token limit",
                ));
            }
            *remaining_tokens -= 1;
        }
        in_token = !whitespace && !delimiter;
    }
    let content = Content::decode_strict(&bytes)
        .map_err(|_| Error::new("the page content cannot be safely parsed"))?;
    if content.operations.len() > *remaining_operations {
        return Err(Error::new(
            "the page exceeds the animation instruction limit",
        ));
    }
    *remaining_operations -= content.operations.len();
    let resources = pdf::inherited(document, page_id, b"Resources")?
        .ok_or_else(|| Error::new("the page has no image resources"))?;
    let resources = pdf::dictionary(document, resources)?;
    let xobjects = resources
        .get(b"XObject")
        .map_err(|_| Error::new("the page has no image objects"))?;
    let xobjects = pdf::dictionary(document, xobjects)?;
    let mut state = GraphicsState {
        transform: Affine::IDENTITY,
        clipped: false,
        custom_graphics: false,
    };
    let mut stack = Vec::new();
    let mut images = Vec::new();
    for (index, operation) in content.operations.iter().enumerate() {
        match operation.operator.as_str() {
            "q" if operation.operands.is_empty() => {
                if stack.len() == MAX_GRAPHICS_DEPTH {
                    return Err(Error::new("graphics state nesting is too deep"));
                }
                stack.push(state);
            }
            "Q" if operation.operands.is_empty() => {
                state = stack
                    .pop()
                    .ok_or_else(|| Error::new("graphics state restores are unbalanced"))?;
            }
            "cm" => {
                if operation.operands.len() != 6 {
                    return Err(Error::new("an image transform is malformed"));
                }
                let mut coefficients = [0.0; 6];
                for (coefficient, object) in coefficients.iter_mut().zip(&operation.operands) {
                    *coefficient = match object {
                        Object::Integer(value) => *value as f64,
                        Object::Real(value) => f64::from(*value),
                        _ => return Err(Error::new("an image transform is malformed")),
                    };
                }
                state.transform *= Affine::new(coefficients);
                if !state
                    .transform
                    .as_coeffs()
                    .iter()
                    .all(|value| value.is_finite())
                {
                    return Err(Error::new("an image transform is non-finite"));
                }
            }
            "W" | "W*" => state.clipped = true,
            "gs" => state.custom_graphics = true,
            "Do" => {
                let [name] = operation.operands.as_slice() else {
                    return Err(Error::new("an image invocation is malformed"));
                };
                let name = name
                    .as_name()
                    .map_err(|_| Error::new("an image name is malformed"))?;
                let object = xobjects
                    .get(name)
                    .map_err(|_| Error::new("an image resource is missing"))?;
                let object = pdf::resolved(document, object)?;
                let stream = object
                    .as_stream()
                    .map_err(|_| Error::new("an image object is malformed"))?;
                let subtype = stream.dict.get(b"Subtype").and_then(Object::as_name).ok();
                if subtype == Some(b"Form") {
                    return Err(Error::new(
                        "the page uses form objects whose poster placement cannot be identified",
                    ));
                }
                if subtype != Some(b"Image") {
                    return Err(Error::new("an image object has an unsupported type"));
                }
                let [a, b, c, d, _, _] = state.transform.as_coeffs();
                let p = state.transform * Point::new(0.0, 0.0);
                let q = state.transform * Point::new(1.0, 1.0);
                // A browser img overlay cannot reproduce a mirrored, rotated or sheared poster.
                let supported = b.abs() <= 1e-6
                    && c.abs() <= 1e-6
                    && a > 0.0
                    && d > 0.0
                    && !state.clipped
                    && !state.custom_graphics;
                let rect = page.pdf_rect_to_display([
                    p.x.min(q.x),
                    p.y.min(q.y),
                    p.x.max(q.x),
                    p.y.max(q.y),
                ])?;
                images.push(ImageDraw {
                    operation: index,
                    rect,
                    supported,
                });
            }
            "q" | "Q" => return Err(Error::new("graphics state instructions are malformed")),
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(Error::new("graphics state saves are unbalanced"));
    }
    Ok((content, images))
}

fn same_rect(a: Rect, b: Rect) -> bool {
    [a.x - b.x, a.y - b.y, a.width - b.width, a.height - b.height]
        .iter()
        .all(|difference| difference.abs() <= 0.01)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn tiny_instructions_are_rejected_before_materializing_operations() {
        let mut document = Document::new();
        let content = document.add_object(Stream::new(Dictionary::new(), b"q ".repeat(1000)));
        let page_id = document.add_object(dictionary! {
            "Contents" => content,
            "Resources" => dictionary! { "XObject" => dictionary! {} },
        });
        let page = PageInfo::new([0.0, 0.0, 640.0, 360.0], 0).unwrap();
        let mut bytes = 4096;
        let mut operations = 250_000;
        let mut tokens = 10;
        let result = scan_page(
            &document,
            page_id,
            &page,
            &mut bytes,
            &mut operations,
            &mut tokens,
        );
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("content-token limit")
        );
        assert_eq!(
            operations, 250_000,
            "The parsed-operation budget must remain untouched"
        );
    }
}
