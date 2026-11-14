// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/ini.cpp, INI.H, inisup.h
//   (INIClass: the constructors, Load(FileClass &), Load(Straw &) without its merge branch, Is_Loaded,
//   Get_Filename, Get_Section_List, Find_Section, Section_Count, Entry_Count, Find_Entry, Get_Entry,
//   Get_String(StringClass &, ...), Strip_Comments, CRC, DuplicateCRCError, Keep_Blank_Entries;
//   INIEntry, INISection, INISection::Find_Entry)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/INDEX.H (IndexClass: Add_Index, Is_Present, Count,
//   operator [], Increase_Table_Size, the archive, Search_For_Node), BSEARCH.H (Binary_Search)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/readline.cpp (Read_Line(Straw &, char *, int, bool &))
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/crc.cpp, CRC.H (CRC::String, the CRC32 macro)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWLib: ini.cpp by Joe L. Bostic, later Eric Cosky, Andrew Jacobs)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! WWLib's INI database (`INIClass`, ini.cpp), as far as `AnimatedSoundMgrClass::Initialize` reads
//! `w3danimsound.ini` with it (GitHub #181 item 3). This is **not** the game's own INI parser (GameEngine's `INI`,
//! [`crate::common::ini`]): it is the Westwood library's, with sections, `entry=value` lines and `;` comments.
//!
//! # What `Load` does (ini.cpp, `Load(Straw &)`)
//!
//! - **Lines** come from `Read_Line` (readline.cpp): bytes up to `'\n'`, every `'\r'` dropped wherever it is, at most
//!   4,095 bytes kept (`MAX_LINE_LENGTH` 4096 with its terminator; the rest of a longer line is read and dropped), then
//!   `strtrim`'d ([`crate::ww::trim::strtrim`], with its `*source` quirk). The line is a C string: a NUL byte ends it.
//! - **Before the first section** every line is skipped. A section line starts with `[` and holds a `]` anywhere
//!   (checked before comments are stripped, so `[a;b]` is the section `a;b`); its name is the text between the `[`
//!   and the first `]`, trimmed. **If the end of the file comes during this scan, `Load` returns false**, even when the
//!   last line (with no newline after it) is a section line.
//! - **Entries**: the comment is cut at the first `;` (and the line trimmed again if there was one). A line that was
//!   empty *before* the cut, or starts with `;` or `=`, or has no `=`, is skipped. The text before the first `=`
//!   (trimmed) is the entry's name and must not be empty; the text after it (trimmed) is the value, and an empty value
//!   skips the line unless `Keep_Blank_Entries(true)` (then the value is `" "`; Zero Hour never calls it).
//! - **A section line on the file's last line, with no newline after it, is not read** (the loop ends at the end of
//!   the file before starting the section).
//! - **Duplicates are by CRC** (`CRC::String`, the CRC-32 of the name's bytes, case sensitive): an entry whose CRC is
//!   already in its section is dropped with `DuplicateCRCError` (a debug string and, in the release build on Windows, a
//!   `MessageBox`; here recorded, [`IniClass::duplicate_crc_errors`]), the first one kept. So two different names with
//!   one CRC are a "duplicate". **Sections are never merged**: a second `[A]` is a second section in the list, and
//!   both are in the section index under one ID.
//! - **A section with no entry is dropped.**
//!
//! # The index (INDEX.H), and the one thing that is the C library's
//!
//! Sections and entries are found by CRC through `IndexClass`, a table sorted by `qsort` when first searched and then
//! binary searched (BSEARCH.H's `Binary_Search`, translated as written), with a one-element cache (the "archive").
//! With two sections of one CRC (`[A]` twice), which of them `Find_Section` answers depends on **the order `qsort`
//! leaves equal IDs in**, which C does not define. Here the sort is stable (Rust's), which is what glibc 2.39's
//! `qsort` (a merge sort for these sizes) does and what the differential test compares against; MSVC's `qsort`, which
//! the shipped game used, is not stable, and its order is **not reproduced** (not available to compare).
//!
//! # Not translated
//!
//! The merge branch of `Load` (a second `Load` into a database that has sections: it calls `Put_String`, and copies
//! the section name into `char section[64]`), and every `Put_*`, `Save`, `Clear` of one section or entry, and the
//! typed getters (`Get_Int`, `Get_Bool`, ...): `Initialize` uses none of them. `Load` into a loaded database is
//! [`IniError::MergeNotTranslated`].

