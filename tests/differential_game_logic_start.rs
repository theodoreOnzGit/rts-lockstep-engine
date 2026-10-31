#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `startNewGame`'s helpers (`logic::world::game_logic_start`) against the C++, layer 12 batch L3a.
//!
//! # Method
//!
//! The C++ side is `GameLogic.cpp` from `placeObjectAtPosition` to the end of `populateRandomStartPosition`, cut
//! verbatim by build.rs and compiled by `differential/wrap_game_logic_start.cpp` in a namespace of logging stand-ins
//! (the thing factory and its objects, players and their default teams, the pathfinder, the partition manager,
//! waypoints, the terrain's height, the player templates, the challenge generals, the colour count, a map cache of
//! one map, the campaign and the load screens), with the GameInfo build's real `SkirmishGameInfo` and the
//! RandomValue build's real game-logic generator. The Rust side runs the same on the translation with a host that
//! answers and logs as the stand-ins do, on the crate's `GameInfo` and `RandomValues`.
//!
//! A program sets the eight slots (state, colour, start position, faction, team; random ones -1, observers -2, and
//! out-of-range values), the factions (which exist, which have a starting building and units, which are old, which
//! name a general and whether it starts enabled), the colour count, the map (cached or not, its player count, which
//! start waypoints it has), the seed; then clears duplicate colours, picks random factions and colours, picks start
//! positions, renames map objects, places a player's starting buildings and units, and chooses load screens, dumping
//! the slots and the seed after each.
//!
//! An operation the Rust side reports as undefined ends the Rust run there; the C++ runs only the operations before
//! it: an endless loop (every colour, or every start position, taken), a modulo by zero (no faction can start), the
//! uncached map dereferenced, a start position past the eight slots, a team number past them, an unknown starting
//! unit (`newObject(NULL)` throws), `tolower` of a byte of 0x80 or more.
//!
//! # Results (2026-10-31, `GLSTART_CASES=20000`, the default)
//!
//! 20,000 programs, 93,883 operations, 258,975 log lines: **0 mismatches**. 7,036 programs cut at an undefined
//! operation: 3,154 past the eight slots (most often more players than the map has start positions, so the farthest
//! search finds none and upstream marks `taken[-1]`), 1,719 at the uncached map dereferenced, 934 at a modulo by zero
//! (no faction can start), 741 at an endless loop (every colour or start position taken), 488 at an unknown starting
//! unit. Paths reached (log lines): 13,613 player notifications of placed buildings and units, 9,237 position searches,
//! 4,808 pathfinder adjustments (2,493 moving a unit), 653 objects the factory refused, 2,535 map objects renamed.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 6 of 6 caught. Three runs end early: once a plant diverges the state, the C++ reaches undefined
//! behaviour (a division by zero, an out-of-range index) the planted Rust side no longer cuts, and the process dies.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 4021 | a duplicate colour kept | 9,904 programs |
//! | 4022 | a locked general's faction drawn at random | from program 1 on (the run then dies) |
//! | 4023 | a teammate placed at the nearest position even if taken | from program 2 on (the run then dies) |
//! | 4024 | the farthest search keeps the first free position | from program 2 on (the run then dies) |
//! | 4025 | `AmericaVehicleHumVee` not renamed | 1,213 programs |
//! | 4026 | starting units not reported to their player | 2,705 programs |

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_info::{GameInfo, GameInfoHost, MapMeta, MAX_SLOTS};
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::game_logic_start::*;
use std::collections::BTreeMap;

struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn chance(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }
}

type Op = [u32; 6];
type Cfg = [u32; 16];
type Slots = [u32; 40];

const THINGS: [&[u8]; 6] = [b"ConYardA", b"ConYardB", b"Tank", b"Ranger", b"AmericaTankCrusader", b"AmericaVehicleHumvee"];

/// `GameInfo`'s own host: only its construction calls it here.
struct InfoHost;

