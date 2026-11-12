#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GlobalLanguage.cpp (the field table, INI::parseLanguageDefinition,
//     GlobalLanguage::GlobalLanguage, parseFontDesc, parseFontFileName, adjustFontSize, FontDesc::FontDesc)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::global_language` (GitHub #148) against GlobalLanguage.cpp, compiled with the real GlobalLanguage.h and
//! FontDesc.h (`differential/wrap_global_language.cpp`), under the real `INI::load` (wrap_ini.cpp's `ref_ini_load_typed`,
//! whose `Language` block reaches the cut handler).
//!
//! # Method
//!
//! A **session** is a fresh `TheGlobalLanguageData` (the constructor's members) at a display width, then 1 to 3 random
//! `Language.ini`-like files loaded with `INI_LOAD_OVERWRITE`: `Language` blocks with every field of the table (the 18
//! fonts with quoted, unquoted and spaced names, sizes good and bad, `Yes`/`No`/other bold tokens, a token missing;
//! `LocalFontFile`s; `ResolutionFontAdjustment` from 0 to 2.5, negative, or not a number; the integers and the boolean),
//! unknown fields, a missing `End`, other block types. After every file, compared: what `INI::load` throws (its kind and
//! message or code), the load's `XferCRC`, and every member of `TheGlobalLanguageData`. At the session's end,
//! `adjustFontSize` of sizes from -5 to 80 (and a random one) at the session's display width (from 1 to 4000 pixels, and
//! 640, 800, 1024, 1600, 1920, a phone's 390).
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches: the parse is the shared `INI` machinery (differential-tested) under one more field table, and
//!   `adjustFontSize` is `float` arithmetic and `REAL_TO_INT_FLOOR`, already compared by #146's test.
//! - Plants 9401 (the constructor's adjustment 1.0) and 9405 (no lower clamp) by every session's `adjustFontSize` at a
//!   width under 800 (9405) or any adjustment left at its default (9401); 9402 (`ResolutionFontAdjustment` not stored), 9403
//!   (a font's bold never read: the token is left for the next field, which then fails or differs) and 9404 (the local
//!   fonts appended, not put in front) by the files with those fields (9404 needs two `LocalFontFile`s in a session).
//!
//! # Results (2026-11-12, `GLOBAL_LANGUAGE_CASES=20000`, the default)
//!
//! - **First run: prediction missed.** 19,641 of 60,118 member dumps differed, every one after a font field whose size
//!   or bold token failed: upstream's `parseFontDesc` stores the name (and the size) **before** the next token throws,
//!   and the port built the whole `FontDesc` and stored it only at the end. Fixed in the port (each member stored as it is
//!   read, `client::global_language::parse_font_desc`); the outcome and `adjustFontSize` had 0 mismatches.
//! - Second run (the generator also made to throw less often: unknown fields, bad sizes and bold tokens rarer): 20,000
//!   sessions, 40,118 loads (22,535 threw), none stopped where upstream is undefined; 1,740,000 `adjustFontSize` calls
//!   (986,759 changed the size): **0 mismatches** in the outcome, the members and `adjustFontSize`.
//! - Plants (mismatching comparisons): 9401 members 38,208, `adjustFontSize` 285,795; 9402 members 21,910, adjust
//!   477,047; 9403 outcome 4,664, members 26,450, adjust 43,573; 9404 members 8,208; 9405 adjust 528,556. **5 of 5
//!   caught**, as predicted.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 9401 | the constructor's `m_resolutionFontSizeAdjustment` is 1.0 |
//! | 9402 | `ResolutionFontAdjustment` is parsed but not stored |
//! | 9403 | `parseFontDesc` does not read the bold token |
//! | 9404 | `LocalFontFile` is appended (`push_back`), not put at the front |
//! | 9405 | `adjustFontSize` does not raise a factor under 1 to 1 |

mod support;

use rts_lockstep_engine::client::global_language::{FontDesc, GlobalLanguage, GLOBAL_LANGUAGE_FIELDS};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use std::ffi::CString;
use support::{Check, Gen};

fn h_language(ini: &mut Ini<'_>, g: &mut GlobalLanguage) -> IniResult<()> {
    ini.init_from_ini(g, GLOBAL_LANGUAGE_FIELDS)
}
fn h_stub(_: &mut Ini<'_>, _: &mut GlobalLanguage) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}
fn blocks() -> Vec<BlockParse<GlobalLanguage>> {
    TYPE_TABLE.iter().map(|&(token, _, _)| BlockParse { token, parse: if token == "Language" { h_language } else { h_stub } }).collect()
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    detail: Vec<u8>,
    crc: u32,
}

/// What the Rust load gave, as upstream's throw; `Err` where upstream is undefined (the session stops there, counted).
fn rust_outcome(r: IniResult<()>, crc: u32) -> Result<Outcome, String> {
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown().ok_or_else(|| format!("{:?}", e.kind))? {
            Thrown::Exception(m) => (1, m),
            Thrown::Status(c) => (2, c.to_le_bytes().to_vec()),
            Thrown::ErrorCode(c) => (3, c.to_le_bytes().to_vec()),
            Thrown::OutOfRange => (4, Vec::new()),
            Thrown::Other => (5, Vec::new()),
        },
    };
    Ok(Outcome { status, detail, crc })
}

