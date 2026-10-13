use crate::{Error, GifAttachment, MAX_GIF_DECODED_PIXELS};

const MAX_GIF_PIXELS: u64 = 16_000_000;
const MAX_FRAMES: usize = 2_000;

/// Validate framing, indexed pixels, and resource requirements.
/// Browser-native decoding remains responsible for actual animation playback.
pub(crate) fn validate_gif_with_budget(
    bytes: Vec<u8>,
    remaining_pixels: &mut u64,
) -> Result<GifAttachment, Error> {
    if bytes.len() < 13 || !matches!(&bytes[..6], b"GIF87a" | b"GIF89a") {
        return Err(Error::new("Attachment is not a GIF"));
    }
    let width = u16::from_le_bytes([bytes[6], bytes[7]]);
    let height = u16::from_le_bytes([bytes[8], bytes[9]]);
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || u64::from(width) * u64::from(height) > MAX_GIF_PIXELS
    {
        return Err(Error::new("GIF dimensions exceed the supported limits"));
    }
    let mut cursor = 13;
    let global_palette_colors = if bytes[10] & 0x80 != 0 {
        let colors = palette_size(bytes[10]) / 3;
        if usize::from(bytes[11]) >= colors {
            return Err(Error::new(
                "GIF background references a missing palette color",
            ));
        }
        skip(&bytes, &mut cursor, palette_size(bytes[10]))?;
        Some(colors)
    } else {
        None
    };
    let mut frames = 0;
    let mut frame_pixels = 0_u64;
    let mut pending_transparency = None;
    let mut pending_disposal = 0;
    let mut previous_disposal = 0;
    let mut has_transparency = global_palette_colors.is_none();
    loop {
        let tag = *bytes
            .get(cursor)
            .ok_or_else(|| Error::new("Truncated GIF"))?;
        cursor += 1;
        match tag {
            0x3b if frames > 0 => break,
            0x2c => {
                let descriptor = bytes
                    .get(cursor..cursor + 9)
                    .ok_or_else(|| Error::new("Truncated GIF image descriptor"))?;
                let left = u16::from_le_bytes([descriptor[0], descriptor[1]]);
                let top = u16::from_le_bytes([descriptor[2], descriptor[3]]);
                let fw = u16::from_le_bytes([descriptor[4], descriptor[5]]);
                let fh = u16::from_le_bytes([descriptor[6], descriptor[7]]);
                let flags = descriptor[8];
                cursor += 9;
                if fw == 0
                    || fh == 0
                    || u32::from(left) + u32::from(fw) > u32::from(width)
                    || u32::from(top) + u32::from(fh) > u32::from(height)
                {
                    return Err(Error::new("GIF frame extends outside its logical canvas"));
                }
                frames += 1;
                let partial_canvas = left != 0 || top != 0 || fw != width || fh != height;
                if partial_canvas && (frames == 1 || matches!(previous_disposal, 2 | 3)) {
                    has_transparency = true;
                }
                previous_disposal = pending_disposal;
                pending_disposal = 0;
                // Browsers can retain composited full-canvas frames even for tiny update rectangles.
                frame_pixels += u64::from(width) * u64::from(height);
                if frames > MAX_FRAMES || frame_pixels > MAX_GIF_DECODED_PIXELS {
                    return Err(Error::new(
                        "GIF frame count or decoded size exceeds the supported limits",
                    ));
                }
                let canvas_pixels = u64::from(width) * u64::from(height);
                if canvas_pixels > *remaining_pixels {
                    return Err(Error::new(
                        "GIF attachments exceed the total 128 million decoded GIF frame-pixel validation limit",
                    ));
                }
                *remaining_pixels -= canvas_pixels;
                let palette_colors = if flags & 0x80 != 0 {
                    skip(&bytes, &mut cursor, palette_size(flags))?;
                    palette_size(flags) / 3
                } else {
                    global_palette_colors
                        .ok_or_else(|| Error::new("GIF frame has no color palette"))?
                };
                if pending_transparency
                    .take()
                    .is_some_and(|index| usize::from(index) >= palette_colors)
                {
                    return Err(Error::new(
                        "GIF transparency references a missing palette color",
                    ));
                }
                let code_size = *bytes
                    .get(cursor)
                    .ok_or_else(|| Error::new("Truncated GIF image data"))?;
                if !(2..=8).contains(&code_size) {
                    return Err(Error::new("Invalid GIF LZW code size"));
                }
                cursor += 1;
                let compressed = image_sub_blocks(&bytes, &mut cursor)?;
                validate_indices(
                    &compressed,
                    code_size,
                    usize::from(fw) * usize::from(fh),
                    palette_colors,
                )?;
            }
            0x21 => {
                let label = *bytes
                    .get(cursor)
                    .ok_or_else(|| Error::new("Truncated GIF extension"))?;
                cursor += 1;
                if label == 0xf9 {
                    let gce = bytes
                        .get(cursor..cursor + 6)
                        .ok_or_else(|| Error::new("Truncated GIF control extension"))?;
                    if gce[0] != 4 || gce[5] != 0 {
                        return Err(Error::new("Invalid GIF control extension"));
                    }
                    pending_transparency = (gce[1] & 1 != 0).then_some(gce[4]);
                    has_transparency |= pending_transparency.is_some();
                    pending_disposal = (gce[1] >> 2) & 7;
                    if pending_disposal > 3 {
                        return Err(Error::new("GIF uses an unsupported frame disposal method"));
                    }
                } else if label == 0x01 {
                    // A control extension applies to the next graphic block,
                    // including a plain-text block rather than a later image.
                    pending_transparency = None;
                    pending_disposal = 0;
                }
                sub_blocks(&bytes, &mut cursor)?;
            }
            _ => return Err(Error::new("Invalid GIF framing or missing image")),
        }
    }
    Ok(GifAttachment {
        bytes,
        width,
        height,
        has_transparency,
        decoded_frame_pixels: frame_pixels,
    })
}

