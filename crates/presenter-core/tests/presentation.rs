#![forbid(unsafe_code)]
use lopdf::{Document, Object, Stream, dictionary};
use presenter_core::{Layout, MAX_RASTER_PIXELS, Presentation, Rect, View};
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name),
    )
    .unwrap()
}
fn load(name: &str) -> Presentation {
    Presentation::load(fixture(name)).unwrap()
}

#[derive(Clone, Copy, Debug)]
enum PdfVariant {
    TranslatedOrigin,
    Cropped,
    Rotated,
    InconsistentPages,
    RotatedGif,
    NestedNameTree,
    DuplicateIdentifiers,
}

fn load_variant(variant: PdfVariant) -> Presentation {
    let base = match variant {
        PdfVariant::TranslatedOrigin
        | PdfVariant::Cropped
        | PdfVariant::Rotated
        | PdfVariant::InconsistentPages => "plain.pdf",
        PdfVariant::RotatedGif | PdfVariant::NestedNameTree | PdfVariant::DuplicateIdentifiers => {
            "single-gif.pdf"
        }
    };
    let mut document = Document::load_mem(&fixture(base)).unwrap();
    let pages: Vec<_> = document.page_iter().collect();
    match variant {
        PdfVariant::TranslatedOrigin => {
            for page in pages {
                // Preserve the original instructions and numeric precision.
                let mut content = b"q\n1 0 0 1 40 25 cm\n".to_vec();
                content.extend(document.get_page_content(page));
                content.extend_from_slice(b"\nQ\n");
                let stream = document.add_object(Stream::new(dictionary! {}, content));
                let page = document.get_dictionary_mut(page).unwrap();
                page.set("Contents", stream);
                let bounds: Vec<Object> = [40, 25, 680, 385].map(Object::from).to_vec();
                page.set("MediaBox", bounds.clone());
                page.set("CropBox", bounds);
            }
        }
        PdfVariant::Cropped => {
            for page in pages {
                document
                    .get_dictionary_mut(page)
                    .unwrap()
                    .set("CropBox", [40, 25, 600, 335].map(Object::from).to_vec());
            }
        }
        PdfVariant::Rotated | PdfVariant::RotatedGif => {
            let page = if matches!(variant, PdfVariant::RotatedGif) {
                pages[0]
            } else {
                pages[1]
            };
            document.get_dictionary_mut(page).unwrap().set("Rotate", 90);
        }
        PdfVariant::InconsistentPages => {
            document
                .get_dictionary_mut(pages[1])
                .unwrap()
                .set("CropBox", [0, 0, 600, 300].map(Object::from).to_vec());
        }
        PdfVariant::NestedNameTree | PdfVariant::DuplicateIdentifiers => {
            let mut names = document
                .get_dict_in_dict(document.catalog().unwrap(), b"Names")
                .unwrap()
                .clone();
            let mut embedded = document
                .get_dict_in_dict(&names, b"EmbeddedFiles")
                .unwrap()
                .clone();
            if matches!(variant, PdfVariant::NestedNameTree) {
                let child = document.add_object(embedded);
                embedded = dictionary! { "Kids" => vec![Object::Reference(child)] };
            } else {
                let entries = embedded.get_mut(b"Names").unwrap().as_array_mut().unwrap();
                let duplicate = entries[..2].to_vec();
                entries.extend(duplicate);
            }
            names.set("EmbeddedFiles", embedded);
            document.catalog_mut().unwrap().set("Names", names);
        }
    }
    Presentation::load(write_pdf(document)).unwrap()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.001, "{a} differs from {b}");
}

#[test]
fn fixture_layout_detection() {
    assert_eq!(
        load("notes-right.pdf").detected_layout(),
        Layout::NotesRight
    );
    for name in ["plain.pdf", "single-gif.pdf"] {
        assert_eq!(load(name).detected_layout(), Layout::Normal, "{name}");
    }
    for variant in [
        PdfVariant::TranslatedOrigin,
        PdfVariant::Cropped,
        PdfVariant::Rotated,
        PdfVariant::InconsistentPages,
    ] {
        assert_eq!(
            load_variant(variant).detected_layout(),
            Layout::Normal,
            "{variant:?}"
        );
    }
}