fn cpp_load(name: &[u8], data: &[u8]) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut msg = vec![0u8; 4096];
    let mut log = vec![0u8; 1 << 16];
    let (mut crc, mut code, mut log_len) = (0u32, 0u32, 0i32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_ini_load_typed(cname.as_ptr(), IniLoadType::Overwrite as i32, 1, log.as_mut_ptr(), log.len() as i32, &mut log_len, &mut crc, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
    };
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail, crc }
}

fn u(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn s(v: &mut Vec<u8>, x: &[u8]) {
    u(v, x.len() as u32);
    v.extend_from_slice(x);
}
fn f(v: &mut Vec<u8>, x: &FontDesc) {
    s(v, &x.name);
    u(v, x.size as u32);
    u(v, u32::from(x.bold));
}

/// wrap_global_language.cpp's dump of the Rust.
fn rust_dump(g: &GlobalLanguage) -> Vec<u8> {
    let mut v = Vec::new();
    s(&mut v, &g.unicode_font_name);
    s(&mut v, &g.unicode_font_file_name);
    u(&mut v, u32::from(g.use_hard_wrap));
    u(&mut v, g.military_caption_speed as u32);
    u(&mut v, g.military_caption_delay_ms as u32);
    for x in [
        &g.copyright_font,
        &g.message_font,
        &g.military_caption_title_font,
        &g.military_caption_font,
        &g.superweapon_countdown_normal_font,
        &g.superweapon_countdown_ready_font,
        &g.named_timer_countdown_normal_font,
        &g.named_timer_countdown_ready_font,
        &g.drawable_caption_font,
        &g.default_window_font,
        &g.default_display_string_font,
        &g.tooltip_font_name,
        &g.native_debug_display,
        &g.draw_group_info_font,
        &g.credits_title_font,
        &g.credits_position_font,
        &g.credits_normal_font,
    ] {
        f(&mut v, x);
    }
    u(&mut v, g.resolution_font_size_adjustment.to_bits());
    u(&mut v, g.local_fonts.len() as u32);
    for l in &g.local_fonts {
        s(&mut v, l);
    }
    v
}

fn cpp_dump() -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 16];
    let n = unsafe { cpp::ref_global_language_dump(buf.as_mut_ptr(), buf.len() as i32) } as usize;
    assert!(n <= buf.len());
    buf.truncate(n);
    buf
}

const FONTS: [&str; 17] = [
    "CopyrightFont",
    "MessageFont",
    "MilitaryCaptionTitleFont",
    "MilitaryCaptionFont",
    "SuperweaponCountdownNormalFont",
    "SuperweaponCountdownReadyFont",
    "NamedTimerCountdownNormalFont",
    "NamedTimerCountdownReadyFont",
    "DrawableCaptionFont",
    "DefaultWindowFont",
    "DefaultDisplayStringFont",
    "TooltipFontName",
    "NativeDebugDisplay",
    "DrawGroupInfoFont",
    "CreditsTitleFont",
    "CreditsMinorTitleFont",
    "CreditsNormalFont",
];
const NAMES: [&str; 6] = ["\"Arial\"", "\"Arial Unicode MS\"", "Arial", "\"Times New Roman\"", "\"\"", "\"Lucida Console\""];

fn pick<'a>(g: &mut Gen, l: &[&'a str]) -> &'a str {
    l[g.below(l.len() as u32) as usize]
}

fn field(g: &mut Gen) -> String {
    match g.below(15) {
        0..=5 => {
            let size = match g.below(25) {
                0 => "abc".to_string(),
                1..=4 => format!("{}", g.below(200) as i32 - 20),
                _ => format!("{}", 6 + g.below(30)),
            };
            let bold = match g.below(30) {
                0 => "maybe",
                1 => "",
                2..=14 => "Yes",
                _ => "No",
            };
            let size = if g.below(40) == 0 { String::new() } else { size };
            format!("{} = {} {} {}", pick(g, &FONTS), pick(g, &NAMES), size, bold)
        }
        5 => format!("LocalFontFile = {}", pick(g, &["a.ttf", "Fonts\\b.ttf", "c.ttf", "\"d e.ttf\""])),
        6 | 7 => format!(
            "ResolutionFontAdjustment = {}",
            match g.below(20) {
                0 => "x".to_string(),
                1 => format!("-{}", g.uniform(0.0, 1.0)),
                _ => format!("{}", g.uniform(0.0, 2.5)),
            }
        ),
        8 => format!("UnicodeFontName = {}", pick(g, &NAMES)),
        9 => format!("MilitaryCaptionSpeed = {}", g.below(100) as i32 - 10),
        10 => format!("MilitaryCaptionDelayMS = {}", g.below(3000)),
        11 => format!("UseHardWordWrap = {}", pick(g, &["Yes", "No", "Yes", "No", "1"])),
        12 if g.below(4) == 0 => "UnicodeFontFileName = x.ttf".to_string(),
        13 if g.below(4) == 0 => "NoSuchField = 1".to_string(),
        _ => format!("LocalFontFile = {}", pick(g, &["e.ttf", "f.ttf"])),
    }
}

