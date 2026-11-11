#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/HotKey.cpp (HotKeyTranslator, HotKeyManager)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Keyboard.cpp (initKeyNames, getPrintableKey)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! GitHub #131: `client::hot_key` (the control bar's hot keys) and `client::keyboard` (the printable key names) against
//! the C++.
//!
//! # Method
//!
//! The C++ side is the upstream text cut verbatim by build.rs (`differential/wrap_hot_key.cpp` lists the cuts and the one
//! token it writes, `return 0;` for `getPrintableKey`'s `return L'';`), compiled on stand-ins for the strings (the real
//! classes' meaning), the windows, the window manager, the audio and the keyboard, with the real `GameMessage`.
//!
//! - [`key_names_match_upstream`]: `initKeyNames` for each of the 10 languages under 11 keyboard layouts (US, the five
//!   French ones, German, UK, Italian, 0 and one with a French low word under other high bits): `m_shift2Key` and
//!   `getPrintableKey(key, state)` of all 256 keys at the states -1 to 3.
//! - [`hot_key_sequences_match_upstream`]: random sequences (a language, the US layout 3 times in 4, else one of those
//!   layouts), each of 1 to 24 steps: `reset`; `addHotKey` of one of 8 windows and a key of 1 to 3 bytes (letters of
//!   either case, digits, punctuation, Latin-1 bytes; repeats of a few keys weighted up so keys collide); a key message
//!   (down or up; a letter, digit or punctuation key, any byte, or a value past a byte; the up or down bit with each
//!   modifier bit a quarter of the time, and 3 times in 4 none); a window's hidden and enabled bits; `searchHotKey` of a
//!   label of 0 to 8 characters (letters, `&` a quarter of the time, Latin-1 and wide characters, a few with a zero low
//!   byte); `executeHotKey` of a key. Compared: every step's answer (disposition, key, TRUE/FALSE) and every
//!   `GBM_SELECTED` (parent, message, window, id) and sound (its name), in order.
//!
//! *Predictions, before the first run* (`issue131_progress.md`): 0 mismatches in both; every plant 9181-9189 caught.
//!
//! # Results (2026-11-11, `HOT_KEY_CASES=20000`, the default)
//!
//! - Key names: 110 language and layout pairs, **0 mismatches** (second run). **The first run had 5**: every language
//!   that reaches the UK table (UK under the 5 layouts that are not French) gave `KEY_4`'s second shift as U+0080 in
//!   the Rust (the generator read `Keyboard.cpp` as Latin-1) and U+FFFD in the C++ (build.rs read it as UTF-8). The file
//!   is Windows-1252, as MSVC compiled it: byte 0x80 is the euro sign, U+20AC. Both sides now decode it so.
//! - Hot key sequences: 20,000, **0 mismatches** (first run): 2,215 keys destroyed, 14,900 labels gave a hot key, 1,272
//!   `executeHotKey` calls answered TRUE, 2,847 sequences pressed a button.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose): **9 of 9 caught** (2026-11-11).
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 9181 | the translator acts on the key's press too | 724 sequences |
//! | 9182 | the translator ignores the modifiers | 624 |
//! | 9183 | `addHotKey` replaces a key already mapped | 423 |
//! | 9184 | no lowercasing | 588 |
//! | 9185 | a hidden window's button is pressed | 142 |
//! | 9186 | no `GUIClickDisabled` | 129 |
//! | 9187 | `searchHotKey` takes the last `&` | 6,675 |
//! | 9188 | the translator reads the key's shifted name | 532 |
//! | 9189 | the French keyboard layout ignored | 54 key-name pairs, 132 sequences |

mod support;

use rts_lockstep_engine::client::hot_key::{HotKeyHost, HotKeyManager, HotKeyTranslator};
use rts_lockstep_engine::client::keyboard::KeyNames;
use rts_lockstep_engine::client::selection_xlat::Disposition;
use rts_lockstep_engine::common::message_types::{MSG_RAW_KEY_DOWN, MSG_RAW_KEY_UP};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

