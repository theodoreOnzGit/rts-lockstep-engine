//! PARTLY DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 random programs (test: tests/differential_draft_ini.rs); not yet
//! wired into the game. Covered: `Enumerate_Entries`, `Put_TextBlock`, `Get_TextBlock`, `Put_Int`, `Get_Int`, `Put_Rect`, `Get_Rect`, `Put_Hex`,
//! `Get_Hex`, `Get_Float`, `Put_Float`, `Put_Double`, `Get_Alloc_String`, `Get_List_Index`, `Get_Int_Bitfield`, `Get_Alloc_Int_Array`, `Put_Bool`,
//! `Get_Bool`, all `Put_Point`/`Get_Point` and the buffer `Get_String`. NOT covered: `Put_UUBlock`, `Get_UUBlock` (both forms), `Get_Wide_String`,
//! `Put_Wide_String` (streams and the wide string class), and `Get_Double` (upstream scans `%lf` into a 4-byte `float`: the C++ aborts with stack
//! smashing, no defined result exists). Corrected 2026-11-20 (#215): `Get_Float` and the float points rounded the text to `double` and then to
//! `float`; `sscanf("%f")` rounds once (`1.00000005960464477539062501` differs by one ulp).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/ini.cpp (the `INIClass` members below)
//!
//! The verified [`crate::ww::ini::IniClass`] is read-only and has no typed getters or `Put_*` ("Not translated" in its
//! module docs). These are the remaining `INIClass` members whose names were found nowhere in the port:
//! `Enumerate_Entries`, `Put_UUBlock`/`Get_UUBlock` (both pairs), `Get_Wide_String`, `Put_Wide_String`,
//! `Put_TextBlock`, `Get_TextBlock`, `Put_Int`, `Put_Rect`/`Get_Rect`, `Put_Hex`/`Get_Hex`, `Put_Float`,
//! `Get_Double`/`Put_Double`, `Get_Alloc_String`, `Get_List_Index`, `Get_Int_Bitfield`, `Put_Bool`, the `Put_Point`
//! overloads. `Get_Alloc_Int_Array`, `Get_Float`, `Get_Bool`, `Get_Int` and `Get_Point` are included where a listed
//! function shares their helpers.
//!
//! Modelling. The database is reached through two traits (compile-time contracts, no `dyn`): [`IniRead`], which
//! [`crate::ww::ini::IniClass`] implements below with its public reads, and [`IniWrite`], which adds `Put_String` and
//! `Clear(section)`. **`IniClass` has no `Put_String`/`Clear`** (its fields are private and the verified file is not
//! edited here), so the write side has no implementor yet: a full `INIClass::Put_String` (ini.cpp:1650, with its
//! `DuplicateCRCError` on a case-different entry of equal CRC) must be added to the verified type, or these functions
//! re-pointed at a new one, before they can run.
//!
//! `Base64Straw`/`Base64Pipe`/`BufferStraw`/`BufferPipe` are replaced by the whole-buffer codec
//! [`crate::drafts::wwlib::base64`]: the pipe codes 4-character blocks and the straw 3-byte packets, so coding the
//! concatenation gives the stream's result (differing only if a line holds characters the decoder skips in the middle
//! of a block).
//!
//! C `sscanf`/`sprintf`/`atoi` are replaced by the small scanners at the end of the file. `%f` prints six decimals;
//! Rust prints `inf`/`NaN` where C prints `inf`/`nan` (the only difference).
//!
//! Upstream defects kept. `Get_Double` scans `%lf` into a `float` (8 bytes written over 4: undefined behaviour); the
//! intended value (the parsed double narrowed to `f32`, then widened) is what is returned. `Put_UUBlock(section, entry,
//! ..)` asks the straw for `len * 3 - 1` characters, which is fewer than the encoding for `len == 1` (2 of 4
//! characters). `Get_Int_Bitfield` and `Get_List_Index` have `assert(lp < 1000)` only. `Put_TextBlock` looks for the
//! break from `buffer[count]`, which at `count == 75` is the terminator, so the scan really starts at 74.

