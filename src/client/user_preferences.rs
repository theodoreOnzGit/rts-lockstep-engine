// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/UserPreferences.h (PreferenceMap, class UserPreferences)
//   GeneralsMD/Code/GameEngine/Source/Common/UserPreferences.cpp (intAsStr, boolAsStr, realAsStr, LINE_LEN,
//     UserPreferences::UserPreferences, ~UserPreferences, load, write, getBool, getReal, getInt, getAsciiString,
//     setBool, setReal, setInt, setAsciiString)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The user's preference files (GitHub #93, #88): `UserPreferences`, a sorted map of keys to values read from and
//! written to a file in the user data folder (`Options.ini`, `Skirmish.ini`, `Network.ini`, ...).
//!
//! # As upstream reads it
//!
//! **`load(fname)`** sets the file name to `TheGlobalData->getPath_UserData()` followed by `fname` and opens it in
//! text mode (`fopen(.., "r")`). Each `fgets` of at most `LINE_LEN - 1` (2,047) bytes is a line: made an `AsciiString`
//! (up to its first NUL), trimmed, split at the first run of `=` by `nextToken(&key, "=")`, and the value is the rest
//! after its first character (`line.str() + 1`). Key and value are trimmed; a line with an empty key or value is
//! skipped; otherwise the value replaces the key's (`(*this)[key] = val`: the last of duplicate keys wins). Load answers
//! whether the file opened; a missing file leaves the map as it was (empty for a new object).
//!
//! **`write()`** (nothing without a file name, i.e. before a `load`) opens the file for writing in text mode and prints
//! each pair in the map's order as `"%s = %s\n"`; it answers whether the file opened.
//!
//! **The readers** look the key up (`getAsciiString`; the default without it) and convert a non-empty value: `getBool`
//! is true for `1`, `t`, `true`, `y`, `yes` or `ok` in any case; `getReal` is `(Real)atof`; `getInt` is `atoi`. **The
//! writers** store `%d` of an `Int` or a `Bool` and `%g` of a `Real`.
//!
//! # Upstream quirks that are kept
//!
//! - A line longer than 2,047 bytes is read as several lines (each `fgets` is processed alone).
//! - A line is cut at its first NUL byte (`AsciiString line = buf`), the rest of that `fgets` lost.
//! - `nextToken(&key, "=")` skips leading `=`: `=a=b` reads key `a`, value `b`... after the *first* character of the
//!   rest, so `a==b` reads key `a` and value `=b`.
//! - A key holding `=` or a value with surrounding blanks or a line break can be set, written, and then reads back
//!   differently (`a=b = c` is key `a`, value `b = c`).
//! - `getBool` of a value that is not one of its six words is false, not the default.
//!
//! # Where upstream is undefined (recorded in [`UserPreferences::undefined`])
//!
//! - `val = line.str() + 1` when nothing follows the key (no `=` in the line, a line of separators only, or an empty
//!   line): `str()` of a string without a buffer is the address of a one-byte static (`TheNullChr`), and the byte after
//!   it is read. Here the value is taken as empty (the line is skipped) and the key recorded
//!   ([`PrefsUndefined::ReadPastEmptyLine`]).
//! - `trim` (and `getBool`'s `toLower`) passes a byte from 0x80 to `isspace` / `tolower`: a negative `char`
//!   ([`PrefsUndefined::NonAsciiCharClass`]); taken here as the C locale reads it (not a space, unchanged).
//! - `atoi` of a value outside `Int`'s range ([`PrefsUndefined::IntOutOfRange`]); taken here as the digits wrapped
//!   to 32 bits (**not re-checked** against MSVC's `atoi`).
//!
//! # Where this differs from upstream
//!
//! - **The file** is the host's ([`UserDataFiles`]): the client keeps the user data folder on disk natively and in the
//!   browser's OPFS (`client/src/user_data.rs`). The bytes `fgets` reads are those of MSVC's text mode
//!   ([`crt_text_read`]: CR LF read as LF, a Ctrl-Z ends the file); a write is its text mode too ([`crt_text_write`]:
//!   LF written as CR LF), so a file is the same as the game writes on Windows.
//! - `atof` reads a decimal number (`[blanks][sign]digits[.digits][e[sign]digits]`) and rounds it once to `double`;
//!   the hexadecimal, `inf` and `nan` forms that later C libraries accept are not read (MSVC's of the time did not:
//!   **not re-checked**).
//! - `%g` (`realAsStr`) is six significant digits, rounded half to even on the exact value, with a three-digit
//!   exponent as MSVC's C library prints it (**not re-checked**; nothing in Zero Hour calls `setReal`).
//! - The subclasses of UserPreferences.cpp for the online service (`QuickMatchPreferences`,
//!   `CustomMatchPreferences`, `GameSpyMiscPreferences`, `IgnorePreferences`, `LadderPreferences`) are not
//!   translated: GameSpy is not part of the client.
//!
//! # Representation
//!
//! [`UserPreferences`]: the map (`std::map<AsciiString, AsciiString>`, ordered by `strcmp`: bytes as unsigned, which is
//! [`BTreeMap`]'s order of `Vec<u8>`) and `m_filename`.