#[test]
fn layout_detection_uses_inherited_cropbox_after_rotation() {
    for (crop, rotation, expected) in [
        ([40, 25, 360, 145], 0, Layout::NotesRight),
        ([40, 25, 360, 145], 180, Layout::NotesRight),
        ([40, 25, 360, 145], 90, Layout::Normal),
        ([40, 25, 160, 345], 90, Layout::NotesRight),
        ([40, 25, 160, 345], 270, Layout::NotesRight),
        ([40, 25, 240, 125], 0, Layout::Normal),
    ] {
        let mut doc = Document::load_mem(&color_pdf(rotation)).unwrap();
        let page_id = doc.page_iter().next().unwrap();
        let parent = doc
            .get_dictionary(page_id)
            .unwrap()
            .get(b"Parent")
            .unwrap()
            .as_reference()
            .unwrap();
        let pages = doc.get_dictionary_mut(parent).unwrap();
        pages.set(
            "MediaBox",
            vec![0.into(), 0.into(), 1000.into(), 1000.into()],
        );
        pages.set("CropBox", crop.map(Object::from).to_vec());
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        assert_eq!(
            Presentation::load(bytes).unwrap().detected_layout(),
            expected,
            "CropBox {crop:?}, rotation {rotation}"
        );
    }
}

#[test]
fn typst_attachment_is_recovered_exactly_and_rectangle_is_aligned() {
    for doc in [
        load("single-gif.pdf"),
        load_variant(PdfVariant::NestedNameTree),
    ] {
        assert_eq!(
            doc.attachment("orbit").unwrap().bytes,
            fixture("assets/orbit.gif")
        );
        let overlays = doc.animations(0, Layout::Normal, View::Audience);
        assert_eq!(overlays.len(), 1, "{:?}", doc.diagnostics());
        assert_eq!(overlays[0].attachment_id, "orbit");
        close(overlays[0].rect.x, 40.0 / 640.0);
        close(overlays[0].rect.y, 60.0 / 360.0);
        close(overlays[0].rect.width, 320.0 / 640.0);
        close(overlays[0].rect.height, 180.0 / 360.0);
        assert!(doc.diagnostics().is_empty(), "{:?}", doc.diagnostics());
    }
}

#[test]
fn simple_typst_gifs_reuse_attachments_and_preserve_the_static_first_frame() {
    let doc = load("simple-gif.pdf");
    assert_eq!(doc.page_count(), 3);
    assert_eq!(doc.detected_layout(), Layout::Normal);
    assert!(doc.diagnostics().is_empty(), "{:?}", doc.diagnostics());
    for (id, asset) in [
        ("gebakuk-gif-0", "assets/orbit.gif"),
        ("gebakuk-gif-1", "assets/pulse.gif"),
    ] {
        assert_eq!(doc.attachment(id).unwrap().bytes, fixture(asset));
    }
    for (page, expected) in [
        (0, vec!["gebakuk-gif-0"]),
        (1, vec!["gebakuk-gif-0", "gebakuk-gif-1"]),
        (2, vec!["gebakuk-gif-0", "gebakuk-gif-1"]),
    ] {
        let overlays = doc.animations(page, Layout::Normal, View::Audience);
        let ids: Vec<_> = overlays
            .iter()
            .map(|overlay| overlay.attachment_id.as_str())
            .collect();
        assert_eq!(ids, expected);
    }
    let first = &doc.animations(0, Layout::Normal, View::Audience)[0];
    close(first.rect.x, 40.0 / 640.0);
    close(first.rect.y, 60.0 / 360.0);
    close(first.rect.width, 320.0 / 640.0);
    close(first.rect.height, 180.0 / 360.0);

    let raster = doc
        .render(0, Layout::Normal, View::Audience, 640, 360)
        .unwrap();
    let poster = load("single-gif.pdf")
        .render(0, Layout::Normal, View::Audience, 640, 360)
        .unwrap();
    assert!(
        raster.rgba == poster.rgba,
        "The GIF's first frame differs from its original PNG poster"
    );
}