use crate::logic::modules::plant;
use crate::drafts::wwlib::base64::{base64_decode, base64_encode};
use crate::ww::ini::IniClass;
use crate::ww::trim::{c_str, strtrim};

/// `Rect` (`TRect<int>`, trect.h): `X, Y, Width, Height`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// `TPoint2D<int>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TPoint2DInt {
    pub x: i32,
    pub y: i32,
}

/// `TPoint3D<int>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TPoint3DInt {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// `TPoint2D<float>`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TPoint2DFloat {
    pub x: f32,
    pub y: f32,
}

/// `TPoint3D<float>`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TPoint3DFloat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// The reads these functions make of an `INIClass`.
pub trait IniRead {
    /// `Find_Entry(section, entry)->Value`: the stored value, `None` if the section or entry is absent.
    fn find_value(&self, section: &[u8], entry: &[u8]) -> Option<Vec<u8>>;
    /// `Entry_Count(section)`.
    fn entry_count(&self, section: &[u8]) -> i32;
    /// `Get_Entry(section, index)`.
    fn get_entry(&self, section: &[u8], index: i32) -> Option<Vec<u8>>;
}

/// The writes: `Put_String` and `Clear(section)`.
pub trait IniWrite: IniRead {
    /// `INIClass::Put_String(section, entry, string)` (an empty string removes the entry).
    fn put_string(&mut self, section: &[u8], entry: &[u8], string: &[u8]) -> bool;
    /// `INIClass::Clear(section)` with no entry: removes the whole section.
    fn clear_section(&mut self, section: &[u8]) -> bool;
}

impl IniRead for IniClass {
    fn find_value(&self, section: &[u8], entry: &[u8]) -> Option<Vec<u8>> {
        self.find_entry(section, entry).map(|e| e.value.clone())
    }
    fn entry_count(&self, section: &[u8]) -> i32 {
        IniClass::entry_count(self, section)
    }
    fn get_entry(&self, section: &[u8], index: i32) -> Option<Vec<u8>> {
        IniClass::get_entry(self, section, index).map(|e| e.to_vec())
    }
}

/// `INIClass::Get_String(section, entry, defvalue, buffer, size)` (ini.cpp:1700): `size < 2` gives nothing (0);
/// otherwise the entry's value or `defvalue`, cut to `size - 1` bytes (`strncpy(.., size)` then `buffer[size-1] = 0`),
/// then `strtrim`'d. Returns the string (its `strlen` is the upstream return).
pub fn get_string_sized<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: &[u8], size: usize) -> Vec<u8> {
    if size < 2 {
        return Vec::new();
    }
    let value = match ini.find_value(section, entry) {
        Some(v) => v,
        None => defvalue.to_vec(),
    };
    let value = c_str(&value);
    let cut = &value[..value.len().min(size - 1)];
    strtrim(cut)
}

/// `INIClass::Enumerate_Entries(Section, Entry_Prefix, StartNumber, EndNumber)` (ini.cpp:862): how many entries
/// `<prefix><n>` exist in a row from `StartNumber`, stopping below `EndNumber`.
pub fn enumerate_entries<T: IniRead>(ini: &T, section: &[u8], entry_prefix: &[u8], start_number: u32, end_number: u32) -> u32 {
    let mut count = start_number;
    loop {
        let mut entry = entry_prefix.to_vec();
        // sprintf(entry, "%s%d", Entry_Prefix, count): %d of an unsigned, so values above i32::MAX print negative.
        entry.extend_from_slice((count as i32).to_string().as_bytes());
        let present = ini.find_value(section, &entry).is_some();
        if present {
            count = count.wrapping_add(1);
        }
        if !(present && count < end_number) {
            break;
        }
    }
    count.wrapping_sub(start_number)
}

