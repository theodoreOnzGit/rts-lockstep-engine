// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/XferCRC.h
//   GeneralsMD/Code/GameEngine/Include/Common/XferDeepCRC.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/XferCRC.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Authors: Colin Day (headers), Matt Campbell (XferCRC.cpp), February 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `XferCRC`: the checksum lockstep peers compare to detect a desync, and
//! `XferDeepCRC`, which also writes everything it hashes.
//!
//! # The checksum, exactly
//!
//! This is **not** the byte-wise [`super::crc::Crc`]. `XferCRC` folds 32-bit
//! words: for each value `w`, `crc = (crc << 1) + w + (old crc >> 31)`
//! (mod 2^32). Upstream reads each 4-byte group of the data as a native
//! (little-endian, on x86) `UnsignedInt` and byte-swaps it with `htonl`
//! before adding, so a full group enters as its **big-endian** value. The 1-3
//! leftover bytes `c[i]` are first assembled as `sum(c[i] << 8*i)`, then
//! swapped by `htonl` twice (once in `xferImplementation`, once in
//! `addCRC`), so they enter as their **little-endian** value. `getCRC`
//! returns the accumulator byte-swapped (`htonl(m_crc)`). This translation
//! computes those values from the bytes explicitly, so it gives upstream's
//! x86 result on every host.

use super::xfer::{Snapshot, Xfer, XferMode, XferResult, XferState, XferStatus};

/// `class XferCRC`.
#[derive(Clone, Debug)]
pub struct XferCrc {
    state: XferState,
    crc: u32,
    /// PROPOSAL: whether [`nan_guard`] applies to `xfer_real` (see there).
    nan_guard: bool,
}

impl Default for XferCrc {
    fn default() -> Self {
        Self::new()
    }
}

impl XferCrc {
    /// `XferCRC()`: mode `XFER_CRC`, CRC zero.
    pub fn new() -> Self {
        XferCrc { state: XferState::new(XferMode::Crc), crc: 0, nan_guard: true }
    }

    /// PROPOSAL: an `XferCRC` without the NaN guard, hashing every `Real`'s
    /// raw bits exactly as upstream does (used by the differential tests).
    pub fn new_without_nan_guard() -> Self {
        XferCrc { nan_guard: false, ..Self::new() }
    }

    /// `XferCRC::addCRC(val)`, with `val` already the value upstream adds
    /// after its `htonl`.
    #[inline]
    fn add_crc_swapped(&mut self, val: u32) {
        let hibit: u32 = if self.crc & 0x8000_0000 != 0 { 1 } else { 0 };
        self.crc <<= 1;
        self.crc = self.crc.wrapping_add(val);
        self.crc = self.crc.wrapping_add(hibit);
    }

    /// `XferCRC::xferImplementation`: fold `data` into the CRC (empty data is
    /// ignored, as upstream's `dataSize < 1` early return).
    pub fn add_bytes(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let mut words = data.chunks_exact(4);
        for w in &mut words {
            // *uintPtr (little-endian load) then htonl: the big-endian value.
            self.add_crc_swapped(u32::from_be_bytes([w[0], w[1], w[2], w[3]]));
        }
        let rest = words.remainder();
        if !rest.is_empty() {
            let mut val: u32 = 0;
            for (i, &c) in rest.iter().enumerate() {
                val = val.wrapping_add(u32::from(c) << (i * 8));
            }
            // htonl here and again inside addCRC: the value enters unswapped.
            self.add_crc_swapped(val);
        }
    }

    /// `XferCRC::getCRC`: `htonl(m_crc)`, "in network byte order".
    pub fn get_crc(&self) -> u32 {
        self.crc.swap_bytes()
    }

    /// `m_crc` itself (before the `htonl` of `getCRC`).
    pub fn raw_crc(&self) -> u32 {
        self.crc
    }
}

impl Xfer for XferCrc {
    fn state(&self) -> &XferState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut XferState {
        &mut self.state
    }

