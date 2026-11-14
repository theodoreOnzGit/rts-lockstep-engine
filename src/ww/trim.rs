// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/trim.cpp (strtrim)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWLib)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! WWLib's `strtrim` (trim.cpp), which `StringClass::Trim`, `Read_Line` and `INIClass` run (moved here from
//! [`crate::formats::w3d::animatedsoundmgr`] for [`crate::ww::ini`], #181 item 3).

use crate::logic::modules::plant;

/// The bytes of a C string: up to its first NUL.
pub fn c_str(s: &[u8]) -> &[u8] {
    &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]
}

/// `strtrim(buffer)` (WWLib/trim.cpp) on a C string, as `StringClass::Trim` runs it: leading bytes up to 32 skipped
/// and the rest copied to the front (`strcpy` over itself, front to back), then trailing bytes up to 32 cut, **while
/// `*source` is not 0**, where `source` still points where the first kept byte was before the copy. So a string whose
/// kept part is as long as its leading white space ends the copy with a NUL at `source` and keeps its trailing white
/// space (`"  a "` gives `"a "`), and a cut that reaches `source` stops there.
pub fn strtrim(s: &[u8]) -> Vec<u8> {
    let s = c_str(s);
    let mut buffer: Vec<u8> = s.to_vec();
    buffer.push(0);
    // Strip leading white space from the string.
    let source = buffer.iter().take_while(|&&c| c != 0 && c <= 32).count();
    if source != 0 {
        let mut i = 0;
        loop {
            let c = buffer[source + i];
            buffer[i] = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
    }
    // Clip trailing white space from the string.
    let len = c_str(&buffer).len();
    for index in (0..len).rev() {
        if (buffer[source] != 0 || plant(10061)) && buffer[index] <= 32 {
            buffer[index] = 0;
        } else {
            break;
        }
    }
    c_str(&buffer).to_vec()
}
