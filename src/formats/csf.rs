// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameText.cpp (the CSF path:
//   init, getCSFInfo, parseCSF, stripSpaces, fetch,
//   getStringsWithLabelPrefix, compareLUT)
//   GeneralsMD/Code/GameEngine/Include/GameClient/GameText.h
//   (and the RAMFile::read the file is read through, from
//   GeneralsMD/Code/GameEngine/Source/Common/System/RAMFile.cpp)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   GameText.cpp: Project RTS3, created 11/07/01
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Localised strings: the compiled string file (`.csf`) loader of
//! `GameTextManager` and its label lookup, reading from an in-memory byte
//! buffer (wasm has no file system).
//!
//! # The format, as upstream reads it
//!
//! All integers are 32-bit little-endian `Int`s; the four-character tags are
//! integers, so `'CSF '` is stored as the bytes `" FSC"`.
//!
//! ```text
//! header (24 bytes): id 'CSF ', version, num_labels, num_strings, skip, langid
//! then until the end of the file, num_labels times (not checked):
//!   'LBL ', num_strings, len, len bytes of label
//!   num_strings times:
//!     'STR ' or 'STRW', len, len 16-bit units of text, each bit-inverted
//!     if 'STRW': len, len bytes of a speech (wave file) name
//! ```
//!
//! Only the first string of a label is kept; its units are inverted (`~`)
//! up to the first unit that is 0 before inversion, the text ends at the
//! first unit that is 0 after it, and runs of spaces are squeezed
//! ([`strip_spaces`]). `version >= 2` makes `langid` the language, else
//! `LANGUAGE_ID_US`. `num_strings` and `skip` of the header are not used.
//! Labels are looked up case-insensitively (`stricmp`) in a table sorted with
//! `qsort` and searched with `bsearch`. The table has `num_labels` entries
//! even when the file holds fewer labels: the rest have empty labels and
//! texts (so `fetch("")` finds one of them).
//!
//! The reads are modelled exactly: the file is a `RAMFile` (the `.csf` comes
//! from a `.big` archive), whose `read` copies what is left and moves the
//! position by the count it returns, so a negative length moves it
//! **backwards**; integers read at the end of the file keep their old bytes
//! above the ones read; the 10240-byte label buffer and the 40960-byte text
//! buffer keep stale bytes past a short read; and a truncated file ends the
//! loop only where the loop condition reads.
//!
//! # What is left out
//!
//! - The `.str` text format (`getStringCount`, `parseStringFile`,
//!   `readToEndOfQuote`, `translateCopy`, `reverseWord`, `readLine`,
//!   `readChar`) and the map string files (`initMapStringFile`,
//!   `parseMapStringFile`, whose table `fetch` also searches). Upstream's
//!   `init` prefers `data\Generals.str` when it exists; this loads the CSF.
//! - Setting the window title after loading, and the `_DEBUG`/`_INTERNAL`
//!   jabber/munkee test modes.
//! - Speech names are read and kept ([`StringInfo::speech`]) but no
//!   upstream method returns them (they are dead data in `GameText.cpp`), so
//!   only their effect on the read position is differential-tested.
//! - The `MISSING:` text of an unknown label is computed by [`missing_text`]
//!   as Win32's `_vsnwprintf` formats it, but is not differential-tested
//!   (only whether the label exists is).
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`CsfError::Undefined`]: a negative label count (`new StringInfo[n]`
//! with `n < 0`); more labels in the file than declared (a heap overrun of
//! the `num_labels`-entry array); a length whose terminating NUL or whose
//! data falls outside its buffer (`m_buffer[len] = 0` with `len` outside
//! 0..10240, text lengths outside 0..20480 for the first string); a read from
//! before the start of the file (after a negative length); `num_strings`, or
//! the first `len`, cut off by the end of the file (uninitialised); and a
//! file position that overflows `Int`.
//!
//! # Upstream quirks that are kept
//!
//! - Duplicate labels (equal under `stricmp`): which one `fetch` finds is
//!   unspecified upstream (`qsort` is not stable, `bsearch` returns any
//!   match). Here the first in file order wins; [`GameText::is_ambiguous`]
//!   tells whether the texts differ.
//! - A label with no strings has an empty text; a `STRW` speech name is kept
//!   only for the first string and only if not empty.
//! - A header that declares far more labels than the file holds: upstream
//!   allocates all of them (and fails for counts near 2^31); here the empty
//!   entries are not stored.
//! - A file that ends inside a label's strings: upstream runs the rest of
//!   the string loop reading nothing, up to 2^31 times for a damaged count;
//!   the Rust skips those iterations, which change nothing
//!   ([`GameText::eof_spin`]).
//! - The file may end anywhere a label starts (or inside the first four
//!   bytes of one): the labels read so far load.

/// `CSF_ID` ('CSF ').
pub const CSF_ID: i32 = i32::from_be_bytes(*b"CSF ");
/// `CSF_LABEL` ('LBL ').
pub const CSF_LABEL: i32 = i32::from_be_bytes(*b"LBL ");
/// `CSF_STRING` ('STR ').
pub const CSF_STRING: i32 = i32::from_be_bytes(*b"STR ");
/// `CSF_STRINGWITHWAVE` ('STRW').
pub const CSF_STRINGWITHWAVE: i32 = i32::from_be_bytes(*b"STRW");
/// `CSF_VERSION` (written by the tools; the reader accepts any).
pub const CSF_VERSION: i32 = 3;
/// `MAX_UITEXT_LENGTH`: the size of `m_buffer`, and half that of `m_tbuffer`
/// in `WideChar`s.
pub const MAX_UITEXT_LENGTH: usize = 10 * 1024;
/// `LANGUAGE_ID_US` (Common/Language.h).
pub const LANGUAGE_ID_US: i32 = 0;
/// `MAX_FORMAT_BUF_LEN` of UnicodeString::format (Common/UnicodeString.h).
pub const MAX_FORMAT_BUF_LEN: usize = 2048;
/// `m_failed`: what `fetch` returns when the manager has no strings.
pub const FAILED_TEXT: &str = "***FATAL*** String Manager failed to initilaize properly";

const TBUFFER_UNITS: usize = MAX_UITEXT_LENGTH * 2;

/// Where upstream is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CsfUndefined {
    /// The header declares fewer than 0 labels.
    NegativeLabelCount,
    /// The file holds more labels than the header declares.
    MoreLabelsThanDeclared,
    /// A length puts data or its NUL outside the buffer.
    BufferOverrun,
    /// A read from before the start of the file.
    ReadBeforeStart,
    /// A variable read only partly (or not at all) before its first use.
    UninitialisedRead,
    /// The file position overflows `Int`.
    PositionOverflow,
}