use std::collections::BTreeMap;

use crate::client::main_menu::MenuHost;
use crate::client::window_manager::GameWindowManager;
use crate::common::ascii_string::AsciiString;
use crate::logic::modules::plant;

/// `typedef std::map<AsciiString, AsciiString> PreferenceMap`.
pub type PreferenceMap = BTreeMap<Vec<u8>, Vec<u8>>;

/// `LINE_LEN`: `load`'s line buffer.
pub const LINE_LEN: usize = 2048;

/// The user data folder's files, as `fopen` / `fgets` / `fprintf` reach them.
pub trait UserDataFiles {
    /// `TheGlobalData->getPath_UserData()`.
    fn path_user_data(&self) -> Vec<u8>;
    /// `fopen(path, "r")`: the file's bytes, or `None` when it does not open.
    fn user_data_read(&mut self, path: &[u8]) -> Option<Vec<u8>>;
    /// `fopen(path, "w")` and the bytes written to it; whether it opened.
    fn user_data_write(&mut self, path: &[u8], bytes: &[u8]) -> bool;
}

/// What upstream leaves undefined while reading or converting (see the module docs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrefsUndefined {
    /// `line.str() + 1` read past the empty line's terminator; the key (empty if none).
    ReadPastEmptyLine(Vec<u8>),
    /// `isspace` / `tolower` of a byte from 0x80.
    NonAsciiCharClass,
    /// `atoi` of a value outside `Int`'s range.
    IntOutOfRange,
}

/// `class UserPreferences : public PreferenceMap`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserPreferences {
    /// The map.
    pub map: PreferenceMap,
    /// `m_filename` (empty until `load`).
    pub filename: Vec<u8>,
    /// What upstream left undefined on this object, in order (not part of upstream's object).
    pub undefined: Vec<PrefsUndefined>,
}

/// MSVC's text-mode read (`fopen(.., "r")`): CR LF is read as LF and a Ctrl-Z (0x1A) ends the file.
pub fn crt_text_read(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let c = raw[i];
        if c == 0x1a {
            break;
        }
        if c == b'\r' && raw.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// MSVC's text-mode write (`fopen(.., "w")`): each LF written as CR LF.
pub fn crt_text_write(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 16);
    for &c in text {
        if c == b'\n' {
            out.push(b'\r');
        }
        out.push(c);
    }
    out
}

/// `fgets(buf, LINE_LEN, fp)` over `text` from `*pos`: up to `LINE_LEN - 1` bytes, through the first LF; `None` at
/// the end.
fn fgets<'a>(text: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
    if *pos >= text.len() {
        return None;
    }
    let rest = &text[*pos..];
    let max = (if plant(8945) { LINE_LEN } else { LINE_LEN - 1 }).min(rest.len());
    let n = rest[..max].iter().position(|&c| c == b'\n').map_or(max, |i| i + 1);
    *pos += n;
    Some(&rest[..n])
}

