// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AISkirmishPlayer.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AISkirmishPlayer.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `AISkirmishPlayer`: the skirmish opponent (`p1w13-sonnet`, layer 10, batch A29). An [`AiPlayer`] with a [`SkirmishState`]: its
//! constructor turns unit building on; its base building (power first when under-powered, dozers that went over to another player
//! or unmanned replaced), its build list (the faction's skirmish list from the AI data, turned to its start position), its enemy
//! (the closest enemy player's base, weighed against the other skirmish AIs' choices, checked every 5 seconds), base defences on
//! the circle around its base toward the approach paths, bridge checks along a path, cluster mines at its base's entrances, and
//! its timers (capped at 3 seconds; teams every 2 seconds).
//!
//! The overrides upstream that only call `AIPlayer`'s (`onUnitProduced`, `selectTeamToReinforce`, `selectTeamToBuild`,
//! `buildSpecificAITeam`, `checkReadyTeams`, `checkQueuedTeams`, `update`, `queueDozer`, `findDozer`) or copy it (`startTraining`,
//! `isAGoodIdeaToBuildTeam`, `processTeamBuilding`) are `AiPlayer`'s own; `recruitSpecificAITeam`'s copy differs in one line (it
//! reports a missing home position), taken by [`AiPlayer::recruit_specific_ai_team`] for a skirmish AI.
//!
//! # Upstream facts kept
//!
//! - `processBaseBuilding` asks `canMakeUnit` about the plan chosen so far, not the entry's own (`canMakeUnit` refuses none): an
//!   automatic entry is taken only after a priority or power plan was.
//! - `adjustBuildList` turns the entries when the list's head (not each entry) is a command centre's.
//! - `xfer` leaves out the enemy and the frame it is next checked at.
//! - `getMyEnemyPlayerIndex` without a current enemy is the first human player, or the player count when there is none.

use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::*;
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::special_power_type::{SpecialPowerType, NUKE_SPECIAL_CLUSTER_MINES, SPECIAL_CLUSTER_MINES};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_data::AiDataEntry;
use crate::logic::ai::ai_player::{legal, AiPlayer, AiPlayerHost, AiPlayerUndefined, PlayerAiCommand, PlayerRef, TemplateRef};
use crate::logic::modules::plant;
use crate::logic::sides_list::BuildListInfo;
use petir::Real;

/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: i32 = 30;
/// `HUGE_DIST` (PartitionManager.h).
const HUGE_DIST: f32 = 1_000_000.0;
/// `PI`.
const PI: f32 = std::f32::consts::PI;
/// The approach paths' labels (Scripts.h's `SKIRMISH_CENTER`, `SKIRMISH_FLANK`, `SKIRMISH_BACKDOOR`).
pub const SKIRMISH_CENTER: &[u8] = b"Center";
pub const SKIRMISH_FLANK: &[u8] = b"Flank";
pub const SKIRMISH_BACKDOOR: &[u8] = b"Backdoor";

/// `AISkirmishPlayer`'s own members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkirmishState {
    /// `m_curFrontBaseDefense`, `m_curFlankBaseDefense`: how many were tried (first is 0).
    pub cur_front_base_defense: i32,
    pub cur_flank_base_defense: i32,
    /// The defences' angles on the base circle, each side of each approach.
    pub cur_front_left_defense_angle: f32,
    pub cur_front_right_defense_angle: f32,
    pub cur_left_flank_left_defense_angle: f32,
    pub cur_left_flank_right_defense_angle: f32,
    pub cur_right_flank_left_defense_angle: f32,
    pub cur_right_flank_right_defense_angle: f32,
    /// `m_frameToCheckEnemy`, `m_currentEnemy`.
    pub frame_to_check_enemy: u32,
    pub current_enemy: Option<PlayerRef>,
}

