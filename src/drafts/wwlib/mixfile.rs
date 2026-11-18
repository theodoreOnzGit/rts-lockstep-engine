//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/mixfile.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/mixfile.cpp
//!
//! `MixFileFactoryClass` (a file factory that serves the sub-files of a "MIX1" archive) and `MixFileCreator` (writes
//! one). The `FileFactoryClass` / `FileClass` / `RawFileClass` machinery is replaced by `std::fs` (the previous pass's
//! note): the "factory" is the directory-relative path of the archive, and `Get_File` returns a [`MixSubFile`], the view
//! `RawFileClass::Bias(offset, size)` makes (a path, an offset and a size). Layout (little-endian): the 12 byte header
//! {"MIX1", i32 header_offset, i32 names_offset} (the creator also writes 4 unused bytes after it), the sub-files each
//! padded to 8 bytes, then at `header_offset` the i32 file count and `count` x {u32 CRC, u32 Offset, u32 Size} sorted by
//! CRC, then at `names_offset` the i32 count and each name as {u8 length including NUL, the name and its NUL}. Names
//! are matched by `CRC_Stringi` only (two names with the same CRC are indistinguishable). Not translated:
//! `Add_Files` / `Setup_Mix_File` (a Windows `FindFirstFile` tool that builds MAKEMIX.MIX), and the `WWDEBUG_SAY` logs.

use crate::ww::realcrc::crc_stringi;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};

/// `FileInfoStruct` of the factory: CRC of the embedded file's name, offset from the start of the data section, size.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileInfoStruct {
    pub crc: u32,
    pub offset: u32,
    pub size: u32,
}

/// `AddInfoStruct`: a file waiting to be added.
#[derive(Clone, Debug, Default)]
pub struct AddInfoStruct {
    pub full_path: String,
    pub filename: String,
}

/// A sub-file of the archive: what `Get_File` returns after `Bias(BaseOffset + info->Offset, info->Size)`.
#[derive(Clone, Debug)]
pub struct MixSubFile {
    /// The archive on disk.
    pub path: String,
    /// Absolute offset of the sub-file's first byte.
    pub offset: u64,
    /// Size of the sub-file.
    pub size: u64,
}

impl MixSubFile {
    /// Reads the whole sub-file.
    pub fn read_all(&self) -> std::io::Result<Vec<u8>> {
        let mut f = File::open(&self.path)?;
        f.seek(SeekFrom::Start(self.offset))?;
        let mut buf = vec![0u8; self.size as usize];
        f.read_exact(&mut buf)?;
        Ok(buf)
    }
}

/// `class MixFileFactoryClass`.
pub struct MixFileFactoryClass {
    file_info: Vec<FileInfoStruct>,
    mix_filename: String,
    base_offset: i32,
    file_count: i32,
    names_offset: i32,
    is_valid: bool,
    filename_list: Vec<String>,
    pending_add_file_list: Vec<AddInfoStruct>,
    is_modified: bool,
}

