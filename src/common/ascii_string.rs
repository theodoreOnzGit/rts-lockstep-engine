// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/AsciiString.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/AsciiString.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class AsciiString`: the engine's single-byte string, with its exact
//! semantics.
//!
//! Upstream shares a reference-counted buffer between copies and copies on
//! write; here an `AsciiString` owns its bytes, which is the same value
//! semantics. Kept from upstream:
//!
//! - **C-string content.** The bytes never contain a NUL: everything that
//!   stores bytes stops at the first NUL (`strlen`, `strcpy`).
//! - **Whether a buffer exists** (`m_data` non-null). An empty string usually
//!   has none (`set("")` and `clear()` release it), but `removeLastChar` on
//!   a one-character string leaves an empty buffer. `getCharAt` and
//!   `isNone` can tell the two apart.
//! - **The length limit.** A buffer needs a 4-byte header plus the
//!   characters, and more than `MAX_LEN` (32,767) bytes is upstream's
//!   `throw ERROR_OUT_OF_MEMORY`: [`AsciiStringError::TooLong`]. Each
//!   operation asks for the buffer size upstream asks for (`nextToken`
//!   asks for the token's length plus 2).
//! - **Case folding** is ASCII only (`_stricmp`, `strnicmp`, `tolower` in
//!   the C locale), and **comparisons** order bytes as unsigned (`strcmp`);
//!   upstream returns the C library's `int`, of which only the sign is
//!   meaningful, so `compare` returns an [`Ordering`].
//! - `translate` converts each `WideChar` to a `char` (its low byte); a
//!   character whose low byte is 0 is dropped (`concat` of an empty C
//!   string).
//!
//! **Overlapping copies.** `set(const char*)` copies with `strcpy` into the
//! string's own buffer when it has room, and `trim()` and `nextToken()` set
//! a string from its own tail, so the copy overlaps (destination first).
//! That is undefined in C, but MSVC's `strcpy` copies forward and so gives
//! the intended result, which the game relies on in these two common
//! operations; glibc's does not (the first differential run produced a
//! corrupted string). **Not an owner decision yet:** this translation
//! treats the copy as the forward copy MSVC makes, and the C++ reference is
//! compiled with a forward `strcpy` (`differential/shim_ascii/ascii_prelude.h`).
//!
//! # Where upstream is undefined
//!
//! - `trim` passes each byte to `isspace` and `toLower` to `tolower`; a byte
//!   from 0x80 is a negative `char`, undefined for both (C standard 7.4):
//!   [`AsciiStringError::NonAsciiCharClass`].
//! - `toLower` copies the string into a 2,048-byte buffer with `strcpy`: a
//!   string of 2,048 characters or more overflows it:
//!   [`AsciiStringError::FormatBufferOverflow`].
//! - `getCharAt(i)` with a buffer reads `peek()[i]` unchecked: past the
//!   terminator it reads uninitialised or foreign memory:
//!   [`AsciiStringError::IndexOutOfRange`]. (Index `getLength()` is the
//!   terminator, 0; without a buffer every index gives 0.)
//!
//! Not translated: `format`/`format_va` (`_vsnprintf`: the MSVC C runtime's
//! printf; its call sites will be translated with their format strings),
//! `getBufferForRead` (raw buffer access; its callers are translated with
//! it), `debugIgnoreLeaks`.

use std::cmp::Ordering;

/// `AsciiString::MAX_FORMAT_BUF_LEN`.
pub const MAX_FORMAT_BUF_LEN: usize = 2048;
/// `AsciiString::MAX_LEN`: the most bytes a buffer may have, header included.
pub const MAX_LEN: usize = 32767;
/// `sizeof(AsciiStringData)` in the release build: two `unsigned short`s.
const HEADER_BYTES: usize = 4;

/// Where upstream throws or is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AsciiStringError {
    /// `throw ERROR_OUT_OF_MEMORY`: a buffer of more than `MAX_LEN` bytes.
    TooLong,
    /// `isspace`/`tolower` of a byte from 0x80 (undefined upstream).
    NonAsciiCharClass,
    /// `toLower`'s 2,048-byte copy overflows (undefined upstream).
    FormatBufferOverflow,
    /// `getCharAt` past the terminator (undefined upstream).
    IndexOutOfRange,
}