/// `isspace` in the C locale (a byte from 0x80 is not a space; that it was asked is recorded by the callers).
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `AsciiString::trim()` (the translated one, `crate::common::ascii_string`), with its undefined `isspace` of a byte
/// from 0x80 recorded and read as the C locale reads it (not a space).
fn trim(s: &mut AsciiString, undefined: &mut Vec<PrefsUndefined>) {
    let mut t = s.clone();
    if t.trim().is_ok() {
        *s = t;
        return;
    }
    undefined.push(PrefsUndefined::NonAsciiCharClass);
    let b = s.str();
    let start = b.iter().position(|&c| !is_c_space(c)).unwrap_or(b.len());
    let end = b.iter().rposition(|&c| !is_c_space(c)).map_or(start, |e| e + 1).max(start);
    let kept = b[start..end].to_vec();
    if start != 0 || end != b.len() {
        // (`set` from the tail releases the buffer when nothing is left; `removeLastChar` keeps it)
        let _ = s.set(&kept);
    }
}

/// `atoi(s)`: leading blanks, a sign, digits. A magnitude outside `Int`'s range is undefined (recorded); the digits
/// are wrapped to 32 bits.
pub fn atoi(s: &[u8], undefined: &mut Vec<PrefsUndefined>) -> i32 {
    let mut i = s.iter().position(|&c| !is_c_space(c)).unwrap_or(s.len());
    let neg = match s.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let mut v: u32 = 0;
    let mut big: u128 = 0;
    for &c in s[i..].iter().take_while(|c| c.is_ascii_digit()) {
        v = v.wrapping_mul(10).wrapping_add(u32::from(c - b'0'));
        big = (big * 10 + u128::from(c - b'0')).min(1 << 40);
    }
    let limit = if neg { 1u128 << 31 } else { (1u128 << 31) - 1 };
    if big > limit {
        undefined.push(PrefsUndefined::IntOutOfRange);
    }
    if neg {
        v.wrapping_neg() as i32
    } else {
        v as i32
    }
}

/// `atof(s)`: leading blanks, a sign, digits with an optional point and fraction, an optional exponent; the longest
/// such prefix rounded once to `double` (0 without one).
pub fn atof(s: &[u8]) -> f64 {
    let mut i = s.iter().position(|&c| !is_c_space(c)).unwrap_or(s.len());
    let start = i;
    if matches!(s.get(i), Some(b'-') | Some(b'+')) {
        i += 1;
    }
    let int_digits = s[i..].iter().take_while(|c| c.is_ascii_digit()).count();
    i += int_digits;
    let mut frac_digits = 0;
    if s.get(i) == Some(&b'.') {
        frac_digits = s[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if int_digits + frac_digits > 0 {
            i += 1 + frac_digits;
        }
    }
    if int_digits + frac_digits == 0 {
        return 0.0;
    }
    let mant_end = i;
    if matches!(s.get(i), Some(b'e') | Some(b'E')) {
        let mut j = i + 1;
        if matches!(s.get(j), Some(b'-') | Some(b'+')) {
            j += 1;
        }
        let e = s[j..].iter().take_while(|c| c.is_ascii_digit()).count();
        if e > 0 {
            i = j + e;
        }
    }
    let text = std::str::from_utf8(&s[start..i]).unwrap_or("0");
    // (Rust's reading of a decimal is correctly rounded, as glibc's `strtod`; a trailing point ("1.") is read by
    // appending a zero digit, and a huge exponent saturates to infinity or zero, as `strtod` does)
    let fixed;
    let text = if s[mant_end - 1] == b'.' {
        fixed = format!("{}0{}", &text[..mant_end - start], &text[mant_end - start..]);
        fixed.as_str()
    } else {
        text
    };
    text.parse::<f64>().unwrap_or_else(|_| {
        // an exponent with more digits than `parse` takes: its sign decides
        let neg = text.starts_with('-');
        let tiny = text.to_ascii_lowercase().contains("e-");
        match (tiny, neg) {
            (true, false) => 0.0,
            (true, true) => -0.0,
            (false, false) => f64::INFINITY,
            (false, true) => f64::NEG_INFINITY,
        }
    })
}

/// `%g` of `v` (six significant digits, the exponent at least `exp_digits` digits: 3 for MSVC's C library, 2 for
/// glibc's).
pub fn format_g(v: f64, exp_digits: usize) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan".into() } else { "nan".into() };
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf".into() } else { "inf".into() };
    }
    let neg = v.is_sign_negative();
    let a = if neg { -v } else { v };
    let sign = if neg { "-" } else { "" };
    if a == 0.0 {
        return format!("{sign}0");
    }
    // the exact decimal expansion (a double has at most 767 significant digits; 800 keeps every one), then rounded to
    // six digits, half to even
    let exact = format!("{a:.800e}");
    let (mant, exp) = exact.split_once('e').expect("e");
    let mut exp: i32 = exp.parse().expect("exponent");
    let digits: Vec<u8> = mant.bytes().filter(|c| c.is_ascii_digit()).map(|c| c - b'0').collect();
    let mut d: Vec<u8> = digits[..6].to_vec();
    let rest = &digits[6..];
    let up = match rest.first() {
        Some(&r) if r > 5 => true,
        Some(&5) => rest[1..].iter().any(|&x| x != 0) || d[5] % 2 == 1,
        _ => false,
    };
    if up {
        let mut k = 5;
        loop {
            if d[k] == 9 {
                d[k] = 0;
                if k == 0 {
                    d.insert(0, 1);
                    d.pop();
                    exp += 1;
                    break;
                }
                k -= 1;
            } else {
                d[k] += 1;
                break;
            }
        }
    }
    let ds: String = d.iter().map(|&x| char::from(b'0' + x)).collect();
    if !(-4..6).contains(&exp) {
        let mut m = format!("{}.{}", &ds[..1], &ds[1..]);
        while m.ends_with('0') {
            m.pop();
        }
        if m.ends_with('.') {
            m.pop();
        }
        let es = if exp < 0 { '-' } else { '+' };
        format!("{sign}{m}e{es}{:0width$}", exp.unsigned_abs(), width = exp_digits)
    } else {
        // fixed: 6 - 1 - exp digits after the point
        let s = if exp >= 0 {
            let p = (exp + 1) as usize;
            format!("{}.{}", &ds[..p], &ds[p..])
        } else {
            format!("0.{}{}", "0".repeat((-exp - 1) as usize), ds)
        };
        let mut s = s;
        if s.contains('.') {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
        format!("{sign}{s}")
    }
}

