// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Xfer.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/Xfer.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/Snapshot.h
//   GeneralsMD/Code/GameEngine/Include/Common/GameType.h (ObjectID, DrawableID)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, February 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The Xfer system: one description of a data structure's contents
//! ([`Snapshot::xfer`]) drives save ([`super::xfer_save`]), load
//! ([`super::xfer_load`]) and the lockstep CRC ([`super::xfer_crc`]).
//!
//! # Translation notes
//!
//! - `class Xfer` becomes the trait [`Xfer`]. Upstream's pure virtual
//!   `xferImplementation(void *data, Int dataSize)` moves bytes in whichever
//!   direction the mode implies; here it takes `&mut [u8]`, and every typed
//!   method converts its value to bytes, calls it, and converts back (so the
//!   same code saves, loads and hashes, as upstream).
//! - **Byte order.** Upstream passes the in-memory bytes of each value, i.e.
//!   little-endian on its x86 target. The translation writes little-endian
//!   explicitly (`to_le_bytes`), on every host. A `Real` is moved as its raw
//!   IEEE-754 bits.
//! - **Errors.** Upstream `throw`s an `XferStatus`; here every method returns
//!   [`XferResult`]. `beginBlock` keeps its own convention (it *returns* a
//!   status or a size, and `XferLoad` reports a failed read as size 0).
//! - **Strings.** `AsciiString` is a byte vector (`Vec<u8>`), `UnicodeString`
//!   a vector of UTF-16 code units (`Vec<u16>`; `WideChar` is 16 bits on the
//!   Win32 target).
//! - `xferSnapshot(NULL)` has no Rust counterpart (references are never null).
//!
//! # Not translated (each needs an engine subsystem that is not here yet)
//!
//! `xferMapName` (`TheGameState` map-path translation), `xferScienceType`,
//! `xferScienceVec` (`TheScienceStore`) and ~~`xferUpgradeMask`
//! (`TheUpgradeCenter`)~~ **CORRECTED 2026-10-18** (p1w2-upgrade):
//! `xferUpgradeMask` is translated as the free function
//! `common::upgrade::xfer_upgrade_mask`, since this trait cannot see an
//! `UpgradeCenter`; checked by Rust tests only, because the Xfer build of the
//! differential tests stands `TheUpgradeCenter` in with functions that abort.
//! Their CRC branches only hash the raw value, but
//! their save/load branches go through those name tables, so they are left
//! whole until the tables exist. (`xferKindOf` was added in Phase 1 with the
//! `KindOfMaskType` names, [`super::kind_of`].)

use super::base_type::{
    Coord2D, Coord3D, ICoord2D, ICoord3D, IRegion2D, IRegion3D, RGBAColorInt, RGBAColorReal, RGBColor, RealRange,
    Region2D, Region3D,
};
use super::kind_of::{KindOfMaskType, KindOfType};
use crate::ww::matrix3d::Matrix3D;

/// `typedef UnsignedByte XferVersion`.
pub type XferVersion = u8;
/// `typedef Int XferBlockSize`.
pub type XferBlockSize = i32;
/// `typedef Int Color`.
pub type Color = i32;
/// `enum ObjectID`, `enum DrawableID` (GameType.h), defined in
/// [`super::game_type`].
pub use super::game_type::{DrawableId, ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};

/// `enum XferMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum XferMode {
    Invalid = 0,
    Save,
    Load,
    Crc,
}

/// `enum XferStatus`: also the error type of every Xfer operation (upstream
/// throws these values).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum XferStatus {
    Invalid = 0,
    /// all is green and good
    Ok,
    /// end of file encountered
    Eof,
    /// requested file does not exist
    FileNotFound,
    /// file was not open
    FileNotOpen,
    /// this xfer is already open
    FileAlreadyOpen,
    /// error reading from file
    ReadError,
    /// error writing to file
    WriteError,
    /// unknown xfer mode
    ModeUnknown,
    /// error skipping file
    SkipError,
    /// mismatched pair calls of begin/end block
    BeginEndMismatch,
    /// out of memory
    OutOfMemory,
    /// error with strings
    StringError,
    /// invalid version encountered
    InvalidVersion,
    /// invalid parameters
    InvalidParameters,
    /// trying to xfer into a list that should be empty, but isn't
    ListNotEmpty,
    /// unrecognized string value
    UnknownString,
    /// unknown error (isn't that useful!)
    ErrorUnknown,
}

