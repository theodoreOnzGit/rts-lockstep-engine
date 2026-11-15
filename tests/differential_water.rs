#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Water.cpp and GeneralsMD/Code/GameEngine/Include/GameClient/Water.h
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIWater.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp (GameLogic::reset's water transparency lines)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #57: the water's settings and a map's `WaterTransparency` override (`client::water`) against Water.cpp,
//! INIWater.cpp and `GameLogic::reset`'s two lines compiled verbatim, with the real `Water.h`, `Overridable.h` and
//! `Override.h`, under the real `INI::load` (`differential/wrap_water.cpp`, wrap_ini.cpp's `ref_ini_load_typed`).
//!
//! # Methodology
//!
//! 20,000 sessions (`WATER_CASES`) of 1 to 6 steps from fresh settings: a load of a random water INI (two in three; the
//! first of a session as the install's, `INI_LOAD_OVERWRITE`; later ones as a map's, `INI_LOAD_CREATE_OVERRIDES`, two
//! times in three), or `GameLogic::reset`'s clean-up. A file holds 1 to 3 blocks: `WaterSet` with a time of day of six
//! names (one, `NOON`, unknown; case varied) or `WaterTransparency`, each with 0 to 6 of its fields (colours, reals,
//! integers, booleans and texture names), now and then an unknown field (1 in 40) or a value that does not parse (1 in
//! 40), and one file in 60 an unknown block. Compared after each step: the outcome of the load (status and exception
//! text, and the CRC of the lines read) or of the reset (done, or undefined with no setting), then every member of the
//! five `WaterSetting`s, the chain of transparency settings from the original (its length and each setting's members)
//! and every `replaceSkyboxTextures` call (the ten names). A second override before a reset is undefined upstream (see
//! `client::water`): the Rust's `EndlessLoop` must meet the wrapper's flag, and the session stops there (counted).
//!
//! # Results
//!
//! See `tests/differential_water.rs`'s entry in `docs/phase2-client.md` (GitHub #57, 2026-11-14).
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 10303 | a second override is made without the cycle (the undefined case taken as sound) |
//! | 10304 | an override is not marked as one (`markAsOverride`), so the reset keeps it |
//! | 10305 | the skybox replacement is asked for on an override load that made no override |
//! | 10306 | the reset keeps the overrides |
//! | 10307 | a `WaterSet`'s `WaterRepeatCount` is not stored |

mod support;

use rts_lockstep_engine::client::water::{WaterSettings, WaterTransparencySetting, WATER_BLOCKS};
use rts_lockstep_engine::common::ini::{BlockParse, BlockParseFn, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, Undefined, TYPE_TABLE};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use std::ffi::CString;
use support::{Check, Gen};

fn h_stub(_: &mut Ini<'_>, _: &mut WaterSettings) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// `theTypeTable`: the water's two rows are `client::water`'s, the others throw `ERROR_BUG` as wrap_ini.cpp's stubs.
fn blocks() -> Vec<BlockParse<WaterSettings>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse { token, parse: WATER_BLOCKS.iter().find(|b| b.token == token).map_or(h_stub as BlockParseFn<WaterSettings>, |b| b.parse) })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    detail: Vec<u8>,
    crc: u32,
}

/// What the Rust load gave, as upstream's throw; `Err` where upstream is undefined.
fn rust_outcome(r: IniResult<()>, crc: u32) -> Result<Outcome, IniErrorKind> {
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown().ok_or(e.kind.clone())? {
            Thrown::Exception(m) => (1, m),
            Thrown::Status(c) => (2, c.to_le_bytes().to_vec()),
            Thrown::ErrorCode(c) => (3, c.to_le_bytes().to_vec()),
            Thrown::OutOfRange => (4, Vec::new()),
            Thrown::Other => (5, Vec::new()),
        },
    };
    Ok(Outcome { status, detail, crc })
}

