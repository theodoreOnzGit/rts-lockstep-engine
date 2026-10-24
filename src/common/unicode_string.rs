// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/UnicodeString.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/UnicodeString.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class UnicodeString`: the engine's double-byte string, with its exact
//! semantics. It mirrors [`super::ascii_string::AsciiString`]; this page
//! lists what differs.
//!
//! `WideChar` is `wchar_t`, which on Win32 (MSVC) is a **16-bit unsigned**
//! type, so the characters here are `u16`. Upstream shares a reference-counted
//! buffer between copies and copies on write; here a `UnicodeString` owns its
//! units, which is the same value semantics. Kept from upstream:
//!
//! - **C-string content.** The units never contain a 0: everything that
//!   stores units stops at the first 0 (`wcslen`, `wcscpy`).
//! - **Whether a buffer exists** (`m_data` non-null), as for `AsciiString`:
//!   `set(L"")` and `clear()` release it, `removeLastChar` on a one-unit
//!   string leaves an empty buffer; [`UnicodeString::has_buffer`] and
//!   [`UnicodeString::try_get_char_at`] can tell the two apart.
//! - **The length limit.** A buffer needs a 4-byte header plus 2 bytes per
//!   unit (the terminator included), and more than `MAX_LEN` (32,767) bytes
//!   is upstream's `throw ERROR_OUT_OF_MEMORY`: [`UnicodeStringError::TooLong`].
//!   So a string holds at most 16,380 units (not 32,762 as `AsciiString`).
//!   Each operation asks for the buffer size upstream asks for (`nextToken`
//!   asks for the token's length plus 2; `translate` appends one unit at a
//!   time and so stops, with the units appended so far kept, at the limit).
//! - **`compare` is `wcscmp`** (units ordered as unsigned) and
//!   **`compareNoCase` is `_wcsicmp`**: each unit folded to *lower* case with
//!   `towlower`, which in the C locale changes only `A`..`Z` (so `_` sorts
//!   after `A`, as for `AsciiString`). Upstream returns the C library's
//!   `int`, of which only the sign is meaningful, so these return an
//!   [`Ordering`]; the six comparison operators are `wcscmp` on `str()` and
//!   so are `PartialEq`/`Ord` here (a string with an empty buffer equals one
//!   without).
//! - `translate(const AsciiString&)` widens each `char` through
//!   `(WideChar)getCharAt(i)`; `char` is signed, so a byte from 0x80 becomes
//!   0xff80..0xffff. A string is built one unit at a time, as upstream.
//!
//! **Overlapping copies.** `set(const WideChar*)` copies with `wcscpy` into
//! the string's own buffer when it has room, and `trim()` and `nextToken()`
//! set a string from its own tail, so the copy overlaps (destination first).
//! That is undefined in C, but MSVC's `wcscpy` copies forward and so gives
//! the intended result (a forward copy with the destination first never reads
//! a unit it has already overwritten). This translation treats the copy as
//! that forward copy, and the C++ reference is compiled with a forward
//! `wcscpy` (`differential/shim_unicode/unicode_prelude.h`; glibc's wide
//! functions work on 32-bit units, so 16-bit versions are needed in any
//! case). The opposite overlap, `concat` of a string onto its own tail (or
//! onto itself) when the buffer has room, makes `wcscat` read units it has
//! just written and never reach the terminator; it is undefined, depends on
//! the allocator's rounding, and has no Rust counterpart (the borrow rules
//! forbid `concat_unicode(self)`).
//!
//! # Where upstream is undefined or not established
//!
//! - `trim` passes each unit it examines to `iswspace`. In the C locale that
//!   is true for 0x09..0x0d and 0x20; MSVC's answer for units from 0x80
//!   (0x85, 0xa0, 0x2000..) was **not established** (it needs the MSVC
//!   runtime), so evaluating `iswspace` on such a unit is
//!   [`UnicodeStringError::NonAsciiCharClass`], not a guess.
//! - `getCharAt(i)` with a buffer reads `peek()[i]` unchecked: past the
//!   terminator it reads uninitialised or foreign memory:
//!   [`UnicodeStringError::IndexOutOfRange`]. (Index `getLength()` is the
//!   terminator, 0; without a buffer every index gives 0.)
//!
//! Not translated: `format`/`format_va` (`_vsnwprintf`, the MSVC runtime's
//! wide printf, with `%s` meaning a wide string and the `< 0` return when
//! 2,047 units do not suffice becoming the throw; its call sites will be
//! translated with their format strings), `getBufferForRead` (raw buffer
//! access; its callers are translated with it), `TheNullChr`.