use std::cell::{Cell, RefCell};

use crate::logic::modules::plant;
use crate::ww::realcrc::crc_memory;
use crate::ww::trim::{c_str, strtrim};

/// An `INIClass` operation this translation does not carry (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IniError {
    /// `Load` into a database that already has sections (the merge branch).
    MergeNotTranslated,
}

/// `INIClass::MAX_LINE_LENGTH` (ini.cpp: 4096), the size of `Load`'s line buffer.
pub const MAX_LINE_LENGTH: usize = 4096;

/// `CRC::String(string)` (crc.cpp), as `INIClass::CRC`, `INIEntry::Index_ID` and `INISection::Index_ID` call it: the
/// CRC-32 (`CRC32` macro, table of poly 0x04C11DB7) of the C string's bytes, from 0, cast to `int`. The macro XORs the
/// sign-extended `char` and masks the low byte, so bytes >= 0x80 count as themselves.
pub fn crc(string: &[u8]) -> i32 {
    crc_memory(c_str(string), 0) as i32
}

/// [`crc`] as the index of `kind` uses it; `planted` names found without case (the differential test's plants).
fn crc_planted(string: &[u8], planted: bool) -> i32 {
    if planted {
        return crc_memory(&c_str(string).to_ascii_uppercase(), 0) as i32;
    }
    crc(string)
}

/// `IndexClass<int, T>` (INDEX.H): IDs mapped to data, sorted (`qsort`) when first searched after an addition, then
/// found by `Binary_Search`, with the last node found kept as the `Archive`. `T` here is a position in the owner's
/// list.
#[derive(Clone, Debug, Default)]
pub struct IndexClass<T> {
    /// `IndexTable[0 .. IndexCount]`: (ID, data).
    table: RefCell<Vec<(i32, T)>>,
    /// `IndexSize`: the table's capacity, grown by `Increase_Table_Size`.
    index_size: usize,
    /// `IsSorted`.
    is_sorted: Cell<bool>,
    /// `Archive`: the position of the node `Is_Present` found last.
    archive: Cell<Option<usize>>,
}

impl<T: Copy> IndexClass<T> {
    pub fn new() -> Self {
        IndexClass { table: RefCell::new(Vec::new()), index_size: 0, is_sorted: Cell::new(false), archive: Cell::new(None) }
    }

    /// `Count()`.
    pub fn count(&self) -> i32 {
        self.table.borrow().len() as i32
    }

    /// `Add_Index(id, data)`: appended (no duplicate check in a release build), the table no longer sorted. When the
    /// table is full it grows by its size (10 the first time), which invalidates the archive.
    pub fn add_index(&mut self, id: i32, data: T) {
        let count = self.table.borrow().len();
        if count + 1 > self.index_size {
            // Increase_Table_Size(IndexSize == 0 ? 10 : IndexSize)
            self.index_size += if self.index_size == 0 { 10 } else { self.index_size };
            self.archive.set(None);
        }
        self.table.borrow_mut().push((id, data));
        self.is_sorted.set(false);
    }

    /// `Is_Present(id)`: the archive if it holds `id`, else a search (which records what it finds as the archive).
    pub fn is_present(&self, id: i32) -> bool {
        if self.table.borrow().is_empty() {
            return false;
        }
        // Is_Archive_Same
        if let Some(a) = self.archive.get() {
            if self.table.borrow()[a].0 == id {
                return true;
            }
        }
        match self.search_for_node(id) {
            Some(p) => {
                self.archive.set(Some(p));
                true
            }
            None => false,
        }
    }

    /// `operator [] (id)`: the data of the node `Is_Present` leaves in the archive; `None` where upstream answers its
    /// default-constructed `static T x` (never read by `INIClass`, which asks `Is_Present` first).
    pub fn get(&self, id: i32) -> Option<T> {
        if self.is_present(id) {
            let a = self.archive.get()?;
            return Some(self.table.borrow()[a].1);
        }
        None
    }

