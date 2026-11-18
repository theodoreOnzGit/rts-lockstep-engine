//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/PersistentStorageThread.h
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/PersistentStorageDefs.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Thread/PersistentStorageThread.cpp
//!
//! The GameSpy persistent-storage ("gstats") thread: player stats records, their `\key\value` text form
//! (`parsePlayerKVPairs` / `formatPlayerKVPairs`), the queue pair, and the thread body that reads/writes stats and
//! sends game results.
//!
//! # SDK and files
//!
//! `InitStatsConnection`, `GetPersistDataValues`, `SetPersistDataValues`, `PreAuthenticatePlayerPM/CD`,
//! `GenerateAuth`, `NewGame`/`SendGameSnapShot`/`FreeGame`, `PersistThink` are the [`PersistSdkHost`]; its callbacks
//! are [`PsThread`] methods the host calls. `UserPreferences` files (`GeneralsOnline\MiscPref<id>.ini`, the stored
//! "added desyncs/discons") are reached through the host too. The thread's `while (running)` body is
//! [`PsThread::thread_step`].
//!
//! # Defects of upstream kept
//!
//! * `PSREQUEST_UPDATEPLAYERSTATS` does `return` (not `break`) when `req.addDesync || req.addDiscon` and
//!   `req.password.size() == 0`, and when `req.player.id == 0`: that returns out of `Thread_Function`, ending the
//!   thread. [`PsThread::handle_request`] returns `false` ("thread exits") there.
//! * The copy constructor `PSPlayerStats(const PSPlayerStats&)` copies maps through `incorporate`, which keeps only
//!   entries greater than 0, and does not copy `QMwinsInARow` / `maxQMwinsInARow` (uninitialised in C++; 0 here).
//!   [`PsPlayerStats::copy_constructed`] reproduces it; `Clone` is the plain field copy a C++ move would not give,
//!   so use `copy_constructed` where upstream copies by value.
//! * `reset()` leaves `lastLadderHost`, `options`, `systemSpec` and the maps untouched (they are empty from the
//!   default constructors) and sets `lastGeneral`, `builtNuke`, ... to 0, so the `>= 0` tests in
//!   `formatPlayerKVPairs` are always true for a fresh record.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, RwLock};

/// `PerGeneralMap`.
pub type PerGeneralMap = BTreeMap<i32, u32>;

/// `class PSPlayerStats`.
#[derive(Clone, Debug, Default)]
pub struct PsPlayerStats {
    pub id: i32,
    pub wins: PerGeneralMap,
    pub losses: PerGeneralMap,
    pub games: PerGeneralMap,
    pub duration: PerGeneralMap,
    pub units_killed: PerGeneralMap,
    pub units_lost: PerGeneralMap,
    pub units_built: PerGeneralMap,
    pub buildings_killed: PerGeneralMap,
    pub buildings_lost: PerGeneralMap,
    pub buildings_built: PerGeneralMap,
    pub earnings: PerGeneralMap,
    pub tech_captured: PerGeneralMap,
    pub discons: PerGeneralMap,
    pub desyncs: PerGeneralMap,
    pub surrenders: PerGeneralMap,
    pub games_of_2p: PerGeneralMap,
    pub games_of_3p: PerGeneralMap,
    pub games_of_4p: PerGeneralMap,
    pub games_of_5p: PerGeneralMap,
    pub games_of_6p: PerGeneralMap,
    pub games_of_7p: PerGeneralMap,
    pub games_of_8p: PerGeneralMap,
    pub custom_games: PerGeneralMap,
    pub qm_games: PerGeneralMap,
    pub locale: i32,
    pub games_as_random: i32,
    pub options: Vec<u8>,
    pub system_spec: Vec<u8>,
    pub last_fps: f32,
    pub last_general: i32,
    pub games_in_row_with_last_general: i32,
    pub challenge_medals: i32,
    pub battle_honors: i32,
    pub qm_wins_in_a_row: i32,
    pub max_qm_wins_in_a_row: i32,
    pub wins_in_a_row: i32,
    pub max_wins_in_a_row: i32,
    pub losses_in_a_row: i32,
    pub max_losses_in_a_row: i32,
    pub discons_in_a_row: i32,
    pub max_discons_in_a_row: i32,
    pub desyncs_in_a_row: i32,
    pub max_desyncs_in_a_row: i32,
    pub built_particle_cannon: i32,
    pub built_nuke: i32,
    pub built_scud: i32,
    pub last_ladder_port: i32,
    pub last_ladder_host: Vec<u8>,
}

/// `INCORPORATE_MAP(x)`: copies only the entries of `other` that are greater than 0.
fn incorporate_map(dst: &mut PerGeneralMap, src: &PerGeneralMap) {
    for (k, v) in src.iter() {
        if *v > 0 {
            dst.insert(*k, *v);
        }
    }
}

impl PsPlayerStats {
    /// `PSPlayerStats::PSPlayerStats()` -> `reset()` (PersistentStorageThread.cpp).
    pub fn new() -> PsPlayerStats {
        let mut s = PsPlayerStats::default();
        s.reset();
        s
    }

    /// `PSPlayerStats::reset` (PersistentStorageThread.cpp): the scalars to 0 (see the module doc for what it leaves).
    pub fn reset(&mut self) {
        self.id = 0;
        self.locale = 0;
        self.games_as_random = 0;
        self.last_fps = 0.0;
        self.last_general = 0;
        self.games_in_row_with_last_general = 0;
        self.built_nuke = 0;
        self.built_scud = 0;
        self.built_particle_cannon = 0;
        self.challenge_medals = 0;
        self.battle_honors = 0;
        self.wins_in_a_row = 0;
        self.max_wins_in_a_row = 0;
        self.losses_in_a_row = 0;
        self.max_losses_in_a_row = 0;
        self.discons_in_a_row = 0;
        self.max_discons_in_a_row = 0;
        self.desyncs_in_a_row = 0;
        self.max_desyncs_in_a_row = 0;
        self.last_ladder_port = 0;
        self.max_qm_wins_in_a_row = 0;
        self.qm_wins_in_a_row = 0;
    }

