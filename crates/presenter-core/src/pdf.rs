use crate::{
    Error, GifAttachment, MAX_ATTACHMENT_BYTES, MAX_GIF_DECODED_PIXELS, MAX_PAGES,
    MAX_TOTAL_ATTACHMENT_BYTES, Overlay, PageInfo, Rect, gif::validate_gif_with_budget,
    poster_filter::PosterFilter,
};
use lopdf::{Dictionary, Document, LoadOptions, Object, ObjectId, Stream};
use std::collections::{BTreeMap, BTreeSet};

const MARKER_PREFIX: &str = "https://presenter.invalid/gif/v1/";
const MAX_TREE_NODES: usize = 4096;
const MAX_ANNOTATIONS_PER_PAGE: usize = 4096;
const MAX_OVERLAYS_PER_PAGE: usize = 64;

#[derive(Default)]
struct Diagnostics(Vec<String>);
impl Diagnostics {
    fn push(&mut self, message: String) {
        if self.0.len() < 99 {
            self.0.push(message);
        } else if self.0.len() == 99 {
            self.0.push("Further PDF diagnostics were omitted.".into());
        }
    }
    fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter()
    }
}

pub(crate) type Parsed = (
    Vec<PageInfo>,
    BTreeMap<String, GifAttachment>,
    Vec<String>,
    Option<PosterFilter>,
);

pub(crate) fn parse(bytes: &[u8]) -> Result<Parsed, Error> {
    let doc = Document::load_mem_with_options(
        bytes,
        LoadOptions::with_max_decompressed_size(32 * 1024 * 1024),
    )
    .map_err(|error| Error::new(format!("Cannot read PDF: {error}")))?;
    if doc.trailer.has(b"Encrypt") {
        return Err(Error::new("Encrypted PDFs are not supported"));
    }
    if doc.objects.len() > 200_000 {
        return Err(Error::new("PDF exceeds the object-count limit"));
    }
    validate_resources(&doc)?;
    let ids: Vec<_> = doc.page_iter().take(MAX_PAGES + 1).collect();
    if ids.is_empty() || ids.len() > MAX_PAGES {
        return Err(Error::new("PDF must contain between 1 and 2000 pages"));
    }
    let mut diagnostics = Diagnostics::default();
    let attachments = extract_attachments(&doc, &mut diagnostics);
    let mut pages = Vec::with_capacity(ids.len());
    for id in ids {
        let media = inherited(&doc, id, b"MediaBox")?
            .ok_or_else(|| Error::new("PDF page has no MediaBox"))?;
        let media = rectangle(&doc, media)?;
        let crop = match inherited(&doc, id, b"CropBox")? {
            Some(value) => rectangle(&doc, value)?,
            None => media,
        };
        let bounds = [
            media[0].max(crop[0]),
            media[1].max(crop[1]),
            media[2].min(crop[2]),
            media[3].min(crop[3]),
        ];
        let rotation = match inherited(&doc, id, b"Rotate")? {
            Some(value) => resolved(&doc, value)?
                .as_i64()
                .map_err(|_| Error::new("Invalid PDF rotation"))?,
            None => 0,
        };
        let mut page = PageInfo::new(bounds, rotation)?;
        page.animations =
            extract_annotations(&doc, id, &page, &attachments, pages.len(), &mut diagnostics);
        if let Some(first) = pages.first() {
            let first: &PageInfo = first;
            if ((page.width - first.width).abs() > 0.01
                || (page.height - first.height).abs() > 0.01)
                && !diagnostics
                    .iter()
                    .any(|message| message.starts_with("Page dimensions vary"))
            {
                diagnostics.push(
                    "Page dimensions vary; each slide will be fitted using its own geometry."
                        .into(),
                );
            }
        }
        pages.push(page);
    }
    let poster_filter = PosterFilter::prepare(doc, &mut pages, &attachments, &mut diagnostics.0);
    Ok((pages, attachments, diagnostics.0, poster_filter))
}

pub(crate) fn resolved<'a>(doc: &'a Document, object: &'a Object) -> Result<&'a Object, Error> {
    doc.dereference(object)
        .map(|(_, value)| value)
        .map_err(|error| Error::new(format!("Invalid PDF object reference: {error}")))
}

pub(crate) fn dictionary<'a>(
    doc: &'a Document,
    object: &'a Object,
) -> Result<&'a Dictionary, Error> {
    resolved(doc, object)?
        .as_dict()
        .map_err(|_| Error::new("Expected a PDF dictionary"))
}

