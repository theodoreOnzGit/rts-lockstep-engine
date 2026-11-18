//! UNTESTED DRAFT (#202, #208): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/Common/LocalFile.h
//!   GeneralsMD/Code/GameEngine/Source/Common/System/LocalFile.cpp
//!   GeneralsMD/Code/GameEngine/Source/GameEngineDevice/Win32Device/Common/Win32LocalFile.cpp (35 lines: `Win32LocalFile`'s only
//!   body is its constructor/destructor shell over `LocalFile`; nothing else to translate)
//!
//! `LocalFile` is `File` over the C runtime's `_open/_read/_write/_lseek/_close` (the `USE_BUFFERED_IO` branch, `fopen`/`fread`, is
//! the same logic over `FILE*`; this draft translates the unbuffered branch, which has the extra `_O_*` flag mapping). The OS
//! calls are the host's: [`LocalFileDevice`]. `File::open/close` (File.cpp, the base class: it records the access mask and
//! the open flag) is reduced here to the three fields `LocalFile` reads. `convertToRAMFile` is not translated (`RAMFile` is
//! not drafted in this area): `// upstream: RAMFile::open(this) - not translated`.

/// `File::FileAccess` bits (File.h).
pub const FILE_READ: i32 = 0x0000_0001;
/// `WRITE`.
pub const FILE_WRITE: i32 = 0x0000_0002;
/// `APPEND`.
pub const FILE_APPEND: i32 = 0x0000_0004;
/// `CREATE`.
pub const FILE_CREATE: i32 = 0x0000_0008;
/// `TRUNCATE`.
pub const FILE_TRUNCATE: i32 = 0x0000_0010;
/// `TEXT`.
pub const FILE_TEXT: i32 = 0x0000_0020;
/// `BINARY`.
pub const FILE_BINARY: i32 = 0x0000_0040;
/// `READWRITE` (`READ | WRITE`).
pub const FILE_READWRITE: i32 = FILE_READ | FILE_WRITE;

/// `File::seekMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekMode {
    /// `START`.
    Start,
    /// `CURRENT`.
    Current,
    /// `END`.
    End,
}

/// `_O_*` flags `LocalFile::open` builds for `_open`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenFlags {
    /// `_O_CREAT`.
    pub creat: bool,
    /// `_O_TRUNC`.
    pub trunc: bool,
    /// `_O_APPEND`.
    pub append: bool,
    /// `_O_TEXT`.
    pub text: bool,
    /// `_O_BINARY`.
    pub binary: bool,
    /// `_O_RDWR`.
    pub rdwr: bool,
    /// `_O_WRONLY`.
    pub wronly: bool,
    /// `_O_RDONLY` (the absence of the two above).
    pub rdonly: bool,
}

/// The C runtime's file calls on a handle.
pub trait LocalFileDevice {
    /// `_open(filename, flags, _S_IREAD | _S_IWRITE)`: a handle, or -1.
    fn open(&mut self, filename: &[u8], flags: OpenFlags) -> i32;
    /// `_close(handle)`.
    fn close(&mut self, handle: i32);
    /// `_read(handle, buf, n)`: bytes read (0 at EOF), -1 on error.
    fn read(&mut self, handle: i32, buf: &mut [u8]) -> i32;
    /// `_write(handle, buf, n)`.
    fn write(&mut self, handle: i32, buf: &[u8]) -> i32;
    /// `_lseek(handle, pos, SEEK_*)`: the new position, -1 on error.
    fn lseek(&mut self, handle: i32, pos: i32, mode: SeekMode) -> i32;
}

/// `static Int s_totalOpen`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalFileTotals {
    /// `s_totalOpen`: the number of open handles.
    pub total_open: i32,
}

/// `class LocalFile : public File`.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalFile {
    /// `File::m_access`.
    pub access: i32,
    /// `File::m_open`.
    pub open: bool,
    /// `File::m_deleteOnClose`.
    pub delete_on_close: bool,
    /// `m_handle`, -1 when closed.
    pub handle: i32,
}

/// `isspace(c)` in the C locale.
fn is_space(c: u8) -> bool {
    c == b' ' || (b'\t'..=b'\r').contains(&c)
}

impl LocalFile {
    /// `LocalFile::LocalFile()`: `m_handle(-1)`.
    pub fn new() -> LocalFile {
        LocalFile { access: 0, open: false, delete_on_close: false, handle: -1 }
    }

    /// `LocalFile::~LocalFile()`: close a still-open handle (`--s_totalOpen`), then `File::close()`.
    pub fn drop_file<D: LocalFileDevice>(&mut self, dev: &mut D, totals: &mut LocalFileTotals) {
        if self.handle != -1 {
            dev.close(self.handle);
            self.handle = -1;
            totals.total_open -= 1;
        }
        self.open = false; // File::close()
    }