fn cpp_load(name: &[u8], data: &[u8], load_type: IniLoadType) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut msg = vec![0u8; 4096];
    let mut log = vec![0u8; 1 << 16];
    let (mut crc, mut code, mut log_len) = (0u32, 0u32, 0i32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_ini_load_typed(cname.as_ptr(), load_type as i32, 1, log.as_mut_ptr(), log.len() as i32, &mut log_len, &mut crc, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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
fn r(v: &mut Vec<u8>, x: f32) {
    u(v, x.to_bits());
}
fn s(v: &mut Vec<u8>, x: &[u8]) {
    u(v, x.len() as u32);
    v.extend_from_slice(x);
}

fn transparency(v: &mut Vec<u8>, t: &WaterTransparencySetting) {
    r(v, t.transparent_water_depth);
    r(v, t.min_water_opacity);
    for c in [&t.standing_water_color, &t.radar_color] {
        r(v, c.red);
        r(v, c.green);
        r(v, c.blue);
    }
    u(v, u32::from(t.additive_blend));
    s(v, &t.standing_water_texture);
    for x in [&t.skybox_texture_n, &t.skybox_texture_e, &t.skybox_texture_s, &t.skybox_texture_w, &t.skybox_texture_t] {
        s(v, x);
    }
}

/// wrap_water.cpp's dump of the Rust.
fn rust_dump(w: &WaterSettings) -> Vec<u8> {
    let mut v = Vec::new();
    for x in &w.settings {
        s(&mut v, &x.sky_texture_file);
        s(&mut v, &x.water_texture_file);
        u(&mut v, x.water_repeat_count as u32);
        r(&mut v, x.sky_texels_per_unit);
        for c in [&x.vertex00_diffuse, &x.vertex10_diffuse, &x.vertex11_diffuse, &x.vertex01_diffuse, &x.water_diffuse_color, &x.transparent_water_diffuse] {
            u(&mut v, c.red);
            u(&mut v, c.green);
            u(&mut v, c.blue);
            u(&mut v, c.alpha);
        }
        r(&mut v, x.u_scroll_per_ms);
        r(&mut v, x.v_scroll_per_ms);
    }
    let mut chain = Vec::new();
    let mut o = w.transparency.as_deref();
    while let Some(n) = o {
        chain.push(&n.data);
        o = n.get_next_override();
    }
    u(&mut v, chain.len() as u32);
    for t in chain {
        transparency(&mut v, t);
    }
    u(&mut v, w.skybox_replacements.len() as u32);
    for (old, new) in &w.skybox_replacements {
        for x in old.iter().chain(new.iter()) {
            s(&mut v, x);
        }
    }
    v
}

fn cpp_dump() -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 18];
    let n = unsafe { cpp::ref_water_dump(buf.as_mut_ptr(), buf.len() as i32) } as usize;
    assert!(n <= buf.len());
    buf.truncate(n);
    buf
}

fn pick<'a>(g: &mut Gen, l: &[&'a str]) -> &'a str {
    l[g.below(l.len() as u32) as usize]
}

fn real(g: &mut Gen) -> String {
    match g.below(6) {
        0 => format!("{}", g.below(10)),
        1 => format!("{:.2}", g.uniform(-5.0, 5.0)),
        2 => format!("{}", g.uniform(0.0, 1.0)),
        3 => format!("-{}.5", g.below(100)),
        4 => format!("{}", g.i32() % 100_000),
        _ => format!("{:.6}", g.uniform(-0.01, 0.01)),
    }
}

fn byte(g: &mut Gen) -> u32 {
    if g.below(10) == 0 { g.below(300) } else { g.below(256) }
}

fn texture(g: &mut Gen) -> String {
    pick(g, &["TWWater01.tga", "MapN.tga", "TSMorningE.tga", "sky_s.tga", "TSNightT.tga", "x"]).to_string()
}

/// One field line of a block, or a bad one now and then.
fn field(g: &mut Gen, transparency: bool) -> String {
    if g.below(40) == 0 {
        return "  NoSuchField = 1\n".to_string();
    }
    if g.below(40) == 0 {
        let name = if transparency { "TransparentWaterDepth" } else { "WaterRepeatCount" };
        return format!("  {name} = {}\n", pick(g, &["abc", "", "R:1"]));
    }
    let rgb = |g: &mut Gen| format!("R:{} G:{} B:{}", byte(g), byte(g), byte(g));
    let rgba = |g: &mut Gen| format!("R:{} G:{} B:{} A:{}", byte(g), byte(g), byte(g), byte(g));
    if transparency {
        match g.below(11) {
            0 => format!("  TransparentWaterDepth = {}\n", real(g)),
            1 => format!("  TransparentWaterMinOpacity = {}\n", real(g)),
            2 => format!("  StandingWaterColor = {}\n", rgb(g)),
            3 => format!("  StandingWaterTexture = {}\n", texture(g)),
            4 => format!("  AdditiveBlending = {}\n", pick(g, &["Yes", "No", "yes", "NO"])),
            5 | 6 => format!("  RadarWaterColor = {}\n", rgb(g)),
            7 => format!("  SkyboxTextureN = {}\n", texture(g)),
            8 => format!("  SkyboxTextureE = {}\n", texture(g)),
            9 => format!("  SkyboxTexture{} = {}\n", pick(g, &["S", "W"]), texture(g)),
            _ => format!("  SkyboxTextureT = {}\n", texture(g)),
        }
    } else {
        match g.below(12) {
            0 => format!("  SkyTexture = {}\n", texture(g)),
            1 => format!("  WaterTexture = {}\n", texture(g)),
            2 => format!("  Vertex00Color = {}\n", rgba(g)),
            3 => format!("  Vertex10Color = {}\n", rgba(g)),
            4 => format!("  Vertex01Color = {}\n", rgba(g)),
            5 => format!("  Vertex11Color = {}\n", rgba(g)),
            6 => format!("  DiffuseColor = {}\n", rgba(g)),
            7 => format!("  TransparentDiffuseColor = {}\n", rgba(g)),
            8 => format!("  UScrollPerMS = {}\n", real(g)),
            9 => format!("  VScrollPerMS = {}\n", real(g)),
            10 => format!("  SkyTexelsPerUnit = {}\n", real(g)),
            _ => format!("  WaterRepeatCount = {}\n", g.i32() % 1000),
        }
    }
}

