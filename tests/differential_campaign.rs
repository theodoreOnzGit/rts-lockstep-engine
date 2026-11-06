// SPDX-License-Identifier: GPL-3.0-only
//! Differential test of the campaign manager and the Generals' Challenge (`client::campaign_manager`,
//! `client::challenge_generals`, GitHub #61; this test GitHub #64) against CampaignManager.cpp and ChallengeGenerals.cpp,
//! compiled with the real CampaignManager.h and ChallengeGenerals.h (`differential/wrap_campaign.cpp`; the stand-ins of
//! `differential/shim_campaign` for AudioEventRTS.h, GameClient.h and GameInfo.h), under the real `INI::load`
//! (wrap_ini.cpp's `ref_ini_load_typed`, whose `Campaign` and `ChallengeGenerals` blocks reach the cut handlers).
//!
//! A **session** is a fresh `TheCampaignManager` and `TheChallengeGenerals`, then 1 to 4 files, each a `Campaign.ini`-like
//! or `ChallengeMode.ini`-like text, loaded with `INI_LOAD_OVERWRITE` (most), `INI_LOAD_MULTIFILE` or
//! `INI_LOAD_CREATE_OVERRIDES`. The files are synthetic and random: `Campaign` blocks (names repeated in other cases, which
//! replace the first), every campaign field, `Mission` blocks with every mission field (repeated missions, `NoSound`
//! briefing voices, good and bad booleans and integers), unknown fields, missing tokens and `End`s, unknown blocks;
//! `ChallengeGenerals` blocks with `GeneralPersona0` to `GeneralPersona11` (and a `GeneralPersona12`, not in the table),
//! every persona field. After every file, compared: what `INI::load` throws (its kind and message or code), the load's
//! `XferCRC`, the **whole campaign list** (every campaign's and mission's every field, in order), the current campaign and
//! mission, the victory flag, the rank points member, the difficulty, `getCurrentMissionNumber`, `getCurrentMap`, and every
//! persona's every field. At the session's end, 1 to 12 operations (`setCampaign`, `setCampaignAndMission` with known,
//! unknown, empty and other-case names, `gotoNextMission` with its result, `SetVictorious`, `setRankPoints`,
//! `setGameDifficulty`), the manager compared after each, and the three persona lookups by random names.
//!
//! Not compared: the personas' four images (`parseMappedImage`: the build has no `TheMappedImageCollection`, so upstream
//! stores nothing; the port keeps the names, its documented deviation; the token each consumes is compared through the
//! fields after it); `CampaignManager::xfer` (the save's, covered by the save tests); the operations during the loads
//! (a repeated campaign deletes the current one upstream, leaving the pointer dangling).
//!
//! **Predicted before the first run:** 0 mismatches in every comparison (the parse is the shared `INI` machinery, already
//! differential-tested, under two field tables); each of the three planted bugs caught: 4421 by the campaign list (a
//! file with a repeated mission), 4422 by the operations (`setCampaign` of an unknown name after a difficulty was set),
//! 4423 by the personas (any `StartsEnabled`).
//!
//! # Results (2026-11-06, GitHub #64; upstream commit 0a05454d, seeds `0x64_0000 + session`)
//!
//! - 20,000 sessions: 49,907 loads (13,823 threw), 129,876 operations, 59,634 lookups; **0 mismatches** in the load
//!   outcome and CRC, the campaign list and manager state, the personas, the operations and the lookups. At the sessions'
//!   ends: 45,504 campaigns, 49,406 missions, 30,695 personas set. 122 sessions stopped where upstream is undefined
//!   (`VoiceLength = 99999999999`: `scanInt`'s `sscanf` overflow, `Undefined(IntOutOfRange)`), before the C++ ran that
//!   file. As predicted, first run but for the harness (its first run expected every Rust error to be a throw, and ran a
//!   stopped session's operations against a C++ that had not loaded the file: fixed in the test).
//! - No port defect found.
//! - Planted bugs at 5,000 sessions, each caught as predicted: 4421 (a repeated mission kept) by the campaign list (2,748)
//!   and the operations (8,146); 4422 (`setCampaign`'s unknown name keeps the difficulty) by the operations (2,388); 4423
//!   (`StartsEnabled` inverted) by the personas (4,404).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::client::campaign_manager::{parse_campaign_definition, CampaignManager, MAX_DISPLAYED_UNITS, MAX_OBJECTIVE_LINES};
use rts_lockstep_engine::client::challenge_generals::{parse_challenge_mode_definition, ChallengeGenerals, GeneralPersona};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use std::ffi::CString;
use support::{Check, Gen};

