//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 200,000 calls in one sequence (test: tests/differential_draft_encrypt.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/Common/encrypt.h
//!   GeneralsMD/Code/GameEngine/Source/Common/System/encrypt.cpp
//!
//! "Ancient Westwood Online password encryption (obfuscation?) code."
//!
//! Also here (small files, one question each):
//! - `System/Snapshot.cpp`: `Snapshot::Snapshot()` and `~Snapshot()` are empty (the destructor's body is a comment); the port's
//!   `Snapshot` trait in [`crate::common::xfer`] carries the interface. Nothing to translate.
//! - `System/CriticalSection.cpp`: five global lock pointers and `TheAsciiStringCriticalSection`; the port is
//!   single-threaded in its logic, so there is nothing to translate.
//! - `System/Compression.cpp`: its only code is `DoCompressTest` under `#ifdef TEST_COMPRESSION` (never defined). Not translated.
//! - `System/Directory.cpp`: the whole file is inside `#if (0)`. Not translated.

use crate::logic::modules::plant;

/// `MAX_ENCRYPTED_STRING`.
pub const MAX_ENCRYPTED_STRING: usize = 8;
/// `Base_String`, 64 characters (`MAX_CHARS` is 65 with the NUL).
const BASE_STRING: &[u8; 64] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789./";

/// `Temp_Buffer` and `Return_Buffer`: the two `static` buffers (`MAX_ENCRYPTED_STRING + 1` each). `Temp_Buffer` keeps what an earlier,
/// longer call left past a shorter string's length: the state is part of upstream's behaviour, so it is held here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EncryptState {
    /// `Temp_Buffer`.
    temp_buffer: [u8; MAX_ENCRYPTED_STRING + 1],
    /// `Return_Buffer`.
    return_buffer: [u8; MAX_ENCRYPTED_STRING + 1],
}

impl EncryptState {
    /// Zeroed, as C++ statics are.
    pub fn new() -> EncryptState {
        EncryptState::default()
    }

    /// `EncryptString(String)`: "a simple one-way encryption that is portable across many platforms". The input is up to 8 chars; the
    /// length is capped at 8. For each `UpCnt` below `Length` (with `DnCnt` counting down from `Length`, so the first partner is the NUL
    /// at `String[Length]`): an odd byte is `(c << 1) & String[DnCnt]`, an even byte `(c << 0) ^ String[DnCnt]`, stored as a `char`.
    /// Then each of the 8 output characters is `Base_String[Temp_Buffer[i] & 0x3F]`. Returns the 8 output characters.
    pub fn encrypt_string(&mut self, string: &[u8]) -> [u8; MAX_ENCRYPTED_STRING] {
        // unsigned int Length = strlen(String);
        let mut length = string.iter().position(|&b| b == 0).unwrap_or(string.len());
        if length > MAX_ENCRYPTED_STRING - if plant(11461) { 1 } else { 0 } {
            length = MAX_ENCRYPTED_STRING - if plant(11461) { 1 } else { 0 };
        }
        let at = |i: usize| -> i32 { string.get(i).copied().unwrap_or(0) as i8 as i32 }; // `char` is signed in MSVC.
        if plant(11467) {
            // a fresh Temp_Buffer per call instead of upstream's static
            self.temp_buffer = [0; MAX_ENCRYPTED_STRING + 1];
        }
        let mut dn_cnt = if plant(11463) { length.saturating_sub(1) } else { length };
        for up_cnt in 0..length {
            let c = at(up_cnt);
            let d = at(dn_cnt);
            let v = if plant(11464) {
                if c & 0x01 == 0 { (c << (c & 0x01)) & d } else { (c << (c & 0x01)) ^ d }
            } else if plant(11465) {
                if c & 0x01 != 0 { (c << (c & 0x01)) & d } else { (c << 1) ^ d }
            } else if c & 0x01 != 0 {
                (c << (c & 0x01)) & d
            } else {
                (c << (c & 0x01)) ^ d
            };
            self.temp_buffer[up_cnt] = v as u8;
            dn_cnt -= 1; // DnCnt-- (reaches 0 only after the last pass)
        }
        for cnt in 0..(if plant(11462) { length } else { MAX_ENCRYPTED_STRING }) {
            self.return_buffer[cnt] = BASE_STRING[if plant(11466) { (self.temp_buffer[cnt] % 63) as usize } else { (self.temp_buffer[cnt] & 0x3F) as usize }];
        }
        self.return_buffer[MAX_ENCRYPTED_STRING] = 0;
        let mut out = [0u8; MAX_ENCRYPTED_STRING];
        out.copy_from_slice(&self.return_buffer[..MAX_ENCRYPTED_STRING]);
        out
    }
}