/// `sin(angle)`, `cos(angle)` of a `Real`: the `double` functions, rounded back (as the reference build calls them).
fn c_sin(x: f32) -> f32 {
    Real::sin(f64::from(x)) as f32
}
fn c_cos(x: f32) -> f32 {
    Real::cos(f64::from(x)) as f32
}

/// `label` then the decimal `n` (`format("%s%d", label, n)`).
fn path_label(label: &[u8], n: i32) -> Vec<u8> {
    let mut v = label.to_vec();
    v.extend_from_slice(n.to_string().as_bytes());
    v
}

impl AiPlayer {
    /// `AISkirmishPlayer(p)`: `AIPlayer(p)`, then the last building's frame again and unit building turned on.
    pub fn new_skirmish<H: AiPlayerHost>(h: &mut H, player: PlayerRef, global_difficulty: crate::common::game_common::GameDifficulty, data: &AiDataEntry) -> Self {
        let mut a = AiPlayer::new(h, player, true, global_difficulty, data);
        a.skirmish_state = Some(SkirmishState::default());
        a.frame_last_building_built = h.frame() as i32;
        h.set_can_build_units(player, true); // "turn on ai production by default."
        a
    }

    fn sk(&mut self) -> &mut SkirmishState {
        self.skirmish_state.as_mut().expect("a skirmish AI")
    }