    /// `PSPlayerStats::incorporate` (PersistentStorageThread.cpp): positive map entries replace, the discons map
    /// is cleared first, scalars replace when positive (or `>= 0` for the "unset is -1" ones), flags OR in.
    pub fn incorporate(&mut self, other: &PsPlayerStats) {
        incorporate_map(&mut self.wins, &other.wins);
        incorporate_map(&mut self.losses, &other.losses);
        incorporate_map(&mut self.games, &other.games);
        incorporate_map(&mut self.duration, &other.duration);
        incorporate_map(&mut self.units_killed, &other.units_killed);
        incorporate_map(&mut self.units_lost, &other.units_lost);
        incorporate_map(&mut self.units_built, &other.units_built);
        incorporate_map(&mut self.buildings_killed, &other.buildings_killed);
        incorporate_map(&mut self.buildings_lost, &other.buildings_lost);
        incorporate_map(&mut self.buildings_built, &other.buildings_built);
        incorporate_map(&mut self.earnings, &other.earnings);
        incorporate_map(&mut self.tech_captured, &other.tech_captured);

        // GS  Clear all disconnects so that we don't retain any that were previously reported as 1 by
        // updateAdditionalGameSpyDisconnections
        self.discons.clear();
        incorporate_map(&mut self.discons, &other.discons);

        incorporate_map(&mut self.desyncs, &other.desyncs);
        incorporate_map(&mut self.surrenders, &other.surrenders);
        incorporate_map(&mut self.games_of_2p, &other.games_of_2p);
        incorporate_map(&mut self.games_of_3p, &other.games_of_3p);
        incorporate_map(&mut self.games_of_4p, &other.games_of_4p);
        incorporate_map(&mut self.games_of_5p, &other.games_of_5p);
        incorporate_map(&mut self.games_of_6p, &other.games_of_6p);
        incorporate_map(&mut self.games_of_7p, &other.games_of_7p);
        incorporate_map(&mut self.games_of_8p, &other.games_of_8p);
        incorporate_map(&mut self.custom_games, &other.custom_games);
        incorporate_map(&mut self.qm_games, &other.qm_games);

        if other.locale > 0 {
            self.locale = other.locale;
        }
        if other.games_as_random > 0 {
            self.games_as_random = other.games_as_random;
        }
        if !other.options.is_empty() {
            self.options = other.options.clone();
        }
        if !other.system_spec.is_empty() {
            self.system_spec = other.system_spec.clone();
        }
        if other.last_fps > 0.0 {
            self.last_fps = other.last_fps;
        }
        if other.battle_honors > 0 {
            self.battle_honors |= other.battle_honors;
        }
        if other.challenge_medals > 0 {
            self.challenge_medals |= other.challenge_medals;
        }
        if other.last_general >= 0 {
            self.last_general = other.last_general;
        }
        if other.games_in_row_with_last_general >= 0 {
            self.games_in_row_with_last_general = other.games_in_row_with_last_general;
        }
        if other.built_particle_cannon >= 0 {
            self.built_particle_cannon = other.built_particle_cannon;
        }
        if other.built_nuke >= 0 {
            self.built_nuke = other.built_nuke;
        }
        if other.built_scud >= 0 {
            self.built_scud = other.built_scud;
        }
        if other.wins_in_a_row >= 0 {
            self.wins_in_a_row = other.wins_in_a_row;
        }
        if other.max_wins_in_a_row >= 0 {
            self.max_wins_in_a_row = other.max_wins_in_a_row;
        }
        if other.losses_in_a_row >= 0 {
            self.losses_in_a_row = other.losses_in_a_row;
        }
        if other.max_losses_in_a_row >= 0 {
            self.max_losses_in_a_row = other.max_losses_in_a_row;
        }
        if other.discons_in_a_row >= 0 {
            self.discons_in_a_row = other.discons_in_a_row;
        }
        if other.max_discons_in_a_row >= 0 {
            self.max_discons_in_a_row = other.max_discons_in_a_row;
        }
        if other.desyncs_in_a_row >= 0 {
            self.desyncs_in_a_row = other.desyncs_in_a_row;
        }
        if other.max_desyncs_in_a_row >= 0 {
            self.max_desyncs_in_a_row = other.max_desyncs_in_a_row;
        }
        if other.last_ladder_port >= 0 {
            self.last_ladder_port = other.last_ladder_port;
        }
        if !other.last_ladder_host.is_empty() {
            self.last_ladder_host = other.last_ladder_host.clone();
        }
    }

    /// `PSPlayerStats::PSPlayerStats(const PSPlayerStats&)` (PersistentStorageThread.cpp): `incorporate(other)` onto
    /// an empty record, then the scalars copied outright. The QM-wins-in-a-row pair is not copied (see the module
    /// doc).
    pub fn copy_constructed(other: &PsPlayerStats) -> PsPlayerStats {
        let mut s = PsPlayerStats::new();
        s.incorporate(other);
        s.id = other.id;
        s.locale = other.locale;
        s.games_as_random = other.games_as_random;
        s.options = other.options.clone();
        s.system_spec = other.system_spec.clone();
        s.last_fps = other.last_fps;
        s.last_general = other.last_general;
        s.games_in_row_with_last_general = other.games_in_row_with_last_general;
        s.built_particle_cannon = other.built_particle_cannon;
        s.built_nuke = other.built_nuke;
        s.built_scud = other.built_scud;
        s.challenge_medals = other.challenge_medals;
        s.battle_honors = other.battle_honors;
        s.wins_in_a_row = other.wins_in_a_row;
        s.max_wins_in_a_row = other.max_wins_in_a_row;
        s.losses_in_a_row = other.losses_in_a_row;
        s.max_losses_in_a_row = other.max_losses_in_a_row;
        s.discons_in_a_row = other.discons_in_a_row;
        s.max_discons_in_a_row = other.max_discons_in_a_row;
        s.desyncs_in_a_row = other.desyncs_in_a_row;
        s.max_desyncs_in_a_row = other.max_desyncs_in_a_row;
        s.last_ladder_host = other.last_ladder_host.clone();
        s.last_ladder_port = other.last_ladder_port;
        s
    }
}

/// C `atoi`: leading whitespace, optional sign, digits; 0 if none. Overflow wraps (undefined in C).
pub fn atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || (9..=13).contains(&s[i])) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
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

