//! UNTESTED DRAFT (#202, #208, #214): compiles; not yet tested against the upstream C++.
//! Differential-tested in part (#215, tests/differential_draft_language_filter.rs): init, filter_line, un_haxor and read_word match the upstream C++ on 200,000 words, 272,539 words read, 4,000 files and 80,000 lines. Not covered: new, drop_filter, reset (calls init), update, create_language_filter (one-line bodies); a file of odd length whose last word starts on the last byte, and a file that starts with a separator (undefined upstream: reads an uninitialised buffer).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameClient/LanguageFilter.h
//!   GeneralsMD/Code/GameEngine/Source/GameClient/LanguageFilter.cpp
//!
//! The chat bad-word filter: `langdata.dat` holds the words as UTF-16 units XOR `0x5555`, separated by a space unit; a line's
//! tokens that match (after "un-haxoring" `ph` to `f`, `1` to `l`, `@` to `a` ... and dropping `-_*'"`) are overwritten with `*`.
//! The map is ordered and compared by `UnicodeString::compareNoCase` (`wcsicmp`); here a case-folded key.

use crate::logic::modules::plant;
use std::collections::BTreeMap;

/// `LANGUAGE_XOR_KEY`.
pub const LANGUAGE_XOR_KEY: u16 = 0x5555;
/// `BadWordFileName`.
pub const BAD_WORD_FILE_NAME: &str = "langdata.dat";
/// `WEOF`.
const WEOF: u16 = 0xFFFF;
/// `ignoredChars[]`: `L"-_*'\""`.
const IGNORED_CHARS: [u16; 5] = [b'-' as u16, b'_' as u16, b'*' as u16, b'\'' as u16, b'"' as u16];
/// The delimiter set of `filterLine`'s `nextToken`.
const TOKEN_SEPARATORS: &str = " ;,.!?:=\\/><`~()&^%#\n\t";
/// The same set with `~` missing (planted 11489).
const TOKEN_SEPARATORS_PLANT: &str = " ;,.!?:=\\/><`()&^%#\n\t";

/// What `LanguageFilter::init` reads: the bad word file's bytes (`TheFileSystem->openFile(BadWordFileName, READ | BINARY)`), `None`
/// when it cannot be opened.
pub trait BadWordFileHost {
    /// The whole file.
    fn open_bad_word_file(&mut self) -> Option<Vec<u8>>;
}

/// `towlower` on one UTF-16 unit in the C locale, which changes only `A` to `Z` (what `_wcsicmp` folds by; see
/// `differential/shim_unicode/unicode_prelude.h`, and the verified `UnicodeString::compareNoCase`).
/// ~~(BMP letters by Rust's Unicode lower-casing)~~ **Corrected 2026-11-18 (#215):** the C locale folds ASCII only.
fn fold(c: u16) -> u16 {
    if plant(11484) {
        c
    } else if (b'A' as u16..=b'Z' as u16).contains(&c) {
        c + 32
    } else {
        c
    }
}

/// The map key: the word folded for `compareNoCase`.
fn key_of(word: &[u16]) -> Vec<u16> {
    word.iter().map(|&c| fold(c)).collect()
}

/// `UnicodeString::nextToken(&token, delimiters)` on a slice: skips leading delimiters; the token runs to the next delimiter; `pos` is
/// left at that delimiter. FALSE (token empty) when nothing is left.
fn next_token(s: &[u16], pos: &mut usize, seps: &[u16]) -> Option<Vec<u16>> {
    let mut start = *pos;
    while start < s.len() && seps.contains(&s[start]) {
        start += 1;
    }
    if start >= s.len() {
        *pos = s.len();
        return None;
    }
    let mut end = start;
    while end < s.len() && !seps.contains(&s[end]) {
        end += 1;
    }
    *pos = end;
    Some(s[start..end].to_vec())
}

/// `class LanguageFilter : public SubsystemInterface`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LanguageFilter {
    /// `m_wordList`: the bad words (un-haxored), keyed case-insensitively.
    pub word_list: BTreeMap<Vec<u16>, bool>,
}

impl LanguageFilter {
    /// `LanguageFilter::LanguageFilter()` (the body is empty: "Modified by Saad, Unnecessary").
    pub fn new() -> LanguageFilter {
        LanguageFilter { word_list: BTreeMap::new() }
    }