use super::ascii_string::AsciiString;
use std::cmp::Ordering;

/// `UnicodeString::MAX_FORMAT_BUF_LEN`.
pub const MAX_FORMAT_BUF_LEN: usize = 2048;
/// `UnicodeString::MAX_LEN`: the most bytes a buffer may have, header included.
pub const MAX_LEN: usize = 32767;
/// `sizeof(UnicodeStringData)` in the release build: two `unsigned short`s.
const HEADER_BYTES: usize = 4;
/// `sizeof(WideChar)` on Win32.
const CHAR_BYTES: usize = 2;

/// Where upstream throws or is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnicodeStringError {
    /// `throw ERROR_OUT_OF_MEMORY`: a buffer of more than `MAX_LEN` bytes.
    TooLong,
    /// `iswspace` of a unit from 0x80 (MSVC's C-locale answer not established).
    NonAsciiCharClass,
    /// `getCharAt` past the terminator (undefined upstream).
    IndexOutOfRange,
}

impl std::fmt::Display for UnicodeStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            UnicodeStringError::TooLong => "UnicodeString: longer than MAX_LEN (ERROR_OUT_OF_MEMORY upstream)",
            UnicodeStringError::NonAsciiCharClass => "UnicodeString::trim: iswspace of a unit >= 0x80 (not established for MSVC's C locale)",
            UnicodeStringError::IndexOutOfRange => "UnicodeString::getCharAt: index past the string (undefined upstream)",
        })
    }
}

/// `wcslen`'s view: up to the first 0 unit.
fn c_str(s: &[u16]) -> &[u16] {
    match s.iter().position(|&c| c == 0) {
        Some(n) => &s[..n],
        None => s,
    }
}

/// `iswspace` in the C locale; `None` for a unit from 0x80 (see the module docs).
fn is_c_space(c: u16) -> Option<bool> {
    if c >= 0x80 {
        None
    } else {
        Some(matches!(c, 0x09..=0x0d | 0x20))
    }
}

/// `towlower` in the C locale: `A`..`Z` only.
fn to_lower(c: u16) -> u16 {
    if (b'A' as u16..=b'Z' as u16).contains(&c) {
        c + 32
    } else {
        c
    }
}

/// The check of `ensureUniqueBufferOfSize(numCharsNeeded)`.
fn need(num_chars: usize) -> Result<(), UnicodeStringError> {
    if HEADER_BYTES + num_chars * CHAR_BYTES > MAX_LEN {
        Err(UnicodeStringError::TooLong)
    } else {
        Ok(())
    }
}

/// `class UnicodeString`.
#[derive(Clone, Debug, Default)]
pub struct UnicodeString {
    /// `m_data`: `None` for a null pointer, else the units (no 0).
    data: Option<Vec<u16>>,
}

/// `UnicodeString::TheEmptyString`: the default `delimiters` of `nextToken`.
pub static THE_EMPTY_STRING: UnicodeString = UnicodeString::new();

impl UnicodeString {
    /// `UnicodeString()`: empty, no buffer.
    pub const fn new() -> Self {
        UnicodeString { data: None }
    }

    /// `UnicodeString(const WideChar* s)`.
    pub fn try_from_chars(s: &[u16]) -> Result<Self, UnicodeStringError> {
        let mut a = UnicodeString::new();
        a.set(s)?;
        Ok(a)
    }

    /// Whether a buffer exists (upstream's `m_data != NULL`).
    pub fn has_buffer(&self) -> bool {
        self.data.is_some()
    }

    /// `str()`: the units.
    pub fn str(&self) -> &[u16] {
        self.data.as_deref().unwrap_or(&[])
    }

