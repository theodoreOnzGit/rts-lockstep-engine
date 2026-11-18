//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/widestring.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/widestring.cpp
//!
//! `WideStringClass`: the UNICODE (double-byte) version of `StringClass`. The previous pass replaced the string
//! classes by `String`; this is the class itself over `Vec<u16>` (the UTF-16 code units, no stored terminator),
//! for callers that need its exact behaviour: the 256-character temporary-string pool (`Get_String(length, is_temp)`
//! and the four `m_TempString` buffers), the shared empty string and the header (`allocated_length`, `length`) are the
//! allocator's business and are not translated. Upstream notes kept: comparisons against a NULL string return -1 /
//! false / true as written (`operator >` and `>=` are TRUE for NULL, `<` and `<=` false); `operator+=(WCHAR ch)` only
//! stores the new length when `ch != 0`; `Format` formats into a 512 code unit buffer (`_vsnwprintf`, so a longer
//! result is cut at 512 units and the call returns -1); `Convert_From(char*)` is `MultiByteToWideChar(CP_ACP, ...)` (the
//! Windows-1252 code page here); `Convert_To(StringClass&)` is `StringClass::Copy_Wide`.

/// The Windows-1252 mapping of bytes 0x80..0x9F (the rest of 0x00..0xFF is Latin-1); undefined bytes map to themselves.
const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161,
    0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
];

/// `class WideStringClass`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WideStringClass {
    buffer: Vec<u16>,
}

/// `wcscmp`: the difference of the first unequal code units (as signed), then of the lengths.
fn wcscmp(a: &[u16], b: &[u16]) -> i32 {
    let n = a.len().min(b.len());
    for i in 0..n {
        if a[i] != b[i] {
            return a[i] as i32 - b[i] as i32;
        }
    }
    a.len() as i32 - b.len() as i32
}

/// `towlower` for the Basic Multilingual Plane (single code unit lower-case where one exists).
fn towlower(c: u16) -> u16 {
    match char::from_u32(c as u32) {
        Some(ch) => {
            let mut it = ch.to_lowercase();
            match (it.next(), it.next()) {
                (Some(l), None) if (l as u32) <= 0xFFFF => l as u32 as u16,
                _ => c,
            }
        }
        None => c,
    }
}

impl WideStringClass {
    /// `WideStringClass(initial_len, hint_temporary)`: an empty string.
    pub fn new() -> Self {
        WideStringClass { buffer: Vec::new() }
    }

    /// `WideStringClass(const WCHAR * string)`: `None` is NULL (empty).
    pub fn from_wide(string: Option<&[u16]>) -> Self {
        let mut s = WideStringClass::new();
        s.assign_wide(string);
        s
    }

    /// `WideStringClass(WCHAR ch)`.
    pub fn from_char(ch: u16) -> Self {
        let mut s = WideStringClass::new();
        s.assign_char(ch);
        s
    }

    /// `WideStringClass(const char * string)`: `Convert_From`.
    pub fn from_ansi(string: Option<&[u8]>) -> Self {
        let mut s = WideStringClass::new();
        s.convert_from(string);
        s
    }

    /// `operator const WCHAR *`: the code units (without terminator).
    pub fn as_wide(&self) -> &[u16] {
        &self.buffer
    }

    /// `Get_Length()`.
    pub fn get_length(&self) -> i32 {
        self.buffer.len() as i32
    }

    /// `Is_Empty()`.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// `operator= (const WCHAR *)`: unchanged for NULL. A NUL inside `string` ends it (`wcslen`).
    pub fn assign_wide(&mut self, string: Option<&[u16]>) {
        if let Some(string) = string {
            let len = string.iter().position(|&c| c == 0).unwrap_or(string.len());
            self.buffer = string[..len].to_vec();
        }
    }

    /// `operator= (WCHAR ch)`: a one character string (an empty-looking string when `ch` is 0, but with length 1 stored).
    pub fn assign_char(&mut self, ch: u16) {
        self.buffer = vec![ch];
    }

    /// `operator+= (const WCHAR *)`.
    pub fn append_wide(&mut self, string: Option<&[u16]>) {
        if let Some(string) = string {
            let src_len = string.iter().position(|&c| c == 0).unwrap_or(string.len());
            self.buffer.extend_from_slice(&string[..src_len]);
        }
    }

    /// `operator+= (WCHAR ch)`: "if (ch != m_NullChar) Store_Length (cur_len + 1)": appending a NUL changes nothing.
    pub fn append_char(&mut self, ch: u16) {
        if ch != 0 {
            self.buffer.push(ch);
        }
    }

    /// `operator+ (a, b)`.
    pub fn concat(string1: &WideStringClass, string2: &WideStringClass) -> WideStringClass {
        let mut r = string1.clone();
        r.append_wide(Some(&string2.buffer));
        r
    }

    /// `Compare(string)`: `wcscmp`, -1 for NULL.
    pub fn compare(&self, string: Option<&[u16]>) -> i32 {
        match string {
            Some(s) => wcscmp(&self.buffer, s),
            None => -1,
        }
    }

    /// `Compare_No_Case(string)`: `_wcsicmp`, -1 for NULL.
    pub fn compare_no_case(&self, string: Option<&[u16]>) -> i32 {
        match string {
            Some(s) => {
                let a: Vec<u16> = self.buffer.iter().map(|&c| towlower(c)).collect();
                let b: Vec<u16> = s.iter().map(|&c| towlower(c)).collect();
                wcscmp(&a, &b)
            }
            None => -1,
        }
    }

