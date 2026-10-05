//! Sprite identity: the same art is recognised wherever it is drawn from.

/// FNV-1a, 64-bit.
pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Identity of a sprite: its pixel data, width and height (not where or how
/// it is drawn: position, flips and palette offset are per draw).
pub fn sprite_hash(data: &[u8], width: u16, height: u8) -> u64 {
    let mut h = fnv64(data);
    h ^= ((width & 0x1ff) as u64) << 8 | height as u64;
    h.wrapping_mul(0x0100_0000_01b3)
}