    /// `getLength()`.
    pub fn get_length(&self) -> i32 {
        self.str().len() as i32
    }

    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.str().is_empty()
    }

    /// `clear()`: release the buffer.
    pub fn clear(&mut self) {
        self.data = None;
    }

    /// `getCharAt(index)`: see the module docs for the undefined cases.
    pub fn try_get_char_at(&self, index: i32) -> Result<u16, UnicodeStringError> {
        match &self.data {
            None => Ok(0),
            Some(d) => {
                if index < 0 || index as usize > d.len() {
                    Err(UnicodeStringError::IndexOutOfRange)
                } else {
                    Ok(d.get(index as usize).copied().unwrap_or(0))
                }
            }
        }
    }

    /// `set(const WideChar* s)`: an empty `s` releases the buffer.
    pub fn set(&mut self, s: &[u16]) -> Result<(), UnicodeStringError> {
        let s = c_str(s);
        if s.is_empty() {
            self.data = None;
        } else {
            need(s.len() + 1)?;
            self.data = Some(s.to_vec());
        }
        Ok(())
    }

    /// `set(const UnicodeString&)` (and `operator=`): the buffer is shared
    /// upstream, copied here; `set(*this)` is a no-op in either.
    pub fn set_unicode(&mut self, src: &UnicodeString) {
        self.data.clone_from(&src.data);
    }

    /// `translate(const AsciiString&)`: clear, then each `char` widened as
    /// `(WideChar)` (sign-extended, then truncated to 16 bits) and appended
    /// one at a time; a failure keeps the units appended so far.
    pub fn translate(&mut self, src: &AsciiString) -> Result<(), UnicodeStringError> {
        self.clear();
        for &b in src.str() {
            self.concat_char(b as i8 as i16 as u16)?;
        }
        Ok(())
    }

    /// `concat(const WideChar* s)`: with a buffer, grows it; without, `set(s)`.
    pub fn concat(&mut self, s: &[u16]) -> Result<(), UnicodeStringError> {
        let s = c_str(s);
        if s.is_empty() {
            return Ok(());
        }
        match &mut self.data {
            Some(d) => {
                need(d.len() + s.len() + 1)?;
                d.extend_from_slice(s);
                Ok(())
            }
            None => self.set(s),
        }
    }

    /// `concat(const UnicodeString&)`: `concat(src.str())`. (Not for the
    /// string itself: see the module docs.)
    pub fn concat_unicode(&mut self, src: &UnicodeString) -> Result<(), UnicodeStringError> {
        self.concat(src.str())
    }

    /// `concat(const WideChar c)`: a 0 appends nothing.
    pub fn concat_char(&mut self, c: u16) -> Result<(), UnicodeStringError> {
        self.concat(&[c])
    }

    /// `trim()`: drop leading and trailing `iswspace` units.
    pub fn trim(&mut self) -> Result<(), UnicodeStringError> {
        let Some(d) = &self.data else { return Ok(()) };
        // `while (c && iswspace(*c)) c++`: iswspace is evaluated on each unit
        // it reaches, up to the first that is not a space.
        let mut start = 0;
        while start < d.len() {
            match is_c_space(d[start]) {
                None => return Err(UnicodeStringError::NonAsciiCharClass),
                Some(false) => break,
                Some(true) => start += 1,
            }
        }
        if start != 0 {
            let rest = d[start..].to_vec();
            self.set(&rest)?;
        }
        if self.data.is_some() {
            let mut i = self.str().len();
            while i > 0 {
                match is_c_space(self.str()[i - 1]) {
                    None => return Err(UnicodeStringError::NonAsciiCharClass),
                    Some(false) => break,
                    Some(true) => {
                        self.remove_last_char()?;
                        i -= 1;
                    }
                }
            }
        }
        Ok(())
    }

    /// `removeLastChar()`: the buffer stays, even when it becomes empty.
    pub fn remove_last_char(&mut self) -> Result<(), UnicodeStringError> {
        if let Some(d) = &mut self.data {
            if !d.is_empty() {
                need(d.len() + 1)?;
                d.pop();
            }
        }
        Ok(())
    }

    /// `compare(s)`: `wcscmp` (units as unsigned).
    pub fn compare(&self, s: &[u16]) -> Ordering {
        self.str().cmp(c_str(s))
    }

    /// `compare(const UnicodeString&)`.
    pub fn compare_unicode(&self, s: &UnicodeString) -> Ordering {
        self.compare(s.str())
    }

    /// `compareNoCase(s)`: `_wcsicmp` (ASCII folded to lower case, then units
    /// as unsigned).
    pub fn compare_no_case(&self, s: &[u16]) -> Ordering {
        let a = self.str().iter().copied().map(to_lower);
        let b = c_str(s).iter().copied().map(to_lower);
        a.cmp(b)
    }

    /// `compareNoCase(const UnicodeString&)`.
    pub fn compare_no_case_unicode(&self, s: &UnicodeString) -> Ordering {
        self.compare_no_case(s.str())
    }

    /// `nextToken(tok, delimiters)`: skip delimiters (default `" \t\n\r"`
    /// when `delimiters` is empty), take the token into `tok`, keep the
    /// rest. False (and nothing changed) on an empty string; false, with
    /// both cleared, when only delimiters are left. (Upstream also returns
    /// false when `tok` is this string, which Rust's borrow rules exclude.)
    pub fn next_token(&mut self, tok: &mut UnicodeString, delimiters: &UnicodeString) -> Result<bool, UnicodeStringError> {
        if self.is_empty() {
            return Ok(false);
        }
        let default: [u16; 4] = [0x20, 0x09, 0x0a, 0x0d];
        let seps: &[u16] = if delimiters.is_empty() { &default } else { delimiters.str() };
        let s = self.str();
        // wcsspn, then wcscspn
        let start = s.iter().position(|c| !seps.contains(c)).unwrap_or(s.len());
        let end = start + s[start..].iter().position(|c| seps.contains(c)).unwrap_or(s.len() - start);
        if end > start {
            let len = end - start;
            // tok->getBufferForRead(len + 1): a buffer of len + 2 units.
            need(len + 2)?;
            let token = s[start..end].to_vec();
            let rest = s[end..].to_vec();
            tok.data = Some(token);
            self.set(&rest)?;
            Ok(true)
        } else {
            self.clear();
            tok.clear();
            Ok(false)
        }
    }
}