const SESSIONS: u64 = 20_000;

struct Ctx {
    campaign: CampaignManager,
    challenge: ChallengeGenerals,
}

fn h_campaign(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    parse_campaign_definition(ini, &mut c.campaign)
}
fn h_challenge(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    parse_challenge_mode_definition(ini, &mut c.challenge)
}
fn h_stub(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}
fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: match token {
                "Campaign" => h_campaign,
                "ChallengeGenerals" => h_challenge,
                _ => h_stub,
            },
        })
        .collect()
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

fn cpp_load(name: &[u8], data: &[u8], load_type: IniLoadType) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut msg = vec![0u8; 4096];
    let mut log = vec![0u8; 1 << 16];
    let (mut crc, mut code, mut log_len) = (0u32, 0u32, 0i32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_ini_load_typed(
            cname.as_ptr(),
            load_type as i32,
            1,
            log.as_mut_ptr(),
            log.len() as i32,
            &mut log_len,
            &mut crc,
            msg.as_mut_ptr() as *mut _,
            msg.len() as i32,
            &mut code,
        )
    };
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail, crc }
}

// ---------------------------------------------------------------------------
// The dumps (wrap_campaign.cpp's layouts)
// ---------------------------------------------------------------------------

fn u(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn s(v: &mut Vec<u8>, x: &[u8]) {
    u(v, x.len() as u32);
    v.extend_from_slice(x);
}

fn rust_campaign_dump(m: &CampaignManager) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, m.campaign_list.len() as u32);
    for c in &m.campaign_list {
        s(&mut v, &c.name);
        s(&mut v, &c.first_mission);
        s(&mut v, &c.campaign_name_label);
        s(&mut v, &c.final_movie_name);
        u(&mut v, u32::from(c.is_challenge_campaign));
        s(&mut v, &c.player_faction_name);
        u(&mut v, c.missions.len() as u32);
        for x in &c.missions {
            s(&mut v, &x.name);
            s(&mut v, &x.map_name);
            s(&mut v, &x.next_mission);
            s(&mut v, &x.movie_label);
            for k in 0..MAX_OBJECTIVE_LINES {
                s(&mut v, &x.mission_objectives_label[k]);
            }
            s(&mut v, &x.briefing_voice.event_name);
            s(&mut v, &x.location_name_label);
            for k in 0..MAX_DISPLAYED_UNITS {
                s(&mut v, &x.unit_names[k]);
            }
            u(&mut v, x.voice_length as u32);
            s(&mut v, &x.general_name);
        }
    }
    match m.get_current_campaign() {
        Some(c) => {
            u(&mut v, 1);
            s(&mut v, &c.name);
        }
        None => u(&mut v, 0),
    }
    match m.get_current_mission() {
        Some(x) => {
            u(&mut v, 1);
            s(&mut v, &x.name);
        }
        None => u(&mut v, 0),
    }
    u(&mut v, u32::from(m.is_victorious()));
    u(&mut v, m.current_rank_points() as u32);
    u(&mut v, m.get_game_difficulty() as u32);
    u(&mut v, m.get_current_mission_number() as u32);
    s(&mut v, &m.get_current_map());
    v
}

fn rust_challenge_dump(c: &ChallengeGenerals) -> Vec<u8> {
    let mut v = Vec::new();
    for g in c.get_challenge_generals() {
        let GeneralPersona {
            starts_enabled,
            bio_name,
            bio_dob,
            bio_birthplace,
            bio_strategy,
            bio_rank,
            bio_branch,
            bio_class_number,
            campaign,
            player_template_name,
            portrait_movie_left_name,
            portrait_movie_right_name,
            defeated,
            victorious,
            selection_sound,
            taunt_sound1,
            taunt_sound2,
            taunt_sound3,
            win_sound,
            loss_sound,
            preview_sound,
            name_sound,
            ..
        } = g;
        u(&mut v, u32::from(*starts_enabled));
        for x in [
            bio_name,
            bio_dob,
            bio_birthplace,
            bio_strategy,
            bio_rank,
            bio_branch,
            bio_class_number,
            campaign,
            player_template_name,
            portrait_movie_left_name,
            portrait_movie_right_name,
            defeated,
            victorious,
            selection_sound,
            taunt_sound1,
            taunt_sound2,
            taunt_sound3,
            win_sound,
            loss_sound,
            preview_sound,
            name_sound,
        ] {
            s(&mut v, x);
        }
    }
    v
}