impl std::fmt::Display for AsciiStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AsciiStringError::TooLong => {
                "AsciiString: longer than MAX_LEN (ERROR_OUT_OF_MEMORY upstream)"
            }
            AsciiStringError::NonAsciiCharClass => {
                "AsciiString: isspace/tolower of a byte >= 0x80 (undefined upstream)"
            }
            AsciiStringError::FormatBufferOverflow => {
                "AsciiString::toLower: 2048-byte buffer overflow (undefined upstream)"
            }
            AsciiStringError::IndexOutOfRange => {
                "AsciiString::getCharAt: index past the string (undefined upstream)"
            }
        })
    }
}

/// `strlen`'s view: up to the first NUL.
fn c_str(s: &[u8]) -> &[u8] {
    match s.iter().position(|&b| b == 0) {
        Some(n) => &s[..n],
        None => s,
    }
}

/// `isspace` in the C locale.
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// The check of `ensureUniqueBufferOfSize(numCharsNeeded)`.
fn need(num_chars: usize) -> Result<(), AsciiStringError> {
    if HEADER_BYTES + num_chars > MAX_LEN {
        Err(AsciiStringError::TooLong)
    } else {
        Ok(())
    }
}

/// `class AsciiString`.
///
/// Equality and hashing follow upstream's `operator==`
/// (`strcmp(s1.str(), s2.str()) == 0`, `AsciiString.h:586`): they compare the
/// characters only, so a string with no buffer equals an empty string that
/// has one. (A derived `PartialEq` told them apart; found by the
/// `STLTypedefs` differential test, 12,388 of 50,000 pairs.)
#[derive(Clone, Debug, Default)]
pub struct AsciiString {
    /// `m_data`: `None` for a null pointer, else the characters (no NUL).
    data: Option<Vec<u8>>,
}

impl PartialEq for AsciiString {
    fn eq(&self, other: &Self) -> bool {
        self.str() == other.str()
    }
}

impl Eq for AsciiString {}

impl core::hash::Hash for AsciiString {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.str().hash(state);
    }
}

impl AsciiString {
    /// `AsciiString()`: empty, no buffer.
    pub const fn new() -> Self {
        AsciiString { data: None }
    }

    /// `AsciiString(const char* s)`.
    pub fn try_from_bytes(s: &[u8]) -> Result<Self, AsciiStringError> {
        let mut a = AsciiString::new();
        a.set(s)?;
        Ok(a)
    }

    /// Whether a buffer exists (upstream's `m_data != NULL`).
    pub fn has_buffer(&self) -> bool {
        self.data.is_some()
    }

    /// `str()`: the characters.
    pub fn str(&self) -> &[u8] {
        self.data.as_deref().unwrap_or(b"")
    }