    /// `AISkirmishPlayer::processBaseBuilding()`: over the build list, holes and captures as `AIPlayer`'s, a dozer that is not the
    /// player's or is unmanned replaced (`setBuilder(NULL)`, a dozer queued, the closest found); then the first priority entry,
    /// else a power plant when under-powered (or automatic) and none is under construction, else the first automatic buildable
    /// entry a dozer can make (see the module's note), built with a dozer.
    pub(crate) fn skirmish_process_base_building<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if !self.ready_to_build_structure {
            return;
        }
        let p = self.player;
        let mut bldg_plan: Option<TemplateRef> = None;
        let mut bldg_info: Option<usize> = None;
        let mut is_priority = false;
        let mut power_plan: Option<TemplateRef> = None;
        let mut power_info: Option<usize> = None;
        let is_under_powered = !h.has_sufficient_power(p);
        let mut power_under_construction = false;
        let mut idx = 0;
        while idx < h.build_list(p).len() {
            let i = idx;
            idx += 1;
            let name = h.build_list(p)[i].template_name.clone();
            if name.is_empty() {
                continue;
            }
            let Some(cur_plan) = h.find_template(&name) else { continue };
            let id = h.build_list(p)[i].object_id;
            // (the building as the entry named it, before the checks below)
            let bldg = h.object_exists(id).then_some(id);
            if id != INVALID_ID {
                if !h.object_exists(id) {
                    let frame = h.frame();
                    {
                        let e = &mut h.build_list(p)[i];
                        e.object_id = INVALID_ID;
                        e.object_timestamp = frame.wrapping_add(1);
                    }
                    for obj in h.objects() {
                        if !h.object_is_kind_of(obj, KINDOF_REBUILD_HOLE) {
                            continue;
                        }
                        if let Some((spawner, _)) = h.rebuild_hole(obj) {
                            if id == spawner {
                                h.build_list(p)[i].object_id = obj;
                            }
                        }
                    }
                } else if h.controlling_player(id) == Some(p) {
                    if h.status_test(id, OBJECT_STATUS_UNDER_CONSTRUCTION) {
                        if h.object_is_kind_of(id, KINDOF_FS_POWER) && !h.object_is_kind_of(id, KINDOF_CASH_GENERATOR) && !plant(3600) {
                            power_under_construction = true;
                        }
                        let builder = h.builder_id(id);
                        let mut my_dozer = h.object_exists(builder).then_some(builder);
                        if let Some(d) = my_dozer {
                            if h.controlling_player(d) != Some(p) || h.is_disabled_unmanned(d) {
                                // "I don't expect this dozer to work well with me."
                                my_dozer = None;
                                if !plant(3601) {
                                    h.clear_builder(id);
                                }
                            }
                        }
                        match my_dozer {
                            None => {
                                self.queue_dozer(h);
                                let pos = h.position(id);
                                match self.find_dozer(h, data, &pos) {
                                    Some(d) if h.has_ai(d) => h.ai_command(d, PlayerAiCommand::ResumeConstruction(id)),
                                    _ => continue,
                                }
                            }
                            Some(d) => h.ai_command(d, PlayerAiCommand::ResumeConstruction(id)),
                        }
                    }
                } else {
                    // "oops, got captured."
                    let frame = h.frame();
                    let e = &mut h.build_list(p)[i];
                    e.object_id = INVALID_ID;
                    e.object_timestamp = frame.wrapping_add(1);
                }
            }
            let (oid, ts) = (h.build_list(p)[i].object_id, h.build_list(p)[i].object_timestamp);
            if oid == INVALID_ID && ts > 0 {
                let delay = data.data.rebuild_delay_seconds.wrapping_mul(LOGICFRAMES_PER_SECOND) as u32;
                if ts.wrapping_add(delay) > h.frame() {
                    continue;
                }
                h.build_list(p)[i].object_timestamp = 0; // "ready to build."
            }
            if bldg.is_some() {
                continue; // "already built."
            }
            let loc = h.build_list(p)[i].location;
            if !self.is_location_safe(h, data, &loc, Some(cur_plan)) {
                continue;
            }
            let (priority, automatic) = (h.build_list(p)[i].priority_build, h.build_list(p)[i].automatically_build);
            if priority && !is_priority {
                bldg_plan = Some(cur_plan);
                bldg_info = Some(i);
                is_priority = true;
            }
            if h.template_is_kind_of(cur_plan, KINDOF_FS_POWER)
                && power_plan.is_none()
                && !h.template_is_kind_of(cur_plan, KINDOF_CASH_GENERATOR)
                && (is_under_powered || automatic)
            {
                power_plan = Some(cur_plan);
                power_info = Some(i);
            }
            if !automatic {
                continue; // "marked to not build automatically."
            }
            let Some(dozer) = self.find_dozer(h, data, &loc) else {
                if is_under_powered {
                    self.queue_dozer(h);
                }
                continue;
            };
            // (the plan chosen so far, as upstream)
            let asked = if plant(3602) { Some(cur_plan) } else { bldg_plan };
            if !h.can_make_unit(dozer, asked) {
                if h.build_list(p)[i].is_buildable() {
                    let mut s = name.clone();
                    s.extend_from_slice(b" - Dozer unable to build - money or technology missing.");
                    h.append_debug_message(&s);
                }
                continue;
            }
            if h.build_list(p)[i].is_buildable() && bldg_plan.is_none() {
                bldg_plan = Some(cur_plan);
                bldg_info = Some(i);
            }
        }
        if let (Some(pp), Some(pi)) = (power_plan, power_info) {
            let equivalent = match bldg_plan {
                Some(b) => h.template_equivalent(pp, b),
                None => false,
            };
            if !equivalent && !power_under_construction {
                bldg_plan = Some(pp); // "Forcing build of power plant."
                bldg_info = Some(pi);
            }
        }
        if let (Some(plan), Some(info)) = (bldg_plan, bldg_info) {
            if let Some(b) = self.build_structure_with_dozer(h, data, plan, info) {
                {
                    let e = &mut h.build_list(p)[info];
                    e.object_id = b;
                    e.decrement_num_rebuilds();
                }
                self.ready_to_build_structure = false;
                self.reset_structure_timer(h, data);
                self.frame_last_building_built = h.frame() as i32;
            }
        }
    }

    /// `AISkirmishPlayer::buildSpecificAIBuilding(thingName)`: the first entry of that template not built and not yet a priority
    /// is marked a priority build (and the build delay ended).
    pub(crate) fn skirmish_build_specific_ai_building<H: AiPlayerHost>(&mut self, h: &mut H, thing_name: &[u8]) {
        let p = self.player;
        let mut found = false;
        let mut found_unbuilt = false;
        for i in 0..h.build_list(p).len() {
            if h.build_list(p)[i].template_name != thing_name {
                continue;
            }
            if h.build_list(p)[i].template_name.is_empty() {
                continue;
            }
            if h.find_template(thing_name).is_none() {
                continue;
            }
            let id = h.build_list(p)[i].object_id;
            found = true;
            if h.object_exists(id) {
                continue; // "already built."
            }
            if h.build_list(p)[i].priority_build {
                continue; // "already marked for priority build."
            }
            found_unbuilt = true;
            h.build_list(p)[i].priority_build = true; // (`markPriorityBuild()`)
            break;
        }
        let msg = if found_unbuilt {
            self.build_delay = 0;
            [b"Queueing building '".as_slice(), thing_name, b"' for construction."].concat()
        } else if found {
            [b"Warning - all instances of building '".as_slice(), thing_name, b"' are already built or queued for build, not queueing."].concat()
        } else {
            [b"Error - could not find building '".as_slice(), thing_name, b"' in the building template list."].concat()
        };
        h.append_debug_message(&msg);
    }

    /// `getMyEnemyPlayerIndex()`: the current enemy's, else the first human player's (the player count when there is none).
    pub fn get_my_enemy_player_index<H: AiPlayerHost>(&mut self, h: &mut H) -> PlayerRef {
        if let Some(e) = self.skirmish_state.as_ref().and_then(|s| s.current_enemy) {
            return e;
        }
        let n = h.player_count();
        let mut i = 0;
        while i < n {
            if h.player_is_human(i) {
                break;
            }
            i += 1;
        }
        i
    }

    /// The answers of `isSkirmishAIPlayer()` and `getCurrentEnemy()` for a player: this AI's own for its player.
    fn skirmish_and_enemy<H: AiPlayerHost>(&self, h: &mut H, k: PlayerRef) -> (bool, Option<PlayerRef>) {
        if k == self.player {
            return (self.skirmish, self.skirmish_state.as_ref().and_then(|s| s.current_enemy));
        }
        (h.player_is_skirmish_ai(k), h.player_current_enemy(k))
    }

    /// `acquireEnemy()`: kept while it has units and a build facility; else the enemy player (with objects) whose base is closest
    /// to this one (a crippled one at half the huge distance squared; 500 squared more for each other skirmish AI already on it,
    /// 25 squared less for each one on this player), announced when it changes.
    pub fn acquire_enemy<H: AiPlayerHost>(&mut self, h: &mut H) {
        let p = self.player;
        let mut best: Option<PlayerRef> = None;
        let mut best_dist_sqr = HUGE_DIST * HUGE_DIST;
        if let Some(e) = self.sk().current_enemy {
            let in_bad_shape = !h.player_has_any_units(e) || !h.player_has_any_build_facility(e);
            if !in_bad_shape {
                return;
            }
        }
        let n = h.player_count();
        for i in 0..n {
            let dt = h.default_team(i);
            if h.relationship_to_team(p, dt) != crate::common::game_common::Relationship::Enemies {
                continue;
            }
            if !h.player_has_any_objects(i) {
                continue; // "not much of an enemy."
            }
            let in_bad_shape = !h.player_has_any_units(i) || !h.player_has_any_build_facility(i);
            let bounds = Self::get_player_structure_bounds(h, i, false);
            let enemy_x = bounds.lo.x + bounds.width() / 2.0;
            let enemy_y = bounds.lo.y + bounds.height() / 2.0;
            let (dx, dy) = (enemy_x - self.base_center.x, enemy_y - self.base_center.y);
            let mut cur = dx * dx + dy * dy;
            if in_bad_shape {
                cur = HUGE_DIST * HUGE_DIST * 0.5;
            }
            for k in 0..n {
                if k == i {
                    continue; // "don't count self."
                }
                let (skirmish, enemy) = self.skirmish_and_enemy(h, k);
                if skirmish && enemy == Some(i) && !plant(3603) {
                    cur += (500 * 500) as f32;
                }
                if skirmish && enemy == Some(p) {
                    cur -= (25 * 25) as f32;
                    if cur < 0.0 {
                        cur = 0.0;
                    }
                }
            }
            if cur < best_dist_sqr {
                best = Some(i);
                best_dist_sqr = cur;
            }
        }
        if let Some(b) = best {
            if Some(b) != self.sk().current_enemy {
                self.sk().current_enemy = Some(b);
                let mut msg = h.player_name(p);
                msg.extend_from_slice(b" acquiring target enemy player: ");
                msg.extend_from_slice(&h.player_name(b));
                h.append_debug_message(&msg);
            }
        }
    }

    /// `AISkirmishPlayer::getAiEnemy()`: re-acquired every 5 seconds.
    pub(crate) fn skirmish_get_ai_enemy<H: AiPlayerHost>(&mut self, h: &mut H) -> Option<PlayerRef> {
        let frame = h.frame();
        if frame >= self.sk().frame_to_check_enemy {
            self.sk().frame_to_check_enemy = frame.wrapping_add((5 * LOGICFRAMES_PER_SECOND) as u32);
            self.acquire_enemy(h);
        }
        self.sk().current_enemy
    }

    /// `AISkirmishPlayer::buildAIBaseDefense(flank)`: the side's first base defence structure.
    pub(crate) fn skirmish_build_ai_base_defense<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, flank: bool) {
        let side = h.side(self.player);
        let Some(info) = data.lists.side_info.iter().find(|s| s.side == side) else { return };
        let name = info.base_defense_structure1.clone();
        self.skirmish_build_ai_base_defense_structure(h, data, &name, flank);
    }

    /// `AISkirmishPlayer::buildAIBaseDefenseStructure(thingName, flank)`: on the circle `baseRadius` plus the AI data's extra
    /// distance around the base centre, toward the approach path's closest waypoint (front: the centre path, else the enemy's
    /// base; flank: alternately the flank and the backdoor paths, none without one), alternately left and right of it at steps of
    /// four structure radii, the first legal place (up to 60 degrees on the left) added to the priority build list.
    pub(crate) fn skirmish_build_ai_base_defense_structure<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, thing_name: &[u8], flank: bool) {
        let p = self.player;
        let Some(t) = h.find_template(thing_name) else { return };
        loop {
            let start = h.mp_start_index(p).wrapping_add(1);
            let label = if flank {
                if self.sk().cur_flank_base_defense & 1 != 0 {
                    path_label(SKIRMISH_FLANK, start)
                } else {
                    path_label(SKIRMISH_BACKDOOR, start)
                }
            } else {
                path_label(SKIRMISH_CENTER, start)
            };
            let mut goal = self.base_center;
            match h.closest_waypoint_on_path(&goal, &label) {
                Some(w) => goal = w,
                None => {
                    if flank {
                        return;
                    }
                    let e = self.get_my_enemy_player_index(h);
                    let bounds = Self::get_player_structure_bounds(h, e, false);
                    goal.x = bounds.lo.x + bounds.width() / 2.0;
                    goal.y = bounds.lo.y + bounds.height() / 2.0;
                }
            }
            let mut offset = Coord2D { x: goal.x - self.base_center.x, y: goal.y - self.base_center.y };
            offset.normalize();
            let mut defense_distance = self.base_radius;
            defense_distance += data.data.skirmish_base_defense_extra_distance;
            offset.x *= defense_distance;
            offset.y *= defense_distance;
            let structure_radius = h.template_bounding_radius(t);
            let base_circumference = 2.0 * PI * defense_distance;
            let angle_offset = 2.0 * PI * (structure_radius * 4.0 / base_circumference);
            let s = self.sk();
            let angle;
            if flank {
                let selector = s.cur_flank_base_defense >> 1;
                if s.cur_flank_base_defense & 1 != 0 {
                    if selector & 1 != 0 {
                        s.cur_left_flank_right_defense_angle -= angle_offset;
                        angle = s.cur_left_flank_right_defense_angle;
                    } else {
                        angle = s.cur_left_flank_left_defense_angle;
                        s.cur_left_flank_left_defense_angle += angle_offset;
                    }
                } else if selector & 1 != 0 {
                    s.cur_right_flank_right_defense_angle -= angle_offset;
                    angle = s.cur_right_flank_right_defense_angle;
                } else {
                    angle = s.cur_right_flank_left_defense_angle;
                    s.cur_right_flank_left_defense_angle += angle_offset;
                }
            } else {
                let selector = s.cur_front_base_defense;
                if selector & 1 != 0 && !plant(3604) {
                    s.cur_front_right_defense_angle -= angle_offset;
                    angle = s.cur_front_right_defense_angle;
                } else {
                    angle = s.cur_front_left_defense_angle;
                    s.cur_front_left_defense_angle += angle_offset;
                }
            }
            if angle > PI / 3.0 {
                break;
            }
            let (sn, c) = (c_sin(angle), c_cos(angle));
            let mut build_pos = self.base_center;
            build_pos.x += offset.x * c - offset.y * sn;
            build_pos.y += offset.y * c + offset.x * sn;
            let place_angle = h.template_placement_angle(t);
            let can_build = h.is_location_legal_to_build(&build_pos, t, place_angle, legal::TERRAIN_RESTRICTIONS | legal::NO_OBJECT_OVERLAP, None, p);
            h.remove_all_bibs();
            if flank {
                self.sk().cur_flank_base_defense += 1;
            } else {
                self.sk().cur_front_base_defense += 1;
            }
            if can_build {
                h.add_to_priority_build_list(p, thing_name, &build_pos, place_angle);
                break;
            }
        }
    }

    /// `AISkirmishPlayer::checkBridges(unit, way)`: along the waypoints from `way`, the first one the unit has no path to whose
    /// way is cut by a broken bridge: the bridge's repair requested, true. False for a unit without an AI.
    pub fn check_bridges<H: AiPlayerHost>(&mut self, h: &mut H, unit: ObjectId, waypoints: &[Coord3D]) -> bool {
        if self.skirmish_state.is_none() {
            return false; // (AIPlayer's)
        }
        if !h.has_ai(unit) {
            return false; // "no ai"
        }
        for w in waypoints {
            if h.quick_does_path_exist(unit, w) {
                continue;
            }
            if let Some(bridge) = h.find_broken_bridge(unit, w) {
                self.repair_structure(h, bridge);
                return true;
            }
        }
        false
    }

    /// `AISkirmishPlayer::doBaseBuilding()`: `AIPlayer`'s with the structure timer capped at 3 seconds while waiting.
    pub(crate) fn skirmish_do_base_building<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if !h.can_build_base(self.player) {
            return;
        }
        if !self.ready_to_build_structure {
            self.structure_timer -= 1;
            if self.structure_timer <= 0 {
                self.ready_to_build_structure = true;
                self.build_delay = 0;
            }
            if self.structure_timer > 3 * LOGICFRAMES_PER_SECOND && !plant(3605) {
                self.structure_timer = 3 * LOGICFRAMES_PER_SECOND;
            }
        }
        self.build_delay -= 1;
        if self.build_delay < 1 {
            if self.ready_to_build_structure {
                self.process_base_building(h, data);
            }
            if self.build_delay < 1 {
                self.build_delay = 2 * LOGICFRAMES_PER_SECOND; // "check again in 2 seconds."
            }
        }
    }

    /// `AISkirmishPlayer::doTeamBuilding()`: `AIPlayer`'s with the team timer capped at 3 seconds while waiting and the queues
    /// checked every 2 seconds.
    pub(crate) fn skirmish_do_team_building<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if !h.can_build_units(self.player) {
            return;
        }
        if !self.ready_to_build_team {
            self.team_timer -= 1;
            if self.team_timer <= 0 {
                self.ready_to_build_team = true;
                self.team_delay = 0;
            }
            if self.team_timer > 3 * LOGICFRAMES_PER_SECOND {
                self.team_timer = 3 * LOGICFRAMES_PER_SECOND;
            }
        }
        self.team_delay -= 1;
        if self.team_delay < 1 {
            self.queue_units(h, data);
            if self.ready_to_build_team {
                self.process_team_building(h, data);
            }
            self.team_delay = if plant(3606) { 5 } else { 2 } * LOGICFRAMES_PER_SECOND; // "check again in 5 seconds."
        }
    }

    /// `adjustBuildList(list)`: the player's command centre (the first in the object list) is taken away (`onStructureUndone`,
    /// off the pathfinding map, destroyed); the list's command centre entries are marked initially built; when the list's head is
    /// one, every entry is turned about the (last) command centre entry by the start position's ninth of the map (with the AI
    /// data's base rotation) plus 135 degrees and moved to the start position.
    fn adjust_build_list<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, list: &mut [BuildListInfo]) {
        let p = self.player;
        let mut start: Option<Coord3D> = None;
        for o in h.objects() {
            if h.controlling_player(o) == Some(p) && h.object_is_kind_of(o, KINDOF_COMMANDCENTER) {
                start = Some(h.position(o));
                h.undo_and_destroy_structure(o);
                break;
            }
        }
        let Some(start_pos) = start else { return }; // "Couldn't find starting command center for ai player."
        let mut build_pos: Option<Coord3D> = None;
        for e in list.iter_mut() {
            if let Some(t) = h.find_template(&e.template_name) {
                if h.template_is_kind_of(t, KINDOF_COMMANDCENTER) {
                    build_pos = Some(e.location);
                    e.is_initially_built = true;
                }
            }
        }
        let bounds = h.maximum_pathfind_extent();
        let mut grid = 0;
        if start_pos.x > bounds.lo.x + bounds.width() / 3.0 {
            grid += 1;
        }
        if start_pos.x > bounds.lo.x + 2.0 * bounds.width() / 3.0 {
            grid += 1;
        }
        if start_pos.y > bounds.lo.y + bounds.height() / 3.0 {
            grid += 3;
        }
        if start_pos.y > bounds.lo.y + 2.0 * bounds.height() / 3.0 {
            grid += 3;
        }
        let mut angle = 0.0f32;
        if data.data.rotate_skirmish_bases {
            angle = match grid {
                1 => PI / 4.0,
                2 => PI / 2.0,
                3 => -PI / 4.0,
                5 => 3.0 * PI / 4.0,
                6 => -PI / 2.0,
                7 => -3.0 * PI / 4.0,
                8 => PI,
                _ => 0.0,
            };
        }
        angle += 3.0 * PI / 4.0;
        let (s, c) = (c_sin(angle), c_cos(angle));
        // (the list's head, not each entry, as upstream)
        let head_is_cc = list.first().and_then(|e| h.find_template(&e.template_name)).is_some_and(|t| h.template_is_kind_of(t, KINDOF_COMMANDCENTER));
        if !head_is_cc && !plant(3607) {
            return;
        }
        let Some(bp) = build_pos else { return };
        for e in list.iter_mut() {
            let mut cur = e.location;
            cur.x -= bp.x;
            cur.y -= bp.y;
            let new_x = cur.x * c - cur.y * s;
            let new_y = cur.y * c + cur.x * s;
            cur.x = new_x + start_pos.x;
            cur.y = new_y + start_pos.y;
            e.location = cur;
        }
    }

    /// `AISkirmishPlayer::newMap()`: the faction's skirmish build list from the AI data, copied, adjusted to the start position
    /// and made the player's (the base's centre and radius computed); then the initially built entries built at once, the others
    /// given a rebuild.
    pub(crate) fn skirmish_new_map<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        let p = self.player;
        let side = h.side(p);
        if let Some(b) = data.lists.side_build_lists.iter().find(|b| b.side == side) {
            let mut list = BuildListInfo::duplicate_chain(&b.build_list);
            self.adjust_build_list(h, data, &mut list);
            *h.build_list(p) = list;
            let (c, r) = self.compute_center_and_radius_of_base(h);
            self.base_center = c;
            self.base_radius = r;
        }
        for i in 0..h.build_list(p).len() {
            let name = h.build_list(p)[i].template_name.clone();
            if name.is_empty() {
                continue;
            }
            let Some(plan) = h.find_template(&name) else { continue };
            if h.build_list(p)[i].is_initially_built {
                self.build_structure_now(h, data, plan, i);
            } else {
                h.build_list(p)[i].increment_num_rebuilds();
            }
        }
    }

    /// `AISkirmishPlayer::computeSuperweaponTarget`: cluster mines go to one of the base's entrances (a random approach path's
    /// closest waypoint, else toward the enemy's base) on the base circle; anything else as `AIPlayer`'s.
    pub(crate) fn skirmish_compute_superweapon_target<H: AiPlayerHost>(
        &mut self,
        h: &mut H,
        power: SpecialPowerType,
        player_ndx: PlayerRef,
        weapon_radius: f32,
    ) -> Result<(Coord3D, bool), AiPlayerUndefined> {
        let _ = Self::get_player_structure_bounds(h, player_ndx, false);
        if power == SPECIAL_CLUSTER_MINES || power == NUKE_SPECIAL_CLUSTER_MINES {
            let mode = h.logic_random_value(0, 2);
            let start = h.mp_start_index(self.player).wrapping_add(1);
            let label = match if plant(3608) { 2 - mode } else { mode } {
                1 => path_label(SKIRMISH_FLANK, start),
                2 => path_label(SKIRMISH_BACKDOOR, start),
                _ => path_label(SKIRMISH_CENTER, start),
            };
            let mut goal = self.base_center;
            match h.closest_waypoint_on_path(&goal, &label) {
                Some(w) => goal = w,
                None => {
                    let e = self.get_my_enemy_player_index(h);
                    let bounds = Self::get_player_structure_bounds(h, e, false);
                    goal.x = bounds.lo.x + bounds.width() / 2.0;
                    goal.y = bounds.lo.y + bounds.height() / 2.0;
                }
            }
            let mut offset = Coord2D { x: goal.x - self.base_center.x, y: goal.y - self.base_center.y };
            offset.normalize();
            offset.x *= self.base_radius;
            offset.y *= self.base_radius;
            let mut r = self.base_center;
            r.x += offset.x;
            r.y += offset.y;
            r.z = h.ground_height(r.x, r.y);
            return Ok((r, true));
        }
        self.base_compute_superweapon_target(h, power, player_ndx, weapon_radius)
    }

    /// `AISkirmishPlayer::xfer` (version 1): `AIPlayer`'s, then the base defence counts and angles.
    pub(crate) fn skirmish_xfer<H: AiPlayerHost>(&mut self, h: &mut H, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.base_xfer(h, xfer)?;
        let s = self.sk();
        xfer.xfer_int(&mut s.cur_front_base_defense)?;
        xfer.xfer_int(&mut s.cur_flank_base_defense)?;
        xfer.xfer_real(&mut s.cur_front_left_defense_angle)?;
        xfer.xfer_real(&mut s.cur_front_right_defense_angle)?;
        xfer.xfer_real(&mut s.cur_left_flank_left_defense_angle)?;
        xfer.xfer_real(&mut s.cur_left_flank_right_defense_angle)?;
        xfer.xfer_real(&mut s.cur_right_flank_left_defense_angle)?;
        xfer.xfer_real(&mut s.cur_right_flank_right_defense_angle)
    }
}