    /// `~LanguageFilter()`: `m_wordList.clear()`.
    pub fn drop_filter(&mut self) {
        self.word_list.clear();
    }

    /// `LanguageFilter::init()`: clears the list, reads the file word by word (`readWord` into a 128-unit buffer that is NOT cleared between
    /// words), skips words whose buffer starts with a NUL, XORs each unit with the key IN PLACE, un-haxors, and records it. No file: the
    /// list stays empty. A file that starts with the separator reads the buffer uninitialised upstream (excluded from the test).
    pub fn init<H: BadWordFileHost>(&mut self, host: &mut H) {
        self.word_list.clear();
        // read in the file already.
        let file = match host.open_bad_word_file() {
            Some(f) => f,
            None => return,
        };
        let mut pos = 0usize;
        let mut word = [0u16; 128];
        while Self::read_word(&file, &mut pos, &mut word) {
            // Int wordLen = wcslen(word);
            let word_len = word.iter().position(|&c| c == 0).unwrap_or(128);
            if word_len == 0 && !plant(11485) {
                continue;
            }
            // for (Int i = 0; i < wordLen; ++i) word[i] = word[i] ^ LANGUAGE_XOR_KEY;  -- IN PLACE: `word` is reused by the next readWord, which
            // writes no terminator when the word's first unit is the separator (an empty word: two spaces in a row), so that word
            // is that space plus whatever the previous word left in `word` (already decoded, here decoded again).
            for c in word[..word_len].iter_mut() {
                *c = if plant(11486) { *c ^ (LANGUAGE_XOR_KEY & 0xFF) } else { *c ^ LANGUAGE_XOR_KEY };
            }
            let mut uniword: Vec<u16> = word[..word_len].to_vec();
            Self::un_haxor(&mut uniword);
            self.word_list.insert(key_of(&uniword), true);
        }
        // file1->close()
    }

    /// `LanguageFilter::reset()`: `init()` again.
    pub fn reset<H: BadWordFileHost>(&mut self, host: &mut H) {
        self.init(host);
    }

    /// `LanguageFilter::update()`: empty.
    pub fn update(&mut self) {}

    /// `LanguageFilter::filterLine(line)`: for each token of the line (separators `TOKEN_SEPARATORS`), found in a copy of the line by
    /// `wcsstr` (first occurrence from the start), measure the ORIGINAL token's length, un-haxor the token and, if it is in the list,
    /// overwrite that many units at the found place with `*`. The line is then replaced by the copy.
    pub fn filter_line(&self, line: &mut Vec<u16>) {
        let mut buf = line.clone();
        let seps: Vec<u16> = if plant(11489) { TOKEN_SEPARATORS_PLANT } else { TOKEN_SEPARATORS }.encode_utf16().collect();
        let mut p = 0usize;
        while let Some(mut token) = next_token(line, &mut p, &seps) {
            // wchar_t *pos = wcsstr(buf, token.str());
            let hay: &[u16] = if plant(11483) { line.as_slice() } else { buf.as_slice() };
            let found = if token.is_empty() { Some(0) } else { hay.windows(token.len()).position(|w| w == token.as_slice()) };
            let pos = match found {
                Some(p) => p,
                None => {
                    // DEBUG_CRASH(("Couldn't find the token in its own string."));
                    continue;
                }
            };
            let mut len = token.len(); // need to get the length of the original word, not the unhaxor'd word.
            Self::un_haxor(&mut token);
            if plant(11482) {
                len = token.len();
            }
            if self.word_list.contains_key(&key_of(&token)) {
                // DEBUG_LOG(("Found word %ls in bad word list. ...
                for i in 0..len {
                    if pos + i < buf.len() {
                        buf[pos + i] = b'*' as u16;
                    }
                }
            }
        }
        *line = buf;
    }

