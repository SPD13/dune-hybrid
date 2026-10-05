//! Machine snapshots: the complete emulated PC (CPU, memory, devices, open
//! files) as a compact byte blob. Taken between instructions, so restoring one
//! resumes exactly where it was taken — the basis of resume and quick save.

use std::collections::VecDeque;

const MAGIC: &[u8; 4] = b"DHS3";

#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.buf.push(v as u8);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn bytes(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.buf.extend_from_slice(v);
    }
    pub fn str(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }
    pub fn deque(&mut self, v: &VecDeque<u8>) {
        self.u32(v.len() as u32);
        self.buf.extend(v.iter());
    }
}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

pub type Result<T> = std::result::Result<T, String>;

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = self.data.get(self.pos..self.pos + n).ok_or("snapshot truncated")?;
        self.pos += n;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn bytes(&mut self) -> Result<&'a [u8]> {
        let n = self.u32()? as usize;
        self.take(n)
    }
    pub fn str(&mut self) -> Result<String> {
        Ok(String::from_utf8_lossy(self.bytes()?).into_owned())
    }
    pub fn deque(&mut self) -> Result<VecDeque<u8>> {
        Ok(self.bytes()?.iter().copied().collect())
    }
}

/// Wrap a raw state in the snapshot container (magic, program id, deflate).
pub fn pack(program_id: u64, raw: &[u8]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&program_id.to_le_bytes());
    out.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(raw, 3));
    out
}

/// Unwrap a snapshot; fails if it was taken with a different program.
pub fn unpack(program_id: u64, data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 12 || &data[..4] != MAGIC {
        return Err("not a snapshot".into());
    }
    let id = u64::from_le_bytes(data[4..12].try_into().unwrap());
    if id != program_id {
        return Err("snapshot belongs to a different DNCDPRG.EXE".into());
    }
    miniz_oxide::inflate::decompress_to_vec(&data[12..]).map_err(|e| format!("snapshot corrupt: {e:?}"))
}

/// FNV-1a, to tie snapshots to the program they were taken with.
pub fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