pub(crate) fn inherited<'a>(
    doc: &'a Document,
    mut id: ObjectId,
    key: &[u8],
) -> Result<Option<&'a Object>, Error> {
    let mut visited = BTreeSet::new();
    for _ in 0..100 {
        if !visited.insert(id) {
            return Err(Error::new("Cyclic PDF page inheritance"));
        }
        let dict = doc
            .get_dictionary(id)
            .map_err(|_| Error::new("Invalid PDF page dictionary"))?;
        if let Ok(value) = dict.get(key) {
            return Ok(Some(value));
        }
        match dict.get(b"Parent").and_then(Object::as_reference) {
            Ok(parent) => id = parent,
            Err(_) => return Ok(None),
        }
    }
    Err(Error::new("PDF page inheritance is excessively deep"))
}

fn number(doc: &Document, value: &Object) -> Result<f64, Error> {
    match resolved(doc, value)? {
        Object::Integer(value) => Ok(*value as f64),
        Object::Real(value) => Ok(f64::from(*value)),
        _ => Err(Error::new("Expected a numeric PDF coordinate")),
    }
}

fn rectangle(doc: &Document, value: &Object) -> Result<[f64; 4], Error> {
    let values = resolved(doc, value)?
        .as_array()
        .map_err(|_| Error::new("Invalid PDF rectangle"))?;
    if values.len() != 4 {
        return Err(Error::new("PDF rectangle must have four coordinates"));
    }
    let a = [
        number(doc, &values[0])?,
        number(doc, &values[1])?,
        number(doc, &values[2])?,
        number(doc, &values[3])?,
    ];
    if !a.iter().all(|value| value.is_finite()) {
        return Err(Error::new("PDF rectangle has a non-finite coordinate"));
    }
    Ok([
        a[0].min(a[2]),
        a[1].min(a[3]),
        a[0].max(a[2]),
        a[1].max(a[3]),
    ])
}

fn text(doc: &Document, value: &Object) -> Result<String, Error> {
    let value = resolved(doc, value)?
        .as_str()
        .map_err(|_| Error::new("Expected a PDF string"))?;
    if value.starts_with(&[0xfe, 0xff]) {
        if value.len() % 2 != 0 {
            return Err(Error::new("Invalid UTF-16 PDF string"));
        }
        let units: Vec<_> = value[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16(&units).map_err(|_| Error::new("Invalid UTF-16 PDF string"))
    } else {
        String::from_utf8(value.to_vec())
            .map_err(|_| Error::new("Attachment or marker name is not UTF-8"))
    }
}

pub(crate) fn marker_id(uri: &str) -> Result<Option<&str>, Error> {
    if !uri.starts_with("https://presenter.invalid/gif/") {
        return Ok(None);
    }
    let id = uri
        .strip_prefix(MARKER_PREFIX)
        .ok_or_else(|| Error::new("Unsupported or malformed animation marker version"))?;
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(Error::new("Malformed animation marker identifier"));
    }
    Ok(Some(id))
}