/// C `atof` for the plain decimal/exponent forms the stats use: the longest numeric prefix, 0 if none.
pub fn atof(s: &[u8]) -> f64 {
    let t: Vec<u8> = {
        let mut i = 0;
        while i < s.len() && (s[i] == b' ' || (9..=13).contains(&s[i])) {
            i += 1;
        }
        s[i..].to_vec()
    };
    let mut end = 0;
    let mut best = 0.0;
    let text = String::from_utf8_lossy(&t).into_owned();
    for e in 1..=text.len() {
        if !text.is_char_boundary(e) {
            continue;
        }
        if let Ok(v) = text[..e].parse::<f64>() {
            if text.as_bytes()[e - 1].is_ascii_digit() || text.as_bytes()[e - 1] == b'.' {
                best = v;
                end = e;
            }
        }
    }
    let _ = end;
    best
}

/// C `printf("%g", v)` (six significant digits, trailing zeros dropped, exponent form outside 1e-4..1e6).
pub fn format_g(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    if !v.is_finite() {
        return if v.is_nan() { "nan".to_string() } else if v < 0.0 { "-inf".to_string() } else { "inf".to_string() };
    }
    let exp = format!("{:.5e}", v); // d.ddddde[-]X
    let (mant, e) = exp.split_once('e').unwrap_or((&exp, "0"));
    let x: i32 = e.parse().unwrap_or(0);
    if x < -4 || x >= 6 {
        let mut m = mant.to_string();
        if m.contains('.') {
            while m.ends_with('0') {
                m.pop();
            }
            if m.ends_with('.') {
                m.pop();
            }
        }
        let sign = if x < 0 { '-' } else { '+' };
        return format!("{}e{}{:02}", m, sign, x.abs());
    }
    let decimals = (5 - x).max(0) as usize;
    let mut s = format!("{:.*}", decimals, v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

fn find_first_of(s: &[u8], c: u8, from: usize) -> Option<usize> {
    if from >= s.len() {
        return None;
    }
    s[from..].iter().position(|&b| b == c).map(|p| p + from)
}

/// `std::string::find_last_not_of("0123456789", pos)`: the last index `<= pos` that is not a digit.
fn find_last_not_of_digits(s: &[u8], pos: usize) -> Option<usize> {
    if s.is_empty() {
        return None;
    }
    let mut i = pos.min(s.len() - 1) as isize;
    while i >= 0 {
        if !s[i as usize].is_ascii_digit() {
            return Some(i as usize);
        }
        i -= 1;
    }
    None
}

/// `GameSpyPSMessageQueueInterface::parsePlayerKVPairs` (PersistentStorageThread.cpp): reads `\key[general]\value`
/// triples; a key ending in digits is a per-general map entry (`CHECK(x)`), otherwise a scalar. After each triple
/// the scan restarts at `thirdMarker - 1`, as upstream. Unknown keys are ignored.
pub fn parse_player_kv_pairs(kv_pairs: &[u8]) -> PsPlayerStats {
    let mut s = PsPlayerStats::new();
    let mut kv = kv_pairs.to_vec();
    kv.push(b'\\');

    let mut offset: usize = 0;
    loop {
        let first_marker = match find_first_of(&kv, b'\\', offset) {
            Some(p) => p,
            None => break,
        };
        let second_marker = match find_first_of(&kv, b'\\', first_marker + 1) {
            Some(p) => p,
            None => break,
        };
        let third_marker = match find_first_of(&kv, b'\\', second_marker + 1) {
            Some(p) => p,
            None => break,
        };
        // second_marker >= 1 always (it is after first_marker)
        let general_marker_pos = find_last_not_of_digits(&kv, second_marker - 1);
        let k: Vec<u8>;
        let general_marker: i32;
        match general_marker_pos {
            Some(gm) if gm == second_marker - 1 => {
                k = kv[first_marker + 1..second_marker].to_vec();
                general_marker = -1;
            }
            Some(gm) => {
                k = kv[first_marker + 1..gm + 1].to_vec();
                let g = &kv[gm + 1..second_marker];
                general_marker = atoi(g);
            }
            None => {
                // Not reachable (the backslash at first_marker is not a digit); upstream's npos arithmetic.
                k = kv[first_marker + 1..].to_vec();
                general_marker = atoi(&kv[..second_marker]);
            }
        }
        let v = &kv[second_marker + 1..third_marker];
        offset = third_marker - 1;

        if general_marker >= 0 {
            let gm = general_marker;
            let val = atoi(v) as u32;
            let target: Option<&mut PerGeneralMap> = match &k[..] {
                b"wins" => Some(&mut s.wins),
                b"losses" => Some(&mut s.losses),
                b"games" => Some(&mut s.games),
                b"duration" => Some(&mut s.duration),
                b"unitsKilled" => Some(&mut s.units_killed),
                b"unitsLost" => Some(&mut s.units_lost),
                b"unitsBuilt" => Some(&mut s.units_built),
                b"buildingsKilled" => Some(&mut s.buildings_killed),
                b"buildingsLost" => Some(&mut s.buildings_lost),
                b"buildingsBuilt" => Some(&mut s.buildings_built),
                b"earnings" => Some(&mut s.earnings),
                b"techCaptured" => Some(&mut s.tech_captured),
                b"discons" => Some(&mut s.discons),
                b"desyncs" => Some(&mut s.desyncs),
                b"surrenders" => Some(&mut s.surrenders),
                b"gamesOf2p" => Some(&mut s.games_of_2p),
                b"gamesOf3p" => Some(&mut s.games_of_3p),
                b"gamesOf4p" => Some(&mut s.games_of_4p),
                b"gamesOf5p" => Some(&mut s.games_of_5p),
                b"gamesOf6p" => Some(&mut s.games_of_6p),
                b"gamesOf7p" => Some(&mut s.games_of_7p),
                b"gamesOf8p" => Some(&mut s.games_of_8p),
                b"customGames" => Some(&mut s.custom_games),
                b"QMGames" => Some(&mut s.qm_games),
                _ => None,
            };
            if let Some(m) = target {
                m.insert(gm, val);
                continue;
            }
        }
        if general_marker < 0 {
            match &k[..] {
                b"locale" => s.locale = atoi(v),
                b"random" => s.games_as_random = atoi(v),
                b"options" => s.options = v.to_vec(),
                b"systemSpec" => s.system_spec = v.to_vec(),
                b"fps" => s.last_fps = atof(v) as f32,
                b"lastGeneral" => s.last_general = atoi(v),
                b"genInRow" => s.games_in_row_with_last_general = atoi(v),
                b"builtNuke" => s.built_nuke = atoi(v),
                b"builtSCUD" => s.built_scud = atoi(v),
                b"builtCannon" => s.built_particle_cannon = atoi(v),
                b"challenge" => s.challenge_medals = atoi(v),
                b"battle" => s.battle_honors = atoi(v),
                b"WinRow" => s.wins_in_a_row = atoi(v),
                b"WinRowMax" => s.max_wins_in_a_row = atoi(v),
                b"LossRow" => s.losses_in_a_row = atoi(v),
                b"LossRowMax" => s.max_losses_in_a_row = atoi(v),
                b"DSRow" => s.desyncs_in_a_row = atoi(v),
                b"DSRowMax" => s.max_desyncs_in_a_row = atoi(v),
                b"DCRow" => s.discons_in_a_row = atoi(v),
                b"DCRowMax" => s.max_discons_in_a_row = atoi(v),
                b"ladderPort" => s.last_ladder_port = atoi(v),
                b"ladderHost" => s.last_ladder_host = v.to_vec(),
                _ => {}
            }
        }
    }
    s
}

/// `GameSpyPSMessageQueueInterface::formatPlayerKVPairs` (PersistentStorageThread.cpp): the stats as
/// `\key<general>\value` text. `ITERATE_OVER` writes only entries greater than 0; `discons` is written for every
/// player template index `0..player_template_count` (zero included); the `*InARow` pair is always written and the
/// `max*` ones only when positive; `options`, `systemSpec` and `ladderHost` lines are cut to 255 characters as
/// `_snprintf(kvbuf, 256)` + `kvbuf[255] = 0` does.
pub fn format_player_kv_pairs(stats: &PsPlayerStats, player_template_count: i32) -> Vec<u8> {
    let mut s: Vec<u8> = Vec::new();
    let iterate = |s: &mut Vec<u8>, name: &str, m: &PerGeneralMap| {
        for (k, v) in m.iter() {
            if *v > 0 {
                s.extend_from_slice(format!("\\{}{}\\{}", name, k, v).as_bytes());
            }
        }
    };
    let text_capped = |s: &mut Vec<u8>, prefix: &str, val: &[u8]| {
        let mut line = prefix.as_bytes().to_vec();
        line.extend_from_slice(val);
        line.truncate(255);
        s.extend_from_slice(&line);
    };

    iterate(&mut s, "wins", &stats.wins);
    iterate(&mut s, "losses", &stats.losses);
    iterate(&mut s, "games", &stats.games);
    iterate(&mut s, "duration", &stats.duration);
    iterate(&mut s, "unitsKilled", &stats.units_killed);
    iterate(&mut s, "unitsLost", &stats.units_lost);
    iterate(&mut s, "unitsBuilt", &stats.units_built);
    iterate(&mut s, "buildingsKilled", &stats.buildings_killed);
    iterate(&mut s, "buildingsLost", &stats.buildings_lost);
    iterate(&mut s, "buildingsBuilt", &stats.buildings_built);
    iterate(&mut s, "earnings", &stats.earnings);
    iterate(&mut s, "techCaptured", &stats.tech_captured);

    // GS  Report all disconnects, even if zero (stats.discons[ptIdx] reads 0 for a missing entry)
    for pt_idx in 0..player_template_count {
        let v = stats.discons.get(&pt_idx).copied().unwrap_or(0);
        s.extend_from_slice(format!("\\discons{}\\{}", pt_idx, v).as_bytes());
    }

    iterate(&mut s, "desyncs", &stats.desyncs);
    iterate(&mut s, "surrenders", &stats.surrenders);
    iterate(&mut s, "gamesOf2p", &stats.games_of_2p);
    iterate(&mut s, "gamesOf3p", &stats.games_of_3p);
    iterate(&mut s, "gamesOf4p", &stats.games_of_4p);
    iterate(&mut s, "gamesOf5p", &stats.games_of_5p);
    iterate(&mut s, "gamesOf6p", &stats.games_of_6p);
    iterate(&mut s, "gamesOf7p", &stats.games_of_7p);
    iterate(&mut s, "gamesOf8p", &stats.games_of_8p);
    iterate(&mut s, "customGames", &stats.custom_games);
    iterate(&mut s, "QMGames", &stats.qm_games);

    if stats.locale > 0 {
        s.extend_from_slice(format!("\\locale\\{}", stats.locale).as_bytes());
    }
    if stats.games_as_random > 0 {
        s.extend_from_slice(format!("\\random\\{}", stats.games_as_random).as_bytes());
    }
    if !stats.options.is_empty() {
        text_capped(&mut s, "\\options\\", &stats.options);
    }
    if !stats.system_spec.is_empty() {
        text_capped(&mut s, "\\systemSpec\\", &stats.system_spec);
    }
    if stats.last_fps > 0.0 {
        s.extend_from_slice(format!("\\fps\\{}", format_g(stats.last_fps as f64)).as_bytes());
    }
    if stats.last_general >= 0 {
        s.extend_from_slice(format!("\\lastGeneral\\{}", stats.last_general).as_bytes());
    }
    if stats.games_in_row_with_last_general >= 0 {
        s.extend_from_slice(format!("\\genInRow\\{}", stats.games_in_row_with_last_general).as_bytes());
    }
    if stats.built_particle_cannon >= 0 {
        s.extend_from_slice(format!("\\builtCannon\\{}", stats.built_particle_cannon).as_bytes());
    }
    if stats.built_nuke >= 0 {
        s.extend_from_slice(format!("\\builtNuke\\{}", stats.built_nuke).as_bytes());
    }
    if stats.built_scud >= 0 {
        s.extend_from_slice(format!("\\builtSCUD\\{}", stats.built_scud).as_bytes());
    }
    if stats.challenge_medals > 0 {
        s.extend_from_slice(format!("\\challenge\\{}", stats.challenge_medals).as_bytes());
    }
    if stats.battle_honors > 0 {
        s.extend_from_slice(format!("\\battle\\{}", stats.battle_honors).as_bytes());
    }

    s.extend_from_slice(format!("\\WinRow\\{}", stats.wins_in_a_row).as_bytes());
    if stats.max_wins_in_a_row > 0 {
        s.extend_from_slice(format!("\\WinRowMax\\{}", stats.max_wins_in_a_row).as_bytes());
    }
    s.extend_from_slice(format!("\\LossRow\\{}", stats.losses_in_a_row).as_bytes());
    if stats.max_losses_in_a_row > 0 {
        s.extend_from_slice(format!("\\LossRowMax\\{}", stats.max_losses_in_a_row).as_bytes());
    }
    s.extend_from_slice(format!("\\DCRow\\{}", stats.discons_in_a_row).as_bytes());
    if stats.max_discons_in_a_row > 0 {
        s.extend_from_slice(format!("\\DCRowMax\\{}", stats.max_discons_in_a_row).as_bytes());
    }
    s.extend_from_slice(format!("\\DSRow\\{}", stats.desyncs_in_a_row).as_bytes());
    if stats.max_desyncs_in_a_row > 0 {
        s.extend_from_slice(format!("\\DSRowMax\\{}", stats.max_desyncs_in_a_row).as_bytes());
    }
    if stats.last_ladder_port > 0 {
        s.extend_from_slice(format!("\\ladderPort\\{}", stats.last_ladder_port).as_bytes());
    }
    if !stats.last_ladder_host.is_empty() {
        text_capped(&mut s, "\\ladderHost\\", &stats.last_ladder_host);
    }
    s
}

/// `PSRequest::requestType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsRequestType {
    /// `PSREQUEST_READPLAYERSTATS`.
    ReadPlayerStats,
    /// `PSREQUEST_UPDATEPLAYERSTATS`.
    UpdatePlayerStats,
    /// `PSREQUEST_UPDATEPLAYERLOCALE`.
    UpdatePlayerLocale,
    /// `PSREQUEST_READCDKEYSTATS`.
    ReadCdKeyStats,
    /// `PSREQUEST_SENDGAMERESTOGAMESPY`.
    SendGameResToGameSpy,
}

/// `class PSRequest`.
#[derive(Clone, Debug)]
pub struct PsRequest {
    /// `requestType`.
    pub request_type: PsRequestType,
    /// `player`.
    pub player: PsPlayerStats,
    /// `cdkey`.
    pub cdkey: Vec<u8>,
    /// `nick`.
    pub nick: Vec<u8>,
    /// `password`.
    pub password: Vec<u8>,
    /// `email`.
    pub email: Vec<u8>,
    /// `addDiscon`.
    pub add_discon: bool,
    /// `addDesync`.
    pub add_desync: bool,
    /// `lastHouse`.
    pub last_house: i32,
    /// `results`.
    pub results: Vec<u8>,
}

impl PsRequest {
    /// `PSRequest::PSRequest()` (PersistentStorageThread.cpp): `player.reset()`, read-stats, no add flags,
    /// `lastHouse = -1`.
    pub fn new() -> PsRequest {
        PsRequest {
            request_type: PsRequestType::ReadPlayerStats,
            player: PsPlayerStats::new(),
            cdkey: Vec::new(),
            nick: Vec::new(),
            password: Vec::new(),
            email: Vec::new(),
            add_discon: false,
            add_desync: false,
            last_house: -1,
            results: Vec::new(),
        }
    }
}

impl Default for PsRequest {
    fn default() -> Self {
        PsRequest::new()
    }
}

/// `PSResponse::responseType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsResponseType {
    /// `PSRESPONSE_PLAYERSTATS`.
    PlayerStats,
    /// `PSRESPONSE_COULDNOTCONNECT`.
    CouldNotConnect,
    /// `PSRESPONSE_PREORDER`.
    Preorder,
}