/// `intAsStr(val)`: `%d`.
pub fn int_as_str(val: i32) -> Vec<u8> {
    format!("{val}").into_bytes()
}

/// `boolAsStr(val)`: `%d` of the `Bool` (0 or 1).
pub fn bool_as_str(val: bool) -> Vec<u8> {
    int_as_str(i32::from(val))
}

/// `realAsStr(val)`: `%g` of the `Real` (promoted to `double`), as MSVC's C library prints it.
pub fn real_as_str(val: f32) -> Vec<u8> {
    format_g(f64::from(val), 3).into_bytes()
}

impl UserPreferences {
    /// `UserPreferences()`: an empty map, no file name.
    pub fn new() -> Self {
        Self::default()
    }

    /// `load(fname)`: the file name is the user data path and `fname`; the file read in text mode
    /// ([`crt_text_read`]) line by line ([`UserPreferences::read_lines`]). Whether it opened.
    pub fn load<F: UserDataFiles + ?Sized>(&mut self, fs: &mut F, fname: &[u8]) -> bool {
        let mut name = fs.path_user_data();
        name.extend_from_slice(fname);
        self.filename = name;
        match fs.user_data_read(&self.filename.clone()) {
            Some(raw) => {
                self.read_lines(&crt_text_read(&raw));
                true
            }
            None => false,
        }
    }

