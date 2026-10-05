//! DUNE.DAT: a table of contents (u16 count, then 25-byte entries: 16-byte
//! zero-padded name, u32 size, u32 offset, one flag byte) followed by the
//! resources, most of them HSQ-compressed.

use std::io::{Read, Seek, SeekFrom};

use crate::hsq;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Position in the table (the game's resource numbers are indices).
    pub index: usize,
    pub name: String,
    pub size: u32,
    pub offset: u32,
}

/// Read the table of contents.
pub fn toc<R: Read>(r: &mut R) -> std::io::Result<Vec<Entry>> {
    let mut head = [0u8; 2];
    r.read_exact(&mut head)?;
    let count = u16::from_le_bytes(head) as usize;
    let mut raw = vec![0u8; count * 25];
    r.read_exact(&mut raw)?;
    Ok(raw
        .as_chunks::<25>()
        .0
        .iter()
        .enumerate()
        .filter_map(|(index, e)| {
            let name: String = e[..16].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
            (!name.is_empty()).then(|| Entry {
                index,
                name,
                size: u32::from_le_bytes(e[16..20].try_into().unwrap()),
                offset: u32::from_le_bytes(e[20..24].try_into().unwrap()),
            })
        })
        .collect())
}

/// A resource's stored bytes.
pub fn raw<R: Read + Seek>(r: &mut R, e: &Entry) -> std::io::Result<Vec<u8>> {
    let mut data = vec![0u8; e.size as usize];
    r.seek(SeekFrom::Start(e.offset as u64))?;
    r.read_exact(&mut data)?;
    Ok(data)
}

/// A resource's contents (decompressed if HSQ).
pub fn load<R: Read + Seek>(r: &mut R, e: &Entry) -> std::io::Result<Vec<u8>> {
    let data = raw(r, e)?;
    hsq::unpack_if_hsq(&data).map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}: {err}", e.name)))
}