/// `class PSResponse`.
#[derive(Clone, Debug)]
pub struct PsResponse {
    /// `responseType` (uninitialised in C++ until set; `PlayerStats` here).
    pub response_type: PsResponseType,
    /// `player`.
    pub player: PsPlayerStats,
    /// `preorder`.
    pub preorder: bool,
}

impl PsResponse {
    /// A fresh response as `PSResponse resp;` (player reset, preorder false).
    pub fn new() -> PsResponse {
        PsResponse { response_type: PsResponseType::PlayerStats, player: PsPlayerStats::new(), preorder: false }
    }
}

impl Default for PsResponse {
    fn default() -> Self {
        PsResponse::new()
    }
}

#[derive(Debug, Default)]
struct PsQueues {
    requests: VecDeque<PsRequest>,
    responses: VecDeque<PsResponse>,
    /// `m_playerStats`.
    player_stats: BTreeMap<i32, PsPlayerStats>,
    /// `m_localPlayerID`.
    local_player_id: i32,
    /// `m_email`, `m_nick`, `m_password`.
    email: Vec<u8>,
    nick: Vec<u8>,
    password: Vec<u8>,
    /// Whether the thread exists (`m_thread != NULL`).
    thread_running: bool,
}

/// `class GameSpyPSMessageQueue` (PersistentStorageThread.cpp).
#[derive(Clone, Debug, Default)]
pub struct PsMessageQueue {
    inner: Arc<RwLock<PsQueues>>,
}