    /// `Search_For_Node(id)`: the table sorted by ID if it is not (`qsort` with `search_compfunc`, the IDs compared as
    /// `int`; stable here, see the module docs), the archive invalidated, then `Binary_Search`.
    fn search_for_node(&self, id: i32) -> Option<usize> {
        let mut table = self.table.borrow_mut();
        if table.is_empty() {
            return None;
        }
        if !self.is_sorted.get() {
            if plant(10247) {
                // planted: equal IDs in reverse order
                table.reverse();
            }
            table.sort_by_key(|n| n.0);
            self.archive.set(None);
            self.is_sorted.set(true);
        }
        if plant(10248) {
            // planted: the first node of an equal run
            return table.iter().position(|n| n.0 == id);
        }
        binary_search(&table, id)
    }
}

/// `Binary_Search(A, n, target)` (BSEARCH.H), on the nodes' IDs: the stride halves until a node equal to the target
/// is met (not necessarily the first of an equal run).
fn binary_search<T>(a: &[(i32, T)], target: i32) -> Option<usize> {
    let mut pointer = 0usize;
    let mut stride = a.len() as i32;
    while 0 < stride {
        let pivot = stride / 2;
        let tryptr = pointer + pivot as usize;
        if target < a[tryptr].0 {
            stride = pivot;
        } else {
            if a[tryptr].0 == target {
                return Some(tryptr);
            }
            pointer = tryptr + 1;
            stride -= pivot + 1;
        }
    }
    None
}

/// `INIEntry` (inisup.h): an entry's name and value (C strings, as `strdup`'d by `Load`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IniEntry {
    /// `Entry`.
    pub entry: Vec<u8>,
    /// `Value`.
    pub value: Vec<u8>,
}

impl IniEntry {
    /// `Index_ID()`: `CRC::String(Entry)`.
    pub fn index_id(&self) -> i32 {
        crc_planted(&self.entry, plant(10250))
    }
}

/// `INISection` (inisup.h): a section's name, its entries in the file's order (`EntryList`) and their index by CRC
/// (`EntryIndex`, holding positions in the list).
#[derive(Clone, Debug, Default)]
pub struct IniSection {
    /// `Section`.
    pub section: Vec<u8>,
    /// `EntryList`.
    pub entry_list: Vec<IniEntry>,
    /// `EntryIndex`.
    entry_index: IndexClass<usize>,
}

impl IniSection {
    /// `INISection(section)`: no entry.
    pub fn new(section: Vec<u8>) -> Self {
        IniSection { section, entry_list: Vec::new(), entry_index: IndexClass::new() }
    }

    /// `Index_ID()`: `CRC::String(Section)`.
    pub fn index_id(&self) -> i32 {
        crc_planted(&self.section, plant(10251))
    }

    /// `INISection::Find_Entry(entry)`: the entry the index holds under the name's CRC.
    pub fn find_entry(&self, entry: &[u8]) -> Option<&IniEntry> {
        let crc = crc_planted(entry, plant(10250));
        if self.entry_index.is_present(crc) {
            return self.entry_index.get(crc).and_then(|p| self.entry_list.get(p));
        }
        None
    }
}

/// `INIClass`: the sections in the file's order (`SectionList`) and their index by CRC (`SectionIndex`).
#[derive(Clone, Debug)]
pub struct IniClass {
    /// `SectionList`.
    section_list: Vec<IniSection>,
    /// `SectionIndex` (positions in `section_list`).
    section_index: IndexClass<usize>,
    /// `Filename` (`"<unknown>"` until a file is loaded).
    filename: Vec<u8>,
    /// `KeepBlankEntries`. Upstream's is a static of the class (`Keep_Blank_Entries`), false unless set, and no Zero
    /// Hour code sets it; here it is the database's own, false by default.
    pub keep_blank_entries: bool,
    /// New: the (section, entry) of each `DuplicateCRCError` call, oldest first (upstream prints them and, in a
    /// release build on Windows, shows a message box).
    duplicate_crc_errors: Vec<(Vec<u8>, Vec<u8>)>,
}

impl Default for IniClass {
    fn default() -> Self {
        Self::new()
    }
}