/// Standard base64 of `block` as `Base64Straw(ENCODE)` yields it (`Base64_Encode` over the whole block).
fn encode_all(block: &[u8]) -> Vec<u8> {
    let dlen = ((block.len() + 2) / 3) * 4 + 4;
    let mut dest = vec![0u8; dlen];
    let n = base64_encode(block, block.len() as i32, &mut dest, dlen as i32);
    dest.truncate(n.max(0) as usize);
    dest
}

/// Decode `source` as a `Base64Pipe(DECODE)` into a `BufferPipe(block, len)` would: at most `len` bytes.
fn decode_all(source: &[u8], len: usize) -> Vec<u8> {
    if source.is_empty() || len == 0 {
        return Vec::new();
    }
    let mut dest = vec![0u8; len];
    let n = base64_decode(source, source.len() as i32, &mut dest, len as i32);
    dest.truncate(n.max(0) as usize);
    dest
}

/// `INIClass::Put_UUBlock(section, block, len)` (ini.cpp:901): clears the section, then stores the base64 text in
/// 70-character entries named `1`, `2`, ...
pub fn put_uublock<T: IniWrite>(ini: &mut T, section: &[u8], block: &[u8], len: i32) -> bool {
    if section.is_empty() || len < 1 {
        return false;
    }
    ini.clear_section(section);
    let block = &block[..(len as usize).min(block.len())];
    let encoded = encode_all(block);
    let mut counter = 1;
    for chunk in encoded.chunks(70) {
        ini.put_string(section, counter.to_string().as_bytes(), chunk);
        counter += 1;
    }
    true
}

/// `INIClass::Get_UUBlock(section, block, len)` (ini.cpp:953): every entry of the section (each read with
/// `Get_String(.., "=", buffer[128])`, so at most 127 characters) decoded in order into at most `len` bytes.
pub fn get_uublock<T: IniRead>(ini: &T, section: &[u8], len: i32) -> Vec<u8> {
    if section.is_empty() || len < 0 {
        return Vec::new();
    }
    let counter = ini.entry_count(section);
    let mut text = Vec::new();
    for index in 0..counter {
        let name = ini.get_entry(section, index).unwrap_or_default();
        text.extend_from_slice(&get_string_sized(ini, section, &name, b"=", 128));
    }
    decode_all(&text, len as usize)
}

/// `INIClass::Put_UUBlock(section, entry, block, len)` (ini.cpp:1071): one entry; the straw is asked for
/// `len * 3 - 1` characters.
pub fn put_uublock_entry<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], block: &[u8], len: i32) -> bool {
    if section.is_empty() || len < 1 {
        return false;
    }
    let block = &block[..(len as usize).min(block.len())];
    let encoded = encode_all(block);
    let want = (len as usize) * 3 - 1;
    let cut = &encoded[..encoded.len().min(want)];
    ini.put_string(section, entry, cut);
    true
}

/// `INIClass::Get_UUBlock(section, entry, block, len)` (ini.cpp:1093): the entry (read with size `len * 3`, default
/// `"="`) decoded into at most `len` bytes.
pub fn get_uublock_entry<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], len: i32) -> Vec<u8> {
    if section.is_empty() || entry.is_empty() || len < 0 {
        return Vec::new();
    }
    let text = get_string_sized(ini, section, entry, b"=", (len as usize) * 3);
    decode_all(&text, len as usize)
}

/// `INIClass::Get_Wide_String(new_string, section, entry, defvalue)` (ini.cpp:996): the base64 of the UTF-16LE text
/// (terminator included) decoded into the 2048-byte `out[1024]`; `defvalue` when the entry is empty or absent. The
/// result is the wide C string (up to the first 0).
pub fn get_wide_string<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: &[u16]) -> Vec<u16> {
    let buffer = get_string_sized(ini, section, entry, b"", 1024);
    if buffer.is_empty() {
        return wide_c_str(defvalue);
    }
    let mut out = decode_all(&buffer, 2048);
    out.resize(2048, 0); // `out` is uninitialised past the decoded bytes upstream
    let mut s = Vec::new();
    for pair in out.chunks(2) {
        let w = u16::from_le_bytes([pair[0], pair[1]]);
        if w == 0 {
            break;
        }
        s.push(w);
    }
    s
}