impl PsMessageQueue {
    /// `createNewMessageQueue` / `GameSpyPSMessageQueue()`: no thread, local player 0.
    pub fn new() -> PsMessageQueue {
        PsMessageQueue::default()
    }
    /// `startThread` (the OS thread is the owner's job).
    pub fn start_thread(&self) {
        if let Ok(mut q) = self.inner.write() {
            q.thread_running = true;
        }
    }
    /// `endThread`.
    pub fn end_thread(&self) {
        if let Ok(mut q) = self.inner.write() {
            q.thread_running = false;
        }
    }
    /// `isThreadRunning`.
    pub fn is_thread_running(&self) -> bool {
        self.inner.read().map(|q| q.thread_running).unwrap_or(false)
    }
    /// `addRequest`.
    pub fn add_request(&self, req: PsRequest) {
        if let Ok(mut q) = self.inner.write() {
            q.requests.push_back(req);
        }
    }
    /// `getRequest`.
    pub fn get_request(&self) -> Option<PsRequest> {
        self.inner.write().ok().and_then(|mut q| q.requests.pop_front())
    }
    /// `addResponse`.
    pub fn add_response(&self, resp: PsResponse) {
        if let Ok(mut q) = self.inner.write() {
            q.responses.push_back(resp);
        }
    }
    /// `getResponse`.
    pub fn get_response(&self) -> Option<PsResponse> {
        self.inner.write().ok().and_then(|mut q| q.responses.pop_front())
    }
    /// `trackPlayerStats` (PersistentStorageThread.cpp): a known id has its record copied by value (the copy
    /// constructor, see the module doc), the new stats incorporated, and stored back; an unknown id stores `stats`.
    pub fn track_player_stats(&self, stats: PsPlayerStats) {
        if let Ok(mut q) = self.inner.write() {
            if let Some(existing) = q.player_stats.get(&stats.id) {
                let mut new_stats = PsPlayerStats::copy_constructed(existing);
                new_stats.incorporate(&stats);
                q.player_stats.insert(stats.id, new_stats);
            } else {
                q.player_stats.insert(stats.id, stats);
            }
        }
    }
    /// `findPlayerStatsByID`: a copy of the record (via the copy constructor), or an empty record with `id` 0.
    pub fn find_player_stats_by_id(&self, id: i32) -> PsPlayerStats {
        if let Ok(q) = self.inner.read() {
            if let Some(s) = q.player_stats.get(&id) {
                return PsPlayerStats::copy_constructed(s);
            }
        }
        let mut empty = PsPlayerStats::new();
        empty.id = 0;
        empty
    }
    /// `getLocalPlayerID`.
    pub fn get_local_player_id(&self) -> i32 {
        self.inner.read().map(|q| q.local_player_id).unwrap_or(0)
    }
    /// `setLocalPlayerID`.
    pub fn set_local_player_id(&self, id: i32) {
        if let Ok(mut q) = self.inner.write() {
            q.local_player_id = id;
        }
    }
    /// `getEmail`.
    pub fn get_email(&self) -> Vec<u8> {
        self.inner.read().map(|q| q.email.clone()).unwrap_or_default()
    }
    /// `getNick`.
    pub fn get_nick(&self) -> Vec<u8> {
        self.inner.read().map(|q| q.nick.clone()).unwrap_or_default()
    }
    /// `getPassword`.
    pub fn get_password(&self) -> Vec<u8> {
        self.inner.read().map(|q| q.password.clone()).unwrap_or_default()
    }
    /// `setEmail`.
    pub fn set_email(&self, v: Vec<u8>) {
        if let Ok(mut q) = self.inner.write() {
            q.email = v;
        }
    }
    /// `setNick`.
    pub fn set_nick(&self, v: Vec<u8>) {
        if let Ok(mut q) = self.inner.write() {
            q.nick = v;
        }
    }
    /// `setPassword`.
    pub fn set_password(&self, v: Vec<u8>) {
        if let Ok(mut q) = self.inner.write() {
            q.password = v;
        }
    }
}