fn extract_attachments(
    doc: &Document,
    diagnostics: &mut Diagnostics,
) -> BTreeMap<String, GifAttachment> {
    let mut output = BTreeMap::new();
    let Ok(catalog) = doc.catalog() else {
        return output;
    };
    let Ok(names) = catalog
        .get(b"Names")
        .map_err(|_| Error::new("No name tree"))
        .and_then(|object| dictionary(doc, object))
    else {
        return output;
    };
    let Ok(root) = names.get(b"EmbeddedFiles") else {
        return output;
    };
    let mut pending = vec![(root, 0_usize)];
    let mut visited = BTreeSet::new();
    let mut nodes = 0;
    let mut names_seen = BTreeSet::new();
    let mut total_bytes = 0;
    let mut total_frame_pixels = 0;
    let mut remaining_validation_pixels = MAX_GIF_DECODED_PIXELS;
    let mut entries_count = 0;
    while let Some((node, depth)) = pending.pop() {
        nodes += 1;
        if nodes > MAX_TREE_NODES || depth > 64 {
            diagnostics.push("Embedded-file name tree exceeds traversal limits.".into());
            break;
        }
        if let Object::Reference(id) = node
            && !visited.insert(*id)
        {
            diagnostics.push("Repeated or cyclic embedded-file name tree node was skipped.".into());
            continue;
        }
        let Ok(node) = dictionary(doc, node) else {
            diagnostics.push("Malformed embedded-file name tree node was skipped.".into());
            continue;
        };
        if let Ok(children) = node.get(b"Kids") {
            match resolved(doc, children).and_then(|value| {
                value
                    .as_array()
                    .map_err(|_| Error::new("Invalid name tree children"))
            }) {
                Ok(children) => {
                    if children.len() > MAX_TREE_NODES {
                        diagnostics.push("Embedded-file name tree has too many children.".into());
                        continue;
                    }
                    pending.extend(children.iter().rev().map(|child| (child, depth + 1)));
                }
                Err(error) => diagnostics.push(error.to_string()),
            }
        }
        let Ok(entries) = node.get(b"Names") else {
            continue;
        };
        let Ok(entries) = resolved(doc, entries).and_then(|value| {
            value
                .as_array()
                .map_err(|_| Error::new("Invalid attachment names"))
        }) else {
            diagnostics.push("Malformed embedded-file name pairs were skipped.".into());
            continue;
        };
        if entries.len() > MAX_TREE_NODES * 2 || entries.len() % 2 != 0 {
            diagnostics.push("Invalid or excessive embedded-file name pairs were skipped.".into());
            continue;
        }
        entries_count += entries.len() / 2;
        if entries_count > MAX_TREE_NODES {
            diagnostics.push("Embedded-file name tree exceeds the attachment-entry limit.".into());
            break;
        }
        for pair in entries.as_chunks::<2>().0 {
            let Ok(name) = text(doc, &pair[0]) else {
                diagnostics.push("Invalid attachment name was skipped.".into());
                continue;
            };
            let Some(id) = name.strip_suffix(".gif") else {
                continue;
            };
            if marker_id(&format!("{MARKER_PREFIX}{id}")).is_err() {
                continue;
            }
            if !names_seen.insert(id.to_owned()) {
                output.remove(id);
                diagnostics.push(format!("Duplicate GIF identifier '{id}' was disabled."));
                continue;
            }
            let attachment = (|| {
                let spec = dictionary(doc, &pair[1])?;
                let ef = dictionary(
                    doc,
                    spec.get(b"EF").map_err(|_| {
                        Error::new("Attachment file specification has no embedded data")
                    })?,
                )?;
                let stream = ef
                    .get(b"UF")
                    .or_else(|_| ef.get(b"F"))
                    .map_err(|_| Error::new("Attachment has no embedded-file stream"))?;
                let stream = resolved(doc, stream)?
                    .as_stream()
                    .map_err(|_| Error::new("Attachment data is not a stream"))?;
                let remaining_bytes = MAX_TOTAL_ATTACHMENT_BYTES - total_bytes;
                if remaining_bytes == 0 {
                    return Err(Error::new("GIF attachments exceed the total 32 MiB limit"));
                }
                let bytes =
                    bounded_content(doc, stream, MAX_ATTACHMENT_BYTES.min(remaining_bytes))?;
                // Failed validation also spends the shared budgets.
                total_bytes += bytes.len();
                validate_gif_with_budget(bytes, &mut remaining_validation_pixels)
            })();
            match attachment {
                Ok(attachment) => {
                    if total_frame_pixels + attachment.decoded_frame_pixels > MAX_GIF_DECODED_PIXELS
                    {
                        diagnostics.push(format!("GIF '{id}' exceeds the total 128 million decoded GIF frame-pixel limit and was disabled."));
                        continue;
                    }
                    total_frame_pixels += attachment.decoded_frame_pixels;
                    output.insert(id.to_owned(), attachment);
                }
                Err(error) => diagnostics.push(format!("GIF '{id}': {error}")),
            }
        }
    }
    output
}