    /// `LocalFile::open(filename, access)`: "opens a file using the standard C open() call. Access flags are mapped to the
    /// appropriate open flags." `File::open` first (records `m_access` and `m_open`); then CREATE/TRUNCATE/APPEND/TEXT/BINARY map to
    /// their `_O_*`, READWRITE is `_O_RDWR`, WRITE alone is `_O_WRONLY | _O_CREAT`, else `_O_RDONLY`. A failed `_open` or a failed
    /// seek to the end for APPEND closes and returns FALSE.
    pub fn open_file<D: LocalFileDevice>(&mut self, dev: &mut D, totals: &mut LocalFileTotals, filename: &[u8], access: i32) -> bool {
        // File::open(filename, access): upstream: File::open — base-class bookkeeping only.
        self.access = access;
        self.open = true;
        let mut flags = OpenFlags::default();
        if self.access & FILE_CREATE != 0 {
            flags.creat = true;
        }
        if self.access & FILE_TRUNCATE != 0 {
            flags.trunc = true;
        }
        if self.access & FILE_APPEND != 0 {
            flags.append = true;
        }
        if self.access & FILE_TEXT != 0 {
            flags.text = true;
        }
        if self.access & FILE_BINARY != 0 {
            flags.binary = true;
        }
        if (self.access & FILE_READWRITE) == FILE_READWRITE {
            flags.rdwr = true;
        } else if self.access & FILE_WRITE != 0 {
            flags.wronly = true;
            flags.creat = true;
        } else {
            flags.rdonly = true;
        }
        self.handle = dev.open(filename, flags);
        if self.handle == -1 {
            self.close_file(dev, totals);
            return false;
        }
        totals.total_open += 1;
        if self.access & FILE_APPEND != 0 && self.seek(dev, 0, SeekMode::End) < 0 {
            self.close_file(dev, totals);
            return false;
        }
        true
    }

    /// `LocalFile::close()`: "Closes the current file if it is open" -- upstream's body is `File::close()`; the handle is closed by the
    /// destructor (`~LocalFile`). Modelled as both, since the instance is dropped after `close`.
    pub fn close_file<D: LocalFileDevice>(&mut self, dev: &mut D, totals: &mut LocalFileTotals) {
        self.drop_file(dev, totals);
    }

    /// `LocalFile::read(buffer, bytes)`: -1 when not open; a NULL buffer skips `bytes` forward (`_lseek(CUR)`) and returns `bytes`.
    pub fn read<D: LocalFileDevice>(&mut self, dev: &mut D, buffer: Option<&mut [u8]>, bytes: i32) -> i32 {
        if !self.open {
            return -1;
        }
        match buffer {
            None => {
                dev.lseek(self.handle, bytes, SeekMode::Current);
                bytes
            }
            Some(b) => dev.read(self.handle, &mut b[..bytes.max(0) as usize]),
        }
    }

    /// `LocalFile::write(buffer, bytes)`: -1 when not open or the buffer is NULL.
    pub fn write<D: LocalFileDevice>(&mut self, dev: &mut D, buffer: Option<&[u8]>, bytes: i32) -> i32 {
        match buffer {
            Some(b) if self.open => dev.write(self.handle, &b[..bytes.max(0) as usize]),
            _ => -1,
        }
    }

    /// `LocalFile::seek(pos, mode)`: `_lseek`; an END seek expects `pos <= 0` (a `DEBUG_ASSERTCRASH`, compiled out of release).
    pub fn seek<D: LocalFileDevice>(&mut self, dev: &mut D, pos: i32, mode: SeekMode) -> i32 {
        dev.lseek(self.handle, pos, mode)
    }

    /// One `_read(m_handle, &c, 1)`.
    fn read_char<D: LocalFileDevice>(&self, dev: &mut D, c: &mut u8) -> i32 {
        let mut b = [0u8; 1];
        let v = dev.read(self.handle, &mut b);
        *c = b[0];
        v
    }

    /// `_lseek(m_handle, -1, SEEK_CUR)`: "put the last read char back, since we didn't use it."
    fn put_back<D: LocalFileDevice>(&self, dev: &mut D) {
        dev.lseek(self.handle, -1, SeekMode::Current);
    }

    /// `LocalFile::scanInt(newInt)`: skips characters that are neither digits nor `-`, then takes the `-`/digit run; `atoi` of it.
    /// FALSE (with `newInt = 0`) at EOF before any.
    pub fn scan_int<D: LocalFileDevice>(&mut self, dev: &mut D, new_int: &mut i32) -> bool {
        *new_int = 0;
        let mut tempstr: Vec<u8> = Vec::new();
        let mut c: u8 = 0;
        let mut val;
        // skip preceding non-numeric characters
        loop {
            val = self.read_char(dev, &mut c);
            if !((val != 0) && (((c < b'0') || (c > b'9')) && (c != b'-'))) {
                break;
            }
        }
        if val == 0 {
            return false;
        }
        loop {
            tempstr.push(c);
            val = self.read_char(dev, &mut c);
            if !((val != 0) && ((c >= b'0') && (c <= b'9'))) {
                break;
            }
        }
        // put the last read char back, since we didn't use it.
        if val != 0 {
            self.put_back(dev);
        }
        *new_int = super::command_line::atoi(&tempstr);
        true
    }