/// `CDAuthInfo` (PersistentStorageThread.cpp).
#[derive(Clone, Copy, Debug, Default)]
pub struct CdAuthInfo {
    /// `success`.
    pub success: bool,
    /// `done`.
    pub done: bool,
    /// `id`.
    pub id: i32,
}

/// The GameSpy stats SDK and the files and globals the thread touches.
pub trait PersistSdkHost {
    /// `IsStatsConnected()`.
    fn is_stats_connected(&self) -> bool;
    /// `InitStatsConnection(0) == GE_NOERROR`.
    fn init_stats_connection(&mut self) -> bool;
    /// `CloseStatsConnection()`.
    fn close_stats_connection(&mut self);
    /// `PersistThink()`; the SDK fires [`PsThread::pers_auth_callback`], [`PsThread::get_persistent_data_callback`]
    /// and friends during it.
    fn persist_think(&mut self);
    /// `GenerateAuth(GetChallenge(NULL), password, out)`: the 32-char validation token.
    fn generate_auth_with_challenge(&mut self, password: &[u8]) -> Vec<u8>;
    /// `GenerateAuth("", cdkey, out)`: the cdkey hash.
    fn generate_auth_empty_challenge(&mut self, cdkey: &[u8]) -> Vec<u8>;
    /// `PreAuthenticatePlayerPM(id, id, validate, persAuthCallback, thread)`.
    fn pre_authenticate_player_pm(&mut self, id: i32, validate: &[u8]);
    /// `PreAuthenticatePlayerCD(0, "preorder", cdkeyHash, validationToken, preAuthCDCallback, &info)`; the
    /// SDK fires [`PsThread::pre_auth_cd_callback`] during `persist_think`.
    fn pre_authenticate_player_cd(&mut self, cdkey_hash: &[u8], validation_token: &[u8]);
    /// `GetPersistDataValues(0, id, pd_public_rw, 0, "", getPersistentDataCallback, thread)`.
    fn get_persist_data_values_public_rw(&mut self, id: i32);
    /// `GetPersistDataValues(0, id, pd_public_ro, 0, "\\preorder", getPreorderCallback, thread)`.
    fn get_persist_data_values_preorder(&mut self, id: i32);
    /// `SetPersistDataValues(0, id, pd_public_rw, 0, kv, callback, thread)`; `locale_only` picks
    /// `setPersistentDataLocaleCallback` over `setPersistentDataCallback`.
    fn set_persist_data_values(&mut self, id: i32, kv: &[u8], locale_only: bool);
    /// `NewGame(0)`.
    fn new_game(&mut self);
    /// `SendGameSnapShot(NULL, results, SNAP_FINAL)`; returns its result.
    fn send_game_snapshot_final(&mut self, results: &[u8]) -> i32;
    /// `FreeGame(NULL)`.
    fn free_game(&mut self);
    /// `UserPreferences::load(filename); getInt(key, default)`.
    fn user_pref_get_int(&mut self, filename: &str, key: &str, default: i32) -> i32;
    /// `load(filename); pref[key] = value ...; write()` for each entry.
    fn user_pref_set_and_write(&mut self, filename: &str, entries: &[(String, String)]);
    /// `load(filename); clear(); write()`.
    fn user_pref_clear_and_write(&mut self, filename: &str);
    /// `TheGameSpyGame && TheGameSpyGame->getUseStats()`.
    fn game_spy_game_uses_stats(&self) -> bool;
    /// `ThePlayerTemplateStore->getPlayerTemplateCount()`.
    fn player_template_count(&self) -> i32;
}