    /// `XferCRC::open`: base open, then the CRC restarts at zero.
    fn open(&mut self, identifier: &[u8]) -> XferResult<()> {
        self.state.open(identifier);
        self.crc = 0;
        Ok(())
    }
    /// `XferCRC::close`: nothing.
    fn close(&mut self) -> XferResult<()> {
        Ok(())
    }
    /// `XferCRC::beginBlock`: returns 0.
    fn begin_block(&mut self) -> XferResult<i32> {
        Ok(0)
    }
    /// `XferCRC::endBlock`: nothing.
    fn end_block(&mut self) -> XferResult<()> {
        Ok(())
    }
    /// `XferCRC::skip`: nothing.
    fn skip(&mut self, _data_size: i32) -> XferResult<()> {
        Ok(())
    }
    /// `XferCRC::xferSnapshot`: runs the snapshot's "light" `crc`.
    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshot) -> XferResult<()> {
        snapshot.crc(self)
    }
    fn xfer_implementation(&mut self, data: &mut [u8]) -> XferResult<()> {
        self.add_bytes(data);
        Ok(())
    }

    /// `Xfer::xferReal`, with the PROPOSED [`nan_guard`] unless built with
    /// [`XferCrc::new_without_nan_guard`]. The caller's value is not changed.
    fn xfer_real(&mut self, v: &mut f32) -> XferResult<()> {
        let bits = if self.nan_guard { nan_guard(*v).to_bits() } else { v.to_bits() };
        self.xfer_implementation(&mut bits.to_le_bytes())
    }
}

/// `class XferDeepCRC : public XferCRC`: CRCs like `XferCRC`, but its mode
/// is `XFER_SAVE` (so snapshots take their save paths) and it also writes
/// every hashed byte, with save-style length-prefixed strings. Upstream
/// writes to a file (`fopen(identifier, "w+b")`); here the "file" is an
/// in-memory buffer, [`XferDeepCrc::bytes`].
#[derive(Clone, Debug)]
pub struct XferDeepCrc {
    inner: XferCrc,
    file: Option<Vec<u8>>,
    last_file: Vec<u8>,
}

impl Default for XferDeepCrc {
    fn default() -> Self {
        Self::new()
    }
}

impl XferDeepCrc {
    /// `XferDeepCRC()`: mode `XFER_SAVE`, no file.
    pub fn new() -> Self {
        let mut inner = XferCrc::new();
        inner.state.mode = XferMode::Save;
        XferDeepCrc { inner, file: None, last_file: Vec::new() }
    }

    /// PROPOSAL: an `XferDeepCRC` without the NaN guard (upstream's raw bits).
    pub fn new_without_nan_guard() -> Self {
        let mut d = Self::new();
        d.inner.nan_guard = false;
        d
    }

    /// `getCRC` (inherited from `XferCRC`).
    pub fn get_crc(&self) -> u32 {
        self.inner.get_crc()
    }

    /// The bytes written so far (the open file), or those of the last closed
    /// one.
    pub fn bytes(&self) -> &[u8] {
        self.file.as_deref().unwrap_or(&self.last_file)
    }
}

impl Xfer for XferDeepCrc {
    fn state(&self) -> &XferState {
        &self.inner.state
    }
    fn state_mut(&mut self) -> &mut XferState {
        &mut self.inner.state
    }

