use crate::{Error, GifAttachment, MAX_GIF_DECODED_PIXELS};

const MAX_GIF_PIXELS: u64 = 16_000_000;
const MAX_FRAMES: usize = 2_000;

/// Validate framing, opaque indexed pixels, and resource requirements.
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
    // A defined opaque logical background avoids transparent untouched canvas pixels.
    if bytes[10] & 0x80 == 0 || usize::from(bytes[11]) >= palette_size(bytes[10]) / 3 {
        return Err(Error::new(
            "GIF overlays require a global palette and an opaque background",
        ));
    }
    skip(&bytes, &mut cursor, palette_size(bytes[10]))?;
    let mut frames = 0;
    let mut frame_pixels = 0_u64;
    let mut pending_disposal = 0;
    let mut previous_disposal = 0;
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
                let complete_canvas = left == 0 && top == 0 && fw == width && fh == height;
                if frames == 1 && !complete_canvas {
                    return Err(Error::new(
                        "GIF overlays require the first frame to cover the complete canvas",
                    ));
                }
                if matches!(previous_disposal, 2 | 3) && !complete_canvas {
                    return Err(Error::new(
                        "GIF frames following background/previous disposal must cover the complete canvas for opaque overlays",
                    ));
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
                    palette_size(bytes[10]) / 3
                };
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
                    if gce[1] & 1 != 0 {
                        return Err(Error::new(
                            "Transparent GIFs are disabled: use an opaque background for correct overlays",
                        ));
                    }
                    pending_disposal = (gce[1] >> 2) & 7;
                    if pending_disposal > 3 {
                        return Err(Error::new("GIF uses an unsupported frame disposal method"));
                    }
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

    #[test]
    fn preserves_opaque_gif_bytes() {
        let bytes = tiny_gif();
        let gif = validate_gif(bytes.clone()).unwrap();
        assert_eq!(gif.bytes, bytes);
        assert_eq!((gif.width, gif.height), (1, 1));
    }

    #[test]
    fn rejects_transparency_in_any_frame() {
        let mut bytes = tiny_gif();
        bytes[22] |= 1;
        assert!(
            validate_gif(bytes)
                .unwrap_err()
                .to_string()
                .contains("Transparent")
        );
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
    fn rejects_uncovered_canvas_even_without_a_transparency_flag() {
        let mut bytes = tiny_gif();
        bytes[6] = 2;
        assert!(
            validate_gif(bytes)
                .unwrap_err()
                .to_string()
                .contains("complete canvas")
        );
    }

    #[test]
    fn disposal_cannot_expose_transparent_canvas_between_opaque_frames() {
        // Valid two-pixel red first frame, then a one-pixel blue update.
        // Browser disposal 2 clears the canvas before the partial update.
        let mut bytes = vec![
            71, 73, 70, 56, 57, 97, 2, 0, 1, 0, 128, 0, 0, 255, 0, 0, 0, 0, 255, 33, 249, 4, 8, 10,
            0, 0, 0, 44, 0, 0, 0, 0, 2, 0, 1, 0, 0, 2, 2, 4, 10, 0, 33, 249, 4, 4, 10, 0, 0, 0, 44,
            0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 76, 1, 0, 59,
        ];
        assert!(
            validate_gif(bytes.clone())
                .unwrap_err()
                .to_string()
                .contains("disposal")
        );
        bytes[22] = 12; // Restore-previous disposal receives the same conservative guard.
        assert!(validate_gif(bytes.clone()).is_err());
        bytes[22] = 4; // Keeping the opaque previous frame makes the partial update safe.
        assert!(validate_gif(bytes).is_ok());
    }

    #[test]
    fn malformed_lzw_and_missing_palette_entries_cannot_make_opaque_frames_transparent() {
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