    /// `LocalFile::scanReal(newReal)`: like `scanInt` but `.` also starts and continues (once) the run; `atof` of it.
    pub fn scan_real<D: LocalFileDevice>(&mut self, dev: &mut D, new_real: &mut f32) -> bool {
        *new_real = 0.0;
        let mut tempstr: Vec<u8> = Vec::new();
        let mut c: u8 = 0;
        let mut val;
        let mut saw_dec = false;
        // skip the preceding white space
        loop {
            val = self.read_char(dev, &mut c);
            if !((val != 0) && (((c < b'0') || (c > b'9')) && (c != b'-') && (c != b'.'))) {
                break;
            }
        }
        if val == 0 {
            return false;
        }
        loop {
            tempstr.push(c);
            if c == b'.' {
                saw_dec = true;
            }
            val = self.read_char(dev, &mut c);
            if !((val != 0) && (((c >= b'0') && (c <= b'9')) || ((c == b'.') && !saw_dec))) {
                break;
            }
        }
        if val != 0 {
            self.put_back(dev);
        }
        // atof(tempstr.str()): the longest valid prefix (a lone "-" or "." is 0).
        let s = String::from_utf8_lossy(&tempstr).into_owned();
        let mut end = s.len();
        while end > 0 && s[..end].parse::<f64>().is_err() {
            end -= 1;
        }
        *new_real = s[..end].parse::<f64>().unwrap_or(0.0) as f32;
        true
    }

    /// `LocalFile::scanString(newString)`: skips whitespace, then takes up to the next whitespace; FALSE at EOF before any.
    pub fn scan_string<D: LocalFileDevice>(&mut self, dev: &mut D, new_string: &mut Vec<u8>) -> bool {
        let mut c: u8 = 0;
        let mut val;
        new_string.clear();
        // skip the preceding whitespace
        loop {
            val = self.read_char(dev, &mut c);
            if !((val != 0) && is_space(c)) {
                break;
            }
        }
        if val == 0 {
            return false;
        }
        loop {
            new_string.push(c);
            val = self.read_char(dev, &mut c);
            if !((val != 0) && !is_space(c)) {
                break;
            }
        }
        if val != 0 {
            self.put_back(dev);
        }
        true
    }

    /// `LocalFile::nextLine(buf, bufSize)`: "scans to the first character after a new-line or at EOF", copying up to `bufSize-1`
    /// characters of the line (the newline included when it fits) into `buf` and NUL-terminating it; the rest is read and dropped.
    /// `buf` of `None` is a NULL buffer. Upstream writes `buf[bufSize] = 0` when the line filled it: one past the end (the caller's
    /// buffer is expected to have the extra byte); here that write is skipped when `buf` has no room.
    pub fn next_line<D: LocalFileDevice>(&mut self, dev: &mut D, mut buf: Option<&mut [u8]>, buf_size: i32) {
        let mut c: u8 = 0;
        let mut val;
        let mut i: i32 = 0;
        // seek to the next new-line.
        loop {
            match buf.as_deref_mut() {
                Some(b) if i < buf_size - 1 => {
                    val = dev.read(self.handle, &mut b[i as usize..i as usize + 1]);
                    c = b[i as usize];
                }
                _ => {
                    val = self.read_char(dev, &mut c);
                }
            }
            i += 1;
            if !((val != 0) && (c != b'\n')) {
                break;
            }
        }
        if let Some(b) = buf {
            let at = if i < buf_size { i } else { buf_size } as usize;
            if at < b.len() {
                b[at] = 0;
            }
        }
    }

    /// `LocalFile::convertToRAMFile()`: builds a `RAMFile` from this one and closes this. upstream: `RAMFile::open(this)` — `RAMFile` not
    /// translated in this area; returns `false` meaning "keeps this file" (upstream's failure branch returns `this`).
    pub fn convert_to_ram_file(&mut self) -> bool {
        false
    }

    /// `LocalFile::readEntireAndClose()`: "Allocate a buffer large enough to hold entire file, read the entire file into the buffer,
    /// then close the file." `size()` is `File::size()` (seek to the end and back), done here with `lseek`.
    pub fn read_entire_and_close<D: LocalFileDevice>(&mut self, dev: &mut D, totals: &mut LocalFileTotals) -> Vec<u8> {
        // File::size(): remember the position, seek to END, seek back.
        let pos = dev.lseek(self.handle, 0, SeekMode::Current);
        let size = dev.lseek(self.handle, 0, SeekMode::End);
        dev.lseek(self.handle, pos, SeekMode::Start);
        let file_size = if size < 0 { 0 } else { size as u32 };
        let mut buffer = vec![0u8; file_size as usize];
        let n = file_size as i32;
        self.read(dev, Some(&mut buffer), n);
        self.close_file(dev, totals);
        buffer
    }
}

impl Default for LocalFile {
    fn default() -> Self {
        LocalFile::new()
    }
}
