#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/CommandLine.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::command_line` (the release build's handlers, `params` and `parse_command_line`) against `CommandLine.cpp` (#215).
//!
//! # Method
//!
//! CommandLine.cpp from `Bool TheDebugIgnoreSyncErrors = FALSE;` to the end is cut verbatim (build.rs) into
//! `differential/wrap_draft_command_line.cpp` and compiled as the shipped game is (no `_DEBUG`, no `_INTERNAL`: the preprocessor keeps
//! the release branches and the release table). Stand-ins for everything the cut calls (listed there): a `GlobalData` of the members the
//! release handlers write, `TheVersion`, the archive and local file systems (`doesFileExist` and `_stat` answer from a hash of the path
//! and the case's seed, which this test computes the same way), `strnicmp`; `AsciiString` is upstream's own. Two parts, from fixed seeds:
//! 1. each of 38 handlers called directly (the ones the release C++ compiles) on 3,000 random cases: a random initial `GlobalData` (every
//!    Bool, the eight numbers, the four strings), 1 to 3 arguments of numbers, map names, mod names and junk;
//! 2. `parseCommandLine` on 30,000 random command lines of 0 to 8 words drawn from the release flags (in any letter case), the debug-only
//!    flags (which a release build does not know), numbers, map and mod names and junk.
//! Compared: the return value, all 25 Bools, the 8 numbers, the 4 strings, `TheDebugIgnoreSyncErrors`, `DX8Wrapper_PreserveFPU`, what
//! `setShowFullVersion` was given (or that it was not called) and how often `loadMods` ran.
//!
//! Excluded: `parseMapName` with a path that has a separator and at least one token but none ending in `.map` (upstream's loop never ends
//! and throws, see `convert_loops`); numbers of 10 or more digits (`atoi` overflow is undefined in C; the draft wraps); `parseFPSLimit`, which the draft translates
//! but which upstream compiles only under `_DEBUG` (so it is not in the release C++ and not compared); arguments with an embedded NUL.
//!
//! # Prediction (written before the first run, 2026-11-18)
//!
//! 0 mismatches (assignments, and the draft's notes already record upstream's quirks: `parseFullVersion` eats 1, `parseMapName` needs
//! exactly two words).
//!
//! # Results
//!
//! 2026-11-18: 38 handlers x 3,000 cases (113,639 compared; 361 excluded) and 30,000 command lines, **0 mismatches** (prediction held). The first run
//! aborted: upstream's `ConvertShortMapPathToLongMapPath` never ends for a path with a separator and no token ending in `.map` (see `convert_loops`;
//! an upstream defect, debug build only), so those inputs are excluded and the draft's different behaviour (it ends the loop) is documented in the draft.
//! Planted bugs, 11 of 11 caught: 11538 parseCommandLine 293 of 30,000 (handlers 1,998 of 113,639); 11539 239 / 3,000; 11540 handlers 998; 11541 81;
//! 11542 1,218 and 1,063; 11543 489 and 473; 11544 5,290 of 30,000; 11545 30,000; 11546 handlers 1,972; 11547 2,280 and 333; 11548 1,493 and 1,038.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 11538 `-xres` eats 1; 11539 `-fullVersion` eats 2; 11540 `-map` accepts more than two words; 11541 the map path keeps the
//! last letter of `.map`; 11542 `-mod`'s directory test inverted; 11543 no trailing `\` on a mod directory; 11544 flags compared
//! case-sensitively; 11545 `loadMods` not called; 11546 `-jumpToFrame` eats 1; 11547 `atoi` does not skip white space; 11548 `-quickstart`
//! does not call `parseNoWindowAnimation`.

mod support;