/// The `Straw` `Load` reads (a `CacheStraw` over the file's `FileStraw`): the file's bytes, one at a time.
struct Straw<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Straw<'_> {
    /// `Get(&c, 1)`: the next byte, or `None` at the end.
    fn get(&mut self) -> Option<u8> {
        let c = self.data.get(self.pos).copied()?;
        self.pos += 1;
        Some(c)
    }
}

/// `Read_Line(file, buffer, len, eof)` (readline.cpp): the bytes up to the next `'\n'` (not kept), each `'\r'`
/// dropped, at most `len - 1` kept; `eof` set when the file ends first. The line is `strtrim`'d and its length
/// (`strlen`) returned; `buffer` is the C string (up to its first NUL).
fn read_line(file: &mut Straw<'_>, buffer: &mut Vec<u8>, len: usize, eof: &mut bool) -> usize {
    buffer.clear();
    let mut count = 0usize;
    loop {
        let Some(c) = file.get() else {
            *eof = true;
            break;
        };
        if c == b'\n' {
            break;
        }
        if plant(10253) && c == 0 {
            // planted: a NUL ends the line
            break;
        }
        let keep_cr = plant(10236);
        let fits = if plant(10237) { count < len } else { count + 1 < len };
        if (c != b'\r' || keep_cr) && fits {
            buffer.push(c);
            count += 1;
        }
    }
    let trimmed = strtrim(buffer);
    *buffer = trimmed;
    buffer.len()
}

/// `buffer[0] == '[' && strchr(buffer, ']') != NULL`: a section line.
fn is_section_line(buffer: &[u8]) -> bool {
    if plant(10254) {
        // planted: the `]` must end the line
        return buffer.first() == Some(&b'[') && buffer.last() == Some(&b']');
    }
    buffer.first() == Some(&b'[') && buffer.contains(&b']')
}

impl IniClass {
    /// `INIClass()`: empty (`Initialize`: the file name `"<unknown>"`).
    pub fn new() -> Self {
        IniClass {
            section_list: Vec::new(),
            section_index: IndexClass::new(),
            filename: b"<unknown>".to_vec(),
            keep_blank_entries: false,
            duplicate_crc_errors: Vec::new(),
        }
    }

    /// `INIClass(FileClass & file)`: empty, then `Load(file)` (its answer not kept). `data` is what the file reads:
    /// `None` when it cannot be opened (its `FileStraw` gives no byte).
    pub fn from_file(file_name: &[u8], data: Option<&[u8]>) -> Result<Self, IniError> {
        let mut ini = IniClass::new();
        ini.load_file(file_name, data)?;
        Ok(ini)
    }

    /// `Load(FileClass & file)`: the file name kept (`File_Name()`), then `Load(Straw &)` of its bytes.
    pub fn load_file(&mut self, file_name: &[u8], data: Option<&[u8]>) -> Result<bool, IniError> {
        self.filename = c_str(file_name).to_vec();
        self.load(data.unwrap_or(&[]))
    }

    /// `Keep_Blank_Entries(keep_blanks)` (upstream a static; see [`IniClass::keep_blank_entries`]).
    pub fn set_keep_blank_entries(&mut self, keep_blanks: bool) {
        self.keep_blank_entries = keep_blanks;
    }