fn cpp_case(kind: i32, input: &[i32]) -> Vec<i32> {
    // (the wrapper's state is global and the tests run on several threads)
    static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _held = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0i32; 1 << 14];
    let n = unsafe { cpp::ref_hot_key_case(kind, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!((n as usize) <= out.len(), "the output buffer is too small");
    out.truncate(n as usize);
    out
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

const LAYOUTS: [u32; 11] = [0x0409, 0x040c, 0x080c, 0x0c0c, 0x100c, 0x140c, 0x0407, 0x0809, 0x0410, 0, 0x0401_040c];

/// `initKeyNames` and `getPrintableKey` for every language and layout (see the module docs).
#[test]
fn key_names_match_upstream() {
    let mut c = Check::new("Keyboard key names");
    for language in 0..10 {
        for &layout in &LAYOUTS {
            let k = KeyNames::init_key_names(language, layout);
            let mut rust = vec![i32::from(k.shift2_key)];
            for key in 0..=255u8 {
                for s in -1..=3 {
                    rust.push(i32::from(k.get_printable_key(key, s)));
                }
            }
            c.eq(rust, cpp_case(0, &[language, layout as i32]), || format!("language {language} layout {layout:#x}"));
        }
    }
    if planted() {
        c.report();
    } else {
        c.finish();
    }
}

/// The C++ stand-ins' windows, in the log's words.
struct Wins {
    hidden: [bool; 8],
    enabled: [bool; 8],
    out: Vec<i32>,
}

impl HotKeyHost for Wins {
    fn win_hidden(&self, win: usize) -> bool {
        self.hidden[win]
    }
    fn win_enabled(&self, win: usize) -> bool {
        self.enabled[win]
    }
    fn send_selected(&mut self, win: usize) {
        // (parent, GBM_SELECTED, the window, its id)
        self.out.extend([2000, 100 + win as i32, 7, win as i32, 200 + win as i32]);
    }
    fn add_audio_event(&mut self, name: &[u8]) {
        self.out.push(3000);
        self.out.extend(name.iter().map(|&b| i32::from(b)));
        self.out.push(0);
    }
}

fn key_bytes(g: &mut Gen) -> Vec<u8> {
    const POOL: &[u8] = b"aAbBcCzZ09-=;&,";
    const COMMON: [&[u8]; 4] = [b"a", b"B", b"1", b"c"];
    if g.below(2) == 0 {
        return COMMON[g.below(4) as usize].to_vec();
    }
    let n = 1 + g.below(3) as usize;
    (0..n)
        .map(|_| match g.below(6) {
            0 => 0xC0 + g.below(0x3F) as u8,
            1 => 0x80 + g.below(0x40) as u8,
            _ => POOL[g.below(POOL.len() as u32) as usize],
        })
        .collect()
}

fn key_code(g: &mut Gen) -> i32 {
    // A, B, C, Z, 1, 0, minus, equals, semicolon, comma, space, F1, keypad 1, delete, a modifier
    const KEYS: [i32; 15] = [0x1E, 0x30, 0x2E, 0x2C, 0x02, 0x0B, 0x0C, 0x0D, 0x27, 0x33, 0x39, 0x3B, 0x4F, 0xD3, 0x2A];
    match g.below(10) {
        0 => g.below(256) as i32,
        1 => 0x100 + g.below(0x300) as i32,
        _ => KEYS[g.below(15) as usize],
    }
}

/// One sequence: its input words for the C++ and the Rust's output.
fn sequence(g: &mut Gen, stats: &mut [u64; 4]) -> (Vec<i32>, Vec<i32>) {
    let language = if g.below(4) == 0 { g.below(10) as i32 } else { 0 };
    let layout = if g.below(4) == 0 { LAYOUTS[g.below(11) as usize] } else { 0x0409 };
    let mut input = vec![language, layout as i32];
    let names = KeyNames::init_key_names(language, layout);
    let mut m = HotKeyManager::new();
    let x = HotKeyTranslator;
    let mut h = Wins { hidden: [false; 8], enabled: [true; 8], out: Vec::new() };
    let steps = 1 + g.below(24);
    for _ in 0..steps {
        match g.below(16) {
            0 => {
                input.push(0);
                m.reset();
                h.out.push(1000);
            }
            1..=4 => {
                let win = g.below(8) as usize;
                let k = key_bytes(g);
                input.extend([1, win as i32, k.len() as i32]);
                input.extend(k.iter().map(|&b| i32::from(b)));
                m.add_hot_key(win, &k);
                h.out.push(1001);
            }
            5..=10 => {
                let down = g.below(4) == 0;
                let key = key_code(g);
                let mut state = if down { 2 } else { 1 };
                if g.below(4) == 0 {
                    for bit in [4, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400] {
                        if g.below(4) == 0 {
                            state |= bit;
                        }
                    }
                }
                input.extend([2, i32::from(down), key, state]);
                let t = if down { MSG_RAW_KEY_DOWN } else { MSG_RAW_KEY_UP };
                let d = x.translate_game_message(t, key, state, &names, &m, &mut h);
                stats[0] += u64::from(d == Disposition::Destroy);
                h.out.extend([1002, i32::from(d == Disposition::Destroy)]);
            }
            11 | 12 => {
                let win = g.below(8) as usize;
                let (hidden, enabled) = (g.below(4) == 0, g.below(3) != 0);
                input.extend([3, win as i32, i32::from(hidden), i32::from(enabled)]);
                h.hidden[win] = hidden;
                h.enabled[win] = enabled;
                h.out.push(1003);
            }
            13 | 14 => {
                let n = g.below(9) as usize;
                let label: Vec<u16> = (0..n)
                    .map(|_| match g.below(12) {
                        0..=2 => u16::from(b'&'),
                        3 => 0xC0 + g.below(0x40) as u16,
                        4 => 0x100 * (1 + g.below(0xFE) as u16) + if g.below(2) == 0 { 0 } else { g.below(0x80) as u16 },
                        _ => u16::from(b"aBcXy19 "[g.below(8) as usize]),
                    })
                    .collect();
                input.extend([4, n as i32]);
                input.extend(label.iter().map(|&c| i32::from(c)));
                let k = HotKeyManager::search_hot_key(&label);
                stats[1] += u64::from(!k.is_empty());
                h.out.extend([1004, k.len() as i32]);
                h.out.extend(k.iter().map(|&b| i32::from(b)));
            }
            _ => {
                let k = key_bytes(g);
                input.extend([5, k.len() as i32]);
                input.extend(k.iter().map(|&b| i32::from(b)));
                let r = m.execute_hot_key(&k, &mut h);
                stats[2] += u64::from(r);
                h.out.extend([1005, i32::from(r)]);
            }
        }
    }
    stats[3] += u64::from(h.out.contains(&2000));
    (input, h.out)
}

/// `HotKeyTranslator` and `HotKeyManager` on random sequences (see the module docs).
#[test]
fn hot_key_sequences_match_upstream() {
    let cases: u64 = std::env::var("HOT_KEY_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let mut g = Gen::new(0x131_0131);
    let mut c = Check::new("HotKeyTranslator and HotKeyManager");
    let mut stats = [0u64; 4];
    for i in 0..cases {
        let (input, rust) = sequence(&mut g, &mut stats);
        c.eq(rust, cpp_case(1, &input), || format!("sequence {i} {input:?}"));
    }
    println!("hot key sequences: {} keys destroyed, {} labels with a hot key, {} executes TRUE, {} sequences pressed a button", stats[0], stats[1], stats[2], stats[3]);
    if planted() {
        c.report();
    } else {
        c.finish();
    }
    assert!(!planted(), "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}