    /// `load`'s loop over the bytes `fgets` reads (after the C library's text mode).
    pub fn read_lines(&mut self, text: &[u8]) {
        let mut pos = 0;
        while let Some(buf) = fgets(text, &mut pos) {
            // AsciiString line = buf; (up to the first NUL)
            let cstr = buf.iter().position(|&c| c == 0).map_or(buf, |n| &buf[..n]);
            let mut line = AsciiString::try_from_bytes(cstr).expect("a line of at most 2,047 bytes");
            trim(&mut line, &mut self.undefined);
            let mut key = AsciiString::new();
            // line.nextToken(&key, "=") (its answer is not read)
            let _ = line.next_token(&mut key, Some(b"="));
            // val = line.str() + 1;
            let rest = line.str();
            let mut val = if rest.is_empty() {
                self.undefined.push(PrefsUndefined::ReadPastEmptyLine(key.str().to_vec()));
                AsciiString::new()
            } else {
                AsciiString::try_from_bytes(&rest[1..]).expect("shorter than the line")
            };
            trim(&mut key, &mut self.undefined);
            if !plant(8942) {
                trim(&mut val, &mut self.undefined);
            }
            if key.is_empty() || val.is_empty() {
                continue;
            }
            if plant(8941) && self.map.contains_key(key.str()) {
                continue;
            }
            self.map.insert(key.str().to_vec(), val.str().to_vec());
        }
    }

    /// `write()`: nothing without a file name; else the file written in text mode ([`crt_text_write`] of
    /// [`UserPreferences::write_text`]). Whether it opened.
    pub fn write<F: UserDataFiles + ?Sized>(&self, fs: &mut F) -> bool {
        if self.filename.is_empty() {
            return false;
        }
        fs.user_data_write(&self.filename, &crt_text_write(&self.write_text()))
    }

    /// What `write`'s `fprintf(fp, "%s = %s\n", ...)` prints, in the map's order.
    pub fn write_text(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (k, v) in &self.map {
            out.extend_from_slice(k);
            out.extend_from_slice(if plant(8943) { b"=" } else { b" = " });
            out.extend_from_slice(v);
            out.push(b'\n');
        }
        out
    }

    /// `getBool(key, defaultValue)`.
    pub fn get_bool(&mut self, key: &[u8], default: bool) -> bool {
        let v = self.get_ascii_string(key, b"");
        if v.is_empty() {
            return default;
        }
        let mut val = AsciiString::try_from_bytes(&v).unwrap_or_default();
        if plant(8944) {
            return [&b"1"[..], b"t", b"true", b"y", b"yes", b"ok"].contains(&v.as_slice());
        }
        if val.to_lower().is_err() {
            // (`tolower` of a byte from 0x80: undefined; the C locale leaves it, and no such value is one of the words)
            self.undefined.push(PrefsUndefined::NonAsciiCharClass);
            let lower = v.to_ascii_lowercase();
            return [&b"1"[..], b"t", b"true", b"y", b"yes", b"ok"].contains(&lower.as_slice());
        }
        let val = val.str();
        [&b"1"[..], b"t", b"true", b"y", b"yes", b"ok"].contains(&val)
    }

    /// `getReal(key, defaultValue)`: `(Real)atof` of a non-empty value.
    pub fn get_real(&self, key: &[u8], default: f32) -> f32 {
        let v = self.get_ascii_string(key, b"");
        if v.is_empty() {
            return default;
        }
        atof(&v) as f32
    }

    /// `getInt(key, defaultValue)`: `atoi` of a non-empty value.
    pub fn get_int(&mut self, key: &[u8], default: i32) -> i32 {
        let v = self.get_ascii_string(key, b"");
        if v.is_empty() {
            return default;
        }
        atoi(&v, &mut self.undefined)
    }

    /// `getAsciiString(key, defaultValue)`.
    pub fn get_ascii_string(&self, key: &[u8], default: &[u8]) -> Vec<u8> {
        self.map.get(key).cloned().unwrap_or_else(|| default.to_vec())
    }

    /// `setBool(key, val)`.
    pub fn set_bool(&mut self, key: &[u8], val: bool) {
        self.map.insert(key.to_vec(), bool_as_str(val));
    }

    /// `setReal(key, val)`.
    pub fn set_real(&mut self, key: &[u8], val: f32) {
        self.map.insert(key.to_vec(), real_as_str(val));
    }

    /// `setInt(key, val)`.
    pub fn set_int(&mut self, key: &[u8], val: i32) {
        self.map.insert(key.to_vec(), int_as_str(val));
    }

    /// `setAsciiString(key, val)`.
    pub fn set_ascii_string(&mut self, key: &[u8], val: &[u8]) {
        self.map.insert(key.to_vec(), val.to_vec());
    }