fn extract_annotations(
    doc: &Document,
    id: ObjectId,
    page: &PageInfo,
    attachments: &BTreeMap<String, GifAttachment>,
    index: usize,
    diagnostics: &mut Diagnostics,
) -> Vec<Overlay> {
    let mut overlays = Vec::new();
    let Ok(dict) = doc.get_dictionary(id) else {
        return overlays;
    };
    let Ok(annots) = dict.get(b"Annots") else {
        return overlays;
    };
    let Ok(annots) = resolved(doc, annots).and_then(|value| {
        value
            .as_array()
            .map_err(|_| Error::new("Invalid annotation array"))
    }) else {
        diagnostics.push(format!("Slide {} has malformed annotations.", index + 1));
        return overlays;
    };
    if annots.len() > MAX_ANNOTATIONS_PER_PAGE {
        diagnostics.push(format!("Slide {} exceeds the annotation limit.", index + 1));
        return overlays;
    }
    for annotation in annots {
        let result = (|| {
            let dict = dictionary(doc, annotation)?;
            if dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Link") {
                return Ok(None);
            }
            let action = match dict.get(b"A") {
                Ok(value) => dictionary(doc, value)?,
                Err(_) => return Ok(None),
            };
            if action.get(b"S").and_then(Object::as_name).ok() != Some(b"URI") {
                return Ok(None);
            }
            let uri = text(
                doc,
                action
                    .get(b"URI")
                    .map_err(|_| Error::new("Link has no URI"))?,
            )?;
            let Some(id) = marker_id(&uri)? else {
                return Ok(None);
            };
            let gif = attachments.get(id).ok_or_else(|| {
                Error::new(format!(
                    "Animation refers to missing or unsupported GIF '{id}'"
                ))
            })?;
            if page.rotation != 0 {
                return Err(Error::new(
                    "GIF overlays on rotated pages are not supported; static image remains visible",
                ));
            }
            if dict.has(b"QuadPoints") {
                return Err(Error::new(
                    "GIF links with transformed QuadPoints are not supported",
                ));
            }
            let bounds = rectangle(
                doc,
                dict.get(b"Rect")
                    .map_err(|_| Error::new("Animation link has no rectangle"))?,
            )?;
            let rect = page.pdf_rect_to_display(bounds)?;
            let visible = Rect {
                x: 0.0,
                y: 0.0,
                width: page.width,
                height: page.height,
            };
            if !visible.contains(rect) {
                return Err(Error::new("Animation extends outside the visible page"));
            }
            let ratio = rect.width / rect.height;
            let gif_ratio = f64::from(gif.width) / f64::from(gif.height);
            if (ratio / gif_ratio - 1.0).abs() > 0.02 {
                return Err(Error::new(
                    "Animation rectangle does not preserve the GIF aspect ratio",
                ));
            }
            Ok(Some(Overlay {
                attachment_id: id.to_owned(),
                rect,
            }))
        })();
        match result {
            Ok(Some(overlay)) => {
                if overlays.len() == MAX_OVERLAYS_PER_PAGE {
                    diagnostics.push(format!("Slide {} exceeds the 64-animation overlay limit; additional markers were disabled.", index + 1));
                    break;
                }
                overlays.push(overlay);
            }
            Ok(None) => {}
            Err(error) => diagnostics.push(format!("Slide {}: {error}", index + 1)),
        }
    }
    overlays
}

fn validate_resources(doc: &Document) -> Result<(), Error> {
    let mut decoded_total = 0;
    let mut image_pixels = 0_u64;
    for object in doc.objects.values() {
        validate_color_definitions(doc, object, 0)?;
        let Object::Stream(stream) = object else {
            continue;
        };
        let subtype = stream
            .dict
            .get(b"Subtype")
            .ok()
            .map(|value| resolved(doc, value))
            .transpose()?
            .and_then(|value| value.as_name().ok());
        if subtype == Some(b"Image") {
            let width = stream
                .dict
                .get(b"Width")
                .ok()
                .map(|value| resolved(doc, value))
                .transpose()?
                .and_then(|value| value.as_i64().ok())
                .unwrap_or(0);
            let height = stream
                .dict
                .get(b"Height")
                .ok()
                .map(|value| resolved(doc, value))
                .transpose()?
                .and_then(|value| value.as_i64().ok())
                .unwrap_or(0);
            if width <= 0
                || height <= 0
                || width > 16_384
                || height > 16_384
                || width * height > 32_000_000
            {
                return Err(Error::new(
                    "PDF image dimensions exceed the supported limits",
                ));
            }
            image_pixels += (width * height) as u64;
            if image_pixels > 256_000_000 {
                return Err(Error::new(
                    "PDF images exceed the total decoded pixel limit",
                ));
            }
        }
        // Inspect every lossless stream before handing the PDF to the renderer.
        let filters = filters(doc, stream)?;
        if filters
            .iter()
            .any(|filter| matches!(*filter, b"JPXDecode" | b"CCITTFaxDecode" | b"JBIG2Decode"))
        {
            return Err(Error::new(
                "JPEG2000, CCITT, and JBIG2 PDF images are not supported by the bounded decoder",
            ));
        }
        if filters.contains(&b"DCTDecode".as_slice()) {
            if filters.len() != 1 || subtype != Some(b"Image") {
                return Err(Error::new(
                    "JPEG filter chains are not supported by the bounded decoder",
                ));
            }
            continue;
        }
        let limit = if subtype == Some(b"Image") {
            128 * 1024 * 1024
        } else {
            32 * 1024 * 1024
        };
        let decoded = bounded_content(doc, stream, limit)?;
        let stream_type = stream
            .dict
            .get(b"Type")
            .ok()
            .map(|value| resolved(doc, value))
            .transpose()?
            .and_then(|value| value.as_name().ok());
        if subtype != Some(b"Image") && stream_type != Some(b"EmbeddedFile") {
            let mut instructions = hayro::hayro_syntax::content::UntypedIter::new(&decoded);
            while let Some(instruction) = instructions.next() {
                if instruction.operator.as_ref() == b"BI" {
                    return Err(Error::new(
                        "Inline PDF images are not supported by the bounded decoder; use ordinary image objects",
                    ));
                }
            }
        }
        decoded_total += decoded.len();
        if decoded_total > 256 * 1024 * 1024 {
            return Err(Error::new(
                "PDF streams exceed the total 256 MiB decoded-data limit",
            ));
        }
    }
    Ok(())
}

