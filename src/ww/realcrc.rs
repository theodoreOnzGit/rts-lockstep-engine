// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/realcrc.cpp, realcrc.h
//   (CRC32_Table, the CRC32 macro, CRC_Memory, CRC_Stringi)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWLib)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! WWLib's `CRC_Memory`: the reflected CRC-32 of polynomial 0x04C11DB7
//! (table of 0xEDB88320), as the mesh loader uses it to share identical
//! texture-coordinate arrays; and `CRC_Stringi`, the same CRC of a string
//! folded to upper case, which the asset manager's hash tables index by
//! ([`crate::formats::w3d::assetmgr`]). Upstream's 256-entry table is the standard one;
//! it is generated here and checked against upstream's by the differential
//! tests.

/// `CRC32_Table`.
pub const CRC32_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

/// The `CRC32(c, crc)` macro: one byte.
#[inline]
pub fn crc32_byte(c: u8, crc: u32) -> u32 {
    CRC32_TABLE[((crc ^ u32::from(c)) & 0xFF) as usize] ^ ((crc >> 8) & 0x00FF_FFFF)
}

/// `CRC_Memory(data, length, crc)`: continues `crc` over `data`.
pub fn crc_memory(data: &[u8], crc: u32) -> u32 {
    let mut crc = crc ^ 0xFFFF_FFFF;
    for &b in data {
        crc = crc32_byte(b, crc);
    }
    crc ^ 0xFFFF_FFFF
}

/// `CRC_Stringi(string, crc)`: the CRC of the C string's bytes, each passed
/// through `toupper` first. `toupper` in the C locale (the game never calls
/// `setlocale`) changes only `a`-`z`, on MSVC and glibc alike; the macro
/// masks the (sign-extended) `char` to its low byte. `string` is the bytes
/// before the terminator.
pub fn crc_stringi(string: &[u8], crc: u32) -> u32 {
    let mut crc = crc ^ 0xFFFF_FFFF;
    for &b in string {
        crc = crc32_byte(b.to_ascii_uppercase(), crc);
    }
    crc ^ 0xFFFF_FFFF
}

/// The inner state of [`crc_memory`] (before the final inversion), for
/// computing a CRC over data that arrives in pieces: start with
/// `crc ^ 0xFFFFFFFF`, feed bytes, finish with `^ 0xFFFFFFFF`.
pub fn crc_feed(state: u32, data: &[u8]) -> u32 {
    data.iter().fold(state, |c, &b| crc32_byte(b, c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_upstream_entries() {
        // the first and last entries of upstream's table (realcrc.cpp lines 46 and 109)
        assert_eq!(&CRC32_TABLE[..4], &[0x0000_0000, 0x7707_3096, 0xEE0E_612C, 0x9909_51BA]);
        assert_eq!(&CRC32_TABLE[252..], &[0xB40B_BE37, 0xC30C_8EA1, 0x5A05_DF1B, 0x2D02_EF8D]);
        assert_eq!(crc_memory(b"123456789", 0), 0xCBF4_3926);
    }

    /// Predicted: `CRC_Stringi` of any casing equals the CRC of the upper-cased
    /// bytes; bytes >= 0x80 are not folded.
    #[test]
    fn crc_stringi_folds_ascii_only() {
        assert_eq!(crc_stringi(b"abc.W3D", 0), crc_memory(b"ABC.W3D", 0));
        assert_eq!(crc_stringi(b"\xe9a", 0), crc_memory(b"\xe9A", 0));
    }
}
