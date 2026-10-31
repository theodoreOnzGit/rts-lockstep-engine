#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `GameLogic::startNewGame` (`logic::world::game_logic_new_game`) against the C++, layer 12 batch L3b.
//!
//! # Method
//!
//! The C++ side is `startNewGame`, `loadMapINI`, `updateLoadProgress`, `deleteLoadScreen` and
//! `findAndSelectCommandCenter`, with the batch L3a helpers, batch L1's `selectObject`, `isProgressComplete`,
//! `lastHeardFrom` and `testTimeOut`, and `isInSinglePlayerGame`, all cut verbatim from `GameLogic.cpp` by build.rs
//! (one edit: the wide literal `L"Observer"`), compiled by `differential/wrap_game_logic_new_game.cpp` in a namespace
//! of stand-ins that log every call that acts. The games' options are the GameInfo build's real `SkirmishGameInfo`
//! (one per source: LAN, GameSpy, the recorder, skirmish, challenge) and the random numbers the RandomValue build's.
//! The Rust side runs [`start_new_game`] with a host that answers and logs as the stand-ins do.
//!
//! A program is one call: the game mode, whether a save is loading, whether a new game was already pending, the
//! subsystems present (background, campaign and whether it is a challenge, network, LAN, recorder and its mode and
//! recorded game mode, display, stats collector, GameSpy queue and info, game text), which sources have game options
//! and their slots, the map name (a pristine map, a map in the save directory, a short name), the map's files, the
//! multiplayer scripts (missing, unparsable, or with scripts), the factions, colours, players (which exist, are
//! human, active, own a command center), the map objects (templates, names to rename, flags, owners), the template
//! kinds, the LOD level, the transition's length and the clock's step. The log holds every call, each side's and
//! team's dictionary whole, and the logic's state at the end.
//!
//! A program the Rust side reports as undefined is not compared past the C++ call: the C++ crashes there, so the
//! program is skipped and counted.
//!
//! # Results (2026-10-31, `GLNEW_CASES=20000`, the default)
//!
//! 20,000 programs: 16,306 compared, 594,522 log lines, **0 mismatches**; 3,694 undefined (the C++ would crash, so it
//! is not run): 2,288 at a NULL dereferenced (most often a colour out of range, no replay observer, or a NULL
//! faction), 534 at no local player, 359 at a command center without a controlling player, 356 at an endless loop and
//! 157 at a modulo by zero (L3a's helpers). Paths reached (log lines): 18,680 sides added, 8,544 maps revealed, 7,444
//! objects made (bridges, wall walkers, map objects, starting buildings and units), 1,305 multiplayer scripts added,
//! 4,446 challenge relationships set, 1,508 skill points given, 408 waits for the other players, 1,783 fades drawn,
//! 289 props, 1,195 landmark bridges.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 6 of 6 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 4031 | fluff not made a prop when it should be | 81 programs |
//! | 4032 | a slot with no team counts others of team -1 as allies | 1,459 programs |
//! | 4033 | an unparsable multiplayer scripts file does not end the setup | 615 programs |
//! | 4034 | a challenge's local player not its own ally | 173 programs |
//! | 4035 | no skill points at a single-player start | 252 programs |
//! | 4036 | every command center of a player selected, not the first | 923 programs |

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_info::{GameInfo, GameInfoHost, MapMeta, MAX_SLOTS};
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::xfer::{Xfer, XferResult};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::game_logic_dispatch::DispatchState;
use rts_lockstep_engine::logic::world::game_logic_new_game::*;
use rts_lockstep_engine::logic::world::game_logic_start::*;
use rts_lockstep_engine::logic::world::game_logic_update::{GameLogicHost, GameLogicState};
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

type Cfg = [u32; 40];
type Slots = [u32; 40];
type R<T> = Result<T, IniError>;

const THINGS: [&[u8]; 6] = [b"ConYardA", b"ConYardB", b"Tank", b"Ranger", b"AmericaTankCrusader", b"AmericaVehicleHumvee"];
const MAPS: [&[u8]; 4] = [b"Maps\\One\\One.map", b"Save\\00000001.map", b"x.map", b"Maps\\Two\\Two.map"];
const OBJ_NAMES: [&[u8]; 4] = [b"AmericaTankLeopard", b"Thing", b"AmericaVehicleHumVee", b"Rock"];

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

#[derive(Clone, Copy)]
struct Obj {
    template: Option<usize>,
    drawable: Option<u32>,
}