#[test]
fn notes_regions_multiple_gifs_and_attachment_reuse() {
    let doc = load("notes-right.pdf");
    assert_eq!(doc.page_count(), 3);
    assert_eq!(
        doc.region(0, Layout::NotesRight, View::Audience).unwrap(),
        Rect {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 360.0
        }
    );
    assert_eq!(
        doc.region(0, Layout::NotesRight, View::Notes).unwrap(),
        Rect {
            x: 640.0,
            y: 0.0,
            width: 640.0,
            height: 360.0
        }
    );
    let first = doc.animations(0, Layout::NotesRight, View::Audience);
    let second = doc.animations(1, Layout::NotesRight, View::Audience);
    assert_eq!(
        (first.len(), second.len()),
        (1, 2),
        "{:?}",
        doc.diagnostics()
    );
    assert_eq!(first[0].attachment_id, second[0].attachment_id);
    assert_eq!(second[1].attachment_id, "pulse");
    assert!(
        doc.animations(0, Layout::NotesRight, View::Notes)
            .is_empty()
    );
    assert!(
        doc.animations(2, Layout::NotesRight, View::Audience)
            .is_empty()
    );
    assert_eq!(
        doc.attachment("pulse").unwrap().bytes,
        fixture("assets/pulse.gif")
    );
}

#[test]
fn malformed_missing_duplicate_animations_have_static_fallback() {
    for (name, doc, diagnostic) in [
        (
            "missing attachment",
            load("missing-attachment.pdf"),
            "missing",
        ),
        (
            "malformed marker",
            load("malformed-marker.pdf"),
            "Malformed",
        ),
        (
            "duplicate identifiers",
            load_variant(PdfVariant::DuplicateIdentifiers),
            "Duplicate",
        ),
        (
            "rotated animation",
            load_variant(PdfVariant::RotatedGif),
            "rotated",
        ),
    ] {
        assert!(
            doc.animations(0, Layout::Normal, View::Audience).is_empty(),
            "{name}"
        );
        assert!(
            doc.diagnostics()
                .iter()
                .any(|item| item.contains(diagnostic)),
            "{name}: {:?}",
            doc.diagnostics()
        );
        let raster = doc
            .render(0, Layout::Normal, View::Audience, 160, 90)
            .unwrap();
        assert_eq!(raster.rgba.len(), 160 * 90 * 4);
    }
    assert!(
        load_variant(PdfVariant::DuplicateIdentifiers)
            .attachment("orbit")
            .is_none()
    );
}

