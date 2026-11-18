//! UNTESTED DRAFT (#202, #208): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/Common/CommandLine.cpp
//!
//! `parseCommandLine` and the handler for each flag of the shipped (release) table, `params[]`.
//!
//! Scope: the release build's table lists only `-noshellmap -win -xres -yres -fullscreen -fullVersion -particleEdit
//! -scriptDebug -playStats -mod -noshaders -quickstart`; every handler compiled outside the
//! `_DEBUG`/`_INTERNAL` guards is translated (their `GlobalData` members exist in the retail configuration of
//! [`crate::common::global_data`]). **Not translated, debug-build only** (their `GlobalData`/`DEBUG_CRC` members are not in the
//! retail configuration): `-noLogOrCrash`'s crash, `-UseCSF`, `-NoInputDisable`, `-nofade`, `-nomilcap`, the `DEBUG_CRC` family
//! (`-DebugCRCFromFrame` ... `-ReplayCRCInterval`, `-noDraw`), `-logToCon`, the latency/packet-loss family, `-lowDetail`
//! ... `-munkee`, `-displayDebug`, `-file`, `-preloadEverything`, `-logAssets`, `-vTune`, `-noshroud`, `-nocinematic`,
//! `-nologo`, `-extraLogging`, `-selectTheUnselectable`, `-RunAhead`, `-saveStats`, `-saveAllStats`, `-localMOTD`,
//! `-cameraDebug`, `-benchmark`, `-stats`, `-ignoreAsserts`, `-ignoreStackTrace`, `-preload`.
//! `-quickstart` therefore takes the release branch (`parseNoSizzle`).

use crate::common::global_data::GlobalData;

/// `TheDebugIgnoreSyncErrors`, `DX8Wrapper_PreserveFPU` and `TheVersion`'s full-version flag: the globals the handlers write.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandLineState {
    /// `TheDebugIgnoreSyncErrors`.
    pub the_debug_ignore_sync_errors: bool,
    /// `DX8Wrapper_PreserveFPU`.
    pub dx8_wrapper_preserve_fpu: i32,
    /// `TheVersion->setShowFullVersion(...)`: `None` until `-fullVersion` runs.
    pub show_full_version: Option<bool>,
}

/// What `parseMod` asks of the file system: `TheLocalFileSystem->doesFileExist` and `_stat`'s directory bit.
pub trait CommandLineHost {
    /// `TheLocalFileSystem->doesFileExist(path)`.
    fn does_file_exist(&mut self, path: &[u8]) -> bool;
    /// `_stat(path, &statBuf) != 0` is `None`; else `Some(statBuf.st_mode & _S_IFDIR != 0)`.
    fn stat_is_dir(&mut self, path: &[u8]) -> Option<bool>;
    /// `TheArchiveFileSystem->loadMods()`, the last line of `parseCommandLine`.
    fn load_mods(&mut self);
}

/// The globals every handler reaches: `TheWritableGlobalData` (always non-null here), the state above and the host.
pub struct CommandLineContext<'a, H: CommandLineHost> {
    /// `TheWritableGlobalData`.
    pub global_data: &'a mut GlobalData,
    /// The other globals.
    pub state: &'a mut CommandLineState,
    /// The file-system calls.
    pub host: &'a mut H,
}

/// `typedef Int (*FuncPtr)( char *args[], int num )` / `struct CommandLineParam`.
pub struct CommandLineParam<'a, H: CommandLineHost> {
    /// `name`.
    pub name: &'static str,
    /// `func`: `args` is `argv+arg` (so `args.len()` is `argc-arg`); the return value is the number of arguments eaten.
    pub func: fn(&mut CommandLineContext<'a, H>, &[Vec<u8>]) -> i32,
}

/// `atoi(s)`: optional leading whitespace and sign, then digits, stopping at the first other byte; no digits is 0;
/// overflow wraps as the CRT's does on 32 bits.
pub fn atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || (b'\t'..=b'\r').contains(&s[i])) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut v: i32 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.wrapping_mul(10).wrapping_add((s[i] - b'0') as i32);
        i += 1;
    }
    if neg {
        v.wrapping_neg()
    } else {
        v
    }
}