/// `INIClass::Put_Wide_String(section, entry, string)` (ini.cpp:1035): empty stores `""`; otherwise the base64 of the
/// text with its 2-byte terminator.
pub fn put_wide_string<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], string: &[u16]) -> bool {
    if section.is_empty() || entry.is_empty() {
        return false;
    }
    let string = wide_c_str(string);
    if string.is_empty() {
        ini.put_string(section, entry, b"");
    } else {
        let mut bytes = Vec::with_capacity(string.len() * 2 + 2);
        for w in &string {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        bytes.extend_from_slice(&[0, 0]);
        let encoded = encode_all(&bytes);
        ini.put_string(section, entry, &encoded);
    }
    true
}

fn wide_c_str(s: &[u16]) -> Vec<u16> {
    s.iter().copied().take_while(|&w| w != 0).collect()
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `INIClass::Put_TextBlock(section, text)` (ini.cpp:1141): clears the section, then stores the text in numbered
/// entries of at most 75 characters broken at white space.
pub fn put_text_block<T: IniWrite>(ini: &mut T, section: &[u8], text: &[u8]) -> bool {
    if section.is_empty() {
        return false;
    }
    ini.clear_section(section);
    let mut text = c_str(text);
    let mut index = 1;
    while !text.is_empty() {
        // strncpy(buffer, text, 75); buffer[75] = '\0'
        let mut buffer: Vec<u8> = text[..text.len().min(75)].to_vec();
        let mut count = buffer.len();
        if count > 0 {
            if if plant(17422) { count > 75 } else { count >= 75 } {
                while count > 0 {
                    // buffer[count] is the terminator at count == 75
                    let c = if count < buffer.len() { buffer[count] } else { 0 };
                    if is_space(c) {
                        break;
                    }
                    count -= 1;
                }
                if count == 0 {
                    break;
                }
                buffer.truncate(count);
            }
            let trimmed = strtrim(&buffer);
            ini.put_string(section, index.to_string().as_bytes(), &trimmed);
            index += 1;
            text = &text[count..];
        } else {
            break;
        }
    }
    true
}

/// `INIClass::Get_TextBlock(section, buffer, len)` (ini.cpp:1213): the section's entries joined by single spaces,
/// each read with the room left; returns the text and upstream's count.
pub fn get_text_block<T: IniRead>(ini: &T, section: &[u8], len: i32) -> (Vec<u8>, i32) {
    let mut out = Vec::new();
    if len <= 0 || len <= 1 {
        return (out, 0);
    }
    let mut len = len;
    let elen = ini.entry_count(section);
    let mut total = 0;
    for index in 0..elen {
        if index > 0 {
            out.push(b' ');
            len -= 1;
            total += 1;
        }
        let name = ini.get_entry(section, index).unwrap_or_default();
        let s = get_string_sized(ini, section, &name, b"", len.max(0) as usize);
        let partial = s.len() as i32;
        out.extend_from_slice(&s);
        total += partial;
        len -= partial;
        if if plant(17419) { len < 1 } else { len <= 1 } {
            break;
        }
    }
    (out, total)
}

/// `INIClass::Put_Int(section, entry, number, format)` (ini.cpp:1272): format 1 is `%Xh`, 2 is `$%X`, anything
/// else `%d`.
pub fn put_int<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], number: i32, format: i32) -> bool {
    let buffer = match format {
        1 => if plant(17411) { format!("{:x}h", number as u32) } else { format!("{:X}h", number as u32) },
        2 => if plant(17412) { format!("{:X}", number as u32) } else { format!("${:X}", number as u32) },
        _ => format!("{}", number),
    };
    ini.put_string(section, entry, buffer.as_bytes())
}