fn cpp_dump(f: unsafe extern "C" fn(*mut u8, i32) -> i32) -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = unsafe { f(buf.as_mut_ptr(), buf.len() as i32) } as usize;
        if n <= buf.len() {
            buf.truncate(n);
            return buf;
        }
        buf = vec![0u8; n];
    }
}

// ---------------------------------------------------------------------------
// The files
// ---------------------------------------------------------------------------

const CAMPAIGNS: [&str; 8] = ["USA", "usa", "China", "GLA", "Training", "Challenge_0", "CHALLENGE_0", "gla"];
const MISSIONS: [&str; 8] = ["M01", "m01", "M02", "M03", "Mission01", "MISSION01", "m04", "NONE"];

fn pick<'a>(g: &mut Gen, v: &[&'a str]) -> &'a str {
    v[g.below(v.len() as u32) as usize]
}
fn word(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(10) == 0 {
        return pick(g, &["", "\"quoted words\"", "a=b", ";"]).to_string();
    }
    pick(g, &["Maps\\USA01\\USA01.map", "CAMPAIGN:USA", "Objective:Line", "USA01Voice", "Tank", "General Granger", "x"]).to_string()
}
fn boolean(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(8) == 0 {
        return pick(g, &["", "maybe", "1", "TRUE"]).to_string();
    }
    pick(g, &["Yes", "No", "yes", "NO"]).to_string()
}
fn int(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(8) == 0 {
        return pick(g, &["", "x", "1.5", "99999999999", "-"]).to_string();
    }
    format!("{}", g.below(90000) as i32 - 100)
}

fn mission(g: &mut Gen, t: &mut Vec<String>, clean: bool) {
    let name = if !clean && g.below(25) == 0 { String::new() } else { pick(g, &MISSIONS).to_string() };
    t.push(format!("  Mission {name}"));
    for _ in 0..g.below(10) {
        let line = match g.below(17) {
            0 => format!("    Map = {}", pick(g, &["Maps\\USA01\\USA01.map", "Maps\\USA02\\USA02.map", "Maps\\GC01\\GC01.map"])),
            1 => format!("    NextMission = {}", pick(g, &MISSIONS)),
            2 => format!("    IntroMovie = {}", word(g, clean)),
            3..=4 => format!("    ObjectiveLine{} = {}", g.below(5), word(g, clean)),
            5 => format!("    BriefingVoice = {}", pick(g, &["USA01Voice", "NoSound", "nosound", "GLA02Voice"])),
            6 => format!("    UnitNames{} = {}", g.below(3), word(g, clean)),
            7 => format!("    GeneralName = {}", word(g, clean)),
            8 => format!("    LocationNameLabel = {}", word(g, clean)),
            9 => format!("    VoiceLength = {}", int(g, clean)),
            10 if !clean => pick(g, &["    Unknown = 3", "    =", "    Map", "    BriefingVoice ="]).to_string(),
            _ => "    ; a comment".to_string(),
        };
        t.push(line);
    }
    if clean || g.below(20) != 0 {
        t.push("  End".to_string());
    }
}

fn campaign_file(g: &mut Gen) -> Vec<u8> {
    let clean = g.below(10) < 6;
    let mut t = Vec::new();
    for _ in 0..1 + g.below(4) {
        if !clean && g.below(15) == 0 {
            t.push(pick(g, &["Unknown Block", "Campaign", "End"]).to_string());
            continue;
        }
        t.push(format!("Campaign {}", pick(g, &CAMPAIGNS)));
        for _ in 0..g.below(9) {
            match g.below(12) {
                0..=3 => mission(g, &mut t, clean),
                4..=5 => t.push(format!("  FirstMission = {}", pick(g, &MISSIONS))),
                6 => t.push(format!("  CampaignNameLabel = {}", word(g, clean))),
                7 => t.push(format!("  FinalVictoryMovie = {}", word(g, clean))),
                8 => t.push(format!("  IsChallengeCampaign = {}", boolean(g, clean))),
                9 => t.push(format!("  PlayerFaction = {}", pick(g, &["FactionAmerica", "FactionChina", "FactionGLA"]))),
                10 if !clean => t.push(pick(g, &["  Unknown = 3", "  =", "  FirstMission"]).to_string()),
                _ => t.push("  ; a comment".to_string()),
            }
        }
        if clean || g.below(15) != 0 {
            t.push("End".to_string());
        }
    }
    let mut out = t.join("\n").into_bytes();
    out.push(b'\n');
    out
}