/// Why the CSF did not load. Upstream's manager then has no strings and
/// `fetch` returns [`FAILED_TEXT`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsfError {
    /// `getCSFInfo` failed: shorter than the header, or not `'CSF '`.
    NotCsf,
    /// The header declares no labels (`m_textCount == 0`).
    NoLabels,
    /// `parseCSF` failed: a tag that is not `'LBL '` where a label starts, or
    /// not `'STR '`/`'STRW'` where a string starts.
    Corrupt,
    /// Upstream is undefined for this file.
    Undefined(CsfUndefined),
}

impl std::fmt::Display for CsfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CsfError::NotCsf => write!(f, "not a CSF string file"),
            CsfError::NoLabels => write!(f, "CSF string file declares no labels"),
            CsfError::Corrupt => write!(f, "CSF string file has a bad label or string tag"),
            CsfError::Undefined(u) => write!(f, "CSF string file outside what the original reader defines: {u:?}"),
        }
    }
}

impl std::error::Error for CsfError {}

/// `struct CSFHeader`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CsfHeader {
    pub id: i32,
    pub version: i32,
    pub num_labels: i32,
    pub num_strings: i32,
    pub skip: i32,
    pub langid: i32,
}

impl CsfHeader {
    /// The header as `file->read(&header, 24)` fills it; `None` if the data
    /// is shorter.
    pub fn read(data: &[u8]) -> Option<CsfHeader> {
        if data.len() < 24 {
            return None;
        }
        let w = |i: usize| i32::from_le_bytes([data[4 * i], data[4 * i + 1], data[4 * i + 2], data[4 * i + 3]]);
        Some(CsfHeader { id: w(0), version: w(1), num_labels: w(2), num_strings: w(3), skip: w(4), langid: w(5) })
    }
}