    /// `(*this)[key]` read: the value, inserting an empty one when the key is missing (`std::map::operator[]`).
    pub fn index(&mut self, key: &[u8]) -> Vec<u8> {
        self.map.entry(key.to_vec()).or_default().clone()
    }

    /// `find(key)`: the value, or `None` (`end()`).
    pub fn find(&self, key: &[u8]) -> Option<&[u8]> {
        self.map.get(key).map(|v| v.as_slice())
    }
}

/// A user data folder in memory (tests, and the shell's view of the client's folder): files by their full path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryUserData {
    /// `getPath_UserData()`.
    pub path: Vec<u8>,
    pub files: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl UserDataFiles for MemoryUserData {
    fn path_user_data(&self) -> Vec<u8> {
        self.path.clone()
    }
    fn user_data_read(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.files.get(path).cloned()
    }
    fn user_data_write(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        self.files.insert(path.to_vec(), bytes.to_vec());
        true
    }
}

/// The shell's host as the user data folder (`MenuHost`'s three file calls).
pub struct HostUserData<'a, H: ?Sized>(pub &'a mut H);

impl<H: MenuHost + ?Sized> UserDataFiles for HostUserData<'_, H> {
    fn path_user_data(&self) -> Vec<u8> {
        self.0.path_user_data()
    }
    fn user_data_read(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.0.user_data_read(path)
    }
    fn user_data_write(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        self.0.user_data_write(path, bytes)
    }
}

/// [`PrefsUndefined`] as the window manager records an undefined step.
pub fn undefined_text(u: &PrefsUndefined) -> &'static str {
    match u {
        PrefsUndefined::ReadPastEmptyLine(_) => "UserPreferences::load: val = line.str() + 1 past an empty line's terminator",
        PrefsUndefined::NonAsciiCharClass => "UserPreferences: isspace/tolower of a byte from 0x80",
        PrefsUndefined::IntOutOfRange => "UserPreferences: atoi of a value outside Int's range",
    }
}

/// A preference object constructed as upstream's subclasses construct theirs (`load(fname)` in the constructor), what
/// the read left undefined recorded on the window manager.
pub fn load_preferences<H: MenuHost + ?Sized>(wm: &mut GameWindowManager, h: &mut H, fname: &[u8]) -> UserPreferences {
    let mut p = UserPreferences::new();
    p.load(&mut HostUserData(h), fname);
    report_undefined(wm, &mut p);
    p
}