#[cfg(test)]
fn validate_gif(bytes: Vec<u8>) -> Result<GifAttachment, Error> {
    let mut budget = MAX_GIF_DECODED_PIXELS;
    validate_gif_with_budget(bytes, &mut budget)
}

fn palette_size(flags: u8) -> usize {
    3 * (1 << (usize::from(flags & 7) + 1))
}

fn skip(bytes: &[u8], cursor: &mut usize, count: usize) -> Result<(), Error> {
    *cursor = cursor
        .checked_add(count)
        .ok_or_else(|| Error::new("Invalid GIF length"))?;
    if *cursor > bytes.len() {
        return Err(Error::new("Truncated GIF"));
    }
    Ok(())
}

fn sub_blocks(bytes: &[u8], cursor: &mut usize) -> Result<(), Error> {
    loop {
        let size = *bytes
            .get(*cursor)
            .ok_or_else(|| Error::new("Truncated GIF data block"))?;
        *cursor += 1;
        if size == 0 {
            return Ok(());
        }
        skip(bytes, cursor, usize::from(size))?;
    }
}

fn image_sub_blocks(bytes: &[u8], cursor: &mut usize) -> Result<Vec<u8>, Error> {
    let mut compressed = Vec::new();
    loop {
        let size = *bytes
            .get(*cursor)
            .ok_or_else(|| Error::new("Truncated GIF image data"))?;
        *cursor += 1;
        if size == 0 {
            return Ok(compressed);
        }
        let start = *cursor;
        skip(bytes, cursor, usize::from(size))?;
        compressed.extend_from_slice(&bytes[start..*cursor]);
    }
}