    /// `operator== (const WCHAR *)`.
    pub fn eq_wide(&self, rvalue: Option<&[u16]>) -> bool {
        self.compare(rvalue) == 0
    }

    /// `operator!= (const WCHAR *)`.
    pub fn ne_wide(&self, rvalue: Option<&[u16]>) -> bool {
        self.compare(rvalue) != 0
    }

    /// `operator < (const WCHAR *)`: false for NULL.
    pub fn lt_wide(&self, string: Option<&[u16]>) -> bool {
        string.map_or(false, |s| wcscmp(&self.buffer, s) < 0)
    }

    /// `operator <= (const WCHAR *)`: false for NULL.
    pub fn le_wide(&self, string: Option<&[u16]>) -> bool {
        string.map_or(false, |s| wcscmp(&self.buffer, s) <= 0)
    }

    /// `operator > (const WCHAR *)`: TRUE for NULL.
    pub fn gt_wide(&self, string: Option<&[u16]>) -> bool {
        string.map_or(true, |s| wcscmp(&self.buffer, s) > 0)
    }

    /// `operator >= (const WCHAR *)`: TRUE for NULL.
    pub fn ge_wide(&self, string: Option<&[u16]>) -> bool {
        string.map_or(true, |s| wcscmp(&self.buffer, s) >= 0)
    }

    /// `operator[] (index)`: asserts `0 <= index < Get_Length()`.
    pub fn at(&self, index: i32) -> u16 {
        assert!(index >= 0 && (index as usize) < self.buffer.len());
        self.buffer[index as usize]
    }

    /// `operator[] (index)` for assignment.
    pub fn at_mut(&mut self, index: i32) -> &mut u16 {
        assert!(index >= 0 && (index as usize) < self.buffer.len());
        &mut self.buffer[index as usize]
    }

    /// `Erase(start_index, char_count)`: removes up to `char_count` code units (clipped to the end) from `start_index`;
    /// nothing if `start_index >= length`. (A negative `start_index` or `char_count` indexes out of range upstream.)
    pub fn erase(&mut self, start_index: i32, char_count: i32) {
        let len = self.get_length();
        if start_index < len {
            let mut char_count = char_count;
            if start_index + char_count > len {
                char_count = len - start_index;
            }
            let s = start_index as usize;
            let e = (start_index + char_count) as usize;
            self.buffer.drain(s..e);
        }
    }

    /// `Trim()`: `wcstrim`, "Trim leading and trailing whitespace (chars <= 32)".
    pub fn trim(&mut self) {
        let start = self.buffer.iter().position(|&c| c > 32).unwrap_or(self.buffer.len());
        let end = self.buffer.iter().rposition(|&c| c > 32).map_or(start, |p| p + 1);
        self.buffer = self.buffer[start..end].to_vec();
    }

    /// `Format(format, ...)` / `Format_Args`: the caller formats; this stores the result as `_vsnwprintf(temp, 512, ..)`
    /// would: at most 512 code units, returning the count or -1 when the text did not fit. `None` is a NULL format
    /// (returns 0, string unchanged).
    pub fn format(&mut self, formatted: Option<&[u16]>) -> i32 {
        let Some(f) = formatted else {
            return 0;
        };
        if f.len() > 512 {
            self.assign_wide(Some(&f[..512]));
            -1
        } else {
            self.assign_wide(Some(f));
            f.len() as i32
        }
    }

    /// `Convert_From(const char * text)` (widestring.cpp:321): `MultiByteToWideChar(CP_ACP, ..)`; false for NULL or an
    /// empty conversion (the result length includes the terminator, so `length > 0` for any non-NULL text).
    pub fn convert_from(&mut self, text: Option<&[u8]>) -> bool {
        if let Some(text) = text {
            let n = text.iter().position(|&c| c == 0).unwrap_or(text.len());
            self.buffer = text[..n]
                .iter()
                .map(|&b| if (0x80..=0x9F).contains(&b) { CP1252_HIGH[(b - 0x80) as usize] } else { b as u16 })
                .collect();
            return true;
        }
        false
    }

    /// `Convert_To(StringClass &)` (`StringClass::Copy_Wide`): the narrow string, '?' for characters outside Windows-1252.
    pub fn convert_to(&self, string: &mut Vec<u8>) -> bool {
        string.clear();
        for &c in self.buffer.iter() {
            let b = if c < 0x80 || (0xA0..=0xFF).contains(&c) {
                c as u8
            } else if let Some(i) = CP1252_HIGH.iter().position(|&h| h == c) {
                0x80 + i as u8
            } else {
                b'?'
            };
            string.push(b);
        }
        true
    }

    /// `Is_ANSI()`: "Check if the string is composed of ANSI range characters. (0-255)".
    pub fn is_ansi(&self) -> bool {
        self.buffer.iter().all(|&c| c <= 255)
    }

    /// `Get_Buffer(new_length)`: resizes to `new_length - 1` units (the length counts the terminator) and returns the
    /// buffer for writing (zero-filled where it grew).
    pub fn get_buffer(&mut self, new_length: i32) -> &mut [u16] {
        let n = if new_length > 0 { (new_length - 1) as usize } else { 0 };
        self.buffer.resize(n, 0);
        &mut self.buffer
    }

    /// `Peek_Buffer()`.
    pub fn peek_buffer(&mut self) -> &mut [u16] {
        &mut self.buffer
    }

    /// `Release_Resources()`: nothing (the temp string pool is not translated).
    pub fn release_resources() {}
}
