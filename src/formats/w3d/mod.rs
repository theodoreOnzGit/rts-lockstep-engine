// SPDX-License-Identifier: GPL-3.0-only
//! W3D files: the format definitions ([`w3d_file`]) and the loaders WW3D2
//! reads meshes ([`mesh`]), hierarchies ([`htree`]), hierarchical LOD
//! models ([`hlod`]), collections ([`collect`]), aggregates ([`agg_def`]),
//! the box, sphere and ring primitives ([`boxrobj`], [`sphereobj`],
//! [`ringobj`], with [`prim_anim`]) and animations ([`hanim`], [`motchan`],
//! [`hmorphanim`]) with, into plain structs ~~(no rendering)~~. All of it reads
//! through the WWLib chunk reader, [`crate::ww::chunkio`]. **Corrected
//! 2026-11-05 (#10, step 4):** the render side without a device is
//! translated too: the shader's device states ([`shader`]), vertex materials
//! ([`vertmaterial`]), a mesh finished for drawing and split into the DX8
//! renderer's polygon groups, with the skin deformation ([`meshmdl`]), and an
//! HLOD bound to its bones, posed, with its textures found and decoded
//! ([`render`]). [`synth`] writes W3D files for tests and placeholder art. ~~The asset
//! manager's file loop (`WW3DAssetManager::Load_3D_Assets`, which picks the
//! loader by top-level chunk id and registers the result) is not
//! translated: each loader is called on its own chunk.~~ **Corrected
//! 2026-11-05 (#10, step 2):** the asset manager's loading side
//! ([`assetmgr`]: `Load_3D_Assets`, registration and lookup by name, loading
//! `<name>.w3d` on demand) picks the loader by top-level chunk id.
//!
//! The loaders return `W3dResult<Option<T>>`: `Ok(Some(_))` is upstream's
//! successful load, `Ok(None)` its load error (the reader is left where
//! upstream leaves it, which can be inside chunks it did not close), and
//! `Err` an input on which upstream is undefined.

pub mod agg_def;
pub mod assetmgr;
pub mod boxrobj;
pub mod collect;
pub mod hanim;
pub mod hlod;
pub mod hmorphanim;
pub mod htree;
pub mod mesh;
pub mod meshmdl;
pub mod motchan;
pub mod prim_anim;
pub mod render;
pub mod ringobj;
pub mod shader;
pub mod sphereobj;
pub mod synth;
pub mod vertmaterial;
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
    /// A `float` converted to an integer type that cannot hold its
    /// truncation, or a NaN converted (the animation evaluation's frame
    /// numbers; the 32-bit original's `long` is 32 bits).
    FloatToInt,
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
            W3dError::Undefined(u) => write!(f, "W3D data outside what the original loader defines: {u:?}"),
            W3dError::Unsupported(what) => write!(f, "W3D data this reader does not support yet: {what}"),
            W3dError::NonTerminating => write!(f, "W3D data on which the original loader loops forever"),
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
pub(crate) fn read_array(cload: &mut ChunkLoad<'_>, nbytes: u32, capacity: usize) -> W3dResult<Option<Vec<u8>>> {
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

/// `cload.Read(buf, nbytes)` into a zero-filled buffer of `nbytes` bytes
/// whose contents are kept whatever the read returns (the game's allocator
/// zero-fills; a read that fits the chunk but finds the file short still
/// copies what the file had). Returns whether the read succeeded and the
/// buffer's first bytes: at most the bytes left in the file, the rest of the
/// `nbytes` being zeros. `nbytes` must be at most `i32::MAX` (the callers
/// check the allocation first).
pub(crate) fn read_partial(cload: &mut ChunkLoad<'_>, nbytes: u32) -> W3dResult<(bool, Vec<u8>)> {
    let f = cload.file();
    let left = f.file_size().wrapping_sub(f.position()).max(0) as usize;
    let mut buf = vec![0u8; (nbytes as usize).min(left)];
    let n = cload.read(&mut buf, nbytes)?;
    Ok((n == nbytes, buf))
}

/// `(int)f` (and Win32's `(long)f`): defined when the truncation fits an
/// `int`; anything else (NaN included) is undefined.
pub(crate) fn f32_to_i32(f: f32) -> W3dResult<i32> {
    let d = f64::from(f);
    if d > -2_147_483_649.0 && d < 2_147_483_648.0 {
        Ok(f as i32)
    } else {
        Err(W3dUndefined::FloatToInt.into())
    }
}

/// `WWMath::Float_To_Long(f)` into an `int`, in the branch `mode` names
/// (GitHub #19). The portable `(long) f` is undefined in C++ when the
/// truncation does not fit the Win32 32-bit `long` (NaN included):
/// [`W3dUndefined::FloatToInt`]. The Win32 `fistp` is defined for every
/// input (the integer indefinite `i32::MIN` outside the range; see
/// [`crate::ww::wwmath::float_to_long_fistp`]).
pub(crate) fn float_to_long(f: f32, mode: crate::ww::wwmath::FloatToLongMode) -> W3dResult<i32> {
    use crate::ww::wwmath::{self, FloatToLongMode};
    match mode {
        FloatToLongMode::Win32Fistp => Ok(wwmath::float_to_long_fistp(f)),
        FloatToLongMode::PortableTruncate => {
            f32_to_i32(f)?;
            Ok(wwmath::float_to_long(f))
        }
    }
}

/// `(uint32)f`: defined when the truncation fits (so `-1 < f < 2^32`).
pub(crate) fn f32_to_u32(f: f32) -> W3dResult<u32> {
    let d = f64::from(f);
    if d > -1.0 && d < 4_294_967_296.0 {
        Ok(f as u32)
    } else {
        Err(W3dUndefined::FloatToInt.into())
    }
}

/// The mutable caches upstream's evaluation keeps inside a loaded object
/// (`TimeCodedMotionChannelClass::CachedIdx`, the adaptive-delta
/// `CacheFrame`/`CacheData`, ...), which its `const` evaluation functions
/// change through non-`const` pointers. Behind a mutex so that loaded assets
/// stay `Send + Sync`; the results still depend on the order of the calls,
/// as upstream's do.
#[derive(Default)]
pub struct EvalCache<T>(std::sync::Mutex<T>);

impl<T> EvalCache<T> {
    pub fn new(v: T) -> Self {
        EvalCache(std::sync::Mutex::new(v))
    }

    /// The cached state, locked.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, T> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl<T: Clone> Clone for EvalCache<T> {
    fn clone(&self) -> Self {
        EvalCache::new(self.lock().clone())
    }
}

impl<T: PartialEq> PartialEq for EvalCache<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other) || *self.lock() == *other.lock()
    }
}

impl<T: Eq> Eq for EvalCache<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for EvalCache<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.lock().fmt(f)
    }
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