    /// `XferDeepCRC::open`: mode `XFER_SAVE`, refuse a second open, base
    /// open, new empty file, CRC zero.
    fn open(&mut self, identifier: &[u8]) -> XferResult<()> {
        self.inner.state.mode = XferMode::Save;
        if self.file.is_some() {
            return Err(XferStatus::FileAlreadyOpen);
        }
        self.inner.state.open(identifier);
        self.file = Some(Vec::new());
        self.inner.crc = 0;
        Ok(())
    }
    /// `XferDeepCRC::close`: `XFER_FILE_NOT_OPEN` if no file; else close it
    /// and clear the identifier.
    fn close(&mut self) -> XferResult<()> {
        match self.file.take() {
            None => Err(XferStatus::FileNotOpen),
            Some(f) => {
                self.last_file = f;
                self.inner.state.identifier.clear();
                Ok(())
            }
        }
    }
    fn begin_block(&mut self) -> XferResult<i32> {
        self.inner.begin_block()
    }
    fn end_block(&mut self) -> XferResult<()> {
        self.inner.end_block()
    }
    fn skip(&mut self, data_size: i32) -> XferResult<()> {
        self.inner.skip(data_size)
    }
    /// Inherited `XferCRC::xferSnapshot`: the snapshot's `crc`.
    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshot) -> XferResult<()> {
        snapshot.crc(self)
    }

    /// `XferDeepCRC::xferImplementation`: ignore empty data; write it (upstream
    /// asserts an open file and then `fwrite`s; without a file the write
    /// fails, `XFER_WRITE_ERROR`); then CRC it.
    fn xfer_implementation(&mut self, data: &mut [u8]) -> XferResult<()> {
        if data.is_empty() {
            return Ok(());
        }
        match self.file.as_mut() {
            None => return Err(XferStatus::WriteError),
            Some(f) => f.extend_from_slice(data),
        }
        self.inner.add_bytes(data);
        Ok(())
    }

    /// `Xfer::xferReal`, with the PROPOSED [`nan_guard`] unless built with
    /// [`XferDeepCrc::new_without_nan_guard`]. What is hashed is also what
    /// is written. The caller's value is not changed.
    fn xfer_real(&mut self, v: &mut f32) -> XferResult<()> {
        let bits = if self.inner.nan_guard { nan_guard(*v).to_bits() } else { v.to_bits() };
        self.xfer_implementation(&mut bits.to_le_bytes())
    }

    /// `XferDeepCRC::xferAsciiString`: at most 16385 bytes, an
    /// `UnsignedShort` length, then the bytes.
    fn xfer_ascii_string(&mut self, s: &mut Vec<u8>) -> XferResult<()> {
        if s.len() > 16385 {
            return Err(XferStatus::StringError);
        }
        let mut len = s.len() as u16;
        self.xfer_unsigned_short(&mut len)?;
        if len > 0 {
            self.xfer_user(&mut s[..usize::from(len)])?;
        }
        Ok(())
    }

    /// `XferDeepCRC::xferUnicodeString`: at most 255 characters, a `Byte`
    /// length, then the characters.
    ///
    /// `Byte` is a signed `char`, so for 128-255 characters the length byte
    /// is negative, `if( len > 0 )` fails, and upstream writes the length but
    /// **not the characters**. Kept as upstream.
    fn xfer_unicode_string(&mut self, s: &mut Vec<u16>) -> XferResult<()> {
        if s.len() > 255 {
            return Err(XferStatus::StringError);
        }
        let mut len = s.len() as u8 as i8;
        self.xfer_byte(&mut len)?;
        if len > 0 {
            let mut b: Vec<u8> = s.iter().flat_map(|c| c.to_le_bytes()).collect();
            self.xfer_user(&mut b)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PROPOSAL (not approved by the owner; kept in one commit so it can be
// reverted on its own): a NaN guard on the CRC path.
// ---------------------------------------------------------------------------

/// The one NaN bit pattern a guarded CRC hashes: the quiet NaN `0x7FC00000`
/// (positive, empty payload).
pub const CANONICAL_NAN_BITS: u32 = 0x7FC0_0000;

/// **PROPOSAL.** What a guarded `XferCRC`/`XferDeepCRC` hashes for a `Real`.
///
/// Why: `XferCRC` hashes the raw bytes of every float. IEEE 754 leaves the
/// sign and payload of a NaN produced by arithmetic unspecified, and so do
/// Rust and WebAssembly (wasm's NaN results are explicitly
/// non-deterministic in their bits), so a native peer and a wasm peer can run
/// identical logic, both produce "a NaN", and hash different bits: a desync
/// reported where the simulations agree, or, worse, logic whose result
/// depends on a NaN going unnoticed until the CRCs differ.
///
/// What it does:
/// - **debug builds and the crate's own tests** (`debug_assertions` or
///   `cfg(test)`): a NaN panics, with a message, because logic that feeds a
///   NaN into the lockstep state is a bug to find, not to hash;
/// - **release builds**: a NaN is replaced by [`CANONICAL_NAN_BITS`] before
///   hashing, so every peer hashes the same bits. Non-NaN values, including
///   infinities, signed zeros and subnormals, pass through unchanged.
///
/// This changes the CRC (and the deep-CRC file) only for inputs that contain
/// a NaN whose bits differ from the canonical one; for everything else it is
/// bit-identical to upstream. Upstream's raw behaviour stays available as
/// `XferCrc::new_without_nan_guard`.
#[inline]
pub fn nan_guard(v: f32) -> f32 {
    if v.is_nan() {
        if cfg!(any(debug_assertions, test)) {
            panic!(
                "XferCRC: NaN real ({:#010x}) xfered into a lockstep CRC; NaN bits are not portable between peers \
                 (native vs wasm), so this would desync. Fix the logic that produced it.",
                v.to_bits()
            );
        }
        return canonical_nan();
    }
    v
}

/// The release-build replacement of [`nan_guard`], on its own so it can be
/// tested in every build.
#[inline]
pub fn canonical_nan() -> f32 {
    f32::from_bits(CANONICAL_NAN_BITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_and_leftover_byte_order() {
        let mut x = XferCrc::new();
        x.open(b"t").unwrap();
        x.add_bytes(&[1, 2, 3, 4]);
        assert_eq!(x.raw_crc(), 0x0102_0304, "a full word enters big-endian");
        let mut y = XferCrc::new();
        y.add_bytes(&[1, 2]);
        assert_eq!(y.raw_crc(), 0x0201, "leftover bytes enter little-endian");
        assert_eq!(y.get_crc(), 0x0102_0000);
        let mut z = XferCrc { crc: 0x8000_0000, ..XferCrc::new() };
        z.add_bytes(&[0, 0, 0, 0]);
        assert_eq!(z.raw_crc(), 1);
    }

    #[test]
    fn nan_guard_passes_non_nan_through() {
        for v in [0.0f32, -0.0, 1.5, f32::INFINITY, f32::NEG_INFINITY, f32::MIN_POSITIVE / 2.0, f32::MAX] {
            assert_eq!(nan_guard(v).to_bits(), v.to_bits());
        }
        assert_eq!(canonical_nan().to_bits(), CANONICAL_NAN_BITS);
        assert!(canonical_nan().is_nan());
        let mut g = XferCrc::new();
        let mut u = XferCrc::new_without_nan_guard();
        for v in [1.0f32, -3.25e-40, f32::INFINITY] {
            g.xfer_real(&mut v.clone()).unwrap();
            u.xfer_real(&mut v.clone()).unwrap();
        }
        assert_eq!(g.get_crc(), u.get_crc());
    }

    #[test]
    #[should_panic(expected = "NaN real (0xffc00001) xfered into a lockstep CRC")]
    fn nan_into_crc_panics_in_tests() {
        let mut x = XferCrc::new();
        x.xfer_real(&mut f32::from_bits(0xFFC0_0001)).unwrap();
    }

    #[test]
    fn unguarded_crc_hashes_raw_nan_bits() {
        let mut a = XferCrc::new_without_nan_guard();
        let mut b = XferCrc::new_without_nan_guard();
        a.xfer_real(&mut f32::from_bits(0x7FC0_0000)).unwrap();
        b.xfer_real(&mut f32::from_bits(0xFFC0_0001)).unwrap();
        assert_ne!(a.get_crc(), b.get_crc(), "upstream: different NaN bits, different CRC");
    }
}
