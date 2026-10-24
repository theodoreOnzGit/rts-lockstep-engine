// SPDX-License-Identifier: GPL-3.0-only
//! W3D files: the format definitions ([`w3d_file`]) and the loaders WW3D2
//! reads meshes ([`mesh`]), hierarchies ([`htree`]) and animations
//! ([`hanim`], [`motchan`]) with, into plain structs (no rendering). All of
//! it reads through the WWLib chunk reader, [`crate::ww::chunkio`]. The
//! asset manager's file loop (`WW3DAssetManager::Load_3D_Assets`, which
//! picks the loader by top-level chunk id and registers the result) is not
//! translated: each loader is called on its own chunk.
//!
//! The loaders return `W3dResult<Option<T>>`: `Ok(Some(_))` is upstream's
//! successful load, `Ok(None)` its load error (the reader is left where
//! upstream leaves it, which can be inside chunks it did not close), and
//! `Err` an input on which upstream is undefined.

pub mod hanim;
pub mod htree;
pub mod mesh;
pub mod motchan;
pub mod w3d_file;

use crate::ww::chunkio::{ChunkIoError, ChunkIoUndefined, ChunkLoad};

/// The largest single allocation the 32-bit original can make; a larger
/// request either overflows its `new[]` size computation or fails.
pub const WIN32_MAX_ALLOCATION: u64 = 0x7FFF_FFFF;

/// Where upstream is undefined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum W3dUndefined {
    /// From the chunk reader.
    ChunkIo(ChunkIoUndefined),
    /// An allocation the 32-bit original cannot make (or a negative count).
    Allocation,
    /// The first element written into a zero-length array.
    WriteToEmptyArray,
    /// A value used without being initialised.
    UninitialisedRead,
    /// `Pivot[0]` of an hierarchy with no pivots.
    NoPivots,
    /// A pointer formed outside its array.
    PointerOutOfRange,
    /// A null pointer dereferenced (a geometry array that was not allocated).
    NullPointer,
    /// A C string that runs past the end of the struct it is read from.
    StringOverrun,
    /// A C string copied into a buffer too small for it.
    StringOverflow,
    /// An array index outside the array.
    IndexOutOfRange,
    /// An array deleted twice (an upstream bug, see [`hanim`]).
    DoubleDelete,
}

/// A W3D loading error: only where upstream is undefined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum W3dError {
    Undefined(W3dUndefined),
    /// A part of the format this translation does not read yet (named).
    Unsupported(&'static str),
    /// Upstream loops forever on this input ([`ChunkIoError::NonTerminating`]).
    NonTerminating,
}

impl From<W3dUndefined> for W3dError {
    fn from(u: W3dUndefined) -> Self {
        W3dError::Undefined(u)
    }
}

impl From<ChunkIoError> for W3dError {
    fn from(e: ChunkIoError) -> Self {
        match e {
            ChunkIoError::Undefined(u) => W3dError::Undefined(W3dUndefined::ChunkIo(u)),
            ChunkIoError::NonTerminating => W3dError::NonTerminating,
        }
    }
}

impl std::fmt::Display for W3dError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            W3dError::Undefined(u) => write!(
                f,
                "W3D data outside what the original loader defines: {u:?}"
            ),
            W3dError::Unsupported(what) => {
                write!(f, "W3D data this reader does not support yet: {what}")
            }
            W3dError::NonTerminating => {
                write!(f, "W3D data on which the original loader loops forever")
            }
        }
    }
}

impl std::error::Error for W3dError {}

/// Result of the W3D loaders.
pub type W3dResult<T> = Result<T, W3dError>;

/// `cload.Read(array, nbytes)` into an array of `capacity` bytes whose
/// contents are discarded unless the read succeeds: `Some(bytes)` on
/// success, `None` on a failed read. A read that would copy more than
/// `capacity` bytes is undefined; so is a "successful" negative-length read
/// (nothing copied, the array left uninitialised). Memory is bounded by the
/// bytes left in the file, not by `nbytes`.
pub(crate) fn read_array(
    cload: &mut ChunkLoad<'_>,
    nbytes: u32,
    capacity: usize,
) -> W3dResult<Option<Vec<u8>>> {
    let f = cload.file();
    let left = f.file_size().wrapping_sub(f.position()).max(0) as usize;
    let mut buf = vec![0u8; capacity.min(left)];
    let n = cload.read(&mut buf, nbytes)?;
    if n != nbytes {
        return Ok(None);
    }
    if nbytes as i32 <= 0 && nbytes != 0 {
        return Err(W3dUndefined::UninitialisedRead.into());
    }
    buf.truncate(nbytes as usize);
    Ok(Some(buf))
}

/// The C string starting at `buf[start]`, as `strlen`/`strcpy` read it:
/// up to the first 0 byte, which must lie inside `buf` (the struct it is in).
pub(crate) fn c_string(buf: &[u8], start: usize) -> W3dResult<&[u8]> {
    let s = &buf[start..];
    match s.iter().position(|&c| c == 0) {
        Some(n) => Ok(&s[..n]),
        None => Err(W3dUndefined::StringOverrun.into()),
    }
}