    /// `Load(Straw & ffile)`: the file's sections and entries added (module docs). False when the file holds no
    /// section line before its last line. The merge branch (a database that has sections) is not translated.
    pub fn load(&mut self, data: &[u8]) -> Result<bool, IniError> {
        let mut end_of_file = false;
        let mut buffer: Vec<u8> = Vec::new();
        //
        // Determine if the INI database has preexisting entries. If it does, then the slower merging method of
        // loading is required.
        //
        if self.section_count() > 0 {
            return Err(IniError::MergeNotTranslated);
        }
        let mut file = Straw { data, pos: 0 };
        //
        // Prescan until the first section is found.
        //
        while !end_of_file {
            read_line(&mut file, &mut buffer, MAX_LINE_LENGTH, &mut end_of_file);
            if end_of_file && !plant(10238) {
                return Ok(false);
            }
            if is_section_line(&buffer) {
                break;
            }
        }
        //
        // Process a section. The buffer is prefilled with the section name line.
        //
        while !end_of_file {
            buffer[0] = b' ';
            let cut = if plant(10239) { buffer.iter().rposition(|&c| c == b']') } else { buffer.iter().position(|&c| c == b']') };
            if let Some(p) = cut {
                buffer.truncate(p);
            }
            let mut secptr = IniSection::new(strtrim(&buffer));
            //
            // Read in the entries of this section.
            //
            while !end_of_file {
                // If this line is the start of another section, then bail out of the entry loop and let the outer
                // section loop take care of it.
                let len = read_line(&mut file, &mut buffer, MAX_LINE_LENGTH, &mut end_of_file);
                if is_section_line(&buffer) {
                    break;
                }
                // Determine if this line is a comment or blank line. Throw it out if it is.
                strip_comments(&mut buffer);
                if len == 0 || buffer.first() == Some(&b';') || buffer.first() == Some(&b'=') {
                    continue;
                }
                // The line isn't an obvious comment. Make sure that there is the "=" character at an appropriate spot.
                let Some(divider) = buffer.iter().position(|&c| c == b'=') else { continue };
                // Split the line into entry and value sections. "=foobar" lines are ignored, while "foobar=" lines
                // might be stored as having " " as their value, depending on KeepBlankEntries.
                let entry = if plant(10255) { buffer[..divider].to_vec() } else { strtrim(&buffer[..divider]) };
                // (never true: the line's first byte is not white space, `=` or `;` here, so the name is not empty)
        if entry.is_empty() {
                    continue;
                }
                let mut value = if plant(10243) { buffer[divider + 1..].to_vec() } else { strtrim(&buffer[divider + 1..]) };
                if value.is_empty() {
                    if self.keep_blank_entries || plant(10241) {
                        value = b" ".to_vec();
                    } else {
                        continue;
                    }
                }
                let entryptr = IniEntry { entry, value };
                // 12/09/97 EHC - check to see if an entry with this ID already exists
                let id = entryptr.index_id();
                let present = if plant(10244) {
                    // planted: duplicates by name, not by CRC
                    secptr.entry_list.iter().any(|e| e.entry == entryptr.entry)
                } else {
                    secptr.entry_index.is_present(id)
                };
                if present {
                    self.duplicate_crc_error(&secptr.section.clone(), &entryptr.entry);
                    if plant(10245) {
                        // planted: the later entry replaces the earlier
                        if let Some(p) = secptr.entry_index.get(id) {
                            secptr.entry_list[p] = entryptr;
                        }
                    }
                    continue;
                }
                let at = secptr.entry_list.len();
                secptr.entry_index.add_index(id, at);
                secptr.entry_list.push(entryptr);
            }
            //
            // All the entries for this section have been parsed. If this section is blank, then don't bother storing it.
            //
            if !secptr.entry_list.is_empty() || plant(10246) {
                let at = self.section_list.len();
                self.section_index.add_index(secptr.index_id(), at);
                self.section_list.push(secptr);
            }
        }
        Ok(true)
    }

    /// `Is_Loaded()`: whether the database has a section.
    pub fn is_loaded(&self) -> bool {
        !self.section_list.is_empty()
    }

    /// `Get_Filename()`: the file's name, or `"<unknown>"`.
    pub fn get_filename(&self) -> &[u8] {
        &self.filename
    }

    /// `Get_Section_List()`: the sections in the order they were loaded (the list's valid nodes, `First()` and
    /// `Next_Valid()`).
    pub fn get_section_list(&self) -> &[IniSection] {
        &self.section_list
    }

    /// `Find_Section(section)`: the section the index holds under the name's CRC (with two sections of one CRC, the
    /// one `Binary_Search` meets, see the module docs).
    pub fn find_section(&self, section: &[u8]) -> Option<&IniSection> {
        let crc = crc_planted(section, plant(10251));
        if self.section_index.is_present(crc) {
            return self.section_index.get(crc).and_then(|p| self.section_list.get(p));
        }
        None
    }

    /// `Section_Present(section)`.
    pub fn section_present(&self, section: &[u8]) -> bool {
        self.find_section(section).is_some()
    }

    /// `Section_Count()`: the index's count (a name loaded twice counts twice).
    pub fn section_count(&self) -> i32 {
        self.section_index.count()
    }