/// `INIClass::Get_Int(section, entry, defvalue)`: `$hex`, `hexh`, or decimal (`atoi`).
pub fn get_int<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: i32) -> i32 {
    if section.is_empty() || entry.is_empty() {
        return defvalue;
    }
    match ini.find_value(section, entry) {
        Some(v) => {
            let v = c_str(&v);
            if v.first() == Some(&b'$') {
                scan_hex(&v[1..]).map(|(n, _)| n).unwrap_or(defvalue)
            } else if v.last().map(|c| if plant(17413) { *c } else { c.to_ascii_lowercase() }) == Some(b'h') {
                // sscanf("%xh")
                match scan_hex(v) {
                    Some((n, rest)) if rest.first() == Some(&b'h') => n,
                    Some((n, _)) => n,
                    None => defvalue,
                }
            } else {
                atoi(v)
            }
        }
        None => defvalue,
    }
}

/// `INIClass::Put_Rect(section, entry, value)` (ini.cpp:1359): `"%d,%d,%d,%d"`.
pub fn put_rect<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], value: Rect) -> bool {
    let buffer = if plant(17414) { format!("{},{},{},{}", value.x, value.y, value.height, value.width) } else { format!("{},{},{},{}", value.x, value.y, value.width, value.height) };
    ini.put_string(section, entry, buffer.as_bytes())
}

/// `INIClass::Get_Rect(section, entry, defvalue)` (ini.cpp:1389): `Get_String(.., "0,0,0,0", buffer[64])`; when that
/// is non-empty, `sscanf` fills as many of the four numbers as parse over a copy of `defvalue`.
pub fn get_rect<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: Rect) -> Rect {
    let buffer = get_string_sized(ini, section, entry, b"0,0,0,0", 64);
    if !buffer.is_empty() {
        let mut retval = defvalue;
        let nums = scan_ints_comma(&buffer, 4);
        if let Some(&n) = nums.first() {
            retval.x = n;
        }
        if let Some(&n) = nums.get(1) {
            retval.y = n;
        }
        if let Some(&n) = nums.get(2) {
            retval.width = if plant(17415) { retval.width } else { n };
        }
        if let Some(&n) = nums.get(3) {
            retval.height = n;
        }
        return retval;
    }
    defvalue
}

/// `INIClass::Put_Hex(section, entry, number)` (ini.cpp:1422): `"%X"`.
pub fn put_hex<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], number: i32) -> bool {
    let buffer = if plant(17416) { format!("{:x}", number as u32) } else { format!("{:X}", number as u32) };
    ini.put_string(section, entry, buffer.as_bytes())
}

/// `INIClass::Get_Hex(section, entry, defvalue)` (ini.cpp:1452): `sscanf("%x")` over the value.
pub fn get_hex<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: i32) -> i32 {
    if section.is_empty() || entry.is_empty() {
        return defvalue;
    }
    match ini.find_value(section, entry) {
        Some(v) => scan_hex(c_str(&v)).map(|(n, _)| n).unwrap_or(defvalue),
        None => defvalue,
    }
}

/// `INIClass::Get_Float(section, entry, defvalue)`: `%f`, divided by 100 when the value holds a `%`.
pub fn get_float<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: f32) -> f32 {
    if section.is_empty() || entry.is_empty() {
        return defvalue;
    }
    match ini.find_value(section, entry) {
        Some(v) => {
            let v = c_str(&v);
            let mut d = defvalue;
            if let Some((x64, x32, _)) = scan_float(v) {
                d = if plant(17421) { x64 as f32 } else { x32 };
            }
            if v.contains(&b'%') && !plant(17418) {
                d /= 100.0f32;
            }
            d
        }
        None => defvalue,
    }
}

/// `INIClass::Put_Float(section, entry, number)` (ini.cpp:1525): `"%f"`.
pub fn put_float<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], number: f32) -> bool {
    let buffer = format!("{:.6}", number);
    ini.put_string(section, entry, buffer.as_bytes())
}