/// `class PSThreadClass` (PersistentStorageThread.cpp).
#[derive(Debug)]
pub struct PsThread {
    /// `TheGameSpyPSMessageQueue`.
    pub queue: PsMessageQueue,
    /// `m_loginOK`.
    login_ok: bool,
    /// `m_doneTryingToLogin`.
    done_trying_to_login: bool,
    /// `m_opCount`.
    op_count: i32,
    /// `m_sawLocalData`.
    saw_local_data: bool,
    /// The `CDAuthInfo` of an in-flight `PSREQUEST_READCDKEYSTATS`.
    cd_auth: CdAuthInfo,
}

impl PsThread {
    /// `PSThreadClass::PSThreadClass()`: all false, op count 0.
    pub fn new(queue: PsMessageQueue) -> PsThread {
        PsThread {
            queue,
            login_ok: false,
            done_trying_to_login: false,
            op_count: 0,
            saw_local_data: false,
            cd_auth: CdAuthInfo::default(),
        }
    }
    /// `decrOpCount`.
    pub fn decr_op_count(&mut self) {
        self.op_count -= 1;
    }
    /// `incrOpCount`.
    pub fn incr_op_count(&mut self) {
        self.op_count += 1;
    }
    /// `getOpCount`.
    pub fn get_op_count(&self) -> i32 {
        self.op_count
    }
    /// `sawLocalPlayerData`.
    pub fn saw_local_player_data(&self) -> bool {
        self.saw_local_data
    }
    /// `gotLocalPlayerData`.
    pub fn got_local_player_data(&mut self) {
        self.saw_local_data = true;
    }

    /// `PSThreadClass::tryConnect` (PersistentStorageThread.cpp): true if already connected, else whether
    /// `InitStatsConnection(0)` succeeded.
    pub fn try_connect<H: PersistSdkHost>(&mut self, host: &mut H) -> bool {
        if host.is_stats_connected() {
            return true;
        }
        host.init_stats_connection()
    }

    /// `persAuthCallback` (PersistentStorageThread.cpp, the static one): `t->persAuthCallback(authenticated != 0)`.
    pub fn pers_auth_callback(&mut self, authenticated: bool) {
        self.login_ok = authenticated;
        self.done_trying_to_login = true;
    }

    /// `PSThreadClass::tryLogin` (PersistentStorageThread.cpp): authenticates `id` with a token made from the
    /// password and the SDK challenge, then pumps `PersistThink` until the callback fires or the connection drops.
    pub fn try_login<H: PersistSdkHost>(&mut self, host: &mut H, id: i32, _nick: &[u8], password: &[u8], _email: &[u8]) -> bool {
        let validate = host.generate_auth_with_challenge(password);
        self.login_ok = false;
        self.done_trying_to_login = false;
        host.pre_authenticate_player_pm(id, &validate);
        while !self.done_trying_to_login && host.is_stats_connected() {
            host.persist_think();
        }
        self.login_ok
    }

    /// The "added in desyncs/discons" block shared by `getPersistentDataCallback` and `PSREQUEST_UPDATEPLAYERSTATS`
    /// (six `MiscPref<id>.ini` ints `"0".."5"`, each negative one read as 10): (desync2, desync3, desync4, discon2,
    /// discon3, discon4).
    fn read_added<H: PersistSdkHost>(&self, host: &mut H) -> [i32; 6] {
        let filename = format!("GeneralsOnline\\MiscPref{}.ini", self.queue.get_local_player_id());
        let mut out = [0; 6];
        for i in 0..6 {
            let mut v = host.user_pref_get_int(&filename, &i.to_string(), 0);
            if v < 0 {
                v = 10;
            }
            out[i] = v;
        }
        out
    }

    /// `getPersistentDataCallback` (PersistentStorageThread.cpp): a failed read posts
    /// `PSRESPONSE_COULDNOTCONNECT` and, when no ops remain and the local record never arrived, requeues a read of
    /// the local player; a good read of the local player (in a stats game) checks the stored added desyncs/discons
    /// and, if any, requeues an update; then posts `PSRESPONSE_PLAYERSTATS`.
    pub fn get_persistent_data_callback<H: PersistSdkHost>(&mut self, host: &mut H, profileid: i32, success: bool, data: &[u8]) {
        self.decr_op_count();

        let mut resp = PsResponse::new();
        if !success {
            resp.response_type = PsResponseType::CouldNotConnect;
            resp.player.id = profileid;
            self.queue.add_response(resp);
            if self.get_op_count() == 0 && !self.saw_local_player_data() {
                // we haven't gotten stats for ourselves - try again
                let mut req = PsRequest::new();
                req.request_type = PsRequestType::ReadPlayerStats;
                req.player.id = self.queue.get_local_player_id();
                self.queue.add_request(req);
            }
            return;
        }

        if profileid == self.queue.get_local_player_id() && host.game_spy_game_uses_stats() {
            self.got_local_player_data();
            let a = self.read_added(host);
            if a.iter().any(|&v| v != 0) {
                // We have a previous discon we can attempt to update!
                let mut req = PsRequest::new();
                req.request_type = PsRequestType::UpdatePlayerStats;
                req.email = self.queue.get_email();
                req.nick = self.queue.get_nick();
                req.password = self.queue.get_password();
                req.player = parse_player_kv_pairs(data);
                req.player.id = profileid;
                req.add_desync = false;
                req.add_discon = false;
                req.last_house = 0;
                self.queue.add_request(req);
            }
        }

        resp.response_type = PsResponseType::PlayerStats;
        resp.player = parse_player_kv_pairs(data);
        resp.player.id = profileid;
        self.queue.add_response(resp);
    }

    /// `setPersistentDataLocaleCallback`: `decrOpCount()`.
    pub fn set_persistent_data_locale_callback(&mut self) {
        self.decr_op_count();
    }

    /// `setPersistentDataCallback`: on success clears the player's `MiscPref<profileid>.ini`; `decrOpCount()`.
    pub fn set_persistent_data_callback<H: PersistSdkHost>(&mut self, host: &mut H, profileid: i32, success: bool) {
        if success {
            let filename = format!("GeneralsOnline\\MiscPref{}.ini", profileid);
            host.user_pref_clear_and_write(&filename);
        }
        self.decr_op_count();
    }

    /// `preAuthCDCallback`: records `success`, `done` and the profile id for the waiting READCDKEYSTATS.
    pub fn pre_auth_cd_callback(&mut self, profileid: i32, authenticated: bool) {
        self.cd_auth.success = authenticated;
        self.cd_auth.done = true;
        self.cd_auth.id = profileid;
    }