/// `struct StringInfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StringInfo {
    pub label: Vec<u8>,
    /// UTF-16 code units (`WideChar` is 16 bits on Win32).
    pub text: Vec<u16>,
    pub speech: Vec<u8>,
}

/// `GameTextManager::stripSpaces`: drop leading spaces and spaces after
/// `\n`/`\t`, squeeze runs of spaces to one, and drop a space before
/// `\n`/`\t` and at the end. Works up to the first 0 unit, as upstream.
pub fn strip_spaces(s: &mut Vec<u16>) {
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    s.truncate(end);
    let sp = u16::from(b' ');
    let (mut out, mut last, mut skipall) = (0usize, 0u16, true);
    for i in 0..s.len() {
        let ch = s[i];
        if ch == sp && (last == sp || skipall) {
            continue;
        }
        if ch == u16::from(b'\n') || ch == u16::from(b'\t') {
            if last == sp {
                out -= 1;
            }
            skipall = true;
            s[out] = ch;
            out += 1;
            last = ch;
            continue;
        }
        s[out] = ch;
        out += 1;
        last = ch;
        skipall = false;
    }
    if last == sp {
        out -= 1;
    }
    s.truncate(out);
}

/// The text `fetch` returns for an unknown label:
/// `format(L"MISSING: '%hs'", label)`, with Win32's `_vsnwprintf`, which
/// widens each byte of the label (the C locale) and fails, throwing
/// `ERROR_OUT_OF_MEMORY` upstream, when the text does not fit its 2047
/// units (`None`). Not differential-tested (see the module docs).
pub fn missing_text(label: &[u8]) -> Option<Vec<u16>> {
    let mut t: Vec<u16> = "MISSING: '".encode_utf16().collect();
    t.extend(label.iter().map(|&b| u16::from(b)));
    t.push(u16::from(b'\''));
    if t.len() > MAX_FORMAT_BUF_LEN - 1 {
        return None;
    }
    Some(t)
}

/// The `RAMFile` the CSF is read through, with `RAMFile::read`'s semantics.
struct RamFile<'a> {
    data: &'a [u8],
    pos: i64,
}

impl RamFile<'_> {
    /// `RAMFile::read(buffer, bytes)` into `dst`: clamps to what is left,
    /// copies only a positive count, moves the position by the count (so a
    /// negative one moves it back), returns the count.
    fn read(&mut self, dst: &mut [u8], bytes: i32) -> Result<i32, CsfUndefined> {
        let left = self.data.len() as i64 - self.pos;
        if i32::try_from(left).is_err() {
            return Err(CsfUndefined::PositionOverflow);
        }
        let n = i64::from(bytes).min(left);
        if n > 0 {
            if self.pos < 0 {
                return Err(CsfUndefined::ReadBeforeStart);
            }
            if n as usize > dst.len() {
                return Err(CsfUndefined::BufferOverrun);
            }
            let p = self.pos as usize;
            dst[..n as usize].copy_from_slice(&self.data[p..p + n as usize]);
        }
        let np = self.pos + n;
        if i32::try_from(np).is_err() {
            return Err(CsfUndefined::PositionOverflow);
        }
        self.pos = np;
        Ok(n as i32)
    }

    /// `file->read(&v, sizeof(Int))`: the bytes read replace the low bytes of
    /// `v` (little-endian), the others keep their value. Returns the count.
    fn read_int(&mut self, v: &mut i32) -> Result<i32, CsfUndefined> {
        let mut b = v.to_le_bytes();
        let n = self.read(&mut b, 4)?;
        *v = i32::from_le_bytes(b);
        Ok(n)
    }
}

fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

fn unit(t: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([t[2 * i], t[2 * i + 1]])
}

