//! Browser host: wraps [`pc::Machine`] for a Web Worker. The worker calls
//! [`Emu::run_ms`] on a timer, ships [`Emu::frame`] to the page and forwards
//! input. DUNE.DAT is read synchronously from the user's `Blob` with
//! `FileReaderSync`; save files live in memory and are reported to the page.

#[cfg(target_arch = "wasm32")]
mod host;

#[cfg(target_arch = "wasm32")]
pub use host::*;