const PERSONA_STRINGS: [&str; 22] = [
    "BioNameString",
    "BioDOBString",
    "BioBirthplaceString",
    "BioStrategyString",
    "BioRankString",
    "BioBranchString",
    "BioClassNumberString",
    "Campaign",
    "PlayerTemplate",
    "PortraitMovieLeftName",
    "PortraitMovieRightName",
    "DefeatedString",
    "VictoriousString",
    "SelectionSound",
    "TauntSound1",
    "TauntSound2",
    "TauntSound3",
    "WinSound",
    "LossSound",
    "PreviewSound",
    "NameSound",
    "BioNameString",
];
const PERSONA_IMAGES: [&str; 4] = ["BioPortraitSmall", "BioPortraitLarge", "DefeatedImage", "VictoriousImage"];

fn challenge_file(g: &mut Gen) -> Vec<u8> {
    let clean = g.below(10) < 6;
    let mut t = vec!["ChallengeGenerals".to_string()];
    for _ in 0..g.below(8) {
        let n = if !clean && g.below(12) == 0 { 12 } else { g.below(12) };
        t.push(format!("  GeneralPersona{n}"));
        for _ in 0..g.below(8) {
            match g.below(10) {
                0..=1 => t.push(format!("    StartsEnabled = {}", boolean(g, clean))),
                2..=6 => {
                    let k = pick(g, &PERSONA_STRINGS);
                    let w = if k == "Campaign" { pick(g, &CAMPAIGNS).to_string() } else { word(g, clean) };
                    t.push(format!("    {k} = {w}"));
                }
                7 => t.push(format!("    {} = {}", pick(g, &PERSONA_IMAGES), pick(g, &["SNPortrait01", "Defeated0", "x"]))),
                8 if !clean => t.push(pick(g, &["    Unknown = 3", "    =", "    StartsEnabled"]).to_string()),
                _ => t.push("    ; a comment".to_string()),
            }
        }
        if clean || g.below(20) != 0 {
            t.push("  End".to_string());
        }
    }
    if clean || g.below(15) != 0 {
        t.push("End".to_string());
    }
    let mut out = t.join("\n").into_bytes();
    out.push(b'\n');
    out
}

// ---------------------------------------------------------------------------
// The sessions
// ---------------------------------------------------------------------------