fn write_pdf(mut document: Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn replace_content(document: &mut Document, content: lopdf::content::Content) {
    let page = document.page_iter().next().unwrap();
    let stream = document.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    document
        .get_dictionary_mut(page)
        .unwrap()
        .set("Contents", stream);
}

fn transparent_document() -> (Document, lopdf::content::Content) {
    let document = Document::load_mem(&fixture("transparent.pdf")).unwrap();
    let page = document.page_iter().next().unwrap();
    let content = lopdf::content::Content::decode_strict(&document.get_page_content(page)).unwrap();
    (document, content)
}

#[test]
fn transparent_animation_preserves_pattern_and_restores_poster_on_failure() {
    let mut presentation = load("transparent.pdf");
    assert!(
        presentation.diagnostics().is_empty(),
        "{:?}",
        presentation.diagnostics()
    );
    let overlays = presentation.animations(0, Layout::Normal, View::Audience);
    assert_eq!(overlays.len(), 1);
    let id = overlays[0].attachment_id.clone();
    assert!(presentation.attachment(&id).unwrap().has_transparency);
    assert_eq!(
        presentation.attachment(&id).unwrap().bytes,
        fixture("assets/transparent.gif")
    );
    let static_raster = presentation
        .render(0, Layout::Normal, View::Audience, 640, 360)
        .unwrap();
    let animated = presentation
        .render_with_animations(0, Layout::Normal, View::Audience, 640, 360)
        .unwrap();
    let (mut reference, mut content) = transparent_document();
    content
        .operations
        .retain(|operation| operation.operator != "Do");
    replace_content(&mut reference, content);
    let reference = Presentation::load(write_pdf(reference))
        .unwrap()
        .render(0, Layout::Normal, View::Audience, 640, 360)
        .unwrap();
    assert_eq!(
        animated.rgba, reference.rgba,
        "Animation background must preserve every original background pixel"
    );
    assert_ne!(
        static_raster.rgba, animated.rgba,
        "The opaque pixels from the first frame must be removed"
    );
    presentation.disable_animation(&id);
    assert!(
        presentation
            .animations(0, Layout::Normal, View::Audience)
            .is_empty()
    );
    assert_eq!(
        presentation
            .render_with_animations(0, Layout::Normal, View::Audience, 640, 360)
            .unwrap()
            .rgba,
        static_raster.rgba
    );
}

#[test]
fn filtering_does_not_change_unrelated_draws_of_the_same_image_resource() {
    use lopdf::content::Operation;
    let (mut document, mut content) = transparent_document();
    let poster_index = content
        .operations
        .iter()
        .position(|operation| operation.operator == "Do")
        .unwrap();
    let image_name = content.operations[poster_index].operands.clone();
    content.operations.extend([
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![
                80.into(),
                0.into(),
                0.into(),
                45.into(),
                480.into(),
                60.into(),
            ],
        ),
        Operation::new("Do", image_name),
        Operation::new("Q", vec![]),
    ]);
    replace_content(&mut document, content.clone());
    let mut actual = Presentation::load(write_pdf(document.clone())).unwrap();
    assert!(
        actual.diagnostics().is_empty(),
        "{:?}",
        actual.diagnostics()
    );
    content.operations.remove(poster_index);
    replace_content(&mut document, content);
    let expected = Presentation::load(write_pdf(document)).unwrap();
    assert_eq!(
        actual
            .render_with_animations(0, Layout::Normal, View::Audience, 640, 360)
            .unwrap()
            .rgba,
        expected
            .render(0, Layout::Normal, View::Audience, 640, 360)
            .unwrap()
            .rgba
    );
}

#[test]
fn transparent_posters_crossing_notes_boundary_remain_static() {
    let mut presentation = load("transparent.pdf");
    for view in [View::Audience, View::Notes] {
        assert!(
            presentation
                .animations(0, Layout::NotesRight, view)
                .is_empty()
        );
        assert_eq!(
            presentation
                .render_with_animations(0, Layout::NotesRight, view, 320, 360)
                .unwrap()
                .rgba,
            presentation
                .render(0, Layout::NotesRight, view, 320, 360)
                .unwrap()
                .rgba
        );
    }
}

#[test]
fn transparent_notes_regions_keep_background_and_independent_fallbacks() {
    let mut presentation = load("transparent-notes-right.pdf");
    assert_eq!(presentation.detected_layout(), Layout::NotesRight);
    assert!(
        presentation.diagnostics().is_empty(),
        "{:?}",
        presentation.diagnostics()
    );
    let mut reference = Document::load_mem(&fixture("transparent-notes-right.pdf")).unwrap();
    let page = reference.page_iter().next().unwrap();
    let mut content =
        lopdf::content::Content::decode_strict(&reference.get_page_content(page)).unwrap();
    content
        .operations
        .retain(|operation| operation.operator != "Do");
    replace_content(&mut reference, content);
    let reference = Presentation::load(write_pdf(reference)).unwrap();
    for view in [View::Audience, View::Notes] {
        assert_eq!(
            presentation.animations(0, Layout::NotesRight, view).len(),
            1
        );
        assert_eq!(
            presentation
                .render_with_animations(0, Layout::NotesRight, view, 640, 360)
                .unwrap()
                .rgba,
            reference
                .render(0, Layout::NotesRight, view, 640, 360)
                .unwrap()
                .rgba
        );
    }
    let id = presentation.animations(0, Layout::NotesRight, View::Audience)[0]
        .attachment_id
        .clone();
    let original_audience = presentation
        .render(0, Layout::NotesRight, View::Audience, 640, 360)
        .unwrap();
    presentation.disable_animation(&id);
    assert!(
        presentation
            .animations(0, Layout::NotesRight, View::Audience)
            .is_empty()
    );
    assert_eq!(
        presentation
            .render_with_animations(0, Layout::NotesRight, View::Audience, 640, 360)
            .unwrap()
            .rgba,
        original_audience.rgba
    );
    assert_eq!(
        presentation
            .animations(0, Layout::NotesRight, View::Notes)
            .len(),
        1
    );
    assert_eq!(
        presentation
            .render_with_animations(0, Layout::NotesRight, View::Notes, 640, 360)
            .unwrap()
            .rgba,
        reference
            .render(0, Layout::NotesRight, View::Notes, 640, 360)
            .unwrap()
            .rgba
    );
}