/// `INIClass::Get_Double(section, entry, defvalue)` (ini.cpp:1553). See the module docs: upstream scans `%lf` into a
/// `float`; the parsed double narrowed to `f32` is used, and `/ 100.0f` when the value holds a `%`.
pub fn get_double<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: f64) -> f64 {
    if section.is_empty() || entry.is_empty() {
        return defvalue;
    }
    match ini.find_value(section, entry) {
        Some(v) => {
            let v = c_str(&v);
            let mut val = defvalue as f32;
            if let Some((x, _, _)) = scan_float(v) {
                val = x as f32;
            }
            let mut d = val as f64;
            if v.contains(&b'%') {
                d /= 100.0f32 as f64;
            }
            d
        }
        None => defvalue,
    }
}

/// `INIClass::Put_Double(section, entry, number)` (ini.cpp:1592): `"%lf"`.
pub fn put_double<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], number: f64) -> bool {
    let buffer = format!("{:.6}", number);
    ini.put_string(section, entry, buffer.as_bytes())
}

/// `INIClass::Get_Alloc_String(section, entry, defvalue)` (ini.cpp:1789): a copy of the entry's value (untrimmed, as
/// stored) or of `defvalue`; `None` for a missing name or a missing value and default. Note an *entry that exists* but
/// has no value gives `defvalue = Value = NULL`, which `Find_Entry` cannot produce here.
pub fn get_alloc_string<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: Option<&[u8]>) -> Option<Vec<u8>> {
    if section.is_empty() || entry.is_empty() {
        return None;
    }
    match ini.find_value(section, entry) {
        Some(v) => Some(v),
        None => defvalue.map(|d| d.to_vec()),
    }
}

/// `INIClass::Get_List_Index(section, entry, defvalue, list)` (ini.cpp:1806): the index of the entry's value in the
/// NULL-terminated `list` (case-insensitive), else `defvalue`.
pub fn get_list_index<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: i32, list: &[&[u8]]) -> i32 {
    if section.is_empty() || entry.is_empty() {
        return 0;
    }
    let value = match ini.find_value(section, entry) {
        Some(v) => v,
        None => return defvalue,
    };
    let value = c_str(&value);
    for (lp, item) in list.iter().enumerate() {
        if value.eq_ignore_ascii_case(item) {
            return lp as i32;
        }
    }
    defvalue
}

/// `INIClass::Get_Int_Bitfield(section, entry, defvalue, list)` (ini.cpp:1823): the value split at `|` and `+`
/// (`strtok`, empty tokens skipped); each token found in `list` (case-insensitive) sets bit `1 << index`.
pub fn get_int_bitfield<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: i32, list: &[&[u8]]) -> i32 {
    let value = match ini.find_value(section, entry) {
        Some(v) => v,
        None => return defvalue,
    };
    let value = c_str(&value);
    let mut retval = 0i32;
    for token in value.split(|&c| c == b'|' || c == b'+').filter(|t| !t.is_empty()) {
        for (lp, item) in list.iter().enumerate() {
            if token.eq_ignore_ascii_case(item) {
                retval |= 1i32.wrapping_shl(lp as u32);
                break;
            }
        }
        // upstream: if the list ended, `assert(lp < 1000)` (an unidentified value; nothing else happens)
    }
    retval
}

/// `INIClass::Get_Alloc_Int_Array(section, entry, listend)`: space-separated `atoi` tokens, then `listend`.
pub fn get_alloc_int_array<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], listend: i32) -> Vec<i32> {
    let mut retval = Vec::new();
    if let Some(v) = ini.find_value(section, entry) {
        for token in c_str(&v).split(|&c| c == b' ').filter(|t| plant(17420) || !t.is_empty()) {
            retval.push(atoi(token));
        }
    }
    retval.push(listend);
    retval
}

/// `INIClass::Put_Bool(section, entry, value)` (ini.cpp:1915): `"yes"` or `"no"`.
pub fn put_bool<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], value: bool) -> bool {
    ini.put_string(section, entry, if value { b"yes" } else { b"no" })
}