use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::drafts::logic_common::command_line::*;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_cl_run(handler: i32, argc: i32, argv: *const std::os::raw::c_char, init: *const u32, strs: *const std::os::raw::c_char, seed: u32, out: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

fn hash_path(p: &[u8], seed: u32) -> u32 {
    let mut h: u32 = 2166136261 ^ seed;
    for &b in p {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

struct Host {
    seed: u32,
    loads: u32,
}

impl CommandLineHost for Host {
    fn does_file_exist(&mut self, path: &[u8]) -> bool {
        (hash_path(path, self.seed) >> 7) % 4 != 0
    }
    fn stat_is_dir(&mut self, path: &[u8]) -> Option<bool> {
        let h = hash_path(path, self.seed ^ 0x5bd1_e995);
        if (h >> 5) % 5 == 0 {
            None
        } else {
            Some((h >> 11) % 2 != 0)
        }
    }
    fn load_mods(&mut self) {
        self.loads += 1;
    }
}

type Handler = fn(&mut CommandLineContext<'_, Host>, &[Vec<u8>]) -> i32;

fn handlers() -> Vec<(&'static str, Handler)> {
    vec![
        ("parseNoLogOrCrash", parse_no_log_or_crash::<Host>),
        ("parseFPUPreserve", parse_fpu_preserve::<Host>),
        ("parseFullVersion", parse_full_version::<Host>),
        ("parseMapName", parse_map_name::<Host>),
        ("parseNoFPSLimit", parse_no_fps_limit::<Host>),
        ("parseJumpToFrame", parse_jump_to_frame::<Host>),
        ("parseSync", parse_sync::<Host>),
        ("parseShellMap", parse_shell_map::<Host>),
        ("parseQuickStart", parse_quick_start::<Host>),
        ("parseMod", parse_mod::<Host>),
        ("parseNoMusic", parse_no_music::<Host>),
        ("parseNoVideo", parse_no_video::<Host>),
        ("parseNoAudio", parse_no_audio::<Host>),
        ("parseNoWin", parse_no_win::<Host>),
        ("parseNoShadows", parse_no_shadows::<Host>),
        ("parseScriptDebug", parse_script_debug::<Host>),
        ("parseParticleEdit", parse_particle_edit::<Host>),
        ("parseBuildMapCache", parse_build_map_cache::<Host>),
        ("parseNoFX", parse_no_fx::<Host>),
        ("parseForceBenchmark", parse_force_benchmark::<Host>),
        ("parseNoMoveCamera", parse_no_move_camera::<Host>),
        ("parseNoShellMap", parse_no_shell_map::<Host>),
        ("parseNoShaders", parse_no_shaders::<Host>),
        ("parseNoSizzle", parse_no_sizzle::<Host>),
        ("parseNoWindowAnimation", parse_no_window_animation::<Host>),
        ("parseWinCursors", parse_win_cursors::<Host>),
        ("parseConstantDebug", parse_constant_debug::<Host>),
        ("parseShowTeamDot", parse_show_team_dot::<Host>),
        ("parseIncrAGPBuf", parse_incr_agp_buf::<Host>),
        ("parseDemoLoadScreen", parse_demo_load_screen::<Host>),
        ("parseDumpAssetUsage", parse_dump_asset_usage::<Host>),
        ("parseUpdateImages", parse_update_images::<Host>),
        ("parseWin", parse_win::<Host>),
        ("parseXRes", parse_x_res::<Host>),
        ("parseYRes", parse_y_res::<Host>),
        ("parseSeed", parse_seed::<Host>),
        ("parseNetMinPlayers", parse_net_min_players::<Host>),
        ("parsePlayStats", parse_play_stats::<Host>),
    ]
}

struct Init {
    words: Vec<u32>,
    strs: Vec<Vec<u8>>,
}

fn rand_string(g: &mut Gen) -> Vec<u8> {
    let n = g.below(14) as usize;
    (0..n)
        .map(|_| match g.below(6) {
            0 => b'\\',
            1 => b'/',
            2 => b'a' + g.below(26) as u8,
            3 => b'A' + g.below(26) as u8,
            4 => b'.',
            _ => [b':', b' ', b'0', b'm'][g.below(4) as usize],
        })
        .collect()
}

fn init_of(g: &mut Gen) -> Init {
    let mut words: Vec<u32> = (0..25).map(|_| g.below(2)).collect();
    words.extend([g.u32(), g.u32(), g.u32(), g.u32(), g.u32(), g.u32(), g.u32(), g.u32()]);
    let strs = vec![rand_string(g), rand_string(g), rand_string(g), rand_string(g), rand_string(g)];
    Init { words, strs }
}

fn apply_init(init: &Init) -> GlobalData {
    let w = &init.words;
    let mut g = GlobalData::new();
    let b = |i: usize| w[i] != 0;
    g.windowed = b(0);
    g.win_cursors = b(1);
    g.music_on = b(2);
    g.video_on = b(3);
    g.use_shadow_volumes = b(4);
    g.use_shadow_decals = b(5);
    g.use_fps_limit = b(6);
    g.use_fx = b(7);
    g.speech_on = b(8);
    g.sounds_on = b(9);
    g.show_team_dot = b(10);
    g.should_update_tga_to_dds = b(11);
    g.shell_map_on = b(12);
    g.script_debug = b(13);
    g.play_sizzle = b(14);
    g.particle_edit = b(15);
    g.load_screen_demo = b(16);
    g.incremental_agp_buf = b(17);
    g.force_benchmark = b(18);
    g.dump_asset_usage = b(19);
    g.disable_camera_movement = b(20);
    g.constant_debug_update = b(21);
    g.build_map_cache = b(22);
    g.audio_on = b(23);
    g.animate_windows = b(24);
    g.x_resolution = w[25] as i32;
    g.y_resolution = w[26] as i32;
    g.play_stats = w[27] as i32;
    g.net_min_players = w[28] as i32;
    g.frames_per_second_limit = w[29] as i32;
    g.fixed_seed = w[30] as i32;
    g.chip_set_type = w[31] as i32;
    g.no_draw = w[32];
    g.map_name = init.strs[0].clone();
    g.shell_map_name = init.strs[1].clone();
    g.mod_dir = init.strs[2].clone();
    g.mod_big = init.strs[3].clone();
    g.set_user_data_dir(&init.strs[4]);
    g
}

fn dump(ret: i32, g: &GlobalData, st: &CommandLineState, loads: u32) -> Vec<u32> {
    let mut o: Vec<u32> = vec![ret as u32];
    for v in [
        g.windowed, g.win_cursors, g.music_on, g.video_on, g.use_shadow_volumes, g.use_shadow_decals, g.use_fps_limit, g.use_fx, g.speech_on, g.sounds_on,
        g.show_team_dot, g.should_update_tga_to_dds, g.shell_map_on, g.script_debug, g.play_sizzle, g.particle_edit, g.load_screen_demo, g.incremental_agp_buf,
        g.force_benchmark, g.dump_asset_usage, g.disable_camera_movement, g.constant_debug_update, g.build_map_cache, g.audio_on, g.animate_windows,
    ] {
        o.push(v as u32);
    }
    o.extend([g.x_resolution as u32, g.y_resolution as u32, g.play_stats as u32, g.net_min_players as u32, g.frames_per_second_limit as u32, g.fixed_seed as u32, g.chip_set_type as u32, g.no_draw]);
    for s in [&g.map_name, &g.shell_map_name, &g.mod_dir, &g.mod_big] {
        o.push(s.len() as u32);
        o.extend(s.iter().map(|&b| b as u32));
    }
    o.push(st.the_debug_ignore_sync_errors as u32);
    o.push(st.dx8_wrapper_preserve_fpu as u32);
    o.push(match st.show_full_version {
        None => 2,
        Some(b) => b as u32,
    });
    o.push(loads);
    o
}

fn number_arg(g: &mut Gen) -> Vec<u8> {
    let mut s: Vec<u8> = Vec::new();
    match g.below(5) {
        0 => s.extend(b"  "),
        1 => s.push(b'\t'),
        _ => {}
    }
    match g.below(6) {
        0 => s.push(b'-'),
        1 => s.push(b'+'),
        _ => {}
    }
    let digits = 1 + g.below(9);
    for _ in 0..digits {
        s.push(b'0' + g.below(10) as u8);
    }
    match g.below(6) {
        0 => s.extend(b"abc"),
        1 => s.extend(b" 7"),
        2 => s.push(b'.'),
        _ => {}
    }
    if g.below(10) == 0 {
        s = b"x12".to_vec();
    }
    s
}

fn map_arg(g: &mut Gen) -> Vec<u8> {
    let names: [&[u8]; 8] = [b"Maps\\Foo.map", b"Maps/Bar.MAP", b"Foo.map", b"Dir\\Sub\\Z.map", b"Maps\\x", b"\\", b"a/b/c/d.Map", b""];
    let mut s = names[g.below(8) as usize].to_vec();
    if g.below(4) == 0 {
        s.extend(rand_string(g));
    }
    s
}

fn mod_arg(g: &mut Gen) -> Vec<u8> {
    let names: [&[u8]; 8] = [b"C:\\mods\\x", b"/abs/path", b"\\unc\\p", b"rel", b"rel\\", b"rel/", b"mod.big", b""];
    let mut s = names[g.below(8) as usize].to_vec();
    if g.below(3) == 0 {
        s.extend(rand_string(g));
    }
    s
}

fn junk_arg(g: &mut Gen) -> Vec<u8> {
    let mut s = rand_string(g);
    s.retain(|&b| b != 0);
    s
}

const FLAGS: [&str; 24] = [
    "-noshellmap", "-win", "-xres", "-yres", "-fullscreen", "-fullVersion", "-particleEdit", "-scriptDebug", "-playStats", "-mod", "-noshaders", "-quickstart",
    "-noaudio", "-map", "-nomusic", "-novideo", "-FPUPreserve", "-fps", "-ignoresync", "-shellmap", "-seed", "-noFPSLimit", "-jumpToFrame", "-netMinPlayers",
];

fn case_scramble(g: &mut Gen, s: &str) -> Vec<u8> {
    s.bytes().map(|b| if g.below(3) == 0 { if b.is_ascii_lowercase() { b.to_ascii_uppercase() } else { b.to_ascii_lowercase() } } else { b }).collect()
}

/// Upstream's `ConvertShortMapPathToLongMapPath` loops `while (!token.endsWithNoCase(".map") && token.getLength() > 0)` on
/// `path.nextToken(&token, "\\/")`, and `AsciiString::nextToken` returns FALSE WITHOUT clearing `token` once the path is used up: so a path
/// with a separator and at least one token, none of which ends in `.map`, repeats its last token for ever, and `actualpath.concat` throws
/// ERROR_OUT_OF_MEMORY at 32,767 characters (an upstream defect, only reachable through `-map`, which is in the debug table). The draft ends
/// the loop on an empty token. Those inputs are excluded.
fn convert_loops(path: &[u8]) -> bool {
    if !path.contains(&b'\\') && !path.contains(&b'/') {
        return false;
    }
    let tokens: Vec<&[u8]> = path.split(|&b| b == b'\\' || b == b'/').filter(|t| !t.is_empty()).collect();
    !tokens.is_empty() && !tokens.iter().any(|t| t.len() >= 4 && t[t.len() - 4..].eq_ignore_ascii_case(b".map"))
}

fn cstr_args(args: &[Vec<u8>]) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::new();
    for a in args {
        v.extend(a);
        v.push(0);
    }
    v
}

fn cstr_strs(strs: &[Vec<u8>]) -> Vec<u8> {
    cstr_args(strs)
}

#[test]
fn handlers_match() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let table = handlers();
    let mut g = Gen::new(0x215_C11A);
    let mut c = Check::new("CommandLine handlers (38 handlers)");
    let mut excluded = 0u32;
    for (k, (name, f)) in table.iter().enumerate() {
        for case in 0..3_000 {
            let init = init_of(&mut g);
            let seed = g.u32();
            let argc = 1 + g.below(3) as usize;
            let mut args: Vec<Vec<u8>> = vec![name.as_bytes().to_vec()];
            for _ in 1..argc {
                args.push(match g.below(5) {
                    0 | 1 => number_arg(&mut g),
                    2 => map_arg(&mut g),
                    3 => mod_arg(&mut g),
                    _ => junk_arg(&mut g),
                });
            }
            if *name == "parseMapName" && args.len() == 2 && convert_loops(&args[1]) {
                excluded += 1;
                continue;
            }
            let argv = cstr_args(&args);
            let strs = cstr_strs(&init.strs);
            let mut cpp = vec![0u32; 4096];
            let n = unsafe { ref_cl_run(k as i32, argc as i32, argv.as_ptr() as *const _, init.words.as_ptr(), strs.as_ptr() as *const _, seed, cpp.as_mut_ptr()) } as usize;
            cpp.truncate(n);

            let mut gd = apply_init(&init);
            let mut st = CommandLineState::default();
            let mut host = Host { seed, loads: 0 };
            let ret = {
                let mut ctx = CommandLineContext { global_data: &mut gd, state: &mut st, host: &mut host };
                f(&mut ctx, &args)
            };
            let rust = dump(ret, &gd, &st, host.loads);
            c.eq(rust == cpp, true, || format!("{name} case {case}: args {:?}", args.iter().map(|a| String::from_utf8_lossy(a).to_string()).collect::<Vec<_>>()));
        }
    }
    println!("(handlers: {excluded} parseMapName cases excluded: upstream's ConvertShortMapPathToLongMapPath never ends)");
    finish(c);
}

#[test]
fn parse_command_line_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C11B);
    let mut c = Check::new("parseCommandLine");
    for case in 0..30_000 {
        let init = init_of(&mut g);
        let seed = g.u32();
        let mut args: Vec<Vec<u8>> = vec![b"game.exe".to_vec()];
        for _ in 0..g.below(9) {
            args.push(match g.below(8) {
                0..=3 => {
                    let f = FLAGS[g.below(FLAGS.len() as u32) as usize];
                    if g.below(3) == 0 { case_scramble(&mut g, f) } else { f.as_bytes().to_vec() }
                }
                4 => number_arg(&mut g),
                5 => map_arg(&mut g),
                6 => mod_arg(&mut g),
                _ => junk_arg(&mut g),
            });
        }
        let argc = args.len();
        let argv = cstr_args(&args);
        let strs = cstr_strs(&init.strs);
        let mut cpp = vec![0u32; 4096];
        let n = unsafe { ref_cl_run(-1, argc as i32, argv.as_ptr() as *const _, init.words.as_ptr(), strs.as_ptr() as *const _, seed, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(n);

        let mut gd = apply_init(&init);
        let mut st = CommandLineState::default();
        let mut host = Host { seed, loads: 0 };
        {
            let mut ctx = CommandLineContext { global_data: &mut gd, state: &mut st, host: &mut host };
            parse_command_line(&mut ctx, &args);
        }
        let rust = dump(0, &gd, &st, host.loads);
        c.eq(rust == cpp, true, || format!("case {case}: args {:?}", args.iter().map(|a| String::from_utf8_lossy(a).to_string()).collect::<Vec<_>>()));
    }
    finish(c);
}