#[test]
fn clipped_and_form_image_placements_preserve_the_original_pdf() {
    use lopdf::content::Operation;
    for form in [false, true] {
        let (mut document, mut content) = transparent_document();
        let page = document.page_iter().next().unwrap();
        if form {
            let resources = document
                .get_dictionary(page)
                .unwrap()
                .get(b"Resources")
                .unwrap()
                .clone();
            let form = document.add_object(Stream::new(dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 640.into(), 360.into()],
                "Resources" => resources,
            }, content.encode().unwrap()));
            document.get_dictionary_mut(page).unwrap().set(
                "Resources",
                dictionary! { "XObject" => dictionary! { "form" => form } },
            );
            content.operations = vec![Operation::new("Do", vec![Object::Name(b"form".to_vec())])];
        } else {
            content.operations.splice(
                0..0,
                [
                    Operation::new("q", vec![]),
                    Operation::new("re", vec![0.into(), 0.into(), 640.into(), 360.into()]),
                    Operation::new("W", vec![]),
                    Operation::new("n", vec![]),
                ],
            );
            content.operations.push(Operation::new("Q", vec![]));
        }
        replace_content(&mut document, content);
        let mut presentation = Presentation::load(write_pdf(document)).unwrap();
        assert!(
            presentation
                .animations(0, Layout::Normal, View::Audience)
                .is_empty()
        );
        assert!(
            presentation
                .diagnostics()
                .iter()
                .any(|message| message.contains("remains static"))
        );
        assert_eq!(
            presentation
                .render_with_animations(0, Layout::Normal, View::Audience, 640, 360)
                .unwrap()
                .rgba,
            presentation
                .render(0, Layout::Normal, View::Audience, 640, 360)
                .unwrap()
                .rgba
        );
    }
}

#[test]
fn ambiguous_transparent_posters_keep_static_pdf_artwork() {
    use lopdf::content::Operation;
    let (mut document, mut content) = transparent_document();
    let image = content
        .operations
        .iter()
        .find(|operation| operation.operator == "Do")
        .unwrap()
        .operands
        .clone();
    content.operations.extend([
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![
                320.into(),
                0.into(),
                0.into(),
                180.into(),
                40.into(),
                120.into(),
            ],
        ),
        Operation::new("Do", image),
        Operation::new("Q", vec![]),
    ]);
    replace_content(&mut document, content);
    let mut presentation = Presentation::load(write_pdf(document)).unwrap();
    assert!(
        presentation
            .animations(0, Layout::Normal, View::Audience)
            .is_empty()
    );
    assert!(
        presentation
            .diagnostics()
            .iter()
            .any(|message| message.contains("ambiguous"))
    );
    assert_eq!(
        presentation
            .render_with_animations(0, Layout::Normal, View::Audience, 640, 360)
            .unwrap()
            .rgba,
        presentation
            .render(0, Layout::Normal, View::Audience, 640, 360)
            .unwrap()
            .rgba
    );
}