impl MixFileFactoryClass {
    /// `MixFileFactoryClass(mix_filename, factory)` (mixfile.cpp:79): reads the header, the file count and the info table;
    /// `IsValid` stays false when the archive is missing, short or does not start with "MIX1".
    pub fn new(mix_filename: &str) -> Self {
        let mut s = MixFileFactoryClass {
            file_info: Vec::new(),
            mix_filename: mix_filename.to_string(),
            base_offset: 0,
            file_count: 0,
            names_offset: 0,
            is_valid: false,
            filename_list: Vec::new(),
            pending_add_file_list: Vec::new(),
            is_modified: false,
        };

        if let Ok(mut file) = File::open(mix_filename) {
            let mut hdr = [0u8; 12];
            s.is_valid = file.read_exact(&mut hdr).is_ok();

            if s.is_valid {
                s.is_valid = &hdr[0..4] == b"MIX1";
            }
            let header_offset = i32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);
            let names_offset = i32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]);

            s.file_count = 0;
            if s.is_valid {
                let mut c = [0u8; 4];
                s.is_valid = file.seek(SeekFrom::Start(header_offset as u64)).is_ok() && file.read_exact(&mut c).is_ok();
                s.file_count = i32::from_le_bytes(c);
            }

            if s.is_valid {
                let size = (s.file_count.max(0) as usize) * 12;
                let mut raw = vec![0u8; size];
                s.is_valid = file.read_exact(&mut raw).is_ok();
                if s.is_valid {
                    for e in raw.chunks_exact(12) {
                        s.file_info.push(FileInfoStruct {
                            crc: u32::from_le_bytes([e[0], e[1], e[2], e[3]]),
                            offset: u32::from_le_bytes([e[4], e[5], e[6], e[7]]),
                            size: u32::from_le_bytes([e[8], e[9], e[10], e[11]]),
                        });
                    }
                }
            }

            if s.is_valid {
                s.base_offset = 0;
                s.names_offset = names_offset;
            } else {
                s.file_info.clear();
            }
        }
        s
    }

    /// `Is_Valid()`.
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// `Build_Filename_List(list)` (mixfile.cpp:124): reads the name table; false when the archive is not valid or
    /// cannot be opened. A short read ends the list quietly (the return is still true).
    pub fn build_filename_list(&self, list: &mut Vec<String>) -> bool {
        if !self.is_valid {
            return false;
        }

        let Ok(mut file) = File::open(&self.mix_filename) else {
            return false;
        };
        let _ = file.seek(SeekFrom::Start(self.names_offset as u64));
        let retval = true;

        let mut c = [0u8; 4];
        if file.read_exact(&mut c).is_ok() {
            let file_count = i32::from_le_bytes(c);
            let mut keep_going = true;
            let mut index = 0;
            while index < file_count && keep_going {
                keep_going = false;
                let mut name_len = [0u8; 1];
                if file.read_exact(&mut name_len).is_ok() {
                    let mut buf = vec![0u8; name_len[0] as usize];
                    if file.read_exact(&mut buf).is_ok() {
                        let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                        list.push(String::from_utf8_lossy(&buf[..n]).into_owned());
                        keep_going = true;
                    }
                }
                index += 1;
            }
        }
        retval
    }

    /// `Build_Internal_Filename_List()`.
    pub fn build_internal_filename_list(&mut self) -> bool {
        let mut l = Vec::new();
        let r = self.build_filename_list(&mut l);
        self.filename_list = l;
        r
    }

    /// `Get_Filename_List(list)`.
    pub fn get_filename_list(&self) -> &Vec<String> {
        &self.filename_list
    }

    /// `Get_File(filename)` (mixfile.cpp:171): binary search of the CRC-sorted table for `CRC_Stringi(filename)`;
    /// the sub-file view, or `None` if absent (or the table is empty).
    pub fn get_file(&self, filename: &str) -> Option<MixSubFile> {
        if self.file_info.is_empty() {
            return None;
        }

        let crc = crc_stringi(filename.as_bytes(), 0);
        let mut info: Option<FileInfoStruct> = None;
        let mut base = 0usize;
        let mut stride = self.file_info.len();
        while stride > 0 {
            let pivot = stride / 2;
            let tryptr = base + pivot;
            if crc < self.file_info[tryptr].crc {
                stride = pivot;
            } else {
                if self.file_info[tryptr].crc == crc {
                    info = Some(self.file_info[tryptr]);
                    break;
                }
                base = tryptr + 1;
                stride -= pivot + 1;
            }
        }

        info.map(|i| MixSubFile { path: self.mix_filename.clone(), offset: (self.base_offset as i64 + i.offset as i64) as u64, size: i.size as u64 })
    }

    /// `Return_File(file)`: nothing to give back.
    pub fn return_file(&self, _file: Option<MixSubFile>) {}

    /// `Add_File(full_path, filename)`: queued until `Flush_Changes`.
    pub fn add_file(&mut self, full_path: &str, filename: &str) {
        self.pending_add_file_list.push(AddInfoStruct { full_path: full_path.to_string(), filename: filename.to_string() });
        self.is_modified = true;
    }

    /// `Delete_File(filename)`: removes the first name (case-insensitive) from the internal list.
    pub fn delete_file(&mut self, filename: &str) {
        if let Some(i) = self.filename_list.iter().position(|n| n.eq_ignore_ascii_case(filename)) {
            self.filename_list.remove(i);
            self.is_modified = true;
        }
    }

    /// `Get_Temp_Filename(path, full_path)`: the first of `<path>_tmpmix01.dat` .. `20.dat` that does not exist.
    fn get_temp_filename(&self, path: &str) -> Option<String> {
        let temp_path = format!("{}_tmpmix", path);
        for index in 0..20 {
            let full_path = format!("{}{:02}.dat", temp_path, index + 1);
            if !std::path::Path::new(&full_path).exists() {
                return Some(full_path);
            }
        }
        None
    }

    /// `Flush_Changes()` (mixfile.cpp:262): rebuilds the archive into a temporary file from the internal name list (each
    /// existing sub-file, and a pending add replacing the same name), then replaces the old archive with it. Upstream
    /// deletes the old archive and moves the temporary file even when no temporary name was found.
    pub fn flush_changes(&mut self) {
        if !self.is_modified {
            return;
        }

        // The directory of the archive (`_splitpath` drive + dir)
        let path = match self.mix_filename.rfind(|c| c == '/' || c == '\\') {
            Some(i) => self.mix_filename[..=i].to_string(),
            None => String::new(),
        };

        let full_path = self.get_temp_filename(&path);
        if let Some(full_path) = &full_path {
            let mut new_mix_file = MixFileCreator::new(full_path);

            // Copy the files that are still wanted
            let names = self.filename_list.clone();
            for filename in names.iter() {
                if let Some(sub) = self.get_file(filename) {
                    if let Ok(data) = sub.read_all() {
                        new_mix_file.add_file_data(filename, &data);

                        // A pending add of the same name is superseded by the copy
                        if let Some(t) = self.pending_add_file_list.iter().position(|p| filename.eq_ignore_ascii_case(&p.filename)) {
                            self.pending_add_file_list.remove(t);
                        }
                    }
                }
            }

            // Add the new files
            for p in self.pending_add_file_list.iter() {
                new_mix_file.add_file(&p.full_path, Some(&p.filename));
            }
            // (the creator finishes the archive when it is dropped)
        }

        let _ = fs::remove_file(&self.mix_filename);
        if let Some(full_path) = &full_path {
            let _ = fs::rename(full_path, &self.mix_filename);
        }
        self.is_modified = false;
        self.pending_add_file_list.clear();
    }

    /// `Build_Ordered_Filename_List(list)` (mixfile.cpp:316): the names ordered by their offset in the archive. The
    /// i-th name is paired with the i-th info entry (upstream's own assumption); `qsort` is unstable, so equal offsets
    /// may come out in either order.
    pub fn build_ordered_filename_list(&self, list: &mut Vec<String>) -> bool {
        if !self.is_valid {
            return false;
        }

        let mut name_list = Vec::new();
        if !self.build_filename_list(&mut name_list) {
            return false;
        }

        let mut local_file_info: Vec<(String, u32)> = Vec::with_capacity(name_list.len());
        for i in 0..name_list.len() {
            local_file_info.push((name_list[i].clone(), self.file_info[i].offset));
        }

        if local_file_info.len() > 1 {
            local_file_info.sort_unstable_by(|a, b| a.1.cmp(&b.1));
        }

        list.clear();
        for e in local_file_info.into_iter() {
            list.push(e.0);
        }
        true
    }
}

