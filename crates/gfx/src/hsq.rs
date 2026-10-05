//! HSQ: Cryo's LZ77 variant with a 6-byte header.
//!
//! Header: `u16` unpacked size, `u8` zero, `u16` packed size, `u8` checksum byte
//! chosen so the six header bytes sum to 0xAB (mod 256). The body interleaves a
//! 16-bit little-endian bit queue (refilled on demand) with literal/offset bytes.

use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum HsqError {
    BadHeader,
    Truncated,
    BadBackReference { at: usize, offset: isize },
}

impl fmt::Display for HsqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HsqError::BadHeader => write!(f, "not an HSQ stream"),
            HsqError::Truncated => write!(f, "HSQ stream truncated"),
            HsqError::BadBackReference { at, offset } => {
                write!(f, "HSQ back reference {offset} before start at output {at}")
            }
        }
    }
}

impl std::error::Error for HsqError {}

/// True if `data` carries a valid HSQ header.
pub fn is_hsq(data: &[u8]) -> bool {
    data.len() >= 6
        && data[..6].iter().fold(0u8, |a, &b| a.wrapping_add(b)) == 0xAB
        && u16::from_le_bytes([data[0], data[1]]) != 0
}

/// Unpacked size declared by the header.
pub fn unpacked_len(data: &[u8]) -> usize {
    u16::from_le_bytes([data[0], data[1]]) as usize
}

/// Decompress if `data` is HSQ, otherwise return it unchanged.
pub fn unpack_if_hsq(data: &[u8]) -> Result<Vec<u8>, HsqError> {
    if is_hsq(data) { decompress(data) } else { Ok(data.to_vec()) }
}

struct Reader<'a> {
    src: &'a [u8],
    pos: usize,
    queue: u32,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, HsqError> {
        let b = *self.src.get(self.pos).ok_or(HsqError::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    fn bit(&mut self) -> Result<bool, HsqError> {
        if self.queue == 1 {
            let lo = self.byte()? as u32;
            let hi = self.byte()? as u32;
            self.queue = 0x1_0000 | lo | (hi << 8);
        }
        let bit = self.queue & 1 != 0;
        self.queue >>= 1;
        Ok(bit)
    }
}

/// Decompress an HSQ stream (header included).
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, HsqError> {
    if !is_hsq(data) {
        return Err(HsqError::BadHeader);
    }
    let len = unpacked_len(data);
    let mut out: Vec<u8> = Vec::with_capacity(len);
    let mut r = Reader { src: data, pos: 6, queue: 1 };
    loop {
        if r.bit()? {
            out.push(r.byte()?);
            continue;
        }
        let (count, offset) = if r.bit()? {
            let lo = r.byte()? as usize;
            let hi = r.byte()? as usize;
            let word = lo | (hi << 8);
            let mut count = word & 7;
            let offset = (word >> 3) as isize - 8192;
            if count == 0 {
                count = r.byte()? as usize;
                if count == 0 {
                    break;
                }
            }
            (count + 2, offset)
        } else {
            let mut count = (r.bit()? as usize) << 1;
            count |= r.bit()? as usize;
            let offset = r.byte()? as isize - 256;
            (count + 2, offset)
        };
        let from = out.len() as isize + offset;
        if from < 0 {
            return Err(HsqError::BadBackReference { at: out.len(), offset });
        }
        for i in 0..count {
            let b = out[from as usize + i];
            out.push(b);
        }
    }
    if out.len() != len {
        // Some archives declare a size that differs from the stream; trust the header.
        out.resize(len, 0);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a header for an unpacked length, fixing the checksum byte.
    fn header(unpacked: u16, packed: u16) -> Vec<u8> {
        let mut h = vec![unpacked as u8, (unpacked >> 8) as u8, 0, packed as u8, (packed >> 8) as u8, 0];
        let sum = h.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        h[5] = 0xABu8.wrapping_sub(sum);
        h
    }

    #[test]
    fn literals_and_short_copy() {
        // bits (LSB first): 1 lit 'A', 1 lit 'B', 0 0 short: count bits 1,1 -> 3+2=5, offset byte 0xFE (-2)
        // then 0 1 long with count 0 + extra 0 => end.
        // bit sequence: 1,1,0,0,1,1,0,1 => 0b1011_0011 = 0xB3, high byte 0
        let mut s = header(7, 0);
        s.extend([0xB3, 0x00, b'A', b'B', 0xFE, 0x00, 0x00, 0x00]);
        assert_eq!(decompress(&s).unwrap(), b"ABABABA");
    }

    #[test]
    fn rejects_non_hsq() {
        assert_eq!(decompress(&[0; 8]), Err(HsqError::BadHeader));
        assert_eq!(unpack_if_hsq(&[1, 2, 3]).unwrap(), vec![1, 2, 3]);
    }
}