#[test]
fn plain_origin_crop_rotation_inconsistent_and_large_pdfs() {
    for (name, doc, page, width, height) in [
        ("plain", load("plain.pdf"), 0, 640.0, 360.0),
        (
            "translated origin",
            load_variant(PdfVariant::TranslatedOrigin),
            0,
            640.0,
            360.0,
        ),
        (
            "cropped",
            load_variant(PdfVariant::Cropped),
            0,
            560.0,
            310.0,
        ),
        (
            "rotated",
            load_variant(PdfVariant::Rotated),
            1,
            360.0,
            640.0,
        ),
    ] {
        close(doc.page(page).unwrap().width, width);
        close(doc.page(page).unwrap().height, height);
        let raster = doc
            .render(page, Layout::Normal, View::Audience, 160, 90)
            .unwrap();
        assert_eq!(raster.rgba.len(), 160 * 90 * 4);
        assert!(
            raster
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[..3] != [255, 255, 255]),
            "{name} should retain visible page content"
        );
    }
    assert!(
        load_variant(PdfVariant::InconsistentPages)
            .diagnostics()
            .iter()
            .any(|message| message.starts_with("Page dimensions vary"))
    );
    let original = load("plain.pdf");
    let translated = load_variant(PdfVariant::TranslatedOrigin);
    for page in 0..original.page_count() {
        assert_eq!(
            translated
                .render(page, Layout::Normal, View::Audience, 160, 90)
                .unwrap()
                .rgba,
            original
                .render(page, Layout::Normal, View::Audience, 160, 90)
                .unwrap()
                .rgba,
            "Translating page boxes and content must preserve page {} pixels",
            page + 1
        );
    }
    let large = load("large.pdf");
    assert_eq!(large.page_count(), 120);
    assert!(
        large
            .render(119, Layout::Normal, View::Audience, 160, 90)
            .is_ok()
    );
}

/// A tiny PDF with nonzero CropBox, inherited metadata, and distinct colored halves.
fn color_pdf(rotation: i64) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"1 0 0 rg 40 25 100 100 re f 0 0 1 rg 140 25 100 100 re f".to_vec(),
    ));
    let page =
        doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "Contents" => content });
    doc.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
            "MediaBox" => vec![0.into(),0.into(),300.into(),200.into()],
            "CropBox" => vec![40.into(),25.into(),240.into(),125.into()],
            "Rotate" => rotation,
            "Resources" => dictionary! {},
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn color_at(raster: &presenter_core::Raster, x: usize, y: usize) -> [u8; 4] {
    let offset = (y * raster.width as usize + x) * 4;
    raster.rgba[offset..offset + 4].try_into().unwrap()
}

#[test]
fn region_only_rasterization_and_all_page_rotations_match_geometry() {
    let doc = Presentation::load(color_pdf(0)).unwrap();
    let audience = doc
        .render(0, Layout::NotesRight, View::Audience, 100, 100)
        .unwrap();
    let notes = doc
        .render(0, Layout::NotesRight, View::Notes, 100, 100)
        .unwrap();
    assert_eq!(color_at(&audience, 50, 50), [255, 0, 0, 255]);
    assert_eq!(color_at(&notes, 50, 50), [0, 0, 255, 255]);
    for rotation in [0, 90, 180, 270] {
        let doc = Presentation::load(color_pdf(rotation)).unwrap();
        let info = doc.page(0).unwrap();
        let raster = doc
            .render(
                0,
                Layout::Normal,
                View::Audience,
                info.width as u32,
                info.height as u32,
            )
            .unwrap();
        for (point, color) in [
            ((90.0, 75.0), [255, 0, 0, 255]),
            ((190.0, 75.0), [0, 0, 255, 255]),
        ] {
            let (x, y) = info.pdf_point_to_display(point.0, point.1);
            assert_eq!(
                color_at(&raster, x as usize, y as usize),
                color,
                "rotation {rotation}"
            );
        }
    }
}