fn set_unit(t: &mut [u8], i: usize, v: u16) {
    t[2 * i..2 * i + 2].copy_from_slice(&v.to_le_bytes());
}

/// `GameTextManager` with a CSF loaded.
#[derive(Clone, Debug)]
pub struct GameText {
    /// The labels read, in file order.
    infos: Vec<StringInfo>,
    /// `m_textCount`: the table size (labels read plus empty entries).
    text_count: usize,
    /// Indices into `infos`, sorted by `stricmp` of the label (stable).
    lut: Vec<usize>,
    language: i32,
    max_label_len: i32,
    eof_spin: u64,
}

impl GameText {
    /// `GameTextManager::init` on a CSF file's bytes (`getCSFInfo`, then
    /// `parseCSF`, then the sorted lookup table).
    pub fn load_csf(data: &[u8]) -> Result<GameText, CsfError> {
        // getCSFInfo
        let header = CsfHeader::read(data).ok_or(CsfError::NotCsf)?;
        if header.id != CSF_ID {
            return Err(CsfError::NotCsf);
        }
        let language = if header.version >= 2 { header.langid } else { LANGUAGE_ID_US };
        if header.num_labels == 0 {
            return Err(CsfError::NoLabels);
        }
        if header.num_labels < 0 {
            return Err(CsfError::Undefined(CsfUndefined::NegativeLabelCount));
        }
        let text_count = header.num_labels as usize;
        let (infos, max_label_len, eof_spin) = parse_csf(data, text_count)?;
        let mut lut: Vec<usize> = (0..infos.len()).collect();
        lut.sort_by(|&a, &b| infos[a].label.to_ascii_lowercase().cmp(&infos[b].label.to_ascii_lowercase()));
        Ok(GameText { infos, text_count, lut, language, max_label_len, eof_spin })
    }

    /// `m_language`.
    pub fn language(&self) -> i32 {
        self.language
    }
    /// `m_textCount`.
    pub fn text_count(&self) -> usize {
        self.text_count
    }
    /// `m_maxLabelLen`.
    pub fn max_label_len(&self) -> i32 {
        self.max_label_len
    }
    /// How many string iterations upstream runs at the end of the file
    /// reading nothing (a label whose string count the file does not hold;
    /// up to 2^31 for a damaged count), which the Rust skips.
    pub fn eof_spin(&self) -> u64 {
        self.eof_spin
    }
    /// The labels read, in file order.
    pub fn strings(&self) -> &[StringInfo] {
        &self.infos
    }
    /// Table entries past the labels read (empty label and text).
    fn padding(&self) -> usize {
        self.text_count - self.infos.len()
    }

    /// The labels read that equal `label` under `stricmp`, in file order.
    fn matches(&self, label: &[u8]) -> Vec<&StringInfo> {
        let key = label.to_ascii_lowercase();
        let lo = self.lut.partition_point(|&i| self.infos[i].label.to_ascii_lowercase() < key);
        self.lut[lo..]
            .iter()
            .take_while(|&&i| self.infos[i].label.to_ascii_lowercase() == key)
            .map(|&i| &self.infos[i])
            .collect()
    }

    /// `fetch(label, &exists)`: the text, or `None` where upstream sets
    /// `exists` to FALSE (and returns [`missing_text`]).
    pub fn fetch(&self, label: &[u8]) -> Option<&[u16]> {
        if label.is_empty() && self.padding() > 0 {
            // the empty entries sort first; with a read label "" as well
            // the match is ambiguous (see is_ambiguous): take the first read
            return Some(self.matches(label).first().map_or(&[][..], |s| s.text.as_slice()));
        }
        self.matches(label).first().map(|s| s.text.as_slice())
    }

    /// Whether upstream's result for `fetch(label)` is unspecified: several
    /// entries match under `stricmp` and their texts differ.
    pub fn is_ambiguous(&self, label: &[u8]) -> bool {
        let m = self.matches(label);
        let mut texts: Vec<&[u16]> = m.iter().map(|s| s.text.as_slice()).collect();
        if label.is_empty() && self.padding() > 0 {
            texts.push(&[]);
        }
        texts.windows(2).any(|w| w[0] != w[1])
    }