impl GameInfoHost for InfoHost {
    fn game_text(&mut self, label: &str) -> Vec<u16> {
        "T:".bytes().chain(label.bytes()).map(u16::from).collect()
    }
    fn tick_count(&mut self) -> u32 {
        1000
    }
    fn default_starting_cash(&self) -> Money {
        Money::new()
    }
    fn net_crc_interval(&self) -> i32 {
        100
    }
    fn has_map_cache(&self) -> bool {
        false
    }
    fn map_cache_find(&self, _lower: &[u8]) -> Option<MapMeta> {
        None
    }
    fn file_exists(&mut self, _path: &[u8]) -> bool {
        false
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
    fn has_multiplayer_settings(&self) -> bool {
        false
    }
    fn num_colors(&self) -> i32 {
        0
    }
    fn show_random_player_template(&self) -> bool {
        false
    }
    fn show_random_color(&self) -> bool {
        false
    }
    fn show_random_start_pos(&self) -> bool {
        false
    }
    fn observer_color(&self) -> Result<i32, IniError> {
        Ok(0)
    }
    fn player_template_count(&self) -> i32 {
        0
    }
    fn player_template_display_name(&self, _n: i32) -> Result<Vec<u16>, IniError> {
        Ok(Vec::new())
    }
    fn record_income(&mut self, _player_index: i32) {}
}

struct Host {
    cfg: Cfg,
    rng: RandomValues,
    next_obj: u32,
    objects: BTreeMap<u32, usize>,
    find_calls: u32,
    log: String,
}

impl Host {
    fn line(&mut self, s: String) {
        self.log.push_str(&s);
        self.log.push('\n');
    }
    fn waypoint_loc(k: usize) -> Coord3D {
        Coord3D { x: 100.0 * (k % 8) as f32 + 7.0 * (k / 8) as f32, y: 37.5 * ((k * 5) % 8) as f32, z: 0.0 }
    }
    fn template_of(&self, obj: u32) -> usize {
        self.objects[&obj]
    }
    fn team_of(&self, player: i32) -> Option<i32> {
        (self.cfg[10] >> player & 1 == 0).then_some(player)
    }
}

impl StartHost for Host {
    fn st_player_template_count(&self) -> i32 {
        (self.cfg[1] % 13) as i32
    }
    fn st_player_template_exists(&self, i: i32) -> bool {
        (0..12).contains(&i) && self.cfg[2] >> i & 1 == 0
    }
    fn st_starting_building(&self, i: i32) -> Vec<u8> {
        if self.cfg[8] >> i & 1 != 0 {
            THINGS[(i % 2) as usize].to_vec()
        } else {
            Vec::new()
        }
    }
    fn st_starting_unit(&self, i: i32, n: i32) -> Vec<u8> {
        if (i + n) % 4 == 0 {
            THINGS[(2 + n % 2) as usize].to_vec()
        } else if (i + n) % 7 == 0 && i >= 10 {
            b"NoSuchThing".to_vec()
        } else {
            Vec::new()
        }
    }
    fn st_is_old_faction(&self, i: i32) -> bool {
        self.cfg[15] >> i & 1 != 0
    }
    fn st_general_starts_locked(&self, i: i32) -> bool {
        (0..12).contains(&i) && self.cfg[3] >> i & 1 != 0 && self.cfg[7] >> i & 1 == 0
    }
    fn st_num_colors(&self) -> i32 {
        (self.cfg[4] % 10) as i32
    }
    fn st_find_map(&self, lower_name: &[u8]) -> Option<MapStartInfo> {
        if lower_name != b"maps\\one\\one.map" || self.cfg[5] & 1 == 0 {
            return None;
        }
        let waypoints = (0..8).filter(|k| self.cfg[14] >> k & 1 != 0).map(|k| (format!("Player_{}_Start", k + 1).into_bytes(), Host::waypoint_loc(k))).collect();
        Some(MapStartInfo { num_players: (self.cfg[6] % 10) as i32 - 1, waypoints })
    }
    fn st_random(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng.game_logic_random_value(lo, hi)
    }
    fn st_random_seed(&self) -> u32 {
        self.rng.get_game_logic_random_seed()
    }
    fn st_find_template(&self, name: &[u8]) -> Option<u32> {
        THINGS.iter().position(|t| *t == name).map(|i| i as u32)
    }
    fn st_waypoint(&self, name: &[u8]) -> Option<Coord3D> {
        for k in 1..=8usize {
            if name == format!("Player_{k}_Start").as_bytes() && self.cfg[14] >> (k - 1) & 1 != 0 {
                return Some(Host::waypoint_loc(k - 1));
            }
            if name == format!("Player_{k}_Rally").as_bytes() && self.cfg[14] >> (k + 7) & 1 != 0 {
                return Some(Host::waypoint_loc(k + 7));
            }
        }
        None
    }
    fn st_ground_height(&self, x: f32, y: f32) -> f32 {
        x * 0.125 - y * 0.0625
    }
    fn st_new_object(&mut self, template: u32, player: i32) -> Option<u32> {
        if self.cfg[11] >> template & 1 != 0 {
            self.line(format!("newobject refused {template}"));
            return None;
        }
        let id = 100 + self.next_obj;
        self.next_obj += 1;
        self.objects.insert(id, template as usize);
        let team = self.team_of(player).unwrap_or(-1);
        self.line(format!("new object {id} template {template} team {team}"));
        Some(id)
    }
    fn st_placement_view_angle(&self, obj: u32) -> f32 {
        0.25 * self.template_of(obj) as f32
    }
    fn st_set_orientation(&mut self, obj: u32, angle: f32) {
        self.line(format!("object {obj} angle {:.6}", f64::from(angle)));
    }
    fn st_set_position(&mut self, obj: u32, pos: Coord3D) {
        self.line(format!("object {obj} at {:.4} {:.4} {:.4}", f64::from(pos.x), f64::from(pos.y), f64::from(pos.z)));
    }
    fn st_build_complete(&mut self, obj: u32) {
        self.line(format!("build complete {obj}"));
    }
    fn st_activate_default_team(&mut self, player: i32) {
        if let Some(t) = self.team_of(player) {
            self.line(format!("team {t} active"));
        }
    }
    fn st_add_to_pathfind_map(&mut self, obj: u32) {
        self.line(format!("pathfind map {obj}"));
    }
    fn st_is_mobile_ai(&self, obj: u32) -> bool {
        let t = self.template_of(obj);
        (t == 2 || t == 3) && !(t == 3 && self.cfg[9] & 1 != 0)
    }
    fn st_adjust_destination(&mut self, obj: u32, mut pos: Coord3D) -> Option<Coord3D> {
        if self.cfg[12] & 1 != 0 {
            self.line(format!("adjust {obj} none"));
            return None;
        }
        pos.x += 2.5;
        pos.y -= 1.25;
        self.line(format!("adjust {obj}"));
        Some(pos)
    }
    fn st_update_goal(&mut self, obj: u32, pos: Coord3D) {
        self.line(format!("goal {obj} {:.4} {:.4} 1", f64::from(pos.x), f64::from(pos.y)));
    }
    fn st_structure_created(&mut self, player: i32, obj: u32) {
        self.line(format!("player {player} structure {obj}"));
        self.line(format!("player {player} complete {obj} 0"));
    }
    fn st_unit_created(&mut self, player: i32, obj: u32) {
        self.line(format!("player {player} unit {obj}"));
    }
    fn st_bounding_sphere_radius(&self, obj: u32) -> f32 {
        10.0 + 3.5 * self.template_of(obj) as f32
    }
    fn st_partition_update(&mut self) {
        self.line("partition update".into());
    }
    fn st_find_position_around(&mut self, pos: Coord3D, min_radius: f32, max_radius: f32) -> Option<Coord3D> {
        let found = self.cfg[13] >> (self.find_calls % 32) & 1 == 0;
        self.find_calls += 1;
        self.line(format!(
            "find around {:.4} {:.4} {:.4} r {:.6} {:.6} -> {}",
            f64::from(pos.x),
            f64::from(pos.y),
            f64::from(pos.z),
            f64::from(min_radius),
            f64::from(max_radius),
            i32::from(found)
        ));
        found.then_some(Coord3D { x: pos.x + min_radius, y: pos.y + max_radius, z: pos.z })
    }
}

fn dump(h: &mut Host, game: &GameInfo) {
    let s: String = game.slots.iter().map(|s| format!(" {}:{}:{}:{}:{}", s.state, s.color, s.start_pos, s.player_template, s.team_number)).collect();
    let seed = h.rng.get_game_logic_random_seed();
    h.line(format!("slots{s} seed {seed}"));
}

struct Cut(String);

fn cut(e: IniError) -> Cut {
    Cut(format!("{e:?}"))
}

fn op(game: &mut GameInfo, h: &mut Host, w: &Op) -> Result<(), Cut> {
    let (a, b, c) = (w[1], w[2], w[3]);
    let null = w[5] & 1 != 0;
    match w[0] {
        0 => {
            check_for_duplicate_colors((!null).then_some(&mut *game), h);
            h.line("duplicates".into());
            dump(h, game);
        }
        1 => {
            populate_random_side_and_color((!null).then_some(&mut *game), h).map_err(cut)?;
            h.line("sides".into());
            dump(h, game);
        }
        2 => {
            populate_random_start_position((!null).then_some(&mut *game), h).map_err(cut)?;
            h.line("starts".into());
            dump(h, game);
        }
        3 => {
            const NAMES: [&[u8]; 4] = [b"AmericaTankLeopard", b"AmericaVehicleHumVee", b"Tank", b"AmericaTankleopard"];
            let mut name = NAMES[(a % 4) as usize].to_vec();
            let mut template = Some(2);
            handle_name_change(&mut name, &mut template, h);
            let t = template.map_or(-1, |t| t as i32);
            h.line(format!("renamed [{}] {t}", String::from_utf8_lossy(&name)));
        }
        4 => {
            let slot = (a % MAX_SLOTS as u32) as usize;
            let start = game.slots[slot].start_pos;
            place_network_buildings_for_player(start, slot as i32, (b % 12) as i32, h).map_err(cut)?;
            h.line("placed".into());
        }
        5 => {
            let campaign = (b & 1 != 0).then_some(b >> 1 & 1 != 0);
            let kind = load_screen_kind((a % 8) as i32, campaign, c & 1 != 0);
            let name = match kind {
                None => "none",
                Some(LoadScreenKind::Shell) => "shell",
                Some(LoadScreenKind::SinglePlayer) => "singleplayer",
                Some(LoadScreenKind::Challenge) => "challenge",
                Some(LoadScreenKind::MultiPlayer) => "multiplayer",
                Some(LoadScreenKind::GameSpy) => "gamespy",
            };
            h.line(format!("loadscreen {name}"));
        }
        6 => dump(h, game),
        _ => h.line("op?".into()),
    }
    Ok(())
}

fn rust_run(cfg: &Cfg, slots: &Slots, ops: &[Op]) -> (String, Option<(usize, String)>) {
    let mut rng = RandomValues::default();
    rng.init_game_logic_random(cfg[0]);
    let mut h = Host { cfg: *cfg, rng, next_obj: 0, objects: BTreeMap::new(), find_calls: 0, log: String::new() };
    let mut game = GameInfo::new(&mut InfoHost);
    game.old_factions_only = cfg[5] >> 1 & 1 != 0;
    game.map_name = if cfg[5] & 4 != 0 {
        b"Maps\\One\\One.map".to_vec()
    } else if cfg[5] & 8 != 0 {
        b"Maps\\Two\\Two.map".to_vec()
    } else {
        b"maps\\one\\one.map".to_vec()
    };
    for (i, s) in game.slots.iter_mut().enumerate() {
        let v = &slots[5 * i..5 * i + 5];
        s.state = (v[0] % 6) as i32;
        s.color = v[1] as i32;
        s.start_pos = v[2] as i32;
        s.player_template = v[3] as i32;
        s.team_number = v[4] as i32;
    }
    for (i, w) in ops.iter().enumerate() {
        let before = h.log.len();
        if let Err(Cut(why)) = op(&mut game, &mut h, w) {
            h.log.truncate(before);
            return (h.log, Some((i, why)));
        }
    }
    (h.log, None)
}

fn cpp_run(cfg: &Cfg, slots: &Slots, ops: &[Op]) -> String {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut cap = 1 << 18;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_glstart_run(cfg.as_ptr(), slots.as_ptr(), flat.as_ptr(), ops.len() as i32, out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

fn small(g: &mut Gen, n: u32) -> u32 {
    // mostly in range or random (-1); sometimes observer (-2); rarely out of range
    match g.below(40) {
        0..=11 => (-1i32) as u32,
        12..=14 => (-2i32) as u32,
        15 => n + g.below(3),
        16 => g.next(),
        _ => g.below(n),
    }
}

fn gen_program(g: &mut Gen) -> (Cfg, Slots, Vec<Op>) {
    let mut cfg: Cfg = [0; 16];
    cfg[0] = g.next();
    cfg[1] = if g.chance(5) { g.below(13) } else { 8 + g.below(5) };
    cfg[2] = if g.chance(3) { g.next() & g.next() } else { 0 };
    cfg[3] = g.next();
    cfg[4] = if g.chance(6) { g.below(10) } else { 6 + g.below(4) };
    cfg[5] = if g.chance(5) { g.next() } else { 1 | (g.next() & 2) };
    // the map's player count (cfg[6] % 10 - 1): mostly eight, enough for every slot
    cfg[6] = if g.chance(4) { g.below(10) } else if g.chance(3) { 3 + g.below(7) } else { 9 };
    cfg[7] = g.next();
    cfg[8] = if g.chance(4) { g.next() } else { !(g.next() & g.next() & g.next()) };
    cfg[9] = g.next();
    cfg[10] = if g.chance(3) { g.next() } else { 0 };
    cfg[11] = if g.chance(4) { g.next() & g.next() } else { 0 };
    cfg[12] = g.next();
    cfg[13] = if g.chance(2) { g.next() } else { 0 };
    cfg[14] = if g.chance(4) { g.next() } else { 0xffff };
    cfg[15] = g.next();
    let mut slots: Slots = [0; 40];
    for i in 0..8 {
        let state = if g.chance(3) { 1 } else { [0, 2, 3, 4, 5, 5][g.below(6) as usize] };
        slots[5 * i] = state;
        slots[5 * i + 1] = small(g, 8);
        // a start position: mostly random, else one of the map's
        slots[5 * i + 2] = if g.chance(2) { (-1i32) as u32 } else { small(g, 6) };
        slots[5 * i + 3] = small(g, 12);
        slots[5 * i + 4] = if g.chance(3) { (-1i32) as u32 } else { small(g, 4) };
    }
    let mut ops: Vec<Op> = Vec::new();
    for _ in 0..2 + g.below(10) {
        let k = match g.below(100) {
            0..=19 => 0,
            20..=44 => 1,
            45..=69 => 2,
            70..=74 => 3,
            75..=89 => 4,
            90..=94 => 5,
            _ => 6,
        };
        ops.push([k, g.next(), g.next(), g.next(), 0, u32::from(g.chance(12))]);
    }
    (cfg, slots, ops)
}

#[test]
fn game_logic_start_matches_upstream() {
    let cases: usize = std::env::var("GLSTART_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x474c_5333_cafe_0001);
    let (mut mismatches, mut cut_n, mut ops_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut cut_why = BTreeMap::<String, usize>::new();
    let mut by_op = [0usize; 7];
    let mut marks = BTreeMap::<&str, usize>::new();
    for case in 0..cases {
        let (cfg, slots, ops) = gen_program(&mut g);
        let (r, undefined_at) = rust_run(&cfg, &slots, &ops);
        let n = undefined_at.as_ref().map_or(ops.len(), |u| u.0);
        if let Some((_, why)) = &undefined_at {
            cut_n += 1;
            *cut_why.entry(why.clone()).or_insert(0) += 1;
        }
        let c = cpp_run(&cfg, &slots, &ops[..n]);
        ops_run += n;
        for o in &ops[..n] {
            by_op[o[0] as usize] += 1;
        }
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let (rl, cl): (Vec<&str>, Vec<&str>) = (r.lines().collect(), c.lines().collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                println!("case {case}: cfg {cfg:?} slots {slots:?} ops {:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}", &ops[..n], rl.get(at), cl.get(at));
            }
        } else {
            lines += r.lines().count();
            for l in r.lines() {
                for m in ["player", "goal", "adjust", "find around", "newobject refused", "renamed [AmericaTankCrusader]", "renamed [AmericaVehicleHumvee]"] {
                    if l.starts_with(m) {
                        *marks.entry(m).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    println!("L3a: {cases} programs ({cut_n} cut at an undefined operation), {ops_run} operations, {lines} lines, {mismatches} mismatches; by kind {by_op:?}");
    println!("reached: {marks:?}");
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