    /// `Entry_Count(section)`: the entries of the section `Find_Section` finds; 0 without one.
    pub fn entry_count(&self, section: &[u8]) -> i32 {
        match self.find_section(section) {
            Some(s) => s.entry_index.count(),
            None => 0,
        }
    }

    /// `Find_Entry(section, entry)`.
    pub fn find_entry(&self, section: &[u8], entry: &[u8]) -> Option<&IniEntry> {
        self.find_section(section)?.find_entry(entry)
    }

    /// `Get_Entry(section, index)`: the name of the section's `index`th entry in the file's order; `None` for an
    /// index not below the count (a negative one walks the whole list and finds none).
    pub fn get_entry(&self, section: &[u8], index: i32) -> Option<&[u8]> {
        let secptr = self.find_section(section)?;
        let mut index = index;
        if plant(10249) {
            // planted: counted from 1
            index -= 1;
        }
        if index < secptr.entry_index.count() {
            for entryptr in &secptr.entry_list {
                if index == 0 {
                    return Some(&entryptr.entry);
                }
                index -= 1;
            }
        }
        None
    }

    /// `Get_String(new_string, section, entry, defvalue)` (the `StringClass` overload): the entry's value as stored, or
    /// `defvalue` when the entry is not found.
    pub fn get_string(&self, section: &[u8], entry: &[u8], defvalue: &[u8]) -> Vec<u8> {
        match self.find_entry(section, entry) {
            Some(e) => e.value.clone(),
            None if plant(10242) => Vec::new(),
            None => c_str(defvalue).to_vec(),
        }
    }

    /// `INIClass::CRC(string)`: `CRC::String(string)` ([`crc`]).
    pub fn crc(string: &[u8]) -> i32 {
        crc(string)
    }

    /// `DuplicateCRCError(message, section, entry)`: recorded (see [`IniClass::duplicate_crc_errors`]).
    fn duplicate_crc_error(&mut self, section: &[u8], entry: &[u8]) {
        self.duplicate_crc_errors.push((section.to_vec(), entry.to_vec()));
    }

    /// New: the (section, entry) of each duplicate `Load` dropped, oldest first.
    pub fn duplicate_crc_errors(&self) -> &[(Vec<u8>, Vec<u8>)] {
        &self.duplicate_crc_errors
    }
}

