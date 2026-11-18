//! UNTESTED DRAFT (#202, #208): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameClient/LanguageFilter.h
//!   GeneralsMD/Code/GameEngine/Source/GameClient/LanguageFilter.cpp
//!
//! The chat bad-word filter: `langdata.dat` holds the words as UTF-16 units XOR `0x5555`, separated by a space unit; a line's
//! tokens that match (after "un-haxoring" `ph` to `f`, `1` to `l`, `@` to `a` ... and dropping `-_*'"`) are overwritten with `*`.
//! The map is ordered and compared by `UnicodeString::compareNoCase` (`wcsicmp`); here a case-folded key.

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

/// What `LanguageFilter::init` reads: the bad word file's bytes (`TheFileSystem->openFile(BadWordFileName, READ | BINARY)`), `None`
/// when it cannot be opened.
pub trait BadWordFileHost {
    /// The whole file.
    fn open_bad_word_file(&mut self) -> Option<Vec<u8>>;
}

/// `towlower` on one UTF-16 unit (BMP letters; the key `compareNoCase` compares by).
fn fold(c: u16) -> u16 {
    match char::from_u32(c as u32) {
        Some(ch) => {
            let mut l = ch.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) if (x as u32) <= 0xFFFF => x as u16,
                _ => c,
            }
        }
        None => c,
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

    /// `LanguageFilter::init()`: clears the list, reads the file word by word (`readWord` into a 128-unit buffer), skips empty words,
    /// XORs each unit with the key, un-haxors, and records it. No file: the list stays empty.
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
            if word_len == 0 {
                continue;
            }
            let mut uniword: Vec<u16> = word[..word_len].iter().map(|&c| c ^ LANGUAGE_XOR_KEY).collect();
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
        let seps: Vec<u16> = TOKEN_SEPARATORS.encode_utf16().collect();
        let mut p = 0usize;
        while let Some(mut token) = next_token(line, &mut p, &seps) {
            // wchar_t *pos = wcsstr(buf, token.str());
            let found = if token.is_empty() { Some(0) } else { buf.windows(token.len()).position(|w| w == token.as_slice()) };
            let pos = match found {
                Some(p) => p,
                None => {
                    // DEBUG_CRASH(("Couldn't find the token in its own string."));
                    continue;
                }
            };
            let len = token.len(); // need to get the length of the original word, not the unhaxor'd word.
            Self::un_haxor(&mut token);
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
            if c == b'p' as u16 || c == b'P' as u16 {
                if i + 1 < len && (word[i + 1] == b'h' as u16 || word[i + 1] == b'H' as u16) {
                    new_word.push(b'f' as u16);
                    i += 1; // skip the h
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
            } else if c == b'@' as u16 {
                new_word.push(b'a' as u16);
            } else if c == b'$' as u16 {
                new_word.push(b's' as u16);
            } else if c == b'+' as u16 {
                new_word.push(b't' as u16);
            } else if !IGNORED_CHARS.contains(&c) {
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
        let read_unit = |pos: &mut usize| -> Option<u16> {
            if *pos + 2 <= file.len() {
                let v = u16::from_le_bytes([file[*pos], file[*pos + 1]]);
                *pos += 2;
                Some(v)
            } else {
                *pos = file.len();
                None
            }
        };
        let mut index = 0usize;
        let mut retval = true;
        let mut c = match read_unit(pos) {
            Some(c) => c,
            None => {
                buf[index] = 0;
                return false;
            }
        };
        let mut put = |buf: &mut [u16; 128], i: usize, v: u16| {
            if i < 128 {
                buf[i] = v;
            }
        };
        put(buf, index, c);
        while c != b' ' as u16 {
            index += 1;
            c = match read_unit(pos) {
                Some(c) => c,
                None => WEOF,
            };
            if c == WEOF || c == b' ' as u16 {
                put(buf, index, 0);
                if c == WEOF {
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