impl std::fmt::Display for XferStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for XferStatus {}

/// The result of an Xfer operation; the error is what upstream throws.
pub type XferResult<T> = Result<T, XferStatus>;

/// `XO_NONE`.
pub const XO_NONE: u32 = 0x0000_0000;
/// `XO_NO_POST_PROCESSING`.
pub const XO_NO_POST_PROCESSING: u32 = 0x0000_0001;
/// `XO_ALL`.
pub const XO_ALL: u32 = 0xFFFF_FFFF;

/// `class Snapshot`: a data structure that takes part in saves, loads and
/// CRC checks.
pub trait Snapshot {
    /// Run the "light" CRC check on this data structure.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()>;
    /// Run save, load, or deep CRC on this data structure; which one depends
    /// on the xfer.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()>;
    /// Post-process phase after a load.
    fn load_post_process(&mut self);
}

/// The data members of `class Xfer`: `m_options`, `m_xferMode`,
/// `m_identifier`.
#[derive(Clone, Debug)]
pub struct XferState {
    pub options: u32,
    pub mode: XferMode,
    pub identifier: Vec<u8>,
}

impl XferState {
    /// `Xfer::Xfer()`: no options, `XFER_INVALID` (the derived constructors
    /// then set their mode).
    pub fn new(mode: XferMode) -> Self {
        XferState { options: XO_NONE, mode, identifier: Vec::new() }
    }

    /// `Xfer::open`: save the identifier.
    pub fn open(&mut self, identifier: &[u8]) {
        self.identifier = identifier.to_vec();
    }
}