/// `static void ConvertShortMapPathToLongMapPath(AsciiString &mapName)`: `Dir\Name.map` becomes `Dir\Name\Name.map`.
/// A name with no `\` or `/` is left as it is (upstream's `DEBUG_CRASH` is compiled out of a release build).
pub fn convert_short_map_path_to_long_map_path(map_name: &mut Vec<u8>) {
    let path = map_name.clone();
    if !path.contains(&b'\\') && !path.contains(&b'/') {
        // DEBUG_CRASH(("Invalid map name %s", mapName.str())) -- compiled out of a release build.
        return;
    }
    let seps: &[u8] = b"\\/";
    // AsciiString::nextToken(&token, "\\/"): skips leading separators, takes up to the next separator, and the rest stays.
    let mut rest: &[u8] = &path;
    let mut next_token = |rest: &mut &[u8]| -> Vec<u8> {
        let mut start = 0;
        while start < rest.len() && seps.contains(&rest[start]) {
            start += 1;
        }
        let mut end = start;
        while end < rest.len() && !seps.contains(&rest[end]) {
            end += 1;
        }
        let tok = rest[start..end].to_vec();
        *rest = &rest[end..];
        tok
    };
    let ends_with_map = |t: &[u8]| t.len() >= 4 && t[t.len() - 4..].eq_ignore_ascii_case(b".map");
    let mut actualpath: Vec<u8> = Vec::new();
    let mut token = next_token(&mut rest);
    while !ends_with_map(&token) && !token.is_empty() {
        actualpath.extend_from_slice(&token);
        actualpath.push(b'\\');
        token = next_token(&mut rest);
    }
    if !ends_with_map(&token) {
        // DEBUG_CRASH(("Invalid map name %s", mapName.str())) -- compiled out of a release build.
    }
    // remove the .map from the end (four removeLastChar calls).
    for _ in 0..4 {
        token.pop();
    }
    actualpath.extend_from_slice(&token);
    actualpath.push(b'\\');
    actualpath.extend_from_slice(&token);
    actualpath.extend_from_slice(b".map");
    *map_name = actualpath;
}

/// `parseNoLogOrCrash(args, num)`: "`#ifdef ALLOW_DEBUG_UTILS` DEBUG_CRASH" -- not in a release build; eats 1.
pub fn parse_no_log_or_crash<H: CommandLineHost>(_ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    1
}

/// `parseFPUPreserve(args, argc)`: `DX8Wrapper_PreserveFPU = atoi(args[1])` when `argc > 1`; eats 2.
pub fn parse_fpu_preserve<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        ctx.state.dx8_wrapper_preserve_fpu = atoi(&args[1]);
    }
    2
}

/// `parseFullVersion(args, num)`: `TheVersion->setShowFullVersion(atoi(args[1]) != 0)` when `num > 1`; eats 1 (not 2: upstream).
pub fn parse_full_version<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        ctx.state.show_full_version = Some(atoi(&args[1]) != 0);
    }
    1
}

/// `parseMapName(args, num)`: `m_mapName.set(args[1])` then the short-to-long conversion, only when `num == 2`; eats 1 (upstream).
pub fn parse_map_name<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() == 2 {
        ctx.global_data.map_name = args[1].clone();
        convert_short_map_path_to_long_map_path(&mut ctx.global_data.map_name);
    }
    1
}

/// `parseNoFPSLimit(args, num)`: `m_useFpsLimit = false; m_framesPerSecondLimit = 30000`.
pub fn parse_no_fps_limit<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.use_fps_limit = false;
    ctx.global_data.frames_per_second_limit = 30000;
    1
}

/// `parseJumpToFrame(args, num)`: with `num > 1`, `parseNoFPSLimit` then `m_noDraw = atoi(args[1])`, eating 2; else eats 1.
pub fn parse_jump_to_frame<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        parse_no_fps_limit(ctx, args);
        ctx.global_data.no_draw = atoi(&args[1]) as u32;
        return 2;
    }
    1
}

/// `parseSync(args, num)`: `TheDebugIgnoreSyncErrors = true`.
pub fn parse_sync<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.state.the_debug_ignore_sync_errors = true;
    1
}

/// `parseShellMap(args, num)`: `m_shellMapName = args[1]` when `num > 1`; eats 2.
pub fn parse_shell_map<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        ctx.global_data.shell_map_name = args[1].clone();
    }
    2
}

/// `parseQuickStart(args, num)`: the release branch -- `parseNoSizzle` (Patch 1.01: skip the sizzle but still show the EA logo,
/// "for legal reasons"), `parseNoShellMap`, `parseNoWindowAnimation`; eats 1.
pub fn parse_quick_start<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    parse_no_sizzle(ctx, args);
    parse_no_shell_map(ctx, args);
    parse_no_window_animation(ctx, args);
    1
}