fn file(g: &mut Gen) -> Vec<u8> {
    let mut t = String::new();
    for _ in 0..1 + g.below(3) {
        match g.below(30) {
            0 => t.push_str("Mouse\nEnd\n"),
            _ => {
                t.push_str("Language\n");
                for _ in 0..g.below(9) {
                    t.push_str("  ");
                    t.push_str(&field(g));
                    t.push('\n');
                }
                if g.below(60) != 0 {
                    t.push_str("End\n");
                }
            }
        }
    }
    t.into_bytes()
}

#[test]
fn language_ini_parses_as_upstream() {
    let sessions: u64 = std::env::var("GLOBAL_LANGUAGE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let table = blocks();
    let mut outcome = Check::new("Language.ini: the load's outcome");
    let mut members = Check::new("Language.ini: TheGlobalLanguageData");
    let mut adjust = Check::new("adjustFontSize");
    let (mut loads, mut threw, mut stopped, mut adjusts, mut moved) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut undefined = Vec::new();
    for session in 0..sessions {
        let mut g = Gen::new(0x148_0192 + session);
        let x_resolution = match g.below(8) {
            0 => 390,
            1 => 640,
            2 => 800,
            3 => 1024,
            4 => 1600,
            5 => 1920,
            _ => 1 + g.below(4000) as i32,
        };
        unsafe { cpp::ref_global_language_reset(x_resolution) };
        let mut lang = GlobalLanguage::default();
        members.eq(rust_dump(&lang), cpp_dump(), || format!("session {session}: the constructor"));
        let mut stopped_here = false;
        for step in 0..1 + g.below(3) {
            let data = file(&mut g);
            let name = format!("Data\\English\\Language{step}.ini").into_bytes();
            let mut crc = XferCrc::new_without_nan_guard();
            let r = Ini::load(&name, &data, IniLoadType::Overwrite, Some(&mut crc as &mut dyn Xfer), &table, &mut lang);
            let rust = match rust_outcome(r, crc.get_crc()) {
                Ok(o) => o,
                Err(kind) => {
                    if undefined.len() < 5 {
                        undefined.push(format!("session {session} step {step}: {kind}: {}", String::from_utf8_lossy(&data)));
                    }
                    stopped += 1;
                    stopped_here = true;
                    break;
                }
            };
            loads += 1;
            threw += u64::from(rust.status != 0);
            let c = cpp_load(&name, &data);
            let tag = || format!("session {session} step {step}: {}", String::from_utf8_lossy(&data));
            outcome.eq(&rust, &c, tag);
            let (rd, cd) = (rust_dump(&lang), cpp_dump());
            if rd != cd && !planted && std::env::var("GL_DEBUG").is_ok() {
                let k = rd.iter().zip(&cd).position(|(a, b)| a != b).unwrap_or(rd.len().min(cd.len()));
                println!("session {session} step {step} first byte {k}: rust {:?} c++ {:?}", &rd[k.saturating_sub(8)..(k + 24).min(rd.len())], &cd[k.saturating_sub(8)..(k + 24).min(cd.len())]);
            }
            members.eq(rd, cd, tag);
        }
        if stopped_here {
            continue;
        }
        for size in (-5..=80).chain([g.i32() % 100_000]) {
            let r = lang.adjust_font_size(size, x_resolution);
            let c = unsafe { cpp::ref_global_language_adjust(size) };
            adjusts += 1;
            moved += u64::from(r != size);
            adjust.eq(r, c, || format!("session {session}: adjustFontSize({size}) at {x_resolution}, adjustment {}", lang.resolution_font_size_adjustment));
        }
    }
    println!("{sessions} sessions: {loads} loads ({threw} threw), {stopped} stopped where upstream is undefined; {adjusts} adjustFontSize ({moved} changed the size)");
    for u in &undefined {
        println!("    undefined: {u}");
    }
    if planted {
        let n = outcome.report() + members.report() + adjust.report();
        println!("PLANT caught by {n} comparisons");
    } else {
        outcome.finish();
        members.finish();
        adjust.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}