    /// `getLength()`.
    pub fn get_length(&self) -> i32 {
        self.str().len() as i32
    }

    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.str().is_empty()
    }

    /// `isNotEmpty()`.
    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    /// `isNone()`: a buffer holding `None` in any case.
    pub fn is_none(&self) -> bool {
        self.data
            .as_deref()
            .is_some_and(|d| d.eq_ignore_ascii_case(b"None"))
    }

    /// `isNotNone()`.
    pub fn is_not_none(&self) -> bool {
        !self.is_none()
    }

    /// `clear()`: release the buffer.
    pub fn clear(&mut self) {
        self.data = None;
    }

    /// `getCharAt(index)`: see the module docs for the undefined cases.
    pub fn try_get_char_at(&self, index: i32) -> Result<u8, AsciiStringError> {
        match &self.data {
            None => Ok(0),
            Some(d) => {
                if index < 0 || index as usize > d.len() {
                    Err(AsciiStringError::IndexOutOfRange)
                } else {
                    Ok(d.get(index as usize).copied().unwrap_or(0))
                }
            }
        }
    }

    /// `set(const char* s)`: an empty `s` releases the buffer.
    pub fn set(&mut self, s: &[u8]) -> Result<(), AsciiStringError> {
        let s = c_str(s);
        if s.is_empty() {
            self.data = None;
        } else {
            need(s.len() + 1)?;
            self.data = Some(s.to_vec());
        }
        Ok(())
    }

    /// `concat(const char* s)`: with a buffer, grows it; without, `set(s)`.
    pub fn concat(&mut self, s: &[u8]) -> Result<(), AsciiStringError> {
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

    /// `concat(const char c)`: a NUL appends nothing.
    pub fn concat_char(&mut self, c: u8) -> Result<(), AsciiStringError> {
        self.concat(&[c])
    }

    /// `translate(const UnicodeString&)`: clear, then each `WideChar`'s low
    /// byte appended (one at a time, as upstream).
    pub fn translate(&mut self, src: &[u16]) -> Result<(), AsciiStringError> {
        self.clear();
        for &c in src {
            self.concat_char(c as u8)?;
        }
        Ok(())
    }

    /// `trim()`: drop leading and trailing `isspace` characters.
    pub fn trim(&mut self) -> Result<(), AsciiStringError> {
        let Some(d) = &self.data else { return Ok(()) };
        // skipWhitespace: stops at the first non-space byte (isspace is
        // evaluated on each byte it reaches).
        let mut start = 0;
        while start < d.len() {
            if d[start] >= 0x80 {
                return Err(AsciiStringError::NonAsciiCharClass);
            }
            if !is_c_space(d[start]) {
                break;
            }
            start += 1;
        }
        if start != 0 {
            let rest = d[start..].to_vec();
            self.set(&rest)?;
        }
        if self.data.is_some() {
            let mut i = self.str().len();
            while i > 0 {
                let b = self.str()[i - 1];
                if b >= 0x80 {
                    return Err(AsciiStringError::NonAsciiCharClass);
                }
                if !is_c_space(b) {
                    break;
                }
                self.remove_last_char()?;
                i -= 1;
            }
        }
        Ok(())
    }

    /// `toLower()`: with a buffer, the lower-cased copy goes through `set`
    /// (so an empty buffer is released).
    pub fn to_lower(&mut self) -> Result<(), AsciiStringError> {
        let Some(d) = &self.data else { return Ok(()) };
        if d.len() >= MAX_FORMAT_BUF_LEN {
            return Err(AsciiStringError::FormatBufferOverflow);
        }
        if d.iter().any(|&b| b >= 0x80) {
            return Err(AsciiStringError::NonAsciiCharClass);
        }
        let lower = d.to_ascii_lowercase();
        self.set(&lower)
    }

    /// `removeLastChar()`: the buffer stays, even when it becomes empty.
    pub fn remove_last_char(&mut self) -> Result<(), AsciiStringError> {
        if let Some(d) = &mut self.data {
            if !d.is_empty() {
                need(d.len() + 1)?;
                d.pop();
            }
        }
        Ok(())
    }

    /// `startsWith(p)`.
    pub fn starts_with(&self, p: &[u8]) -> bool {
        let p = c_str(p);
        p.is_empty() || self.str().starts_with(p)
    }

    /// `startsWithNoCase(p)` (`strnicmp`).
    pub fn starts_with_no_case(&self, p: &[u8]) -> bool {
        let p = c_str(p);
        let s = self.str();
        p.is_empty() || (s.len() >= p.len() && s[..p.len()].eq_ignore_ascii_case(p))
    }

    /// `endsWith(p)`.
    pub fn ends_with(&self, p: &[u8]) -> bool {
        let p = c_str(p);
        p.is_empty() || self.str().ends_with(p)
    }

    /// `endsWithNoCase(p)`.
    pub fn ends_with_no_case(&self, p: &[u8]) -> bool {
        let p = c_str(p);
        let s = self.str();
        p.is_empty() || (s.len() >= p.len() && s[s.len() - p.len()..].eq_ignore_ascii_case(p))
    }

    /// `compare(s)`: `strcmp` (bytes as unsigned).
    pub fn compare(&self, s: &[u8]) -> Ordering {
        self.str().cmp(c_str(s))
    }

    /// `compareNoCase(s)`: `_stricmp` (ASCII folded to lower case, then
    /// bytes as unsigned).
    pub fn compare_no_case(&self, s: &[u8]) -> Ordering {
        let a = self.str().iter().map(u8::to_ascii_lowercase);
        let b = c_str(s).iter().map(u8::to_ascii_lowercase);
        a.cmp(b)
    }

    /// `find(c)`: the offset of the first `c` (`strchr`; `c == 0` finds the
    /// terminator), or `None`.
    pub fn find(&self, c: u8) -> Option<usize> {
        let s = self.str();
        if c == 0 {
            return Some(s.len());
        }
        s.iter().position(|&b| b == c)
    }

    /// `reverseFind(c)`: the offset of the last `c` (`strrchr`).
    pub fn reverse_find(&self, c: u8) -> Option<usize> {
        let s = self.str();
        if c == 0 {
            return Some(s.len());
        }
        s.iter().rposition(|&b| b == c)
    }

    /// `nextToken(tok, seps)`: skip separators (default `" \n\r\t"`), take
    /// the token into `tok`, keep the rest. False (and nothing changed) on
    /// an empty string; false, with both cleared, when only separators are
    /// left. (Upstream also returns false when `tok` is this string, which
    /// Rust's borrow rules exclude.)
    pub fn next_token(
        &mut self,
        tok: &mut AsciiString,
        seps: Option<&[u8]>,
    ) -> Result<bool, AsciiStringError> {
        if self.is_empty() {
            return Ok(false);
        }
        let seps = c_str(seps.unwrap_or(b" \n\r\t"));
        let s = self.str();
        let start = s.iter().position(|b| !seps.contains(b)).unwrap_or(s.len());
        let end = start
            + s[start..]
                .iter()
                .position(|b| seps.contains(b))
                .unwrap_or(s.len() - start);
        if end > start {
            let len = end - start;
            // tok->getBufferForRead(len + 1): a buffer of len + 2 chars.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream `operator==` compares characters only (`strcmp` of `str()`).
    #[test]
    fn equality_ignores_the_buffer() {
        use core::hash::{Hash, Hasher};
        let none = AsciiString::new();
        let mut empty = AsciiString::try_from_bytes(b"x").unwrap();
        empty.remove_last_char().unwrap();
        assert!(!none.has_buffer() && empty.has_buffer() && empty.is_empty());
        assert_eq!(none, empty);
        let hash = |s: &AsciiString| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            s.hash(&mut h);
            h.finish()
        };
        assert_eq!(hash(&none), hash(&empty));
        assert_ne!(none, AsciiString::try_from_bytes(b"a").unwrap());
    }

    #[test]
    fn buffer_state_and_tokens() {
        let mut a = AsciiString::try_from_bytes(b"x").unwrap();
        a.remove_last_char().unwrap();
        assert!(
            a.has_buffer() && a.is_empty(),
            "removeLastChar keeps the buffer"
        );
        assert_eq!(a.try_get_char_at(0), Ok(0));
        assert_eq!(a.try_get_char_at(1), Err(AsciiStringError::IndexOutOfRange));
        a.to_lower().unwrap();
        assert!(!a.has_buffer(), "toLower of an empty buffer releases it");
        assert_eq!(a.try_get_char_at(5), Ok(0), "no buffer: 0 at any index");
        let mut s = AsciiString::try_from_bytes(b"  alpha beta").unwrap();
        let mut t = AsciiString::new();
        assert_eq!(s.next_token(&mut t, None), Ok(true));
        assert_eq!((t.str(), s.str()), (&b"alpha"[..], &b" beta"[..]));
        let mut w = AsciiString::new();
        w.translate(&[0x41, 0x100, 0x142]).unwrap();
        assert_eq!(w.str(), b"AB", "0x100 has a zero low byte and is dropped");
        assert!(AsciiString::try_from_bytes(b"NONE").unwrap().is_none());
        let long = vec![b'a'; MAX_LEN - HEADER_BYTES];
        assert_eq!(
            AsciiString::try_from_bytes(&long),
            Err(AsciiStringError::TooLong)
        );
        assert!(AsciiString::try_from_bytes(&long[1..]).is_ok());
    }
}