/// `parseMod(args, num)`: with `num > 1`, `-mod X`: a full path (contains `:` or starts with `/` or `\`) is used as it is, else
/// `getPath_UserData() + X`. A missing path or one `_stat` fails on eats 2 and sets nothing; a directory gets a trailing `\`
/// (when it has neither `\` nor `/` at the end) and becomes `m_modDir`; anything else becomes `m_modBIG`. Without `num > 1` eats 1.
pub fn parse_mod<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        let mut mod_path: Vec<u8> = args[1].clone();
        if mod_path.contains(&b':') || mod_path.starts_with(b"/") || mod_path.starts_with(b"\\") {
            // full path passed in.  Don't append base path.
        } else {
            let mut p = ctx.global_data.get_path_user_data().to_vec();
            p.extend_from_slice(&args[1]);
            mod_path = p;
        }
        // DEBUG_LOG(("Looking for mod '%s'\n", modPath.str()));
        if !ctx.host.does_file_exist(&mod_path) {
            // DEBUG_LOG(("Mod does not exist.\n"));
            return 2; // no such file/dir.
        }
        // now check for dir-ness
        let is_dir = match ctx.host.stat_is_dir(&mod_path) {
            Some(d) => d,
            None => return 2, // could not stat the file/dir.
        };
        if is_dir {
            if !mod_path.ends_with(b"\\") && !mod_path.ends_with(b"/") {
                mod_path.push(b'\\');
            }
            // DEBUG_LOG(("Mod dir is '%s'.\n", modPath.str()));
            ctx.global_data.mod_dir = mod_path;
        } else {
            // DEBUG_LOG(("Mod file is '%s'.\n", modPath.str()));
            ctx.global_data.mod_big = mod_path;
        }
        return 2;
    }
    1
}

// ---- the handlers of one or two plain assignments (written out as upstream has them) ----

/// `parseNoMusic(args, num)`.
pub fn parse_no_music<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.music_on = false;
    1
}

/// `parseNoVideo(args, num)`.
pub fn parse_no_video<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.video_on = false;
    1
}

/// `parseNoAudio(args, num)`.
pub fn parse_no_audio<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.audio_on = false;
    ctx.global_data.speech_on = false;
    ctx.global_data.sounds_on = false;
    ctx.global_data.music_on = false;
    1
}

/// `parseNoWin(args, num)`.
pub fn parse_no_win<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.windowed = false;
    1
}

/// `parseNoShadows(args, num)`.
pub fn parse_no_shadows<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.use_shadow_volumes = false;
    ctx.global_data.use_shadow_decals = false;
    1
}

/// `parseScriptDebug(args, num)`.
pub fn parse_script_debug<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.script_debug = true;
    ctx.global_data.win_cursors = true;
    1
}

/// `parseParticleEdit(args, num)`.
pub fn parse_particle_edit<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.particle_edit = true;
    ctx.global_data.win_cursors = true;
    ctx.global_data.windowed = true;
    1
}

/// `parseBuildMapCache(args, num)`.
pub fn parse_build_map_cache<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.build_map_cache = true;
    1
}

/// `parseNoFX(args, num)`.
pub fn parse_no_fx<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.use_fx = false;
    1
}

/// `parseForceBenchmark(args, num)`.
pub fn parse_force_benchmark<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.force_benchmark = true;
    1
}

/// `parseNoMoveCamera(args, num)`.
pub fn parse_no_move_camera<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.disable_camera_movement = true;
    1
}

/// `parseNoShellMap(args, num)`.
pub fn parse_no_shell_map<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.shell_map_on = false;
    1
}

/// `parseNoShaders(args, num)`.
pub fn parse_no_shaders<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.chip_set_type = 1; // force to a voodoo card which uses least amount of features.
    1
}

/// `parseNoSizzle(args, num)`.
pub fn parse_no_sizzle<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.play_sizzle = false;
    1
}

/// `parseNoWindowAnimation(args, num)`.
pub fn parse_no_window_animation<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.animate_windows = false;
    1
}

/// `parseWinCursors(args, num)`.
pub fn parse_win_cursors<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.win_cursors = true;
    1
}

/// `parseConstantDebug(args, num)`.
pub fn parse_constant_debug<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.constant_debug_update = true;
    1
}