/// Moves `p`'s undefined steps to the window manager.
pub fn report_undefined(wm: &mut GameWindowManager, p: &mut UserPreferences) {
    for u in p.undefined.drain(..) {
        wm.set_undefined(undefined_text(&u));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &[u8]) -> UserPreferences {
        let mut p = UserPreferences::new();
        p.read_lines(text);
        p
    }

    /// #93: `load`'s line rules, predicted from UserPreferences.cpp and AsciiString.cpp before the run: blanks trimmed
    /// around the line, the key and the value; the last duplicate wins; `a==b` is `a` = `=b`; `=x=y` is `x` = `y`;
    /// empty keys and values skipped; a line without `=` read past its end (recorded); a NUL cuts the line.
    ///
    /// **Result (2026-11-10):** one miss, in the prediction: ` = v` also reads past its end (see the assertion).
    #[test]
    fn load_reads_lines_as_upstream() {
        let p = read(b"  Alpha = 1 \nBeta=two words\t\r\nalpha=3\nAlpha = 4\na==b\n=x=y\n = v\nk = \nnoequals\n\nn\0ul=1\nz=9");
        let m: Vec<(&[u8], &[u8])> = p.map.iter().map(|(k, v)| (k.as_slice(), v.as_slice())).collect();
        assert_eq!(m, vec![(&b"Alpha"[..], &b"4"[..]), (b"Beta", b"two words"), (b"a", b"=b"), (b"alpha", b"3"), (b"x", b"y"), (b"z", b"9")]);
        // `noequals`, the empty line and the NUL-cut `n` read past the terminator; `k = ` does not (a rest after its
        // key). Missed in the prediction: ` = v` does too (its leading `=` is skipped as a separator, so ` v` is the
        // key and nothing follows it)
        assert_eq!(
            p.undefined,
            vec![
                PrefsUndefined::ReadPastEmptyLine(b" v".to_vec()),
                PrefsUndefined::ReadPastEmptyLine(b"noequals".to_vec()),
                PrefsUndefined::ReadPastEmptyLine(Vec::new()),
                PrefsUndefined::ReadPastEmptyLine(b"n".to_vec())
            ]
        );
        // a line of 3,000 bytes is two `fgets`: 2,047 and the rest
        let mut long = b"k1=".to_vec();
        long.extend(std::iter::repeat_n(b'x', 2044));
        long.extend_from_slice(b"k2=yy\n");
        let p = read(&long);
        assert_eq!(p.map.get(&b"k1"[..]).map(|v| v.len()), Some(2044));
        assert_eq!(p.map.get(&b"k2"[..]).map(|v| v.as_slice()), Some(&b"yy"[..]));
    }

    /// #93: the round trip through the folder: written in the map's order as `key = value` lines in text mode (CR LF),
    /// read back the same; a file name only after `load`; a missing file keeps the map and answers false.
    #[test]
    fn preference_files_round_trip() {
        let mut fs = MemoryUserData { path: b"User\\".to_vec(), files: BTreeMap::new() };
        let mut p = UserPreferences::new();
        p.set_int(b"Zed", -5);
        assert!(!p.write(&mut fs), "no file name before load");
        assert!(!p.load(&mut fs, b"Options.ini"));
        assert_eq!(p.filename, b"User\\Options.ini");
        assert_eq!(p.map.len(), 1, "a missing file leaves the map");
        p.set_bool(b"Bool", true);
        p.set_real(b"Real", 0.25);
        p.set_ascii_string(b"Name", b"two words");
        assert!(p.write(&mut fs));
        assert_eq!(fs.files[&b"User\\Options.ini"[..]], b"Bool = 1\r\nName = two words\r\nReal = 0.25\r\nZed = -5\r\n");
        let mut q = UserPreferences::new();
        assert!(q.load(&mut fs, b"Options.ini"));
        assert_eq!(q.map, p.map);
        assert!(q.get_bool(b"Bool", false) && !q.get_bool(b"Name", true) && q.get_bool(b"None", true));
        assert_eq!((q.get_int(b"Zed", 0), q.get_real(b"Real", 0.0), q.get_int(b"None", 7)), (-5, 0.25, 7));
        // MSVC's text mode: CR LF is LF, Ctrl-Z ends the file
        assert_eq!(crt_text_read(b"a=1\r\nb=2\r\x1ac=3"), b"a=1\nb=2\r");
    }

    /// #93: the conversions, predicted: `%g` six digits with a three-digit exponent; `atoi` wraps out of range
    /// (recorded); `atof` stops at the first character that does not continue the number.
    #[test]
    fn conversions() {
        assert_eq!(format_g(1234565.0, 3), "1.23456e+006", "a tie rounds to even");
        assert_eq!(format_g(1234575.0, 2), "1.23458e+06");
        assert_eq!(format_g(0.0001, 3), "0.0001");
        assert_eq!(format_g(0.00001, 3), "1e-005");
        assert_eq!(format_g(100000.0, 3), "100000");
        assert_eq!(format_g(999999.5, 3), "1e+006");
        assert_eq!(format_g(-0.0, 3), "-0");
        assert_eq!(String::from_utf8(real_as_str(0.1)).unwrap(), "0.1");
        let mut u = Vec::new();
        assert_eq!(atoi(b"  -42xyz", &mut u), -42);
        assert!(u.is_empty());
        assert_eq!(atoi(b"4294967297", &mut u), 1);
        assert_eq!(u, vec![PrefsUndefined::IntOutOfRange]);
        assert_eq!((atof(b" 1.5e3x"), atof(b"-.5"), atof(b"7."), atof(b"e5"), atof(b"1e"), atof(b"1e999")), (1500.0, -0.5, 7.0, 0.0, 1.0, f64::INFINITY));
        assert_eq!((bool_as_str(true), int_as_str(i32::MIN)), (b"1".to_vec(), b"-2147483648".to_vec()));
    }
}