    /// `getPreorderCallback`: `decrOpCount()`; on success posts `PSRESPONSE_PREORDER` with whether the data is
    /// exactly `\preorder\1`.
    pub fn get_preorder_callback(&mut self, success: bool, data: &[u8]) {
        self.decr_op_count();
        if !success {
            return;
        }
        let mut resp = PsResponse::new();
        resp.response_type = PsResponseType::Preorder;
        resp.preorder = data == b"\\preorder\\1";
        self.queue.add_response(resp);
    }

    /// One pass of `Thread_Function`'s `while (running)` body: deal with one request, then pump the connection and
    /// close it when no operations remain. Returns false if the thread function returned (see the module doc);
    /// the owner must then stop calling.
    pub fn thread_step<H: PersistSdkHost>(&mut self, host: &mut H, running: bool) -> bool {
        if let Some(req) = self.queue.get_request() {
            if !self.handle_request(host, req, running) {
                return false;
            }
        }

        // update the network
        if host.is_stats_connected() {
            host.persist_think();
            if self.op_count <= 0 {
                host.close_stats_connection();
                self.op_count = 0;
            }
        }
        true
    }

    /// The tail of `Thread_Function` after `running` went false.
    pub fn thread_end<H: PersistSdkHost>(&mut self, host: &mut H) {
        if host.is_stats_connected() {
            host.close_stats_connection();
        }
    }

    /// The request `switch` of `Thread_Function`. False means upstream `return`ed from the thread function.
    pub fn handle_request<H: PersistSdkHost>(&mut self, host: &mut H, mut req: PsRequest, running: bool) -> bool {
        match req.request_type {
            PsRequestType::SendGameResToGameSpy => {
                if self.try_connect(host) {
                    host.new_game();
                    let _res = host.send_game_snapshot_final(&req.results);
                    host.free_game();
                }
            }
            PsRequestType::ReadPlayerStats => {
                if self.queue.get_local_player_id() == 0 {
                    self.queue.set_local_player_id(req.player.id); // first request is for ourselves
                    self.queue.set_email(req.email.clone());
                    self.queue.set_nick(req.nick.clone());
                    self.queue.set_password(req.password.clone());
                }
                if self.try_connect(host) {
                    self.incr_op_count();
                    host.get_persist_data_values_public_rw(req.player.id);
                }
            }
            PsRequestType::UpdatePlayerLocale => {
                if self.try_connect(host) && self.try_login(host, req.player.id, &req.nick, &req.password, &req.email) {
                    let kv = format!("\\locale\\{}", req.player.locale);
                    self.incr_op_count();
                    host.set_persist_data_values(req.player.id, kv.as_bytes(), true);
                }
            }
            PsRequestType::UpdatePlayerStats => {
                // NOTE THAT THIS IS HIGHLY DEPENDENT ON INI ORDERING FOR THE PLAYERTEMPLATES!!!
                let filename = format!("GeneralsOnline\\MiscPref{}.ini", self.queue.get_local_player_id());
                let a = self.read_added(host);
                let (added_desyncs2, added_desyncs3, added_desyncs4) = (a[0], a[1], a[2]);
                let (added_discons2, added_discons3, added_discons4) = (a[3], a[4], a[5]);
                let add_desync = req.add_desync as i32;
                let add_discon = req.add_discon as i32;

                if req.add_desync || req.add_discon {
                    let mut entries: Vec<(String, String)> = Vec::new();
                    if req.last_house == 2 {
                        entries.push(("0".to_string(), (added_desyncs2 + add_desync).to_string()));
                        entries.push(("3".to_string(), (added_discons2 + add_discon).to_string()));
                    } else if req.last_house == 3 {
                        entries.push(("1".to_string(), (added_desyncs3 + add_desync).to_string()));
                        entries.push(("4".to_string(), (added_discons3 + add_discon).to_string()));
                    } else {
                        entries.push(("2".to_string(), (added_desyncs4 + add_desync).to_string()));
                        entries.push(("5".to_string(), (added_discons4 + add_discon).to_string()));
                    }
                    host.user_pref_set_and_write(&filename, &entries);
                    if req.password.is_empty() {
                        return false; // upstream `return`s out of Thread_Function
                    }
                }
                if req.player.id == 0 {
                    return false; // Bailing because ID is NULL! (upstream `return`s out of Thread_Function)
                }
                let bump = |m: &mut PerGeneralMap, k: i32, by: i32| {
                    let e = m.entry(k).or_insert(0);
                    *e = e.wrapping_add(by as u32);
                };
                bump(&mut req.player.desyncs, 2, added_desyncs2);
                bump(&mut req.player.games, 2, added_desyncs2);
                bump(&mut req.player.discons, 2, added_discons2);
                bump(&mut req.player.games, 2, added_discons2);
                bump(&mut req.player.desyncs, 3, added_desyncs3);
                bump(&mut req.player.games, 3, added_desyncs3);
                bump(&mut req.player.discons, 3, added_discons3);
                bump(&mut req.player.games, 3, added_discons3);
                bump(&mut req.player.desyncs, 4, added_desyncs4);
                bump(&mut req.player.games, 4, added_desyncs4);
                bump(&mut req.player.discons, 4, added_discons4);
                bump(&mut req.player.games, 4, added_discons4);
                if self.try_connect(host) && self.try_login(host, req.player.id, &req.nick, &req.password, &req.email) {
                    self.queue.track_player_stats(PsPlayerStats::copy_constructed(&req.player));
                    let kv = format_player_kv_pairs(&req.player, host.player_template_count());
                    self.incr_op_count();
                    host.set_persist_data_values(req.player.id, &kv, false);
                }
            }
            PsRequestType::ReadCdKeyStats => {
                if self.try_connect(host) {
                    self.incr_op_count();
                    self.cd_auth = CdAuthInfo::default();
                    let validation_token = host.generate_auth_with_challenge(&req.cdkey);
                    let cdkey_hash = host.generate_auth_empty_challenge(&req.cdkey);
                    host.pre_authenticate_player_cd(&cdkey_hash, &validation_token);

                    while running && host.is_stats_connected() && !self.cd_auth.done {
                        host.persist_think();
                    }

                    if self.cd_auth.done && self.cd_auth.success {
                        host.get_persist_data_values_preorder(self.cd_auth.id);
                    } else {
                        self.decr_op_count();
                    }
                }
            }
        }
        true
    }
}