/// `parseShowTeamDot(args, num)`.
pub fn parse_show_team_dot<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.show_team_dot = true;
    1
}

/// `parseIncrAGPBuf(args, num)`.
pub fn parse_incr_agp_buf<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.incremental_agp_buf = true;
    1
}

/// `parseDemoLoadScreen(args, num)`.
pub fn parse_demo_load_screen<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.load_screen_demo = true;
    1
}

/// `parseDumpAssetUsage(args, num)`.
pub fn parse_dump_asset_usage<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.dump_asset_usage = true;
    1
}

/// `parseUpdateImages(args, num)`.
pub fn parse_update_images<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.should_update_tga_to_dds = true;
    1
}

/// `parseWin(args, num)`.
pub fn parse_win<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, _args: &[Vec<u8>]) -> i32 {
    ctx.global_data.windowed = true;
    1
}

/// `parseXRes(args, num)`: `atoi(args[1])` when there is one (eats 2), else eats 1.
pub fn parse_x_res<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        ctx.global_data.x_resolution = atoi(&args[1]);
        return 2;
    }
    1
}

/// `parseFPSLimit(args, num)`: `atoi(args[1])` when `num > 1`; eats 2 either way.
pub fn parse_fps_limit<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        ctx.global_data.frames_per_second_limit = atoi(&args[1]);
    }
    2
}

/// `parseSeed(args, num)`: `atoi(args[1])` when `num > 1`; eats 2 either way.
pub fn parse_seed<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        ctx.global_data.fixed_seed = atoi(&args[1]);
    }
    2
}

/// `parseNetMinPlayers(args, num)`: `atoi(args[1])` when `num > 1`; eats 2 either way.
pub fn parse_net_min_players<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        ctx.global_data.net_min_players = atoi(&args[1]);
    }
    2
}

/// `parsePlayStats(args, num)`: `atoi(args[1])` when `num > 1`; eats 2 either way.
pub fn parse_play_stats<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, args: &[Vec<u8>]) -> i32 {
    if args.len() > 1 {
        ctx.global_data.play_stats = atoi(&args[1]);
    }
    2
}

/// `static CommandLineParam params[]`: the release build's table, in upstream's order (the `_DEBUG`/`_INTERNAL` entries are
/// not translated, see the module doc).
pub fn params<'a, H: CommandLineHost>() -> Vec<CommandLineParam<'a, H>> {
    vec![
        CommandLineParam { name: "-noshellmap", func: parse_no_shell_map },
        CommandLineParam { name: "-win", func: parse_win },
        CommandLineParam { name: "-xres", func: parse_x_res },
        CommandLineParam { name: "-yres", func: parse_y_res },
        CommandLineParam { name: "-fullscreen", func: parse_no_win },
        CommandLineParam { name: "-fullVersion", func: parse_full_version },
        CommandLineParam { name: "-particleEdit", func: parse_particle_edit },
        CommandLineParam { name: "-scriptDebug", func: parse_script_debug },
        CommandLineParam { name: "-playStats", func: parse_play_stats },
        CommandLineParam { name: "-mod", func: parse_mod },
        CommandLineParam { name: "-noshaders", func: parse_no_shaders },
        CommandLineParam { name: "-quickstart", func: parse_quick_start },
    ]
}

/// `parseCommandLine(argc, argv)`: "Parse command-line parameters." Loops over the arguments from index 1; each is compared
/// (same length, then `strnicmp`) against every table name, and the first match's handler is called on `argv+arg` and its
/// return value skips that many arguments. An argument matching nothing skips 1. Ends with `TheArchiveFileSystem->loadMods()`.
pub fn parse_command_line<H: CommandLineHost>(ctx: &mut CommandLineContext<H>, argv: &[Vec<u8>]) {
    let argc = argv.len() as i32;
    let table = params::<H>();
    let mut arg: i32 = 1;
    while arg < argc {
        // Look at arg #i
        let mut found = false;
        let mut param = 0;
        while !found && param < table.len() {
            let name = table[param].name.as_bytes();
            let a = &argv[arg as usize];
            param += 1;
            if a.len() != name.len() {
                continue;
            }
            if a.eq_ignore_ascii_case(name) {
                arg += (table[param - 1].func)(ctx, &argv[arg as usize..]);
                found = true;
            }
        }
        if !found {
            arg += 1;
        }
    }
    ctx.host.load_mods();
}