fn validate_color_definitions(doc: &Document, object: &Object, depth: usize) -> Result<(), Error> {
    if depth > 100 {
        return Err(Error::new("PDF color definitions are excessively nested"));
    }
    match object {
        Object::Array(values) => {
            let kind = values
                .first()
                .map(|value| resolved(doc, value))
                .transpose()?
                .and_then(|value| value.as_name().ok());
            if kind == Some(b"DeviceN") {
                let components = values
                    .get(1)
                    .ok_or_else(|| Error::new("Invalid DeviceN PDF color space"))?;
                let components = resolved(doc, components)?
                    .as_array()
                    .map_err(|_| Error::new("Invalid DeviceN PDF color components"))?;
                if components.is_empty() || components.len() > 4 {
                    return Err(Error::new(
                        "PDF image color spaces are limited to four components",
                    ));
                }
            }
            if kind == Some(b"ICCBased") {
                let profile = values
                    .get(1)
                    .ok_or_else(|| Error::new("Invalid ICC PDF color space"))?;
                let profile = resolved(doc, profile)?
                    .as_stream()
                    .map_err(|_| Error::new("Invalid ICC PDF profile"))?;
                let components = profile
                    .dict
                    .get(b"N")
                    .map_err(|_| Error::new("ICC PDF profile has no component count"))?;
                let components = resolved(doc, components)?
                    .as_i64()
                    .map_err(|_| Error::new("Invalid ICC PDF component count"))?;
                if !matches!(components, 1 | 3 | 4) {
                    return Err(Error::new(
                        "PDF ICC color spaces must have one, three, or four components",
                    ));
                }
            }
            for value in values {
                validate_color_definitions(doc, value, depth + 1)?;
            }
        }
        Object::Dictionary(dict) => {
            for (_, value) in dict.iter() {
                validate_color_definitions(doc, value, depth + 1)?;
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter() {
                validate_color_definitions(doc, value, depth + 1)?;
            }
        }
        // Every indirect object is visited by the outer document object loop.
        _ => {}
    }
    Ok(())
}

fn canonical_filter(value: &[u8]) -> Result<&'static [u8], Error> {
    match value {
        b"FlateDecode" | b"Fl" => Ok(b"FlateDecode"),
        b"LZWDecode" | b"LZW" => Ok(b"LZWDecode"),
        b"ASCIIHexDecode" | b"AHx" => Ok(b"ASCIIHexDecode"),
        b"ASCII85Decode" | b"A85" => Ok(b"ASCII85Decode"),
        b"RunLengthDecode" | b"RL" => Ok(b"RunLengthDecode"),
        b"DCTDecode" | b"DCT" => Ok(b"DCTDecode"),
        b"JPXDecode" => Ok(b"JPXDecode"),
        b"CCITTFaxDecode" | b"CCF" => Ok(b"CCITTFaxDecode"),
        b"JBIG2Decode" => Ok(b"JBIG2Decode"),
        _ => Err(Error::new("PDF uses unsupported stream compression")),
    }
}