struct Host {
    cfg: Cfg,
    rng: RandomValues,
    log: String,
    start_new_game: bool,
    loading_map: bool,
    frame: u32,
    rank_limit: i32,
    render: bool,
    tivo: bool,
    width: f32,
    height: f32,
    pristine: Vec<u8>,
    background: bool,
    load_screen: Option<LoadScreenKind>,
    games: [Option<GameInfo>; 5],
    taken: Option<usize>,
    source: Option<usize>,
    objects: BTreeMap<u32, Obj>,
    next_obj: u32,
    next_drawable: u32,
    next_group: u32,
    find_calls: u32,
    time: u32,
    transition_calls: u32,
    local_player: Option<i32>,
    map_objects: Vec<MapObj>,
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

impl Host {
    fn line(&mut self, s: String) {
        self.log.push_str(&s);
        self.log.push('\n');
    }
    fn flag(&self, n: u32) -> bool {
        self.cfg[1] >> n & 1 != 0
    }
    fn waypoint_loc(k: usize) -> Coord3D {
        Coord3D { x: 100.0 * (k % 8) as f32 + 7.0 * (k / 8) as f32, y: 37.5 * ((k * 5) % 8) as f32, z: 0.0 }
    }
    fn player_name(i: i32) -> Vec<u8> {
        match i {
            0..=7 => format!("player{i}").into_bytes(),
            8 => b"ReplayObserver".to_vec(),
            9 => b"ThePlayer".to_vec(),
            10 => b"PlyrCivilian".to_vec(),
            _ => b"Neutral".to_vec(),
        }
    }
    fn exists(&self, i: i32) -> bool {
        (0..12).contains(&i) && self.cfg[16] >> i & 1 != 0
    }
    fn template_name(&self, i: i32) -> Vec<u8> {
        if i == (self.cfg[12] % 16) as i32 {
            b"FactionObserver".to_vec()
        } else {
            format!("Faction{i}").into_bytes()
        }
    }
    fn template_kind(&self, t: u32, kind: TemplateKind) -> bool {
        let (word, bit) = match kind {
            TemplateKind::WalkOnTopOfWall => (29, 4 * t),
            TemplateKind::Shrubbery => (29, 4 * t + 1),
            TemplateKind::OptimizedTree => (29, 4 * t + 2),
            TemplateKind::Prop => (29, 4 * t + 3),
            TemplateKind::ClearedByBuild => (30, 3 * t),
        };
        self.cfg[word] >> bit & 1 != 0
    }
    fn make_object(&mut self, t: u32, team: i32) -> Option<u32> {
        if self.cfg[25] >> t & 1 != 0 {
            self.line(format!("newobject refused {t}"));
            return None;
        }
        let id = 100 + self.next_obj;
        self.next_obj += 1;
        let drawable = (t % 2 == 1).then(|| {
            let d = 500 + self.next_drawable % 80;
            self.next_drawable += 1;
            d
        });
        self.objects.insert(id, Obj { template: Some(t as usize), drawable });
        self.line(format!("new object {id} template {t} team {team}"));
        Some(id)
    }
    fn template_of(&self, obj: u32) -> usize {
        self.objects[&obj].template.expect("a template")
    }
    fn ls(&self) -> &'static str {
        match self.load_screen.expect("a load screen") {
            LoadScreenKind::Shell => "shell",
            LoadScreenKind::SinglePlayer => "singleplayer",
            LoadScreenKind::Challenge => "challenge",
            LoadScreenKind::MultiPlayer => "multiplayer",
            LoadScreenKind::GameSpy => "gamespy",
        }
    }
}