fn file(g: &mut Gen) -> Vec<u8> {
    let mut t = String::new();
    for _ in 0..1 + g.below(3) {
        let transparency = g.below(2) == 0;
        if transparency {
            t.push_str("WaterTransparency\n");
        } else {
            let name = pick(g, &["MORNING", "Afternoon", "evening", "NIGHT", "NONE", "NOON"]);
            t.push_str(&format!("WaterSet {name}\n"));
        }
        for _ in 0..g.below(7) {
            t.push_str(&field(g, transparency));
        }
        t.push_str("End\n");
        if g.below(60) == 0 {
            t.push_str("Bogus\nEnd\n");
        }
    }
    t.into_bytes()
}

#[test]
fn water_ini_and_its_overrides_match_upstream() {
    let sessions: u64 = std::env::var("WATER_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let table = blocks();
    let mut outcome = Check::new("Water INI: the step's outcome");
    let mut members = Check::new("Water INI: the settings and the chain");
    let (mut loads, mut overrides, mut threw, mut resets, mut stopped, mut skybox) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for session in 0..sessions {
        let mut g = Gen::new(0x57_0057 + session);
        unsafe { cpp::ref_water_reset() };
        let mut w = WaterSettings::default();
        members.eq(rust_dump(&w), cpp_dump(), || format!("session {session}: the constructors"));
        for step in 0..1 + g.below(6) {
            let tag;
            if step > 0 && g.below(3) == 0 {
                resets += 1;
                let rr = w.delete_overrides();
                let c = unsafe { cpp::ref_water_delete_overrides() };
                let rs = match rr {
                    Ok(()) => 0,
                    Err(e) if matches!(e.kind, IniErrorKind::Undefined(Undefined::NullObject)) => -1,
                    Err(e) => panic!("the reset: {e:?}"),
                };
                outcome.eq(rs, c, || format!("session {session} step {step}: the reset"));
                tag = format!("session {session} step {step}: the reset");
            } else {
                let data = file(&mut g);
                let load_type = if step > 0 && g.below(3) != 0 { IniLoadType::CreateOverrides } else { IniLoadType::Overwrite };
                let name = format!("Maps\\M\\map{step}.ini").into_bytes();
                let mut crc = XferCrc::new_without_nan_guard();
                let before = w.skybox_replacements.len();
                let rr = Ini::load(&name, &data, load_type, Some(&mut crc as &mut dyn Xfer), &table, &mut w);
                match rust_outcome(rr, crc.get_crc()) {
                    Ok(rust) => {
                        loads += 1;
                        overrides += u64::from(load_type == IniLoadType::CreateOverrides);
                        threw += u64::from(rust.status != 0);
                        skybox += (w.skybox_replacements.len() - before) as u64;
                        let c = cpp_load(&name, &data, load_type);
                        let u = unsafe { cpp::ref_water_undefined() };
                        outcome.eq((&rust, 0), (&c, u), || format!("session {session} step {step} ({load_type:?}): {}", String::from_utf8_lossy(&data)));
                    }
                    Err(IniErrorKind::Undefined(Undefined::EndlessLoop)) => {
                        // the second override's cycle: the wrapper must have stopped at it too
                        let _ = cpp_load(&name, &data, load_type);
                        let u = unsafe { cpp::ref_water_undefined() };
                        outcome.eq(1, u, || format!("session {session} step {step}: the cycle: {}", String::from_utf8_lossy(&data)));
                        stopped += 1;
                        break;
                    }
                    Err(other) => {
                        // undefined upstream for another reason: the session stops (counted)
                        println!("session {session} step {step}: undefined {other:?}");
                        stopped += 1;
                        break;
                    }
                }
                tag = format!("session {session} step {step} ({load_type:?}): {}", String::from_utf8_lossy(&data));
            }
            members.eq(rust_dump(&w), cpp_dump(), || tag);
        }
    }
    println!(
        "{sessions} sessions: {loads} loads ({overrides} as overrides, {threw} threw), {resets} resets, {skybox} skybox replacements, {stopped} sessions stopped where upstream is undefined"
    );
    if planted {
        let n = outcome.report() + members.report();
        println!("PLANT caught by {n} comparisons");
    } else {
        outcome.finish();
        members.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}
