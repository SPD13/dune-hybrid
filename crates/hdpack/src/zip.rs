//! A minimal ZIP writer: stored (uncompressed) entries, which the web app
//! reads in place from the imported file. PNGs are already compressed.

use std::io::{self, Seek, Write};

pub struct ZipWriter<W: Write + Seek> {
    out: W,
    entries: Vec<(String, u32, u32, u32)>,
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    !data.iter().fold(!0u32, |c, &b| table[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8))
}

impl<W: Write + Seek> ZipWriter<W> {
    pub fn new(out: W) -> Self {
        ZipWriter { out, entries: Vec::new() }
    }

    pub fn add(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        let offset = self.out.stream_position()? as u32;
        let crc = crc32(data);
        let len = data.len() as u32;
        let mut h = Vec::with_capacity(30 + name.len());
        h.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        h.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]); // version, flags, method 0, time, date
        h.extend_from_slice(&crc.to_le_bytes());
        h.extend_from_slice(&len.to_le_bytes());
        h.extend_from_slice(&len.to_le_bytes());
        h.extend_from_slice(&(name.len() as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(name.as_bytes());
        self.out.write_all(&h)?;
        self.out.write_all(data)?;
        self.entries.push((name.to_string(), crc, len, offset));
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        let cd_start = self.out.stream_position()? as u32;
        for (name, crc, len, offset) in &self.entries {
            let mut c = Vec::with_capacity(46 + name.len());
            c.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            c.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]); // made by, needed, flags, method, time, date
            c.extend_from_slice(&crc.to_le_bytes());
            c.extend_from_slice(&len.to_le_bytes());
            c.extend_from_slice(&len.to_le_bytes());
            c.extend_from_slice(&(name.len() as u16).to_le_bytes());
            c.extend_from_slice(&[0; 12]); // extra, comment, disk, internal attr, external attr
            c.extend_from_slice(&offset.to_le_bytes());
            c.extend_from_slice(name.as_bytes());
            self.out.write_all(&c)?;
        }
        let cd_len = self.out.stream_position()? as u32 - cd_start;
        let n = self.entries.len() as u16;
        let mut e = Vec::with_capacity(22);
        e.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        e.extend_from_slice(&[0, 0, 0, 0]);
        e.extend_from_slice(&n.to_le_bytes());
        e.extend_from_slice(&n.to_le_bytes());
        e.extend_from_slice(&cd_len.to_le_bytes());
        e.extend_from_slice(&cd_start.to_le_bytes());
        e.extend_from_slice(&0u16.to_le_bytes());
        self.out.write_all(&e)?;
        Ok(self.out)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc_of_known_string() {
        assert_eq!(super::crc32(b"123456789"), 0xcbf4_3926);
    }
}