// --- p1w13-sonnet --- (batch R12)
std::thread_local! {
    /// A load read a byte other than 0 or 1 into a `bool` since [`take_invalid_bool_read`] was last called.
    static INVALID_BOOL_READ: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a load read a byte other than 0 or 1 into a `bool` (`Undefined::InvalidBoolLoaded`) since the last call; clears the note.
pub fn take_invalid_bool_read() -> bool {
    INVALID_BOOL_READ.with(|f| f.replace(false))
}
// --- end p1w13-sonnet ---

/// `class Xfer`. Implementors supply the state accessors, the mode-specific
/// operations and [`Xfer::xfer_implementation`]; the typed methods are
/// upstream's defaults and may be overridden as upstream does
/// (`XferSave`/`XferLoad` override the string methods).
pub trait Xfer {
    /// The `Xfer` data members.
    fn state(&self) -> &XferState;
    /// The `Xfer` data members.
    fn state_mut(&mut self) -> &mut XferState;

    /// `getXferMode`.
    fn get_xfer_mode(&self) -> XferMode {
        self.state().mode
    }
    /// `getIdentifier`.
    fn get_identifier(&self) -> &[u8] {
        &self.state().identifier
    }
    /// `setOptions`: `BitSet(m_options, options)`.
    fn set_options(&mut self, options: u32) {
        self.state_mut().options |= options;
    }
    /// `clearOptions`: `BitClear(m_options, options)`.
    fn clear_options(&mut self, options: u32) {
        self.state_mut().options &= !options;
    }
    /// `getOptions`.
    fn get_options(&self) -> u32 {
        self.state().options
    }

    /// xfer open event
    fn open(&mut self, identifier: &[u8]) -> XferResult<()>;
    /// xfer close event
    fn close(&mut self) -> XferResult<()>;
    /// xfer begin block event (returns a block size or a status, as upstream)
    fn begin_block(&mut self) -> XferResult<i32>;
    /// xfer end block event
    fn end_block(&mut self) -> XferResult<()>;
    /// xfer skip data
    fn skip(&mut self, data_size: i32) -> XferResult<()>;
    /// entry point for xfering a snapshot
    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshot) -> XferResult<()>;
    /// the actual xfer implementation that each derived class implements
    fn xfer_implementation(&mut self, data: &mut [u8]) -> XferResult<()>;

    /// `xferVersion`: after the xfer the version may not exceed `current`.
    fn xfer_version(&mut self, version_data: &mut XferVersion, current_version: XferVersion) -> XferResult<()> {
        let mut b = [*version_data];
        self.xfer_implementation(&mut b)?;
        *version_data = b[0];
        if *version_data > current_version {
            return Err(XferStatus::InvalidVersion);
        }
        Ok(())
    }
    /// `xferByte` (`Byte` is `char`, signed on the target).
    fn xfer_byte(&mut self, v: &mut i8) -> XferResult<()> {
        let mut b = v.to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *v = i8::from_le_bytes(b);
        r
    }
    /// `xferUnsignedByte`.
    fn xfer_unsigned_byte(&mut self, v: &mut u8) -> XferResult<()> {
        let mut b = [*v];
        self.xfer_implementation(&mut b)?;
        *v = b[0];
        Ok(())
    }
    /// `xferBool`: one byte, 0 or 1. On load, upstream stores the raw byte
    /// into a `bool` (a value other than 0/1 is undefined behaviour in C++);
    /// here any nonzero byte reads as `true`.
    fn xfer_bool(&mut self, v: &mut bool) -> XferResult<()> {
        let mut b = [u8::from(*v)];
        self.xfer_implementation(&mut b)?;
        // batch R12: a byte that is no `bool` is undefined upstream; noted for the caller that has the world ([`take_invalid_bool_read`])
        if b[0] > 1 {
            INVALID_BOOL_READ.with(|f| f.set(true));
        }
        *v = b[0] != 0;
        Ok(())
    }
    /// `xferInt`.
    fn xfer_int(&mut self, v: &mut i32) -> XferResult<()> {
        let mut b = v.to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *v = i32::from_le_bytes(b);
        r
    }
    /// `xferInt64`.
    fn xfer_int64(&mut self, v: &mut i64) -> XferResult<()> {
        let mut b = v.to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *v = i64::from_le_bytes(b);
        r
    }
    /// `xferUnsignedInt`.
    fn xfer_unsigned_int(&mut self, v: &mut u32) -> XferResult<()> {
        let mut b = v.to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *v = u32::from_le_bytes(b);
        r
    }
    /// `xferShort`.
    fn xfer_short(&mut self, v: &mut i16) -> XferResult<()> {
        let mut b = v.to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *v = i16::from_le_bytes(b);
        r
    }
    /// `xferUnsignedShort`.
    fn xfer_unsigned_short(&mut self, v: &mut u16) -> XferResult<()> {
        let mut b = v.to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *v = u16::from_le_bytes(b);
        r
    }
    /// `xferReal`: the raw IEEE-754 bits.
    fn xfer_real(&mut self, v: &mut f32) -> XferResult<()> {
        let mut b = v.to_bits().to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *v = f32::from_bits(u32::from_le_bytes(b));
        r
    }
    /// `xferMarkerLabel`: purely for readability, discarded (a no-op in
    /// every upstream implementation).
    fn xfer_marker_label(&mut self, _label: &[u8]) -> XferResult<()> {
        Ok(())
    }
    /// `xferAsciiString` (base): the string's bytes, no length.
    fn xfer_ascii_string(&mut self, s: &mut Vec<u8>) -> XferResult<()> {
        self.xfer_implementation(s.as_mut_slice())
    }
    /// `xferUnicodeString` (base): the string's `WideChar`s, no length.
    fn xfer_unicode_string(&mut self, s: &mut Vec<u16>) -> XferResult<()> {
        let mut b: Vec<u8> = s.iter().flat_map(|c| c.to_le_bytes()).collect();
        self.xfer_implementation(&mut b)?;
        for (c, pair) in s.iter_mut().zip(b.chunks_exact(2)) {
            *c = u16::from_le_bytes([pair[0], pair[1]]);
        }
        Ok(())
    }
    /// `xferCoord3D`.
    fn xfer_coord3d(&mut self, c: &mut Coord3D) -> XferResult<()> {
        self.xfer_real(&mut c.x)?;
        self.xfer_real(&mut c.y)?;
        self.xfer_real(&mut c.z)
    }
    /// `xferICoord3D`.
    fn xfer_icoord3d(&mut self, c: &mut ICoord3D) -> XferResult<()> {
        self.xfer_int(&mut c.x)?;
        self.xfer_int(&mut c.y)?;
        self.xfer_int(&mut c.z)
    }
    /// `xferRegion3D`.
    fn xfer_region3d(&mut self, r: &mut Region3D) -> XferResult<()> {
        self.xfer_coord3d(&mut r.lo)?;
        self.xfer_coord3d(&mut r.hi)
    }
    /// `xferIRegion3D`.
    fn xfer_iregion3d(&mut self, r: &mut IRegion3D) -> XferResult<()> {
        self.xfer_icoord3d(&mut r.lo)?;
        self.xfer_icoord3d(&mut r.hi)
    }
    /// `xferCoord2D`.
    fn xfer_coord2d(&mut self, c: &mut Coord2D) -> XferResult<()> {
        self.xfer_real(&mut c.x)?;
        self.xfer_real(&mut c.y)
    }
    /// `xferICoord2D`.
    fn xfer_icoord2d(&mut self, c: &mut ICoord2D) -> XferResult<()> {
        self.xfer_int(&mut c.x)?;
        self.xfer_int(&mut c.y)
    }
    /// `xferRegion2D`.
    fn xfer_region2d(&mut self, r: &mut Region2D) -> XferResult<()> {
        self.xfer_coord2d(&mut r.lo)?;
        self.xfer_coord2d(&mut r.hi)
    }
    /// `xferIRegion2D`.
    fn xfer_iregion2d(&mut self, r: &mut IRegion2D) -> XferResult<()> {
        self.xfer_icoord2d(&mut r.lo)?;
        self.xfer_icoord2d(&mut r.hi)
    }
    /// `xferRealRange`.
    fn xfer_real_range(&mut self, r: &mut RealRange) -> XferResult<()> {
        self.xfer_real(&mut r.lo)?;
        self.xfer_real(&mut r.hi)
    }
    /// `xferColor`: `xferImplementation(color, sizeof(Color))`.
    fn xfer_color(&mut self, color: &mut Color) -> XferResult<()> {
        let mut b = color.to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *color = i32::from_le_bytes(b);
        r
    }
    /// `xferRGBColor`.
    fn xfer_rgb_color(&mut self, c: &mut RGBColor) -> XferResult<()> {
        self.xfer_real(&mut c.red)?;
        self.xfer_real(&mut c.green)?;
        self.xfer_real(&mut c.blue)
    }
    /// `xferRGBAColorReal`.
    fn xfer_rgba_color_real(&mut self, c: &mut RGBAColorReal) -> XferResult<()> {
        self.xfer_real(&mut c.red)?;
        self.xfer_real(&mut c.green)?;
        self.xfer_real(&mut c.blue)?;
        self.xfer_real(&mut c.alpha)
    }
    /// `xferRGBAColorInt`.
    fn xfer_rgba_color_int(&mut self, c: &mut RGBAColorInt) -> XferResult<()> {
        self.xfer_unsigned_int(&mut c.red)?;
        self.xfer_unsigned_int(&mut c.green)?;
        self.xfer_unsigned_int(&mut c.blue)?;
        self.xfer_unsigned_int(&mut c.alpha)
    }
    /// `xferObjectID`: `xferImplementation(objectID, sizeof(ObjectID))`.
    fn xfer_object_id(&mut self, id: &mut ObjectId) -> XferResult<()> {
        let mut b = id.to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *id = i32::from_le_bytes(b);
        r
    }
    /// `xferDrawableID`.
    fn xfer_drawable_id(&mut self, id: &mut DrawableId) -> XferResult<()> {
        let mut b = id.to_le_bytes();
        // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
        let r = self.xfer_implementation(&mut b);
        *id = i32::from_le_bytes(b);
        r
    }
    /// `xferSTLObjectIDVector` (version 1).
    fn xfer_stl_object_id_vector(&mut self, v: &mut Vec<ObjectId>) -> XferResult<()> {
        xfer_id_sequence(self, v, |x, id| x.xfer_object_id(id))
    }
    /// `xferSTLObjectIDList` (version 1); a `std::list` upstream, the same
    /// bytes as the vector form.
    fn xfer_stl_object_id_list(&mut self, v: &mut Vec<ObjectId>) -> XferResult<()> {
        xfer_id_sequence(self, v, |x, id| x.xfer_object_id(id))
    }
    /// `xferSTLIntList` (version 1).
    fn xfer_stl_int_list(&mut self, v: &mut Vec<i32>) -> XferResult<()> {
        xfer_id_sequence(self, v, |x, i| x.xfer_int(i))
    }
    /// `xferUser`: raw bytes.
    fn xfer_user(&mut self, data: &mut [u8]) -> XferResult<()> {
        self.xfer_implementation(data)
    }
    /// `xferKindOf` (version 1). Save: the kind's name as an `AsciiString`
    /// (`getNameFromSingleBit`; an index outside the table gives `NULL`,
    /// which saves the empty string). Load: the name read, looked up with
    /// `getSingleBitFromName`; **an unknown name leaves `*kind` unchanged**
    /// and is not an error (upstream quirk, kept). CRC: the 4 bytes of the
    /// `int`-sized enum.
    fn xfer_kind_of(&mut self, kind: &mut KindOfType) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        self.xfer_version(&mut version, current)?;
        match self.get_xfer_mode() {
            XferMode::Save => {
                let mut name = KindOfMaskType::get_name_from_single_bit(*kind).unwrap_or("").as_bytes().to_vec();
                self.xfer_ascii_string(&mut name)
            }
            XferMode::Load => {
                let mut name = Vec::new();
                self.xfer_ascii_string(&mut name)?;
                let bit = KindOfMaskType::get_single_bit_from_name(&name);
                if bit != -1 {
                    *kind = bit;
                }
                Ok(())
            }
            XferMode::Crc => {
                let mut b = kind.to_le_bytes();
                // `fread` copies the bytes there are before it fails: a short read leaves them in the value (batch R9)
                let r = self.xfer_implementation(&mut b);
                *kind = KindOfType::from_le_bytes(b);
                r
            }
            XferMode::Invalid => Err(XferStatus::ModeUnknown),
        }
    }
    /// `xferMatrix3D` (version 1): the twelve members, row by row.
    fn xfer_matrix3d(&mut self, mtx: &mut Matrix3D) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        self.xfer_version(&mut version, current_version)?;
        for r in 0..3 {
            let row = &mut mtx.row[r];
            self.xfer_real(&mut row.x)?;
            self.xfer_real(&mut row.y)?;
            self.xfer_real(&mut row.z)?;
            self.xfer_real(&mut row.w)?;
        }
        Ok(())
    }
}

/// The shared body of `xferSTLObjectIDVector`, `xferSTLObjectIDList` and
/// `xferSTLIntList`: version 1, a `UnsignedShort` count, then the items.
///
/// Upstream stores `size()` into an `UnsignedShort`, so a sequence of 65,536
/// or more items records a truncated count but still saves every item (the
/// file then does not load back). Kept as upstream.
fn xfer_id_sequence<X: Xfer + ?Sized>(
    x: &mut X,
    v: &mut Vec<i32>,
    mut item: impl FnMut(&mut X, &mut i32) -> XferResult<()>,
) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    x.xfer_version(&mut version, current_version)?;

    let mut list_count = v.len() as u16;
    x.xfer_unsigned_short(&mut list_count)?;

    match x.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for &value in v.iter() {
                // upstream copies each element into a local and xfers that
                let mut d = value;
                item(x, &mut d)?;
            }
            Ok(())
        }
        XferMode::Load => {
            if !v.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            for _ in 0..list_count {
                let mut d = 0;
                item(x, &mut d)?;
                v.push(d);
            }
            Ok(())
        }
        XferMode::Invalid => Err(XferStatus::ModeUnknown),
    }
}