#[test]
fn invalid_inputs_and_unbounded_rasters_are_controlled_errors() {
    assert!(Presentation::load(b"not a pdf".to_vec()).is_err());
    let doc = load("plain.pdf");
    for (w, h) in [(0, 10), (9000, 1), (4000, 4000)] {
        assert!(doc.render(0, Layout::Normal, View::Audience, w, h).is_err());
    }
    assert_eq!(MAX_RASTER_PIXELS, 8_000_000);
    assert!(
        doc.render(100, Layout::Normal, View::Audience, 160, 90)
            .is_err()
    );
    assert!(doc.region(0, Layout::Normal, View::Notes).is_err());
    assert!(Presentation::load(color_pdf(4_294_967_580)).is_err());
}

#[test]
fn resource_guards_cover_inline_images_indirect_metadata_and_color_components() {
    let mut doc = Document::load_mem(&color_pdf(0)).unwrap();
    let page_id = doc.page_iter().next().unwrap();
    let contents = doc.get_page_contents(page_id)[0];
    doc.objects.insert(
        contents,
        Object::Stream(Stream::new(
            dictionary! {},
            b"q BI /W 100000 /H 100000 /CS /RGB /BPC 8 ID abc EI Q".to_vec(),
        )),
    );
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    assert!(
        Presentation::load(bytes)
            .err()
            .unwrap()
            .to_string()
            .contains("Inline")
    );

    let mut doc = Document::load_mem(&color_pdf(0)).unwrap();
    let subtype = doc.add_object(Object::Name(b"Image".to_vec()));
    doc.add_object(Stream::new(
        dictionary! { "Subtype" => subtype, "Width" => 100000, "Height" => 100000 },
        vec![0],
    ));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    assert!(
        Presentation::load(bytes)
            .err()
            .unwrap()
            .to_string()
            .contains("dimensions")
    );

    let mut doc = Document::load_mem(&color_pdf(0)).unwrap();
    doc.add_object(Object::Array(vec![
        Object::Name(b"DeviceN".to_vec()),
        Object::Array(vec![Object::Name(b"Ink".to_vec()); 255]),
    ]));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    assert!(
        Presentation::load(bytes)
            .err()
            .unwrap()
            .to_string()
            .contains("four components")
    );

    let mut doc = Document::load_mem(&color_pdf(0)).unwrap();
    doc.add_object(Stream::new(dictionary! {
        "Filter" => "FlateDecode",
        "DecodeParms" => dictionary! { "Predictor" => 15, "Columns" => 1_000_000_000, "Colors" => 1, "BitsPerComponent" => 8 },
    }, vec![0x78,0x9c,0x4b,0x04,0x00,0x00,0x62,0x00,0x62]));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    assert!(
        Presentation::load(bytes)
            .err()
            .unwrap()
            .to_string()
            .contains("predictor")
    );
}

#[test]
fn compressed_attachment_size_cannot_bypass_the_total_gif_decode_budget() {
    // The checked-in GIF has twelve 320×180 frames (691,200 composited pixels).
    // 186 separately named attachments exceed 128 million pixels despite small GIF bytes.
    let mut doc = Document::load_mem(&color_pdf(0)).unwrap();
    let embedded = doc.add_object(Stream::new(
        dictionary! { "Type" => "EmbeddedFile" },
        fixture("assets/orbit.gif"),
    ));
    let specification = doc.add_object(dictionary! { "Type" => "Filespec", "F" => Object::string_literal("orbit.gif"), "EF" => dictionary! { "F" => embedded } });
    let entries: Vec<Object> = (0..186)
        .flat_map(|index| {
            [
                Object::string_literal(format!("animation-{index}.gif")),
                specification.into(),
            ]
        })
        .collect();
    let names = doc.add_object(dictionary! { "Names" => entries });
    doc.catalog_mut()
        .unwrap()
        .set("Names", dictionary! { "EmbeddedFiles" => names });
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    let presentation = Presentation::load(bytes).unwrap();
    assert!(presentation.attachment("animation-0").is_some());
    // PDF name-tree order is deterministic and the final attachment is disabled.
    assert!(presentation.attachment("animation-185").is_none());
    assert!(
        presentation
            .diagnostics()
            .iter()
            .any(|message| message.contains("total 128 million"))
    );
}