/// `operator==` and friends: `wcscmp` of `str()`.
impl PartialEq for UnicodeString {
    fn eq(&self, other: &Self) -> bool {
        self.str() == other.str()
    }
}

impl Eq for UnicodeString {}

impl PartialOrd for UnicodeString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UnicodeString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare_unicode(other)
    }
}

impl std::hash::Hash for UnicodeString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.str().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn buffer_state_tokens_and_limits() {
        let mut a = UnicodeString::try_from_chars(&w("x")).unwrap();
        a.remove_last_char().unwrap();
        assert!(a.has_buffer() && a.is_empty(), "removeLastChar keeps the buffer");
        assert_eq!(a.try_get_char_at(0), Ok(0));
        assert_eq!(a.try_get_char_at(1), Err(UnicodeStringError::IndexOutOfRange));
        assert_eq!(a, UnicodeString::new(), "equality is wcscmp of str()");
        a.trim().unwrap();
        assert!(a.has_buffer(), "trim of an empty buffer leaves it");

        let mut s = UnicodeString::try_from_chars(&w("  alpha beta")).unwrap();
        let mut t = UnicodeString::new();
        assert_eq!(s.next_token(&mut t, &THE_EMPTY_STRING), Ok(true));
        assert_eq!((t.str(), s.str()), (&w("alpha")[..], &w(" beta")[..]));

        let mut b = UnicodeString::new();
        b.translate(&AsciiString::try_from_bytes(b"A\xe9").unwrap()).unwrap();
        assert_eq!(b.str(), &[0x41, 0xffe9], "char is signed: 0xe9 widens to 0xffe9");

        // 16,380 units is the most a buffer holds: 4 + 2 * 16,381 <= 32,767.
        let long = vec![b'a' as u16; 16380];
        assert!(UnicodeString::try_from_chars(&long).is_ok());
        assert_eq!(UnicodeString::try_from_chars(&vec![b'a' as u16; 16381]), Err(UnicodeStringError::TooLong));
        // trim on a unit from 0x80 is not established
        let mut n = UnicodeString::try_from_chars(&[0xa0, 0x41]).unwrap();
        assert_eq!(n.trim(), Err(UnicodeStringError::NonAsciiCharClass));
    }

    #[test]
    fn comparisons_and_overlapping_sets() {
        let a = UnicodeString::try_from_chars(&w("A")).unwrap();
        assert_eq!(a.compare_no_case(&w("_")), Ordering::Greater, "folded to lower case: 'a' > '_'");
        assert_eq!(a.compare(&w("_")), Ordering::Less);
        assert_eq!(a.compare(&[0xffff]), Ordering::Less, "units order as unsigned");
        let mut t = UnicodeString::try_from_chars(&w(" \t x y \r\n")).unwrap();
        t.trim().unwrap();
        assert_eq!(t.str(), &w("x y")[..]);
        let mut u = UnicodeString::new();
        u.concat_char(0).unwrap();
        assert!(!u.has_buffer(), "a 0 appends nothing");
    }
}