impl StartHost for Host {
    fn st_player_template_count(&self) -> i32 {
        (self.cfg[9] % 13) as i32
    }
    fn st_player_template_exists(&self, i: i32) -> bool {
        i >= 0 && i < self.st_player_template_count() && self.cfg[10] >> i & 1 == 0
    }
    fn st_starting_building(&self, i: i32) -> Vec<u8> {
        if self.cfg[11] >> i & 1 != 0 {
            THINGS[(i % 2) as usize].to_vec()
        } else {
            Vec::new()
        }
    }
    fn st_starting_unit(&self, i: i32, n: i32) -> Vec<u8> {
        if (i + n) % 4 == 0 {
            THINGS[(2 + n % 2) as usize].to_vec()
        } else {
            Vec::new()
        }
    }
    fn st_is_old_faction(&self, i: i32) -> bool {
        self.cfg[15] >> i & 1 != 0
    }
    fn st_general_starts_locked(&self, i: i32) -> bool {
        (0..12).contains(&i) && self.cfg[13] >> i & 1 != 0 && self.cfg[14] >> i & 1 == 0
    }
    fn st_num_colors(&self) -> i32 {
        (self.cfg[8] % 10) as i32
    }
    fn st_find_map(&self, lower_name: &[u8]) -> Option<MapStartInfo> {
        if lower_name != b"maps\\one\\one.map" {
            return None;
        }
        let waypoints = (0..8).filter(|k| self.cfg[24] >> k & 1 != 0).map(|k| (format!("Player_{}_Start", k + 1).into_bytes(), Host::waypoint_loc(k))).collect();
        Some(MapStartInfo { num_players: 8, waypoints })
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
            if name == format!("Player_{k}_Start").as_bytes() && self.cfg[24] >> (k - 1) & 1 != 0 {
                return Some(Host::waypoint_loc(k - 1));
            }
            if name == format!("Player_{k}_Rally").as_bytes() && self.cfg[24] >> (k + 7) & 1 != 0 {
                return Some(Host::waypoint_loc(k + 7));
            }
        }
        None
    }
    fn st_ground_height(&self, x: f32, y: f32) -> f32 {
        x * 0.125 - y * 0.0625
    }
    fn st_new_object(&mut self, template: u32, player: i32) -> Option<u32> {
        self.make_object(template, player)
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
        self.line(format!("team {player} active"));
    }
    fn st_add_to_pathfind_map(&mut self, obj: u32) {
        self.line(format!("pathfind map {obj}"));
    }
    fn st_is_mobile_ai(&self, obj: u32) -> bool {
        let t = self.template_of(obj);
        (t == 2 || t == 3) && !(t == 3 && self.cfg[31] & 1 != 0)
    }
    fn st_adjust_destination(&mut self, obj: u32, mut pos: Coord3D) -> Option<Coord3D> {
        if self.cfg[31] & 2 != 0 {
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
        let found = self.cfg[26] >> (self.find_calls % 32) & 1 == 0;
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

impl GameLogicHost for Host {
    fn u_frame(&self) -> u32 {
        self.frame
    }
    fn u_advance_frame(&mut self) {
        unreachable!()
    }
    fn u_start_new_game_pending(&self) -> bool {
        self.start_new_game
    }
    fn u_set_start_new_game_pending(&mut self, on: bool) {
        self.start_new_game = on;
    }
    fn u_is_in_update(&self) -> bool {
        unreachable!()
    }
    fn u_set_in_update(&mut self, _on: bool) {
        unreachable!()
    }
    fn u_game_mode(&self) -> i32 {
        (self.cfg[2] % 7) as i32
    }
    fn u_start_new_game(&mut self, _loading_save_game: bool) {
        unreachable!()
    }
    fn u_objects(&self) -> Vec<u32> {
        unreachable!()
    }
    fn u_object_is_disabled(&self, _id: u32) -> bool {
        unreachable!()
    }
    fn u_check_disabled_status(&mut self, _id: u32) {
        unreachable!()
    }
    fn u_object_is_mass_selectable(&self, _id: u32) -> bool {
        true
    }
    fn u_object_has_drawable(&self, id: u32) -> bool {
        self.objects.get(&id).is_some_and(|o| o.drawable.is_some())
    }
    fn u_run_sleepy_updates(&mut self) {
        unreachable!()
    }
    fn u_process_destroy_list(&mut self) {
        unreachable!()
    }
    fn u_command_list_contains(&self, _t: MessageType) -> bool {
        unreachable!()
    }
    fn u_command_list(&self) -> Vec<u32> {
        unreachable!()
    }
    fn u_dispatch(&mut self, _state: &mut DispatchState, _msg: u32) -> R<()> {
        unreachable!()
    }
    fn u_command_list_reset(&mut self) {
        unreachable!()
    }
    fn u_send_logic_crc(&mut self, _crc: u32, _playback: bool) {
        unreachable!()
    }
    fn u_set_fp_mode(&mut self) {
        self.line("fpmode".into());
    }
    fn u_movie_playing(&self) -> bool {
        unreachable!()
    }
    fn u_client_set_frame(&mut self, _frame: u32) {
        unreachable!()
    }
    fn u_script_engine_update(&mut self) {
        unreachable!()
    }
    fn u_view_time_frozen(&self) -> bool {
        unreachable!()
    }
    fn u_view_camera_movement_finished(&self) -> bool {
        unreachable!()
    }
    fn u_script_time_frozen_debug(&self) -> bool {
        unreachable!()
    }
    fn u_script_time_frozen_script(&self) -> bool {
        unreachable!()
    }
    fn u_script_force_unfreeze_time(&mut self) {
        unreachable!()
    }
    fn u_terrain_update(&mut self) {
        unreachable!()
    }
    fn u_has_recorder(&self) -> bool {
        self.flag(7)
    }
    fn u_recorder_is_multiplayer(&self) -> bool {
        self.flag(9)
    }
    fn u_recorder_playback(&self) -> bool {
        self.flag(8)
    }
    fn u_recorder_update(&mut self) {
        unreachable!()
    }
    fn u_game_info_crc_interval(&self) -> Option<i32> {
        unreachable!()
    }
    fn u_replay_crc_interval(&self) -> i32 {
        unreachable!()
    }
    fn u_stats_collector_update(&mut self) -> bool {
        unreachable!()
    }
    fn u_ai_update(&mut self) {
        unreachable!()
    }
    fn u_build_assistant_update(&mut self) {
        unreachable!()
    }
    fn u_partition_update(&mut self) {
        unreachable!()
    }
    fn u_weapon_store_update(&mut self) {
        unreachable!()
    }
    fn u_locomotor_store_update(&mut self) {
        unreachable!()
    }
    fn u_victory_conditions_update(&mut self) {
        unreachable!()
    }
    fn u_crc_object(&mut self, _id: u32, _xfer: &mut dyn Xfer) -> XferResult<()> {
        unreachable!()
    }
    fn u_random_seed_crc(&self) -> u32 {
        unreachable!()
    }
    fn u_crc_partition_manager(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        unreachable!()
    }
    fn u_crc_player_list(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        unreachable!()
    }
    fn u_crc_ai(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        unreachable!()
    }
    fn u_crc_game_state(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        unreachable!()
    }
    fn u_has_network(&self) -> bool {
        self.flag(5)
    }
    fn u_network_saw_crc_mismatch(&self) -> bool {
        unreachable!()
    }
    fn u_network_set_saw_crc_mismatch(&mut self) {
        unreachable!()
    }
    fn u_network_player_connected(&self, _slot: i32) -> bool {
        unreachable!()
    }
    fn u_network_num_players(&self) -> i32 {
        unreachable!()
    }
    fn u_time_get_time(&mut self) -> i32 {
        let t = self.time;
        self.time = self.time.wrapping_add(self.cfg[23]);
        self.line(format!("time {}", t as i32));
        t as i32
    }
    fn u_load_screen_process_progress(&mut self, _player: i32, _percentage: i32) -> bool {
        unreachable!()
    }
    fn u_each_player_from_mask(&self, mask: &mut u16) -> Option<i32> {
        for i in 0..16 {
            let m = 1u16 << i;
            if self.exists(i) && m & *mask != 0 {
                *mask &= !m;
                return Some(i);
            }
        }
        *mask = 0;
        None
    }
    fn u_create_group(&mut self) -> u32 {
        let g = self.next_group;
        self.next_group += 1;
        self.line(format!("create group {g}"));
        g
    }
    fn u_destroy_group(&mut self, group: u32) {
        self.line(format!("destroy group {group}"));
    }
    fn u_group_add(&mut self, group: u32, id: u32) {
        self.line(format!("group {group} add {id}"));
    }
    fn u_group_remove(&mut self, _group: u32, _id: u32) -> bool {
        unreachable!()
    }
    fn u_player_set_selected_group(&mut self, player: i32, group: Option<u32>) {
        match group {
            Some(g) => self.line(format!("player {player} select group {g}")),
            None => self.line(format!("player {player} select none")),
        }
    }
    fn u_player_add_group_to_selection(&mut self, player: i32, group: u32) {
        self.line(format!("player {player} add group {group}"));
    }
    fn u_player_selection_as_group(&mut self, _player: i32, _group: u32) {
        unreachable!()
    }
    fn u_ui_select_drawable(&mut self, id: u32) {
        let d = self.objects[&id].drawable.expect("a drawable");
        self.line(format!("ui select {d}"));
    }
    fn u_ui_deselect_drawable(&mut self, _id: u32) {
        unreachable!()
    }
    fn u_ui_input_enabled(&self) -> bool {
        unreachable!()
    }
    fn u_ui_set_input_enabled(&mut self, _on: bool) {
        unreachable!()
    }
    fn u_mouse_visible(&self) -> bool {
        unreachable!()
    }
    fn u_mouse_set_visible(&mut self, on: bool) {
        self.line(format!("mouse visible {}", i32::from(on)));
    }
    fn u_mouse_set_arrow_cursor(&mut self) {
        unreachable!()
    }
    fn u_audio_pause(&mut self, _affect: i32) {
        unreachable!()
    }
    fn u_audio_resume(&mut self, _affect: i32) {
        unreachable!()
    }
}

impl NewGameHost for Host {
    fn sn_set_loading_map(&mut self, on: bool) {
        self.loading_map = on;
        self.line(format!("loadingmap {}", i32::from(on)));
    }
    fn sn_map_name(&self) -> Vec<u8> {
        MAPS[(self.cfg[6] % 4) as usize].to_vec()
    }
    fn sn_set_pristine_map_name(&mut self, name: &[u8]) {
        self.pristine = name.to_vec();
        self.line(format!("pristine [{}]", lossy(name)));
    }
    fn sn_pristine_map_name(&self) -> Vec<u8> {
        self.pristine.clone()
    }
    fn sn_is_in_save_directory(&self, path: &[u8]) -> bool {
        path.starts_with(b"Save\\")
    }
    fn sn_destroy_background(&mut self) -> bool {
        if self.background {
            self.line("background destroyed".into());
            self.background = false;
            return true;
        }
        false
    }
    fn sn_has_load_screen(&self) -> bool {
        self.load_screen.is_some()
    }
    fn sn_set_load_screen(&mut self, kind: Option<LoadScreenKind>) {
        self.load_screen = kind;
    }
    fn sn_load_screen_init(&mut self, with_game: bool) {
        let k = self.ls();
        self.line(format!("loadscreen {k} init {}", i32::from(with_game)));
    }
    fn sn_load_screen_update(&mut self, progress: i32) {
        let k = self.ls();
        self.line(format!("loadscreen {k} update {progress}"));
    }
    fn sn_delete_load_screen(&mut self) {
        let k = self.ls();
        self.line(format!("loadscreen {k} deleted"));
        self.load_screen = None;
    }
    fn sn_set_load_screen_render(&mut self, on: bool) {
        self.render = on;
    }
    fn sn_clear_tivo_fast_mode(&mut self) {
        self.tivo = false;
    }
    fn sn_set_defaults(&mut self, loading_save_game: bool) {
        self.line(format!("setdefaults {}", i32::from(loading_save_game)));
    }
    fn sn_set_rank_level_limit(&mut self, limit: i32) {
        self.rank_limit = limit;
    }
    fn sn_set_frame(&mut self, frame: u32) {
        self.frame = frame;
    }
    fn sn_current_campaign(&self) -> Option<bool> {
        self.flag(3).then_some(self.flag(4))
    }
    fn sn_campaign_not_victorious(&mut self) -> bool {
        self.line("campaign victorious 0".into());
        true
    }
    fn sn_has_lan(&self) -> bool {
        self.flag(6)
    }
    fn sn_select_game(&mut self, source: GameSource) -> bool {
        self.source = match source {
            GameSource::None => None,
            GameSource::Lan => Some(0),
            GameSource::GameSpy => Some(1),
            GameSource::Recorder => Some(2),
            GameSource::Skirmish => Some(3),
            GameSource::Challenge => Some(4),
        };
        // TheGameInfo = game: a source without options is NULL
        if let Some(s) = self.source {
            if self.games[s].is_none() {
                self.source = None;
            }
        }
        self.source.is_some()
    }
    fn sn_take_game(&mut self) -> GameInfo {
        let s = self.source.expect("a game");
        self.taken = Some(s);
        self.games[s].take().expect("the game's options")
    }
    fn sn_put_game(&mut self, game: GameInfo) {
        let s = self.taken.take().expect("taken");
        self.games[s] = Some(game);
    }
    fn sn_delete_skirmish_game_info(&mut self) {
        self.games[3] = None;
    }
    fn sn_has_map_cache(&self) -> bool {
        true
    }
    fn sn_file_exists(&mut self, path: &[u8]) -> bool {
        let has = |n: &[u8]| path.windows(n.len()).any(|w| w == n);
        let e = (has(b"map.ini") && self.cfg[5] & 1 != 0) || (has(b"solo.ini") && self.cfg[5] & 2 != 0) || (has(b"map.str") && self.cfg[5] & 4 != 0);
        self.line(format!("exists [{}] {}", lossy(path), i32::from(e)));
        e
    }
    fn sn_load_ini(&mut self, path: &[u8]) -> R<()> {
        self.line(format!("ini [{}] 2", lossy(path)));
        Ok(())
    }
    fn sn_init_map_string_file(&mut self, path: &[u8]) {
        self.line(format!("mapstr [{}]", lossy(path)));
    }
    fn sn_asset_purge_and_preload(&mut self, path: &[u8]) -> bool {
        self.line(format!("assets [{}]", lossy(path)));
        true
    }
    fn sn_terrain_load_map(&mut self, name: &[u8]) -> R<()> {
        self.line(format!("loadmap [{}] 0", lossy(name)));
        Ok(())
    }
    fn sn_is_multiplayer_session(&self) -> bool {
        self.flag(12)
    }
    fn sn_sides_prepare_for_mp(&mut self) {
        self.line("sides prepare".into());
    }
    fn sn_observer_template(&self) -> Option<i32> {
        (0..self.st_player_template_count()).find(|&i| self.st_player_template_exists(i) && self.template_name(i) == b"FactionObserver")
    }
    fn sn_template_name(&self, i: i32) -> Vec<u8> {
        self.template_name(i)
    }
    fn sn_color(&self, index: i32) -> Option<(i32, i32)> {
        (index >= 0 && index < self.st_num_colors()).then(|| (0x0011_0000 * (index + 1), 0x0000_1100 * (index + 1)))
    }
    fn sn_add_side(&mut self, side: &SideDict) {
        let display: String = side.display_name.iter().map(|c| format!("{c:04x}")).collect();
        let faction = side.faction.as_ref().map_or("none".to_string(), |f| format!("[{}]", lossy(f)));
        let (skirmish, difficulty) = match side.skirmish {
            None => ("none".to_string(), "none".to_string()),
            Some(d) => ("1".to_string(), d.map_or("none".to_string(), |d| d.to_string())),
        };
        self.line(format!(
            "side name=[{}] human={} display=[{display}] faction={faction} preorder={} allies=[{}] enemies=[{}] color={} night={} start={} local={} skirmish={skirmish} difficulty={difficulty}",
            lossy(&side.player_name),
            i32::from(side.is_human),
            if side.is_preorder { "1" } else { "none" },
            lossy(&side.allies),
            lossy(&side.enemies),
            side.color,
            side.night_color,
            side.start_index,
            i32::from(side.is_local)
        ));
    }
    fn sn_add_team(&mut self, team_name: &[u8], owner: &[u8]) {
        self.line(format!("team name=[{}] owner=[{}] singleton=1", lossy(team_name), lossy(owner)));
    }
    fn sn_validate_sides(&mut self) {
        self.line("sides validate".into());
    }
    fn sn_team_factory_reset(&mut self) {
        self.line("teamfactory reset".into());
    }
    fn sn_player_list_new_game(&mut self) {
        self.line("playerlist newgame".into());
    }
    fn sn_script_engine_new_map(&mut self) {
        self.line("script newmap".into());
    }
    fn sn_add_multiplayer_scripts(&mut self) -> Option<bool> {
        let open = self.cfg[7] & 3 != 0;
        self.line(format!("mpscripts open [Data\\Scripts\\MultiplayerScripts.scb] {}", i32::from(open)));
        if !open {
            return None;
        }
        let parsed = self.cfg[7] & 3 != 1;
        self.line(format!("mpscripts parse {}", i32::from(parsed)));
        if !parsed {
            return Some(false);
        }
        let count = (self.cfg[7] >> 2) % 4;
        let n = (self.cfg[7] >> 4) % 4;
        if count > 0 {
            for i in 0..n {
                self.line(format!("mpscripts add {i} at 0"));
            }
        }
        Some(true)
    }
    fn sn_radar_new_map(&mut self) {
        self.line("radar newmap".into());
    }
    fn sn_ui_client_not_quiet(&mut self) {
        self.line("ui quiet 0".into());
    }
    fn sn_victory_setup(&mut self) {
        self.line("victory cache".into());
        self.line("victory set 1".into());
    }
    fn sn_terrain_extent(&self) -> (Coord3D, Coord3D) {
        (Coord3D { x: -10.0, y: 5.0, z: 0.0 }, Coord3D { x: 1270.5, y: 990.25, z: 100.0 })
    }
    fn sn_set_world_size(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
        self.line(format!("width {:.4}", f64::from(width)));
        self.line(format!("height {:.4}", f64::from(height)));
    }
    fn sn_partition_init(&mut self) {
        self.line("partition init".into());
        self.line("partition shroud".into());
    }
    fn sn_local_player(&self) -> Option<i32> {
        self.local_player
    }
    fn sn_ghost_reset(&mut self, local_player: i32) {
        self.line(format!("ghost local {local_player}"));
        self.line("ghost reset".into());
    }
    fn sn_terrain_new_map(&mut self, loading_save_game: bool) {
        self.line(format!("terrain newmap {}", i32::from(loading_save_game)));
    }
    fn sn_map_objects(&self) -> Vec<MapObj> {
        self.map_objects.clone()
    }
    fn sn_set_map_object(&mut self, index: usize, name: &[u8], template: Option<u32>) {
        self.map_objects[index].name = name.to_vec();
        self.map_objects[index].template = template;
    }
    fn sn_template_is_bridge(&self, t: u32) -> bool {
        self.cfg[28] >> t & 1 != 0
    }
    fn sn_template_is(&self, t: u32, kind: TemplateKind) -> bool {
        self.template_kind(t, kind)
    }
    fn sn_template_fence_width(&self, t: u32) -> f32 {
        if self.cfg[30] >> (3 * t + 2) & 1 != 0 {
            0.0
        } else {
            12.5
        }
    }
    fn sn_has_neutral_player(&self) -> bool {
        self.flag(22)
    }
    fn sn_new_object_neutral(&mut self, t: u32) -> R<Option<u32>> {
        Ok(self.make_object(t, 11))
    }
    fn sn_new_object_for_team(&mut self, t: u32, team: Option<u32>) -> R<Option<u32>> {
        Ok(self.make_object(t, team.map_or(-1, |t| t as i32)))
    }
    fn sn_recorder_game_mode(&self) -> i32 {
        (self.cfg[4] % 7) as i32
    }
    fn sn_add_landmark_bridge(&mut self, obj: u32) {
        self.line(format!("landmark bridge {obj}"));
    }
    fn sn_add_wall_piece(&mut self, obj: u32) {
        self.line(format!("wall piece {obj}"));
    }
    fn sn_update_obj_from_map(&mut self, obj: u32, map_object: usize) {
        self.line(format!("map values {obj} {map_object}"));
    }
    fn sn_radar_refresh_and_pathfinder_new_map(&mut self) {
        self.line("radar refresh".into());
        self.line("pathfinder newmap".into());
    }
    fn sn_find_player(&self, name: &[u8]) -> Option<i32> {
        // cfg[31] bit 3: player 9 is not named "ThePlayer"
        (0..11).find(|&i| Host::player_name(i) == name && !(i == 9 && self.cfg[31] & 8 != 0)).filter(|&i| self.exists(i))
    }
    fn sn_reveal_permanently(&mut self, player: i32) {
        self.line(format!("reveal permanently {player}"));
    }
    fn sn_reveal(&mut self, player: i32) {
        self.line(format!("reveal {player}"));
    }
    fn sn_shroud_in_multiplayer(&self) -> bool {
        self.flag(13)
    }
    fn sn_use_trees(&self) -> bool {
        self.flag(14)
    }
    fn sn_static_lod(&self) -> StaticLod {
        match self.cfg[21] % 3 {
            0 => StaticLod::High,
            1 => StaticLod::Custom,
            _ => StaticLod::Other,
        }
    }
    fn sn_use_shadow_volumes(&self) -> bool {
        self.flag(15)
    }
    fn sn_tree_drawable(&mut self, t: u32, angle: f32, pos: Coord3D) {
        let d = 500 + self.next_drawable % 80;
        self.next_drawable += 1;
        self.line(format!("new drawable {d} template {t}"));
        self.line(format!("drawable {d} angle {:.6}", f64::from(angle)));
        self.line(format!("drawable {d} at {:.4} {:.4} {:.4}", f64::from(pos.x), f64::from(pos.y), f64::from(pos.z)));
        self.line(format!("drawable {d} destroyed"));
    }
    fn sn_add_prop(&mut self, t: u32, pos: Coord3D, angle: f32) {
        self.line(format!("prop {t} {:.4} {:.4} {:.4} {:.6}", f64::from(pos.x), f64::from(pos.y), f64::from(pos.z), f64::from(angle)));
    }
    fn sn_validate_team(&mut self, owner: &[u8]) -> R<Option<u32>> {
        for i in 0..8u32 {
            if owner == format!("teamplayer{i}").as_bytes() {
                return Ok(Some(i));
            }
        }
        if !self.flag(22) {
            return Err(IniError::undefined(rts_lockstep_engine::common::ini::Undefined::NullObject));
        }
        Ok(Some(11))
    }
    fn sn_player_exists(&self, i: i32) -> bool {
        self.exists(i)
    }
    fn sn_object_casts_reflections(&self, obj: u32) -> bool {
        let t = self.template_of(obj) as u32;
        self.cfg[30] >> (3 * t + 1) & 1 != 0
    }
    fn sn_draws_in_mirror(&mut self, obj: u32) {
        if let Some(d) = self.objects[&obj].drawable {
            self.line(format!("drawable {d} status 64"));
        }
    }
    fn sn_set_layer_for(&mut self, obj: u32, pos: Coord3D) {
        let l = if pos.x > 500.0 { 2 } else { 1 };
        self.line(format!("layer {obj} {l}"));
    }
    fn sn_team_set_active(&mut self, team: u32) {
        self.line(format!("team {team} active"));
    }
    fn sn_set_slot_player_template(&mut self, slot: usize, t: i32) {
        let s = self.source.expect("a game");
        self.games[s].as_mut().expect("the game").slots[slot].set_player_template(t);
    }
    fn sn_is_internet_game(&self) -> bool {
        self.u_game_mode() == 5
    }
    fn sn_preload_assets(&mut self) {
        if self.flag(16) {
            if self.flag(17) {
                for td in 1..5 {
                    self.line(format!("preload {td}"));
                }
            } else {
                self.line("preload 2".into());
            }
        }
        self.line("communicator 0".into());
    }
    fn sn_view_defaults(&mut self) {
        self.line("view angle default".into());
        self.line("view zoom default".into());
    }
    fn sn_recorder_init_controls(&mut self) {
        self.line("recorder initcontrols".into());
    }
    fn sn_initial_camera_name(&self) -> Vec<u8> {
        // the stand-in's name keys never registered TheKey_InitialCameraPosition: keyToName gives ""
        Vec::new()
    }
    fn sn_view_look_at(&mut self, pos: Coord3D) {
        self.line(format!("lookat {:.4} {:.4} {:.4}", f64::from(pos.x), f64::from(pos.y), f64::from(pos.z)));
    }
    fn sn_view_init_height(&mut self) {
        self.line("view initheight".into());
    }
    fn sn_player_list_new_map(&mut self) {
        self.line("playerlist newmap".into());
    }
    fn sn_players_with_relationship(&self, _player: i32, _allowed: u32) -> u16 {
        self.cfg[20] as u16
    }
    fn sn_set_relationship(&mut self, a: i32, b: i32, rel: i32) {
        self.line(format!("relationship {a} {b} {rel}"));
    }
    fn sn_player_count(&self) -> i32 {
        12
    }
    fn sn_neutral_player(&self) -> Option<i32> {
        self.flag(22).then_some(11)
    }
    fn sn_player_is_human(&self, i: i32) -> bool {
        self.exists(i) && self.cfg[17] >> i & 1 != 0
    }
    fn sn_add_skill_points(&mut self, player: i32, points: i32) {
        self.line(format!("skill {player} {points}"));
    }
    fn sn_network_load_progress_complete(&mut self) {
        self.line("network loadcomplete".into());
        self.line("network liteupdate".into());
    }
    fn sn_sleep(&mut self, ms: u32) {
        self.line(format!("sleep {ms}"));
    }
    fn sn_transition_fade(&mut self) {
        self.line("transition [FadeWholeScreen]".into());
    }
    fn sn_transition_finished(&mut self) -> bool {
        let f = self.transition_calls >= self.cfg[22] % 4;
        self.transition_calls += 1;
        self.line(format!("transition finished {}", i32::from(f)));
        f
    }
    fn sn_window_manager_update(&mut self) {
        self.line("windows update".into());
    }
    fn sn_display_draw(&mut self) {
        self.line("display draw".into());
    }
    fn sn_shell_screen(&mut self) {
        if self.cfg[33].is_multiple_of(2) {
            self.line("shell push [Menus/MainMenu.wnd]".into());
        } else {
            self.line("shell top hide 0".into());
            self.line("shell top forward".into());
        }
        self.line("controlbar hide".into());
    }
    fn sn_stats_collector_reset(&mut self) {
        if self.flag(18) || self.flag(19) {
            self.line("stats reset".into());
        }
    }
    fn sn_replay_local_player(&mut self, observer: Option<i32>) {
        match observer {
            Some(o) => self.line(format!("local player {o}")),
            None => self.line("local player null".into()),
        }
        self.local_player = observer.or(self.sn_neutral_player());
        self.line("radar forceon 1".into());
        self.line("partition shroud".into());
    }
    fn sn_control_bar_scheme(&mut self) {
        self.line("controlbar scheme".into());
    }
    fn sn_view_ok_to_adjust_height(&mut self) {
        self.line("view adjust height 1".into());
    }
    fn sn_player_active(&self, i: i32) -> bool {
        self.exists(i) && self.cfg[18] >> i & 1 != 0
    }
    fn sn_player_objects(&self, i: i32) -> Vec<u32> {
        if (0..8).contains(&i) {
            vec![200 + 2 * i as u32, 201 + 2 * i as u32]
        } else {
            Vec::new()
        }
    }
    fn sn_is_command_center(&self, obj: u32) -> bool {
        let (i, k) = ((obj - 200) / 2, (obj - 200) % 2);
        self.cfg[19] >> (i + 8 * (1 - k)) & 1 != 0
    }
    fn sn_controlling_player_mask(&self, obj: u32) -> Option<u16> {
        let i = (obj - 200) / 2;
        (self.cfg[36] >> i & 1 == 0).then_some(1 << i)
    }
    fn sn_locally_controlled(&self, obj: u32) -> bool {
        let i = ((obj - 200) / 2) as i32;
        let controller = (self.cfg[36] >> i & 1 == 0).then_some(i);
        controller == self.local_player
    }
    fn sn_control_bar_finish(&mut self, shell: bool) {
        self.line("controlbar specialpower".into());
        self.line(if shell { "controlbar hide".into() } else { "controlbar show 0".into() });
    }
    fn sn_gamespy_playing(&mut self) {
        if self.flag(20) && self.games[1].is_some() {
            self.line("buddy 7 3 [Playing] [lobby]".into());
        }
    }
    fn sn_drawables_on_level_start(&mut self) {
        for i in 0..self.cfg[32] % 4 {
            self.line(format!("drawable {} level start", 570 + i));
        }
    }
    fn sn_gamespy_disconnections(&mut self) {
        if self.flag(21) {
            self.line("gamespy disconnections 1".into());
        }
    }
    fn sn_replay_message(&mut self) {
        if self.flag(25) {
            self.line("ui message".into());
        }
    }
}

fn dump_end(h: &mut Host, st: &GameLogicState) {
    let src = match h.source {
        None => "none",
        Some(0) => "lan",
        Some(1) => "gamespy",
        Some(2) => "recorder",
        Some(3) => "skirmish",
        _ => "challenge",
    };
    let line = format!(
        "end frame {} ranklimit {} render {} tivo {} markers {} icon {} lod {} hulk {} superweapon {} pending {} loadingmap {} gameinfo {src} skirmishinfo {} loadscreen {} force {} width {:.4} height {:.4}",
        h.frame,
        h.rank_limit,
        i32::from(h.render),
        i32::from(h.tivo),
        i32::from(st.show_behind_building_markers),
        i32::from(st.draw_icon_ui),
        i32::from(st.show_dynamic_lod),
        st.script_hulk_max_lifetime_override,
        st.superweapon_restriction,
        i32::from(h.start_new_game),
        i32::from(h.loading_map),
        i32::from(h.games[3].is_some()),
        i32::from(h.load_screen.is_some()),
        i32::from(st.force_game_start_by_timeout),
        f64::from(h.width),
        f64::from(h.height)
    );
    h.line(line);
    let pc: String = (0..MAX_SLOTS).map(|i| format!(" {}:{}", i32::from(st.progress_complete[i]), st.progress_complete_timeout[i])).collect();
    h.line(format!("progress{pc}"));
    for k in 0..5 {
        let Some(g) = &h.games[k] else {
            continue;
        };
        let s: String = g.slots.iter().map(|s| format!(" {}:{}:{}:{}:{}", s.state, s.color, s.start_pos, s.player_template, s.team_number)).collect();
        h.line(format!("game {k} slots{s}"));
    }
    let seed = h.rng.get_game_logic_random_seed();
    let (name, t) = match h.map_objects.first() {
        Some(m) => (lossy(&m.name), if m.template.is_some() { "t" } else { "-" }),
        None => ("none".to_string(), "-"),
    };
    h.line(format!("seed {seed} mapobjects {name} {t}"));
}

fn rust_run(cfg: &Cfg, slots: &Slots) -> (String, Option<String>) {
    let mut rng = RandomValues::default();
    rng.init_game_logic_random(cfg[0]);
    let flag = |n: u32| cfg[1] >> n & 1 != 0;
    let mut games: [Option<GameInfo>; 5] = Default::default();
    for (k, slot) in games.iter_mut().enumerate() {
        if cfg[3] >> k & 1 == 0 {
            continue;
        }
        let mut g = GameInfo::new(&mut InfoHost);
        g.map_name = b"maps\\one\\one.map".to_vec();
        g.superweapon_restriction = (k + 3) as u16;
        g.in_game = cfg[31] >> 2 & 1 != 0;
        g.local_ip = 77;
        for (i, s) in g.slots.iter_mut().enumerate() {
            let v = &slots[5 * i..5 * i + 5];
            s.state = (v[0] % 6) as i32;
            s.color = v[1] as i32;
            s.start_pos = v[2] as i32;
            s.player_template = v[3] as i32;
            s.team_number = v[4] as i32;
            s.name = vec![u16::from(b'A') + (i % 3) as u16, u16::from(b'a') + i as u16];
            s.ip = if i as u32 == cfg[35] % 8 { 77 } else { 0 };
        }
        *slot = Some(g);
    }
    let nobj = cfg[37] % 9;
    let map_objects: Vec<MapObj> = (0..nobj)
        .map(|i| {
            let t = (cfg[27] >> (4 * i)) & 7;
            let owner = if cfg[38] >> (16 + i) & 1 != 0 { format!("teamplayer{}", i % 8).into_bytes() } else { Vec::new() };
            let flags = (cfg[39] >> (3 * i)) & 7;
            MapObj {
                name: OBJ_NAMES[((cfg[38] >> (2 * i)) & 3) as usize].to_vec(),
                template: (t < 6).then_some(t),
                bridge_or_road: flags & 3 != 0,
                draws_in_mirror: flags & 4 != 0,
                location: Coord3D { x: 160.0 * i as f32 + 3.5, y: 90.0 * (i % 3) as f32, z: 1.5 },
                angle: -7.0 + 2.75 * i as f32,
                original_owner: owner,
            }
        })
        .collect();
    let mut h = Host {
        cfg: *cfg,
        rng,
        log: String::new(),
        start_new_game: flag(1),
        loading_map: false,
        frame: 55,
        rank_limit: 7,
        render: false,
        tivo: true,
        width: 0.0,
        height: 0.0,
        pristine: b"Maps\\Pristine\\Pristine.map".to_vec(),
        background: flag(2),
        load_screen: None,
        games,
        taken: None,
        source: None,
        objects: BTreeMap::new(),
        next_obj: 0,
        next_drawable: 0,
        next_group: 100,
        find_calls: 0,
        time: cfg[34],
        transition_calls: 0,
        local_player: flag(23).then_some((cfg[35] % 11) as i32),
        map_objects,
    };
    for i in 0..8u32 {
        for k in 0..2 {
            h.objects.insert(200 + 2 * i + k, Obj { template: None, drawable: None });
        }
    }
    let mut st = GameLogicState { superweapon_restriction: 9, script_hulk_max_lifetime_override: 5, rank_points_to_add_at_game_start: (cfg[16] >> 16) as i32, ..Default::default() };
    if let Err(e) = start_new_game(&mut st, flag(0), &mut h) {
        return (h.log, Some(format!("{e:?}")));
    }
    h.line("started".into());
    dump_end(&mut h, &st);
    (h.log, None)
}

fn cpp_run(cfg: &Cfg, slots: &Slots) -> String {
    let mut cap = 1 << 18;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_glnew_run(cfg.as_ptr(), slots.as_ptr(), out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

fn gen_program(g: &mut Gen) -> (Cfg, Slots) {
    let mut c: Cfg = [0; 40];
    c[0] = g.next();
    // flags: mostly a game that loads, with the singletons a game has
    let mut f = g.next();
    f &= !0b11; // not loading, no pending new game (the usual call)
    if g.chance(4) {
        f |= 1; // loading a save
    }
    if g.chance(5) {
        f |= 2; // a new game already pending
    }
    f |= 1 << 22 | 1 << 23; // a neutral player and a local player, mostly
    if g.chance(10) {
        f &= !(1 << 22);
    }
    if g.chance(10) {
        f &= !(1 << 23);
    }
    if !g.chance(8) {
        f |= 1 << 7; // a recorder
    }
    c[1] = f;
    c[2] = g.below(7);
    c[3] = if g.chance(5) { g.next() } else { 0b11111 };
    c[4] = g.below(7);
    c[5] = g.below(8);
    c[6] = g.below(4);
    c[7] = g.next();
    c[8] = if g.chance(6) { g.below(10) } else { 8 + g.below(2) };
    c[9] = if g.chance(5) { g.below(13) } else { 12 };
    c[10] = if g.chance(12) { g.next() & g.next() } else { 0 };
    c[11] = if g.chance(4) { g.next() } else { 0xfff };
    c[12] = if g.chance(3) { 12 + g.below(4) } else { g.below(12) };
    c[13] = g.next();
    c[14] = g.next();
    c[15] = g.next();
    c[16] = (if g.chance(8) { g.next() & 0xfff } else { 0xfff }) | (g.below(50) << 16);
    c[17] = g.next();
    c[18] = g.next();
    c[19] = g.next();
    c[20] = g.next() & 0xffff;
    c[21] = g.below(3);
    c[22] = g.below(4);
    c[23] = 7000 + g.below(30000);
    c[24] = if g.chance(4) { g.next() } else { 0xffff };
    c[25] = if g.chance(4) { g.next() & g.next() } else { 0 };
    c[26] = if g.chance(2) { g.next() } else { 0 };
    c[27] = g.next();
    c[28] = g.next() & g.next();
    c[29] = g.next() & g.next();
    c[30] = g.next();
    // bit 2: the games are in progress (getLocalSlotNum finds the local slot), mostly
    c[31] = if g.chance(8) { g.next() } else { g.next() | 4 } & !8;
    c[32] = g.below(4);
    c[33] = g.below(2);
    c[34] = g.below(100_000);
    c[35] = g.next();
    c[36] = if g.chance(4) { g.next() } else { 0 };
    c[37] = g.below(9);
    c[38] = g.next();
    c[39] = g.next() & g.next();
    // a challenge campaign now and then: single player, a challenge campaign, no network, no replay
    if g.chance(8) {
        c[2] = 0;
        c[1] |= 1 << 3 | 1 << 4;
        c[1] &= !(1 << 5 | 1 << 8);
        c[3] |= 1 << 4;
        if g.chance(2) {
            c[31] |= 8; // no player named "ThePlayer"
        } else {
            c[31] &= !8;
        }
    }
    let mut s: Slots = [0; 40];
    for i in 0..8 {
        s[5 * i] = if g.chance(3) { 1 } else { [0, 2, 3, 4, 5, 5][g.below(6) as usize] };
        s[5 * i + 1] = if g.chance(3) { (-1i32) as u32 } else { g.below(8) };
        s[5 * i + 2] = if g.chance(2) { (-1i32) as u32 } else { g.below(6) };
        s[5 * i + 3] = match g.below(8) {
            0 => (-1i32) as u32,
            1 => (-2i32) as u32,
            _ => g.below(12),
        };
        s[5 * i + 4] = if g.chance(3) { (-1i32) as u32 } else { g.below(4) };
    }
    // the local slot (cfg[35] % 8) is mostly a human
    if !g.chance(8) {
        s[5 * (c[35] % 8) as usize] = 5;
    }
    (c, s)
}

#[test]
fn game_logic_new_game_matches_upstream() {
    let cases: usize = std::env::var("GLNEW_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x474c_4e33_cafe_0001);
    let (mut mismatches, mut undefined, mut compared, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut why = BTreeMap::<String, usize>::new();
    let mut marks = BTreeMap::<&str, usize>::new();
    for case in 0..cases {
        let (cfg, slots) = gen_program(&mut g);
        let (r, u) = rust_run(&cfg, &slots);
        if let Some(w) = u {
            undefined += 1;
            *why.entry(w).or_insert(0) += 1;
            continue;
        }
        if std::env::var("GLNEW_TRACE").is_ok() {
            println!("running case {case}");
        }
        let c = cpp_run(&cfg, &slots);
        compared += 1;
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let (rl, cl): (Vec<&str>, Vec<&str>) = (r.lines().collect(), c.lines().collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                println!("case {case}: cfg {cfg:?} slots {slots:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}", rl.get(at), cl.get(at));
            }
        } else {
            lines += r.lines().count();
            for l in r.lines() {
                for m in ["side name", "mpscripts add", "new object", "prop", "reveal", "relationship", "skill", "player", "transition finished 0", "sleep 100", "drawable", "landmark", "ui message", "buddy"] {
                    if l.starts_with(m) {
                        *marks.entry(m).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    println!("L3b: {cases} programs, {compared} compared ({undefined} undefined, not run in C++), {lines} lines, {mismatches} mismatches");
    println!("reached: {marks:?}");
    println!("undefined: {why:?}");
    assert_eq!(mismatches, 0);
}