#[test]
fn campaign_and_challenge_match_upstream() {
    let sessions: u64 = std::env::var("CAMPAIGN_SESSIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(SESSIONS);
    let table = blocks();
    let mut outcome = Check::new("Campaign/ChallengeMode load outcome (throw, CRC)");
    let mut campaigns = Check::new("campaign list and manager state");
    let mut personas = Check::new("challenge personas (every field)");
    let mut ops = Check::new("manager operations and their results");
    let mut lookups = Check::new("persona lookups");
    let (mut loads, mut threw, mut campaigns_seen, mut missions_seen, mut nonempty_personas, mut op_count) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut stopped, mut undefined) = (0u64, Vec::new());
    for session in 0..sessions {
        let mut g = Gen::new(0x64_0000 + session);
        unsafe { cpp::ref_campaign_reset() };
        let mut ctx = Ctx { campaign: CampaignManager::new(), challenge: ChallengeGenerals::default() };
        let steps = 1 + g.below(4);
        let mut stopped_here = false;
        for step in 0..steps {
            let challenge = g.below(3) == 0;
            let data = if challenge { challenge_file(&mut g) } else { campaign_file(&mut g) };
            let load_type = match g.below(8) {
                0 => IniLoadType::CreateOverrides,
                1 => IniLoadType::Multifile,
                _ => IniLoadType::Overwrite,
            };
            let name = format!("Data\\INI\\{}{step}.ini", if challenge { "ChallengeMode" } else { "Campaign" }).into_bytes();
            let mut crc = XferCrc::new_without_nan_guard();
            let r = Ini::load(&name, &data, load_type, Some(&mut crc as &mut dyn Xfer), &table, &mut ctx);
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
            if rust.status != 0 {
                threw += 1;
            }
            let c = cpp_load(&name, &data, load_type);
            let tag = || format!("session {session} step {step} {load_type:?}: {}", String::from_utf8_lossy(&data));
            outcome.eq(&rust, &c, tag);
            campaigns.eq(rust_campaign_dump(&ctx.campaign), cpp_dump(cpp::ref_campaign_dump), tag);
            personas.eq(rust_challenge_dump(&ctx.challenge), cpp_dump(cpp::ref_challenge_dump), tag);
        }
        if stopped_here {
            // (the Rust stopped where upstream is undefined, before the C++ loaded that file: the two no longer hold the
            // same files, so the session ends here)
            continue;
        }
        campaigns_seen += ctx.campaign.campaign_list.len() as u64;
        missions_seen += ctx.campaign.campaign_list.iter().map(|c| c.missions.len() as u64).sum::<u64>();
        nonempty_personas += ctx.challenge.get_challenge_generals().iter().filter(|p| **p != GeneralPersona::default()).count() as u64;
        // the operations
        let mut done = Vec::new();
        for _ in 0..1 + g.below(12) {
            let op = g.below(6) as i32;
            let a = if g.below(10) == 0 { String::new() } else { pick(&mut g, &CAMPAIGNS).to_string() };
            let b = if g.below(10) == 0 { String::new() } else { pick(&mut g, &MISSIONS).to_lowercase() };
            let b = if g.below(4) == 0 { b.to_uppercase() } else { b };
            let n = g.below(5) as i32 - 1;
            let rust_r = match op {
                0 => {
                    ctx.campaign.set_campaign(a.as_bytes());
                    0
                }
                1 => {
                    ctx.campaign.set_campaign_and_mission(a.as_bytes(), b.as_bytes());
                    0
                }
                2 => i32::from(ctx.campaign.goto_next_mission().is_some()),
                3 => {
                    ctx.campaign.set_victorious(n != 0);
                    0
                }
                4 => {
                    ctx.campaign.set_rank_points(n * 100);
                    0
                }
                _ => {
                    ctx.campaign.set_game_difficulty(n);
                    0
                }
            };
            let (ca, cb) = (CString::new(a.clone()).unwrap(), CString::new(b.clone()).unwrap());
            let cpp_r = unsafe { cpp::ref_campaign_op(op, ca.as_ptr(), cb.as_ptr(), if op == 4 { n * 100 } else { n }) };
            op_count += 1;
            done.push(format!("{op}({a:?},{b:?},{n})"));
            let tag = || format!("session {session} ops {done:?}");
            ops.eq((rust_r, rust_campaign_dump(&ctx.campaign)), (cpp_r, cpp_dump(cpp::ref_campaign_dump)), tag);
        }
        for kind in 0..3 {
            let name = match kind {
                0 => pick(&mut g, &CAMPAIGNS).to_string(),
                1 => pick(&mut g, &["General Granger", "x", "general granger", ""]).to_string(),
                _ => pick(&mut g, &["FactionAmerica", "factionamerica", "Tank", ""]).to_string(),
            };
            let c = &ctx.challenge;
            let found = match kind {
                0 => c.get_player_general_by_campaign_name(name.as_bytes()),
                1 => c.get_general_by_general_name(name.as_bytes()),
                _ => c.get_general_by_template_name(name.as_bytes()),
            };
            let rust_i = found.map_or(-1, |p| c.get_challenge_generals().iter().position(|q| std::ptr::eq(q, p)).unwrap() as i32);
            let cn = CString::new(name.clone()).unwrap();
            let cpp_i = unsafe { cpp::ref_challenge_lookup(kind, cn.as_ptr()) };
            lookups.eq(rust_i, cpp_i, || format!("session {session} lookup {kind} {name:?}"));
        }
    }
    println!(
        "COVERAGE: {sessions} sessions, {loads} loads ({threw} threw), {stopped} stopped (undefined upstream), {op_count} operations; at the ends: {campaigns_seen} campaigns, {missions_seen} missions, {nonempty_personas} personas set"
    );
    for u in &undefined {
        println!("    undefined: {u}");
    }
    // (a planted bug, `L9_PLANT`: reported per comparison, and caught by at least one)
    if std::env::var("L9_PLANT").is_ok() {
        let caught: u64 = [outcome, campaigns, personas, ops, lookups].into_iter().map(Check::report).sum();
        assert!(caught > 0, "the planted bug was not caught");
        return;
    }
    for c in [outcome, campaigns, personas, ops, lookups] {
        c.finish();
    }
}