/// `INIClass::Get_Bool(section, entry, defvalue)`: first character `Y T 1` true, `N F 0` false, else `defvalue`.
pub fn get_bool<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: bool) -> bool {
    if section.is_empty() || entry.is_empty() {
        return defvalue;
    }
    if let Some(v) = ini.find_value(section, entry) {
        match v.first().map(|c| c.to_ascii_uppercase()) {
            Some(b'Y') | Some(b'T') | Some(b'1') => return true,
            Some(b'N') | Some(b'F') | Some(b'0') => return plant(17417),
            _ => {}
        }
    }
    defvalue
}

/// `INIClass::Put_Point(section, entry, TPoint2D<int>)` (ini.cpp:1992): `"%d,%d"`.
pub fn put_point_2d_int<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], value: TPoint2DInt) -> bool {
    let buffer = format!("{},{}", value.x, value.y);
    ini.put_string(section, entry, buffer.as_bytes())
}

/// `INIClass::Put_Point(section, entry, TPoint3D<int>)` (ini.cpp:2051): `"%d,%d,%d"`.
pub fn put_point_3d_int<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], value: TPoint3DInt) -> bool {
    let buffer = format!("{},{},{}", value.x, value.y, value.z);
    ini.put_string(section, entry, buffer.as_bytes())
}

/// `INIClass::Put_Point(section, entry, TPoint3D<float>)` (ini.cpp:2111): `"%f,%f,%f"`.
pub fn put_point_3d_float<T: IniWrite>(ini: &mut T, section: &[u8], entry: &[u8], value: TPoint3DFloat) -> bool {
    let buffer = if plant(17423) { format!("{:.5},{:.5},{:.5}", value.x, value.y, value.z) } else { format!("{:.6},{:.6},{:.6}", value.x, value.y, value.z) };
    ini.put_string(section, entry, buffer.as_bytes())
}

/// `INIClass::Get_Point(.., TPoint2D<int>)`: `sscanf("%d,%d")` (unparsed numbers are uninitialised upstream; 0 here).
pub fn get_point_2d_int<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: TPoint2DInt) -> TPoint2DInt {
    let buffer = get_string_sized(ini, section, entry, b"", 64);
    if !buffer.is_empty() {
        let n = scan_ints_comma(&buffer, 2);
        return TPoint2DInt { x: n.first().copied().unwrap_or(0), y: n.get(1).copied().unwrap_or(0) };
    }
    defvalue
}

/// `INIClass::Get_Point(.., TPoint3D<int>)`: `sscanf("%d,%d,%d")`.
pub fn get_point_3d_int<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: TPoint3DInt) -> TPoint3DInt {
    let buffer = get_string_sized(ini, section, entry, b"", 64);
    if !buffer.is_empty() {
        let n = scan_ints_comma(&buffer, 3);
        return TPoint3DInt {
            x: n.first().copied().unwrap_or(0),
            y: n.get(1).copied().unwrap_or(0),
            z: n.get(2).copied().unwrap_or(0),
        };
    }
    defvalue
}

/// `INIClass::Get_Point(.., TPoint3D<float>)`: `sscanf("%f,%f,%f")`.
pub fn get_point_3d_float<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: TPoint3DFloat) -> TPoint3DFloat {
    let buffer = get_string_sized(ini, section, entry, b"", 64);
    if !buffer.is_empty() {
        let n = scan_floats_comma(&buffer, 3);
        return TPoint3DFloat {
            x: n.first().copied().unwrap_or(0.0),
            y: n.get(1).copied().unwrap_or(0.0),
            z: n.get(2).copied().unwrap_or(0.0),
        };
    }
    defvalue
}

/// `INIClass::Get_Point(.., TPoint2D<float>)`: `sscanf("%f,%f")`.
pub fn get_point_2d_float<T: IniRead>(ini: &T, section: &[u8], entry: &[u8], defvalue: TPoint2DFloat) -> TPoint2DFloat {
    let buffer = get_string_sized(ini, section, entry, b"", 64);
    if !buffer.is_empty() {
        let n = scan_floats_comma(&buffer, 2);
        return TPoint2DFloat { x: n.first().copied().unwrap_or(0.0), y: n.get(1).copied().unwrap_or(0.0) };
    }
    defvalue
}

