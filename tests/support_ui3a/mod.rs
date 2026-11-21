// SPDX-License-Identifier: GPL-3.0-only
//! Shared helpers of the group ui3a differential tests (#215): a seeded generator and the text form of a UTF-16 string.
#![allow(dead_code)]

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    pub fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

/// Text as the wrapper prints it: printable ASCII as itself, the rest (and `\`) as `\uXXXX`.
pub fn narrow(t: &[u16]) -> String {
    t.iter().map(|&c| if (0x20..0x7F).contains(&c) && c != u16::from(b'\\') { char::from(c as u8).to_string() } else { format!("\\u{c:04x}") }).collect()
}