fn filters(doc: &Document, stream: &Stream) -> Result<Vec<&'static [u8]>, Error> {
    let Ok(value) = stream.dict.get(b"Filter") else {
        return Ok(Vec::new());
    };
    let value = resolved(doc, value)?;
    let values: Vec<_> = match value {
        Object::Array(values) => values.iter().collect(),
        _ => vec![value],
    };
    if values.len() > 8 {
        return Err(Error::new("PDF stream has too many compression filters"));
    }
    values
        .into_iter()
        .map(|value| {
            let value = resolved(doc, value)?
                .as_name()
                .map_err(|_| Error::new("Invalid PDF stream filter"))?;
            canonical_filter(value)
        })
        .collect()
}

fn resolved_copy(
    doc: &Document,
    object: &Object,
    depth: usize,
    budget: &mut usize,
) -> Result<Object, Error> {
    if depth > 32 || *budget == 0 {
        return Err(Error::new("PDF decoding parameters are excessively nested"));
    }
    *budget -= 1;
    match resolved(doc, object)? {
        Object::Array(values) => values
            .iter()
            .map(|value| resolved_copy(doc, value, depth + 1, budget))
            .collect::<Result<Vec<_>, _>>()
            .map(Object::Array),
        Object::Dictionary(dict) => {
            let mut result = Dictionary::new();
            for (key, value) in dict.iter() {
                result.set(key.clone(), resolved_copy(doc, value, depth + 1, budget)?);
            }
            Ok(Object::Dictionary(result))
        }
        Object::Stream(_) => Err(Error::new("Unexpected stream in PDF decoding parameters")),
        value => Ok(value.clone()),
    }
}

fn bounded_content(doc: &Document, stream: &Stream, limit: usize) -> Result<Vec<u8>, Error> {
    let filters = filters(doc, stream)?;
    if filters.is_empty() {
        if stream.content.len() > limit {
            return Err(Error::new("PDF stream exceeds decoded-data limit"));
        }
        return Ok(stream.content.clone());
    }
    let mut normalized = stream.clone();
    normalized.dict.set(
        "Filter",
        Object::Array(
            filters
                .iter()
                .map(|filter| Object::Name(filter.to_vec()))
                .collect(),
        ),
    );
    if let Ok(params) = stream.dict.get(b"DecodeParms") {
        let params = resolved_copy(doc, params, 0, &mut 4096)?;
        validate_predictor_parameters(&params)?;
        normalized.dict.set("DecodeParms", params);
    }
    normalized
        .decompressed_content_with_limit(limit)
        .map_err(|error| {
            Error::new(format!(
                "PDF stream exceeds decoding limits or cannot be decoded: {error}"
            ))
        })
}

fn validate_predictor_parameters(params: &Object) -> Result<(), Error> {
    match params {
        Object::Array(values) => {
            for value in values {
                validate_predictor_parameters(value)?;
            }
        }
        Object::Dictionary(dict) => {
            for (key, default, maximum) in [
                (b"Columns".as_slice(), 1, 16_384),
                (b"Colors".as_slice(), 1, 4),
            ] {
                let value = match dict.get(key) {
                    Ok(value) => value
                        .as_i64()
                        .map_err(|_| Error::new("Invalid PDF predictor parameter"))?,
                    Err(_) => default,
                };
                if value < 1 || value > maximum {
                    return Err(Error::new(
                        "PDF predictor Columns/Colors exceeds the supported limit",
                    ));
                }
            }
            if let Ok(bits) = dict.get(b"BitsPerComponent") {
                let bits = bits
                    .as_i64()
                    .map_err(|_| Error::new("Invalid PDF predictor bit depth"))?;
                if !matches!(bits, 1 | 2 | 4 | 8 | 16) {
                    return Err(Error::new("PDF predictor bit depth is unsupported"));
                }
            }
        }
        Object::Null => {}
        _ => return Err(Error::new("Invalid PDF decoding parameters")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_version_and_identifier_validation() {
        assert_eq!(
            marker_id("https://presenter.invalid/gif/v1/demo-1").unwrap(),
            Some("demo-1")
        );
        assert_eq!(marker_id("https://example.org/hello").unwrap(), None);
        for uri in [
            "https://presenter.invalid/gif/v2/demo",
            "https://presenter.invalid/gif/v1/",
            "https://presenter.invalid/gif/v1/../../escape",
            "https://presenter.invalid/gif/v1/demo?x=1",
            "https://presenter.invalid/gif/v1/a%20b",
        ] {
            assert!(marker_id(uri).is_err(), "{uri}");
        }
    }
}
