//! Sprite pixel data: 4-bit or 8-bit, raw or RLE.
//!
//! Rows of a 4-bit sprite are `2 * ceil(w / 4)` bytes (whole 16-bit words of
//! four pixels), low nibble first. RLE codes, per row: a byte `n < 80h` is
//! followed by `n + 1` literal bytes; `n >= 80h` by one byte repeated
//! `101h - n` times. 8-bit sprites have `w`-byte rows and the same codes.

use crate::ops::Format;

/// A decoded sprite in its own coordinates: palette-local values (nibbles
/// 0-15 for 4-bit sprites, bytes for 8-bit ones), `stride` ≥ width.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub px: Vec<u8>,
}

/// Bytes per row of source data.
pub fn row_bytes(format: Format, width: u16) -> usize {
    let w = (width & 0x1ff) as usize;
    match format {
        Format::Nibble { .. } => w.div_ceil(4) * 2,
        _ => w,
    }
}

/// Pixels per decoded row (4-bit rows are padded to a multiple of 4).
pub fn row_pixels(format: Format, width: u16) -> usize {
    match format {
        Format::Nibble { .. } => row_bytes(format, width) * 2,
        _ => (width & 0x1ff) as usize,
    }
}

/// Expand one row of RLE data into `out` (row bytes); returns bytes consumed,
/// or None if the codes overrun the row (which the driver would smear).
pub fn unrle_row(data: &[u8], row_bytes: usize, out: &mut Vec<u8>) -> Option<usize> {
    let mut i = 0;
    let start = out.len();
    while out.len() - start < row_bytes {
        let n = *data.get(i)?;
        i += 1;
        if n < 0x80 {
            let count = n as usize + 1;
            out.extend_from_slice(data.get(i..i + count)?);
            i += count;
        } else {
            let count = 0x101 - n as usize;
            let v = *data.get(i)?;
            i += 1;
            out.extend(std::iter::repeat_n(v, count));
        }
    }
    (out.len() - start == row_bytes).then_some(i)
}

/// Decode a sprite. `data` starts at its pixel data; returns the image and
/// the number of data bytes it used.
pub fn decode(data: &[u8], wflags: u16, height: u8, format: Format) -> Option<(Image, usize)> {
    let rb = row_bytes(format, wflags);
    let h = height as usize;
    let mut bytes = Vec::with_capacity(rb * h);
    let used = if wflags & 0x8000 != 0 {
        let mut pos = 0;
        for _ in 0..h {
            pos += unrle_row(&data[pos.min(data.len())..], rb, &mut bytes)?;
        }
        pos
    } else {
        bytes.extend_from_slice(data.get(..rb * h)?);
        rb * h
    };
    let width = (wflags & 0x1ff) as usize;
    let image = match format {
        Format::Nibble { .. } => {
            let stride = rb * 2;
            let px = bytes.iter().flat_map(|&b| [b & 15, b >> 4]).collect();
            Image { width, height: h, stride, px }
        }
        _ => Image { width, height: h, stride: rb, px: bytes },
    };
    Some((image, used))
}