    /// `getStringsWithLabelPrefix(prefix)`: the labels that start with
    /// `prefix` (case-sensitive), in table order. Among labels equal under
    /// `stricmp` the order is unspecified upstream; here it is file order.
    pub fn get_strings_with_label_prefix(&self, prefix: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        if prefix.is_empty() {
            out.extend(std::iter::repeat_n(Vec::new(), self.padding()));
        }
        for &i in &self.lut {
            if self.infos[i].label.starts_with(prefix) {
                out.push(self.infos[i].label.clone());
            }
        }
        if prefix.is_empty() {
            // the empty entries and any read empty labels sort together
            out.sort_by_key(|l| !l.is_empty());
        }
        out
    }
}

/// `GameTextManager::parseCSF` into a table of `text_count` entries: the
/// labels read, in file order, and `m_maxLabelLen`.
fn parse_csf(data: &[u8], text_count: usize) -> Result<(Vec<StringInfo>, i32, u64), CsfError> {
    let ub = CsfError::Undefined;
    let mut file = RamFile { data, pos: 0 };
    let mut header = [0u8; 24];
    if file.read(&mut header, 24).map_err(ub)? != 24 {
        return Err(CsfError::Corrupt); // cannot happen: getCSFInfo read it
    }
    let mut buffer = vec![0u8; MAX_UITEXT_LENGTH]; // m_buffer (zeroed by the constructor)
    let mut tbuffer = vec![0u8; TBUFFER_UNITS * 2]; // m_tbuffer (zero-filled allocation)
    let mut infos: Vec<StringInfo> = Vec::new();
    let mut max_label_len = 0i32;
    let mut id = 0i32;
    let mut len = 0i32;
    let mut len_set = false;
    let mut eof_spin = 0u64;

    // `len` is read into `buffer` (`m_buffer[len] = 0` must be in range)
    let read_bytes = |file: &mut RamFile<'_>, buffer: &mut Vec<u8>, len: i32| -> Result<(), CsfError> {
        if len != 0 {
            file.read(buffer, len).map_err(ub)?;
        }
        if !(0..MAX_UITEXT_LENGTH as i32).contains(&len) {
            return Err(ub(CsfUndefined::BufferOverrun));
        }
        buffer[len as usize] = 0;
        Ok(())
    };

    while file.read_int(&mut id).map_err(ub)? == 4 {
        if id != CSF_LABEL {
            return Err(CsfError::Corrupt);
        }
        let mut num_strings = 0i32;
        if file.read_int(&mut num_strings).map_err(ub)? != 4 {
            return Err(ub(CsfUndefined::UninitialisedRead));
        }
        let n = file.read_int(&mut len).map_err(ub)?;
        if n != 4 && !len_set {
            return Err(ub(CsfUndefined::UninitialisedRead));
        }
        len_set = true;
        read_bytes(&mut file, &mut buffer, len)?;
        if infos.len() >= text_count {
            return Err(ub(CsfUndefined::MoreLabelsThanDeclared));
        }
        let mut info = StringInfo { label: c_str(&buffer).to_vec(), ..StringInfo::default() };
        if len > max_label_len {
            max_label_len = len;
        }
        let mut num = 0;
        while num < num_strings {
            // At the end of the file, with a length whose reads copy nothing
            // and do not move back, every remaining string reads 0 bytes and
            // changes nothing (id stays a valid tag, the STRW NUL lands where
            // it did): upstream spins through them; skip them, counted.
            if num >= 1 && file.pos == data.len() as i64 && len >= 0 && len.checked_mul(2).is_some() {
                eof_spin += (num_strings - num) as u64;
                break;
            }
            file.read_int(&mut id).map_err(ub)?;
            if id != CSF_STRING && id != CSF_STRINGWITHWAVE {
                return Err(CsfError::Corrupt);
            }
            file.read_int(&mut len).map_err(ub)?;
            if len != 0 {
                file.read(&mut tbuffer, len.wrapping_mul(2)).map_err(ub)?;
            }
            if num == 0 {
                // only use the first string found
                if !(0..TBUFFER_UNITS as i32).contains(&len) {
                    return Err(ub(CsfUndefined::BufferOverrun));
                }
                set_unit(&mut tbuffer, len as usize, 0);
                let mut k = 0;
                while unit(&tbuffer, k) != 0 {
                    let inverted = !unit(&tbuffer, k);
                    set_unit(&mut tbuffer, k, inverted);
                    k += 1;
                }
                // stripSpaces works in place on m_tbuffer
                let n = (0..).position(|i| unit(&tbuffer, i) == 0).unwrap_or(TBUFFER_UNITS);
                let mut text: Vec<u16> = (0..n).map(|i| unit(&tbuffer, i)).collect();
                strip_spaces(&mut text);
                for (i, &c) in text.iter().enumerate() {
                    set_unit(&mut tbuffer, i, c);
                }
                set_unit(&mut tbuffer, text.len(), 0);
                info.text = text;
            }
            if id == CSF_STRINGWITHWAVE {
                file.read_int(&mut len).map_err(ub)?;
                read_bytes(&mut file, &mut buffer, len)?;
                if num == 0 && len != 0 {
                    info.speech = c_str(&buffer).to_vec();
                }
            }
            num += 1;
        }
        infos.push(info);
    }
    Ok((infos, max_label_len, eof_spin))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(v: &mut Vec<u8>, x: i32) {
        v.extend_from_slice(&x.to_le_bytes());
    }

    fn csf(labels: &[(&str, &str, Option<&str>)], declared: i32) -> Vec<u8> {
        let mut v = Vec::new();
        for x in [CSF_ID, CSF_VERSION, declared, labels.len() as i32, 0, 4] {
            w(&mut v, x);
        }
        for (l, t, s) in labels {
            w(&mut v, CSF_LABEL);
            w(&mut v, 1);
            w(&mut v, l.len() as i32);
            v.extend_from_slice(l.as_bytes());
            w(&mut v, if s.is_some() { CSF_STRINGWITHWAVE } else { CSF_STRING });
            let units: Vec<u16> = t.encode_utf16().collect();
            w(&mut v, units.len() as i32);
            for u in units {
                v.extend_from_slice(&(!u).to_le_bytes());
            }
            if let Some(s) = s {
                w(&mut v, s.len() as i32);
                v.extend_from_slice(s.as_bytes());
            }
        }
        v
    }

    fn u(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn loads_and_fetches() {
        assert_eq!(&CSF_ID.to_le_bytes(), b" FSC");
        let data = csf(&[("GUI:Ok", "  O  K ", None), ("gui:cancel", "Cancel\n  x", Some("speech.wav"))], 3);
        let t = GameText::load_csf(&data).unwrap();
        assert_eq!(t.language(), 4);
        assert_eq!(t.fetch(b"gui:ok"), Some(&u("O K")[..]));
        assert_eq!(t.fetch(b"GUI:CANCEL"), Some(&u("Cancel\nx")[..]));
        assert_eq!(t.strings()[1].speech, b"speech.wav");
        assert_eq!(t.fetch(b"nope"), None);
        assert_eq!(t.fetch(b""), Some(&[][..])); // one empty entry (3 declared, 2 read)
        assert_eq!(t.get_strings_with_label_prefix(b"G"), vec![b"GUI:Ok".to_vec()]);
        assert_eq!(GameText::load_csf(&data[..23]).unwrap_err(), CsfError::NotCsf);
        assert_eq!(GameText::load_csf(&csf(&[("a", "b", None)], 0)).unwrap_err(), CsfError::NoLabels);
        assert_eq!(
            GameText::load_csf(&csf(&[("a", "b", None), ("c", "d", None)], 1)).unwrap_err(),
            CsfError::Undefined(CsfUndefined::MoreLabelsThanDeclared)
        );
        assert_eq!(missing_text(b"x").unwrap(), u("MISSING: 'x'"));
    }

    #[test]
    fn strip_spaces_as_upstream() {
        let mut s = u("  a  b \n  c\t d  ");
        strip_spaces(&mut s);
        assert_eq!(s, u("a b\nc\td"));
    }
}
