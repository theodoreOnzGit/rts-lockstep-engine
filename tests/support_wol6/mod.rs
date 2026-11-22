// SPDX-License-Identifier: GPL-3.0-only
//! Shared helpers of the group wol6 differential tests (#215): a seeded generator, the hex form of a UTF-16 text, and the stand-in hosts of the
//! big WOL menu drafts (`lobby_host.rs`, `game_setup_host.rs`): every method of the host trait the tests do not reach is `unreachable!`.
#![allow(dead_code)]

pub mod game_setup_host;
pub mod lobby_host;

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

/// UTF-16 units in hex, comma separated; `-` for the empty text (the wrappers' format).
pub fn hex(t: &[u16]) -> String {
    if t.is_empty() {
        return "-".to_string();
    }
    t.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}