// ---- C library stand-ins -------------------------------------------------------------------------------------

fn skip_ws(s: &[u8]) -> &[u8] {
    let n = s.iter().take_while(|&&c| is_space(c)).count();
    &s[n..]
}

/// `atoi`: white space, sign, digits; wraps like a 32-bit accumulation.
fn atoi(s: &[u8]) -> i32 {
    scan_int(s).map(|(n, _)| n).unwrap_or(0)
}

/// `%d`: returns the number and the rest.
fn scan_int(s: &[u8]) -> Option<(i32, &[u8])> {
    let s = skip_ws(s);
    let (neg, s) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let digits = s.iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let mut n: i32 = 0;
    for &c in &s[..digits] {
        n = n.wrapping_mul(10).wrapping_add((c - b'0') as i32);
    }
    Some((if neg { n.wrapping_neg() } else { n }, &s[digits..]))
}

/// `%x`: optional sign and `0x`, hex digits.
fn scan_hex(s: &[u8]) -> Option<(i32, &[u8])> {
    let s = skip_ws(s);
    let (neg, s) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let s = if s.len() > 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') && s[2].is_ascii_hexdigit() { &s[2..] } else { s };
    let digits = s.iter().take_while(|c| c.is_ascii_hexdigit()).count();
    if digits == 0 {
        return None;
    }
    let mut n: u32 = 0;
    for &c in &s[..digits] {
        n = n.wrapping_mul(16).wrapping_add((c as char).to_digit(16).unwrap());
    }
    let n = n as i32;
    Some((if neg { n.wrapping_neg() } else { n }, &s[digits..]))
}

/// `%f`/`%lf`: the longest prefix that is a decimal float (sign, digits, `.`, exponent).
/// `sscanf("%f")` / `("%lf")` of the leading number: the value rounded once to `f64`, the value rounded once to `f32` (`%f` stores a `float`
/// directly: narrowing the `double` would round twice and differ for a decimal text just above a halfway point), and the rest.
fn scan_float(s: &[u8]) -> Option<(f64, f32, &[u8])> {
    let s = skip_ws(s);
    let mut i = 0;
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        i += 1;
    }
    let ds = i;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    let mut had = i > ds;
    if i < s.len() && s[i] == b'.' {
        i += 1;
        let fs = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        had = had || i > fs;
    }
    if !had {
        return None;
    }
    if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
        let mut j = i + 1;
        if j < s.len() && (s[j] == b'-' || s[j] == b'+') {
            j += 1;
        }
        let es = j;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        if j > es {
            i = j;
        }
    }
    let text = std::str::from_utf8(&s[..i]).ok()?;
    let v: f64 = text.parse().ok()?;
    let v32: f32 = text.parse().ok()?;
    Some((v, v32, &s[i..]))
}

/// `sscanf(s, "%d,%d,...")` for up to `n` numbers: stops at the first failure (or missing comma).
fn scan_ints_comma(s: &[u8], n: usize) -> Vec<i32> {
    let mut out = Vec::new();
    let mut rest = s;
    for k in 0..n {
        if k > 0 {
            if rest.first() != Some(&b',') {
                break;
            }
            rest = &rest[1..];
        }
        match scan_int(rest) {
            Some((v, r)) => {
                out.push(v);
                rest = r;
            }
            None => break,
        }
    }
    out
}

/// `sscanf(s, "%f,%f,...")` for up to `n` numbers.
fn scan_floats_comma(s: &[u8], n: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let mut rest = s;
    for k in 0..n {
        if k > 0 {
            if rest.first() != Some(&b',') {
                break;
            }
            rest = &rest[1..];
        }
        match scan_float(rest) {
            Some((_, v, r)) => {
                out.push(v);
                rest = r;
            }
            None => break,
        }
    }
    out
}