    /// `LanguageFilter::unHaxor(word)`: `ph`/`Ph` (either case of both) becomes `f` (and skips the `h`); `1 3 4 5 6 7 0 @ $ +` become
    /// `l e a s b t o a s t`; the ignored characters `-_*'"` are dropped; anything else is kept.
    pub fn un_haxor(word: &mut Vec<u16>) {
        let len = word.len();
        let mut new_word: Vec<u16> = Vec::new();
        let mut i = 0usize;
        while i < len {
            let c = word[i];
            if c == b'p' as u16 || (c == b'P' as u16 && !plant(11481)) {
                if i + 1 < len && (word[i + 1] == b'h' as u16 || word[i + 1] == b'H' as u16) {
                    new_word.push(b'f' as u16);
                    if !plant(11477) {
                        i += 1; // skip the h
                    }
                } else {
                    // not a problem at all.
                    new_word.push(c);
                }
            } else if c == b'1' as u16 {
                new_word.push(b'l' as u16);
            } else if c == b'3' as u16 {
                new_word.push(b'e' as u16);
            } else if c == b'4' as u16 {
                new_word.push(b'a' as u16);
            } else if c == b'5' as u16 {
                new_word.push(b's' as u16);
            } else if c == b'6' as u16 {
                new_word.push(b'b' as u16);
            } else if c == b'7' as u16 {
                new_word.push(b't' as u16);
            } else if c == b'0' as u16 {
                new_word.push(b'o' as u16);
            } else if c == b'@' as u16 && !plant(11478) {
                new_word.push(b'a' as u16);
            } else if c == b'$' as u16 {
                new_word.push(b's' as u16);
            } else if c == b'+' as u16 && !plant(11479) {
                new_word.push(b't' as u16);
            } else if !(if plant(11480) { &IGNORED_CHARS[..4] } else { &IGNORED_CHARS[..] }).contains(&c) {
                // wcsrchr(ignoredChars, c) == NULL (note: wcsrchr also finds the terminating NUL when c == 0)
                new_word.push(c);
            }
            i += 1;
        }
        *word = new_word;
    }

    /// `LanguageFilter::readWord(file, buf)`: "returning true means that there are more words in the file." Reads UTF-16 units until a
    /// space unit or the end of the file; the word is NUL-terminated in `buf`. At the end of the file with nothing read: `buf[0] = 0`,
    /// FALSE. A word that ends at the end of the file is returned with FALSE. Upstream never checks the 128-unit buffer's end; an
    /// overlong word is a buffer overrun there: stored here only while it fits (the extra units are read and dropped).
    pub fn read_word(file: &[u8], pos: &mut usize, buf: &mut [u16; 128]) -> bool {
        // `UnsignedShort c;` is read by `file1->read(&c, sizeof(UnsignedShort))`, which copies the bytes that remain (RAMFile::read
        // and LocalFile::read return how many) and returns that count: with ONE byte left, the low byte of `c` is replaced
        // and the high byte keeps what `c` held. `c` starts uninitialised: a first read with one byte left reads garbage
        // there (excluded from the test; 0 here).
        let read_unit = |pos: &mut usize, c: &mut u16| -> i32 {
            let left = file.len().saturating_sub(*pos);
            let n = left.min(2);
            let mut bytes = c.to_le_bytes();
            bytes[..n].copy_from_slice(&file[*pos..*pos + n]);
            *c = u16::from_le_bytes(bytes);
            *pos += n;
            n as i32
        };
        let mut index = 0usize;
        let mut retval = true;
        let mut c: u16 = 0;
        let val = read_unit(pos, &mut c);
        if val == -1 || val == 0 {
            buf[index] = 0;
            return false;
        }
        let put = |buf: &mut [u16; 128], i: usize, v: u16| {
            if i < 128 {
                buf[i] = v;
            }
        };
        put(buf, index, c);
        while c != b' ' as u16 {
            index += 1;
            let val = read_unit(pos, &mut c);
            let eof = val == -1 || val == 0;
            if eof {
                c = WEOF;
            }
            // (planted 11487: only the end of the file, not a data unit equal to WEOF, ends the word)
            let is_weof = if plant(11487) { eof } else { c == WEOF };
            if is_weof || c == b' ' as u16 {
                put(buf, index, 0);
                if is_weof && !plant(11488) {
                    retval = false;
                }
                break;
            }
            put(buf, index, c);
        }
        retval
    }
}

/// `createLanguageFilter()`: `NEW LanguageFilter`.
pub fn create_language_filter() -> LanguageFilter {
    LanguageFilter::new()
}