/// `MixFileCreator::FileInfoStruct`: as the factory's plus the saved name.
#[derive(Clone, Debug, Default)]
struct CreatorFileInfo {
    crc: u32,
    offset: u32,
    size: u32,
    filename: String,
}

/// `class MixFileCreator`: writes the archive; the index and header are written when it is dropped (`~MixFileCreator`).
pub struct MixFileCreator {
    file_info: Vec<CreatorFileInfo>,
    mix_file: Option<File>,
}

impl MixFileCreator {
    /// `MixFileCreator(filename)`: "MIX1" and three zero words (header offset, names offset, unused).
    pub fn new(filename: &str) -> Self {
        let mut mix_file = File::create(filename).ok();
        if let Some(f) = mix_file.as_mut() {
            let _ = f.write_all(b"MIX1");
            let _ = f.write_all(&0i32.to_le_bytes());
            let _ = f.write_all(&0i32.to_le_bytes());
            let _ = f.write_all(&0i32.to_le_bytes());
        }
        MixFileCreator { file_info: Vec::new(), mix_file }
    }

    fn tell(f: &mut File) -> u64 {
        f.stream_position().unwrap_or(0)
    }

    /// `Add_File(source_filename, saved_filename)`: copies a file from disk; the saved name defaults to the source's.
    pub fn add_file(&mut self, source_filename: &str, saved_filename: Option<&str>) {
        let saved_filename = saved_filename.unwrap_or(source_filename);
        if self.mix_file.is_some() {
            match fs::read(source_filename) {
                Ok(data) => self.add_file_data(saved_filename, &data),
                Err(_) => { /* "MixFileCreator::Failed to open" */ }
            }
        }
    }