fn validate_indices(
    compressed: &[u8],
    code_size: u8,
    expected: usize,
    palette_colors: usize,
) -> Result<(), Error> {
    let mut decoder = weezl::decode::Decoder::new(weezl::BitOrder::Lsb, code_size);
    let mut buffer = [0_u8; 16_384];
    let mut consumed = 0;
    let mut produced = 0;
    loop {
        let result = decoder.decode_bytes(&compressed[consumed..], &mut buffer);
        consumed += result.consumed_in;
        produced += result.consumed_out;
        if produced > expected {
            return Err(Error::new(
                "GIF image data contains more pixels than its frame bounds",
            ));
        }
        if buffer[..result.consumed_out]
            .iter()
            .any(|index| usize::from(*index) >= palette_colors)
        {
            return Err(Error::new(
                "GIF image data references a missing palette color",
            ));
        }
        match result.status {
            Ok(weezl::LzwStatus::Done) if produced == expected => return Ok(()),
            Ok(weezl::LzwStatus::Done) => {
                return Err(Error::new(
                    "GIF image data does not fill its complete frame",
                ));
            }
            Err(_) => return Err(Error::new("GIF image data has invalid LZW codes")),
            Ok(weezl::LzwStatus::NoProgress) => {
                return Err(Error::new(
                    "GIF image data is truncated or lacks an end code",
                ));
            }
            Ok(weezl::LzwStatus::Ok) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn tiny_gif() -> Vec<u8> {
        vec![
            71, 73, 70, 56, 57, 97, 1, 0, 1, 0, 128, 0, 0, 255, 0, 0, 0, 0, 255, 33, 249, 4, 0, 10,
            0, 0, 0, 44, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 68, 1, 0, 59,
        ]
    }

    const PALETTE: [u8; 6] = [255, 0, 0, 0, 0, 255];

    fn frame(left: u16, top: u16, control: Option<(u8, u8)>, local: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        if let Some((flags, transparent_index)) = control {
            bytes.extend_from_slice(&[0x21, 0xf9, 4, flags, 0, 0, transparent_index, 0]);
        }
        bytes.push(0x2c);
        bytes.extend_from_slice(&left.to_le_bytes());
        bytes.extend_from_slice(&top.to_le_bytes());
        bytes.extend_from_slice(&[1, 0, 1, 0, if local { 0x80 } else { 0 }]);
        if local {
            bytes.extend_from_slice(&PALETTE);
        }
        // LSB codes [clear=4, index=0, end=5], exactly one decoded pixel.
        bytes.extend_from_slice(&[2, 2, 0x44, 1, 0]);
        bytes
    }

    fn framed_gif(width: u16, height: u16, global: bool, frames: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        // Without a GCT the background index is unspecified and ignored.
        bytes.extend_from_slice(&[
            if global { 0x80 } else { 0 },
            if global { 0 } else { 255 },
            0,
        ]);
        if global {
            bytes.extend_from_slice(&PALETTE);
        }
        for frame in frames {
            bytes.extend_from_slice(frame);
        }
        bytes.push(0x3b);
        bytes
    }

    #[test]
    fn preserves_opaque_gif_bytes() {
        let bytes = tiny_gif();
        let gif = validate_gif(bytes.clone()).unwrap();
        assert_eq!(gif.bytes, bytes);
        assert_eq!((gif.width, gif.height), (1, 1));
        assert!(!gif.has_transparency);
    }

    #[test]
    fn full_opaque_frames_remain_opaque_with_background_disposal() {
        for bytes in [
            include_bytes!("../../../fixtures/assets/orbit.gif").as_slice(),
            include_bytes!("../../../fixtures/assets/pulse.gif").as_slice(),
        ] {
            let gif = validate_gif(bytes.to_vec()).unwrap();
            assert!(!gif.has_transparency);
        }
    }

    #[test]
    fn partial_updates_expose_pixels_only_after_clearing_or_restoring() {
        for disposal in 0..=3 {
            let mut first = frame(0, 0, Some((disposal << 2, 0)), false);
            first[13] = 2; // Full two-pixel first frame.
            first[20] = 4;
            first[21] = 10; // LZW clear, index 0, index 0, end.
            let bytes = framed_gif(2, 1, true, &[first, frame(1, 0, None, false)]);
            let gif = validate_gif(bytes).unwrap();
            assert_eq!(gif.has_transparency, matches!(disposal, 2 | 3));
        }
    }

    #[test]
    fn accepts_transparency_in_first_or_later_frames() {
        for transparent_frame in 0..2 {
            let frames: Vec<_> = (0..2)
                .map(|index| frame(0, 0, Some((u8::from(index == transparent_frame), 1)), false))
                .collect();
            let bytes = framed_gif(1, 1, true, &frames);
            let gif = validate_gif(bytes.clone()).unwrap();
            assert_eq!(gif.bytes, bytes);
            assert!(gif.has_transparency);
            assert_eq!(gif.decoded_frame_pixels, 2);
        }
    }

    #[test]
    fn rejects_truncation_and_excessive_canvas() {
        let mut bytes = tiny_gif();
        bytes.truncate(30);
        assert!(validate_gif(bytes).is_err());
        let mut bytes = tiny_gif();
        bytes[6] = 255;
        bytes[7] = 255;
        assert!(validate_gif(bytes).is_err());
    }

    #[test]
    fn accepts_uncovered_canvas_without_a_transparency_flag() {
        let mut bytes = tiny_gif();
        bytes[6] = 2;
        let gif = validate_gif(bytes).unwrap();
        assert!(gif.has_transparency);
        assert_eq!(gif.decoded_frame_pixels, 2);
    }

    #[test]
    fn accepts_partial_frames_with_background_or_previous_disposal() {
        for disposal in [2, 3] {
            let bytes = framed_gif(
                2,
                2,
                true,
                &[
                    frame(0, 0, Some((disposal << 2, 0)), false),
                    frame(1, 1, Some((1, 1)), false),
                ],
            );
            let gif = validate_gif(bytes).unwrap();
            assert!(gif.has_transparency);
            // Even tiny update rectangles spend the complete canvas budget.
            assert_eq!(gif.decoded_frame_pixels, 8);
        }
    }

    #[test]
    fn accepts_local_palettes_without_a_global_palette() {
        let bytes = framed_gif(1, 1, false, &[frame(0, 0, Some((1, 1)), true)]);
        let gif = validate_gif(bytes.clone()).unwrap();
        assert_eq!(gif.bytes, bytes);
        assert!(gif.has_transparency);
    }

    #[test]
    fn transparency_uses_the_active_palette_and_only_the_next_graphic() {
        let mut bytes = framed_gif(
            1,
            1,
            true,
            &[frame(0, 0, Some((1, 3)), false), frame(0, 0, None, true)],
        );
        bytes[10] = 0x81; // Four global entries, but only two local entries.
        bytes.splice(19..19, [0, 255, 0, 255, 255, 255]);
        assert!(validate_gif(bytes).is_ok());

        let bytes = framed_gif(1, 1, true, &[frame(0, 0, Some((1, 2)), false)]);
        assert!(
            validate_gif(bytes)
                .unwrap_err()
                .to_string()
                .contains("transparency")
        );
        let bytes = framed_gif(1, 1, false, &[frame(0, 0, Some((1, 2)), true)]);
        assert!(
            validate_gif(bytes)
                .unwrap_err()
                .to_string()
                .contains("transparency")
        );
        // The index has no meaning when transparency is disabled.
        let bytes = framed_gif(1, 1, true, &[frame(0, 0, Some((0, 255)), false)]);
        assert!(validate_gif(bytes).is_ok());
    }

    #[test]
    fn plain_text_consumes_control_extensions_but_comments_do_not() {
        for plain_text in [true, false] {
            let mut graphic = vec![0x21, 0xf9, 4, 1, 0, 0, 3, 0];
            if plain_text {
                // Plain text uses global colors and consumes its preceding GCE.
                graphic.extend_from_slice(&[
                    0x21, 0x01, 12, 0, 0, 0, 0, 1, 0, 1, 0, 1, 1, 0, 1, 1, b'A', 0,
                ]);
            } else {
                graphic.extend_from_slice(&[0x21, 0xfe, 1, b'A', 0]);
            }
            graphic.extend_from_slice(&frame(0, 0, None, true));
            let mut bytes = framed_gif(1, 1, true, &[graphic]);
            bytes[10] = 0x81; // Four global entries, two in the image's local palette.
            bytes.splice(19..19, [0, 255, 0, 255, 255, 255]);
            if plain_text {
                assert!(validate_gif(bytes).is_ok());
            } else {
                assert!(
                    validate_gif(bytes)
                        .unwrap_err()
                        .to_string()
                        .contains("transparency")
                );
            }
        }
    }

    #[test]
    fn rejects_missing_palettes_invalid_background_and_frame_bounds() {
        let bytes = framed_gif(1, 1, false, &[frame(0, 0, None, false)]);
        assert!(
            validate_gif(bytes)
                .unwrap_err()
                .to_string()
                .contains("no color palette")
        );
        let mut bytes = tiny_gif();
        bytes[11] = 2;
        assert!(
            validate_gif(bytes)
                .unwrap_err()
                .to_string()
                .contains("background")
        );
        for (left, top) in [(2, 0), (0, 2)] {
            let bytes = framed_gif(2, 2, true, &[frame(left, top, None, false)]);
            assert!(
                validate_gif(bytes)
                    .unwrap_err()
                    .to_string()
                    .contains("outside")
            );
        }
        let bytes = framed_gif(1, 1, true, &[frame(0, 0, Some((4 << 2, 0)), false)]);
        assert!(
            validate_gif(bytes)
                .unwrap_err()
                .to_string()
                .contains("disposal")
        );
    }

    #[test]
    fn partial_frames_still_obey_full_canvas_and_frame_count_budgets() {
        let bytes = framed_gif(2, 2, true, &[frame(0, 0, Some((1, 1)), false)]);
        let mut budget = 3;
        assert!(validate_gif_with_budget(bytes, &mut budget).is_err());
        let frames = vec![frame(0, 0, None, false); MAX_FRAMES + 1];
        assert!(
            validate_gif(framed_gif(1, 1, true, &frames))
                .unwrap_err()
                .to_string()
                .contains("frame count")
        );
        let frames = vec![frame(0, 0, Some((1, 1)), false); 9];
        assert!(
            validate_gif(framed_gif(4000, 4000, true, &frames))
                .unwrap_err()
                .to_string()
                .contains("decoded size")
        );
    }

    #[test]
    fn rejects_malformed_lzw_missing_palette_indices_and_wrong_pixel_counts() {
        let mut bytes = tiny_gif();
        bytes[39] = 255;
        bytes[40] = 255;
        assert!(validate_gif(bytes).unwrap_err().to_string().contains("LZW"));
        let mut bytes = tiny_gif();
        bytes.splice(38..42, [0]);
        assert!(validate_gif(bytes).is_err());
        let mut bytes = tiny_gif();
        // LSB codes [clear=4, index=2, end=5] with a two-color palette.
        bytes[39] = 0x54;
        assert!(
            validate_gif(bytes)
                .unwrap_err()
                .to_string()
                .contains("palette color")
        );
        let mut bytes = tiny_gif();
        // Declared two-pixel complete frame with image data for only one pixel.
        bytes[6] = 2;
        bytes[32] = 2;
        assert!(
            validate_gif(bytes)
                .unwrap_err()
                .to_string()
                .contains("complete frame")
        );
    }
}