/// `Strip_Comments(buffer)`: the C string cut at its first `;`, then trimmed (only when there was one).
pub fn strip_comments(buffer: &mut Vec<u8>) {
    let s = c_str(buffer).len();
    buffer.truncate(s);
    if let Some(comment) = buffer.iter().position(|&c| c == b';') {
        buffer.truncate(comment);
        if !plant(10240) {
            let t = strtrim(buffer);
            *buffer = t;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(ini: &IniClass) -> Vec<Vec<u8>> {
        ini.get_section_list().iter().map(|s| s.section.clone()).collect()
    }

    #[test]
    fn sections_entries_comments_and_trimming() {
        let text = b"junk=1\r\n  [ Alpha ] trailing\r\n a = 1 ; c\n;x=2\n=v\nk=\nnoeq\n b=x=y\n[Empty]\n;only\n[a;b]\nq=w\n[Last]";
        let mut ini = IniClass::new();
        assert_eq!(ini.load(text), Ok(true));
        assert_eq!(names(&ini), vec![b"Alpha".to_vec(), b"a;b".to_vec()]);
        assert_eq!(ini.section_count(), 2);
        assert!(ini.is_loaded());
        assert_eq!(ini.entry_count(b"Alpha"), 2);
        assert_eq!(ini.get_entry(b"Alpha", 0), Some(&b"a"[..]));
        assert_eq!(ini.get_entry(b"Alpha", 1), Some(&b"b"[..]));
        assert_eq!(ini.get_entry(b"Alpha", 2), None);
        assert_eq!(ini.get_entry(b"Alpha", -1), None);
        assert_eq!(ini.get_string(b"Alpha", b"a", b"d"), b"1");
        assert_eq!(ini.get_string(b"Alpha", b"b", b"d"), b"x=y");
        assert_eq!(ini.get_string(b"Alpha", b"k", b"d"), b"d");
        assert_eq!(ini.get_string(b"alpha", b"a", b""), b"");
        assert!(!ini.section_present(b"Last"));
        assert_eq!(ini.get_filename(), b"<unknown>");
        // blank entries kept as " " when asked
        let mut ini = IniClass::new();
        ini.set_keep_blank_entries(true);
        ini.load(b"[s]\nk=\n").unwrap();
        assert_eq!(ini.get_string(b"s", b"k", b""), b" ");
        // a database with sections is not merged into
        assert_eq!(ini.load(b"[t]\na=1\n"), Err(IniError::MergeNotTranslated));
    }

    #[test]
    fn the_prescan_needs_a_line_after_the_first_section() {
        let mut ini = IniClass::new();
        assert_eq!(ini.load(b"x\n[A]"), Ok(false));
        assert_eq!(ini.load(b""), Ok(false));
        assert_eq!(ini.load(b"[A]\n"), Ok(true));
        assert!(!ini.is_loaded());
    }

    #[test]
    fn lines_are_cut_at_4095_bytes_and_at_a_nul() {
        let mut long = b"[S]\nk=".to_vec();
        long.extend(std::iter::repeat(b'v').take(5000));
        long.extend_from_slice(b"\nz=1\x00ignored\n");
        let ini = IniClass::from_file(b"f.ini", Some(&long)).unwrap();
        assert_eq!(ini.get_string(b"S", b"k", b"").len(), MAX_LINE_LENGTH - 1 - 2);
        assert_eq!(ini.get_string(b"S", b"z", b""), b"1");
        assert_eq!(ini.get_filename(), b"f.ini");
        assert!(!IniClass::from_file(b"g", None).unwrap().is_loaded());
    }

    #[test]
    fn duplicates_go_by_crc_and_sections_are_not_merged() {
        // "WlZMkQ" and "ylvDlZ" have one CRC-32 (0xba1c21fb)
        assert_eq!(IniClass::crc(b"WlZMkQ"), IniClass::crc(b"ylvDlZ"));
        assert_eq!(IniClass::crc(b"WlZMkQ"), 0xba1c_21fbu32 as i32);
        let ini = IniClass::from_file(b"d", Some(b"[A]\nWlZMkQ=1\nylvDlZ=2\nWlZMkQ=3\n[B]\nx=1\n[A]\nother=5\n")).unwrap();
        assert_eq!(ini.entry_count(b"A"), 1);
        assert_eq!(ini.section_count(), 3);
        assert_eq!(names(&ini), vec![b"A".to_vec(), b"B".to_vec(), b"A".to_vec()]);
        assert_eq!(ini.duplicate_crc_errors(), &[(b"A".to_vec(), b"ylvDlZ".to_vec()), (b"A".to_vec(), b"WlZMkQ".to_vec())]);
        // equal IDs stay in load order (stable sort); Binary_Search over [A, A, B] sorted by ID meets one of them
        let a = ini.find_section(b"A").unwrap();
        assert!(a.entry_list[0].entry == b"WlZMkQ" || a.entry_list[0].entry == b"other");
        assert_eq!(ini.find_entry(b"B", b"x").map(|e| e.value.clone()), Some(b"1".to_vec()));
    }

    #[test]
    fn binary_search_and_the_archive() {
        let mut ix: IndexClass<usize> = IndexClass::new();
        assert!(!ix.is_present(3));
        for (i, id) in [5, -2, 9, 5, 0, 7, 7, 7, 1, 2, 3, 4].into_iter().enumerate() {
            ix.add_index(id, i);
        }
        assert_eq!(ix.count(), 12);
        assert_eq!(ix.get(-2), Some(1));
        assert_eq!(ix.get(9), Some(2));
        assert_eq!(ix.get(6), None);
        assert!(ix.is_present(7));
        assert!(matches!(ix.get(7), Some(5..=7)));
        let t: Vec<(i32, usize)> = (0..11).map(|i| (i * 2, i as usize)).collect();
        for (i, &(id, _)) in t.iter().enumerate() {
            assert_eq!(binary_search(&t, id), Some(i));
            assert_eq!(binary_search(&t, id + 1), None);
        }
    }
}