    /// `Add_File(filename, file)`: the sub-file's bytes written at the current end, then padded to a multiple of 8.
    pub fn add_file_data(&mut self, filename: &str, data: &[u8]) {
        if let Some(f) = self.mix_file.as_mut() {
            let offset = Self::tell(f) as u32;
            self.file_info.push(CreatorFileInfo { crc: crc_stringi(filename.as_bytes(), 0), offset, size: data.len() as u32, filename: filename.to_string() });

            let _ = f.write_all(data);

            let pos = Self::tell(f) as i64;
            let pad = ((8 - (pos & 7)) & 7) as usize;
            if pad != 0 {
                let _ = f.write_all(&[0u8; 8][..pad]);
            }
        }
    }
}

impl Drop for MixFileCreator {
    /// `~MixFileCreator` (mixfile.cpp:476): the count and the CRC-sorted table, the name table, then the two offsets
    /// patched into the header. A name must be shorter than 255 bytes with its NUL (`WWASSERT(size < 255)`).
    fn drop(&mut self) {
        let Some(f) = self.mix_file.as_mut() else {
            return;
        };

        let header_offset = Self::tell(f) as i32;
        let num_files = self.file_info.len() as i32;
        let _ = f.write_all(&num_files.to_le_bytes());

        if num_files > 1 {
            self.file_info.sort_unstable_by(|a, b| a.crc.cmp(&b.crc));
        }
        for fi in self.file_info.iter() {
            let _ = f.write_all(&fi.crc.to_le_bytes());
            let _ = f.write_all(&fi.offset.to_le_bytes());
            let _ = f.write_all(&fi.size.to_le_bytes());
        }

        let names_offset = Self::tell(f) as i32;
        let _ = f.write_all(&num_files.to_le_bytes());
        for fi in self.file_info.iter() {
            let size = fi.filename.len() + 1;
            assert!(size < 255);
            let _ = f.write_all(&[size as u8]);
            let mut name = fi.filename.as_bytes().to_vec();
            name.push(0);
            let _ = f.write_all(&name);
        }

        let _ = f.seek(SeekFrom::Start(4));
        let _ = f.write_all(&header_offset.to_le_bytes());
        let _ = f.write_all(&names_offset.to_le_bytes());
    }
}
