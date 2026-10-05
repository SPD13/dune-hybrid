//! Files the DOS layer can open. The host decides where they come from: a
//! directory natively, the user's DUNE.DAT `Blob` plus in-memory saves in the
//! browser.

use std::{
    collections::HashMap,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

/// An open file.
pub trait DosFile: Read + Write + Seek {
    /// Called on close (and on exit) so writable files can be persisted.
    fn close(&mut self) {}
}

impl DosFile for std::fs::File {}

/// File provider. Names arrive normalised: upper case, no drive or path.
pub trait FileSystem {
    fn open(&mut self, name: &str) -> Option<Box<dyn DosFile>>;
    fn create(&mut self, name: &str) -> Option<Box<dyn DosFile>>;
    fn exists(&mut self, name: &str) -> bool;
}

/// Strip drive and directories, upper-case.
pub fn normalize(name: &str) -> String {
    let base = name.rsplit(['\\', '/', ':']).next().unwrap_or(name);
    base.trim().to_ascii_uppercase()
}

/// Native provider: a directory on disk (names matched case-insensitively).
pub struct DirFs {
    pub root: PathBuf,
}

impl DirFs {
    fn find(&self, name: &str) -> Option<PathBuf> {
        let rd = std::fs::read_dir(&self.root).ok()?;
        rd.flatten()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
            .map(|e| e.path())
    }
}

impl FileSystem for DirFs {
    fn open(&mut self, name: &str) -> Option<Box<dyn DosFile>> {
        let path = self.find(name)?;
        let f = std::fs::File::open(&path).ok()?;
        Some(Box::new(f))
    }

    fn create(&mut self, name: &str) -> Option<Box<dyn DosFile>> {
        let path = self.find(name).unwrap_or_else(|| self.root.join(name));
        let f = std::fs::File::create(path).ok()?;
        Some(Box::new(f))
    }

    fn exists(&mut self, name: &str) -> bool {
        self.find(name).is_some()
    }
}

/// An in-memory file whose contents are handed to `on_close` when closed.
pub struct MemFile {
    pub data: Cursor<Vec<u8>>,
    pub name: String,
    pub dirty: bool,
    pub on_close: Option<Box<dyn FnMut(&str, &[u8])>>,
}

impl Read for MemFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.data.read(buf)
    }
}

impl Write for MemFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.dirty = true;
        self.data.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for MemFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.data.seek(pos)
    }
}

impl DosFile for MemFile {
    fn close(&mut self) {
        if self.dirty {
            if let Some(cb) = &mut self.on_close {
                cb(&self.name, self.data.get_ref());
            }
            self.dirty = false;
        }
    }
}

/// A read-only provider over named byte buffers (tests, small files).
#[derive(Default)]
pub struct MemFs {
    pub files: HashMap<String, Vec<u8>>,
}

impl FileSystem for MemFs {
    fn open(&mut self, name: &str) -> Option<Box<dyn DosFile>> {
        let data = self.files.get(name)?.clone();
        Some(Box::new(MemFile { data: Cursor::new(data), name: name.into(), dirty: false, on_close: None }))
    }
    fn create(&mut self, name: &str) -> Option<Box<dyn DosFile>> {
        self.files.insert(name.into(), Vec::new());
        Some(Box::new(MemFile { data: Cursor::new(Vec::new()), name: name.into(), dirty: false, on_close: None }))
    }
    fn exists(&mut self, name: &str) -> bool {
        self.files.contains_key(name)
    }
}

/// Reads game files from `game`, but creates and prefers files in `saves`, so
/// the game directory is never written to.
pub struct OverlayFs {
    pub game: DirFs,
    pub saves: DirFs,
}

impl FileSystem for OverlayFs {
    fn open(&mut self, name: &str) -> Option<Box<dyn DosFile>> {
        self.saves.open(name).or_else(|| self.game.open(name))
    }
    fn create(&mut self, name: &str) -> Option<Box<dyn DosFile>> {
        self.saves.create(name)
    }
    fn exists(&mut self, name: &str) -> bool {
        self.saves.exists(name) || self.game.exists(name)
    }
}
