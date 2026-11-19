#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BattlePlanUpdate.cpp (createVisionObject)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/ChinookAIUpdate.cpp (private___TellPortableStructureToAttackWithMe)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGroup.cpp (groupDoCommandButtonUsingWaypoints)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIStateMachine::setGoalSquad, setGoalAIGroup)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::{battle_plan_update, chinook_ai_update, ai_group, ai_states}` against upstream's (#215).
//!
//! # Method
//!
//! The functions are cut verbatim (build.rs, drafts_guilogic3), with `Squad::squadFromAIGroup` (Squad.cpp), into
//! `differential/wrap_draft_hosts.cpp`. Each asks things of the world; the wrapper's stand-ins and the test's host both write what they are
//! asked, in order, and the logs (and the final state) are compared. 20,000 random cases each:
//!
//! - `createVisionObject`: a vision object id that is invalid or set, a template name (`none` has no template), a neutral player or none,
//!   a factory that makes the object or fails, and arbitrary bit patterns for the owner's position and bounding radius.
//! - `private___TellPortableStructureToAttackWithMe`: a chinook with or without a contain, with or without a rider, the rider portable
//!   or not, each of the four disabled types on or off, with or without an AI; random victim, shots and command source.
//! - `groupDoCommandButtonUsingWaypoints`: 0 to 12 members, a button, a waypoint or none, a command source.
//! - `setGoalSquad` / `setGoalAIGroup`: programs of 1 to 8 operations over a machine with no goal squad at first (a squad of random ids and cached
//!   objects; a group of random ids or no group; a dump). The group's ids reach the draft through the verified `World`'s `PlayerSquadHost`.
//!
//! Excluded / not observable: `friend_getRider`'s contain being absent is one host answer (`None`) in the draft (the contain and the rider
//! are one query), so the log cannot tell "no contain" from "no rider"; both leave no effect upstream either. `setGoalAIGroup` with a group
//! whose ids are not in the verified World's table is not generated.
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches: straight-line calls. The place to look is the order of the position and range calls in `createVisionObject`.
//!
//! # Results
//!
//! (filled in after the run)

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::CommandSourceType;
use rts_lockstep_engine::drafts::sweep_gui_logic::{ai_group, ai_states, battle_plan_update as bp, chinook_ai_update as ck};
use rts_lockstep_engine::logic::ai::ai_update::AiMachineExtras;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::player::Squad;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_bp_run(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_ck_run(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_ag_run(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_as_run(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn call(f: unsafe extern "C" fn(*const i32, i32, *mut i32, i32) -> i32, inp: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 2048];
    let n = unsafe { f(inp.as_ptr(), inp.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

// ---- BattlePlanUpdate ----

struct BpHost {
    have_player: bool,
    team: u32,
    have_new: bool,
    new_id: i32,
    pos: Coord3D,
    radius: f32,
    log: std::cell::RefCell<Vec<i32>>,
}

impl bp::BattlePlanVisionHost for BpHost {
    fn find_template(&mut self, name: &[u8]) -> Option<u16> {
        self.log.borrow_mut().extend([1, name.len() as i32]);
        self.log.borrow_mut().extend(name.iter().map(|&b| b as i32));
        if name == b"none" {
            None
        } else {
            Some(name.len() as u16 + 1)
        }
    }
    fn neutral_player_default_team(&mut self) -> Option<u32> {
        if self.have_player {
            Some(self.team)
        } else {
            None
        }
    }
    fn new_object(&mut self, template: u16, team: u32) -> Option<i32> {
        self.log.borrow_mut().extend([4, template as i32, team as i32]);
        if self.have_new {
            Some(self.new_id)
        } else {
            None
        }
    }
    fn set_position(&mut self, _object: i32, pos: &Coord3D) {
        self.log.borrow_mut().extend([6, pos.x.to_bits() as i32, pos.y.to_bits() as i32, pos.z.to_bits() as i32]);
    }
    fn set_shroud_clearing_range(&mut self, _object: i32, range: f32) {
        self.log.borrow_mut().extend([8, range.to_bits() as i32]);
    }
    fn position(&self, _object: i32) -> Coord3D {
        self.log.borrow_mut().push(5);
        self.pos
    }
    fn bounding_sphere_radius(&self, _object: i32) -> f32 {
        self.log.borrow_mut().push(7);
        self.radius
    }
}

fn bp_case(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let id0 = if g.below(2) == 0 { 0 } else { 1 + g.below(20) as i32 };
    let name: &[u8] = [b"none".as_slice(), b"VisionObj", b"x", b""][g.below(4) as usize];
    let mut h = BpHost {
        have_player: g.below(5) != 0,
        team: g.below(50),
        have_new: g.below(5) != 0,
        new_id: 1 + g.below(1000) as i32,
        pos: Coord3D { x: f32::from_bits(g.u32()), y: f32::from_bits(g.u32()), z: f32::from_bits(g.u32()) },
        radius: f32::from_bits(g.u32()),
        log: Default::default(),
    };
    let mut inp = vec![id0, name.len() as i32];
    inp.extend(name.iter().map(|&b| b as i32));
    inp.extend([h.have_player as i32, h.team as i32, h.have_new as i32, h.new_id, h.pos.x.to_bits() as i32, h.pos.y.to_bits() as i32, h.pos.z.to_bits() as i32, h.radius.to_bits() as i32]);
    let mut id = id0;
    bp::create_vision_object(&mut h, 99, name, &mut id);
    let mut want = h.log.borrow().clone();
    want.extend([-1, id]);
    (inp, want)
}

// ---- ChinookAIUpdate ----

struct CkHost {
    has_contain: bool,
    has_rider: bool,
    portable: bool,
    disabled: [bool; 4],
    has_ai: bool,
    log: std::cell::RefCell<Vec<i32>>,
}

impl ck::ChinookPortableStructureHost for CkHost {
    fn contain_rider(&self, _chinook: i32) -> Option<i32> {
        if self.has_contain && self.has_rider {
            Some(7)
        } else {
            None
        }
    }
    fn is_portable_structure(&self, rider: i32) -> bool {
        self.log.borrow_mut().extend([11, rider]);
        self.portable
    }
    fn is_disabled_hacked(&self, rider: i32) -> bool {
        self.log.borrow_mut().extend([12, 0, rider]);
        self.disabled[0]
    }
    fn is_disabled_emp(&self, rider: i32) -> bool {
        self.log.borrow_mut().extend([12, 1, rider]);
        self.disabled[1]
    }
    fn is_disabled_subdued(&self, rider: i32) -> bool {
        self.log.borrow_mut().extend([12, 2, rider]);
        self.disabled[2]
    }
    fn is_disabled_paralyzed(&self, rider: i32) -> bool {
        self.log.borrow_mut().extend([12, 3, rider]);
        self.disabled[3]
    }
    fn rider_ai_attack_object(&mut self, rider: i32, victim: i32, max_shots_to_fire: i32, cmd_source: CommandSourceType) -> bool {
        if self.has_ai {
            self.log.borrow_mut().extend([13, rider, victim, max_shots_to_fire, cmd_source as i32]);
        }
        self.has_ai
    }
}

fn ck_case(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let mut h = CkHost { has_contain: g.below(6) != 0, has_rider: g.below(6) != 0, portable: g.below(4) != 0, disabled: [g.below(4) == 0, g.below(4) == 0, g.below(4) == 0, g.below(4) == 0], has_ai: g.below(6) != 0, log: Default::default() };
    let (victim, shots, src) = (1 + g.below(500) as i32, g.below(20) as i32 - 2, g.below(5) as i32);
    let rider_id = 7;
    let mut inp = vec![h.has_contain as i32, h.has_rider as i32, rider_id, h.portable as i32];
    inp.extend(h.disabled.iter().map(|&b| b as i32));
    inp.extend([h.has_ai as i32, victim, shots, src]);
    ck::private_tell_portable_structure_to_attack_with_me(&mut h, 1, victim, shots, CommandSourceType::from_i32(src));
    let want = h.log.borrow().clone();
    (inp, want)
}

// ---- AIGroup ----

struct AgHost {
    log: Vec<i32>,
}

impl ai_group::AiGroupCommandHost for AgHost {
    fn object_do_command_button_using_waypoints(&mut self, member: i32, command_button: usize, waypoint: Option<u32>, cmd_source: CommandSourceType) {
        self.log.extend([20, member, command_button as i32, waypoint.map_or(-1, |w| w as i32), cmd_source as i32]);
    }
}

fn ag_case(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let n = g.below(13) as usize;
    let members: Vec<i32> = (0..n).map(|_| 1 + g.below(300) as i32).collect();
    let (b, w, s) = (g.below(8) as usize, if g.below(3) == 0 { -1 } else { g.below(8) as i32 }, g.below(5) as i32);
    let mut inp = vec![n as i32];
    inp.extend(members.iter().copied());
    inp.extend([b as i32, w, s]);
    let mut h = AgHost { log: Vec::new() };
    ai_group::group_do_command_button_using_waypoints(&mut h, &members, b, if w < 0 { None } else { Some(w as u32) }, CommandSourceType::from_i32(s));
    (inp, h.log)
}

// ---- AIStateMachine goal squad ----

fn as_case(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let mut m = AiMachineExtras::default();
    let mut world = World::default();
    let mut inp = Vec::new();
    let mut want = Vec::new();
    for _ in 0..(1 + g.below(8)) {
        match g.below(4) {
            0 => {
                let ids: Vec<i32> = (0..g.below(6)).map(|_| 1 + g.below(40) as i32).collect();
                let cache: Vec<i32> = (0..g.below(4)).map(|_| g.below(40) as i32).collect();
                inp.push(0);
                inp.push(ids.len() as i32);
                inp.extend(ids.iter().copied());
                inp.push(cache.len() as i32);
                inp.extend(cache.iter().copied());
                ai_states::aism_set_goal_squad(&mut m, &Squad { object_ids: ids, objects_cached: cache });
            }
            1 => {
                let has = g.below(4) != 0;
                let ids: Vec<i32> = (0..g.below(6)).map(|_| 1 + g.below(40) as i32).collect();
                inp.extend([1, has as i32, ids.len() as i32]);
                inp.extend(ids.iter().copied());
                world.cuts.live_ai = false;
                world.cuts.player_host.groups.insert(1, ids);
                ai_states::aism_set_goal_ai_group(&mut m, &world, if has { Some(1) } else { None });
            }
            _ => {
                inp.push(2);
                want.push(m.goal_squad.is_some() as i32);
                if let Some(s) = &m.goal_squad {
                    want.push(s.object_ids.len() as i32);
                    want.extend(s.object_ids.iter().copied());
                    want.push(s.objects_cached.len() as i32);
                    want.extend(s.objects_cached.iter().copied());
                }
            }
        }
    }
    inp.push(2);
    want.push(m.goal_squad.is_some() as i32);
    if let Some(s) = &m.goal_squad {
        want.push(s.object_ids.len() as i32);
        want.extend(s.object_ids.iter().copied());
        want.push(s.objects_cached.len() as i32);
        want.extend(s.objects_cached.iter().copied());
    }
    (inp, want)
}

#[test]
fn hosts() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B0A7);
    let mut cs = vec![
        Check::new("BattlePlanUpdate::createVisionObject"),
        Check::new("ChinookAIUpdate::private___TellPortableStructureToAttackWithMe"),
        Check::new("AIGroup::groupDoCommandButtonUsingWaypoints"),
        Check::new("AIStateMachine::setGoalSquad, setGoalAIGroup"),
    ];
    for i in 0..20_000 {
        let (inp, want) = bp_case(&mut g);
        cs[0].eq(want, call(ref_bp_run, &inp), || format!("case {i}: {inp:?}"));
        let (inp, want) = ck_case(&mut g);
        cs[1].eq(want, call(ref_ck_run, &inp), || format!("case {i}: {inp:?}"));
        let (inp, want) = ag_case(&mut g);
        cs[2].eq(want, call(ref_ag_run, &inp), || format!("case {i}: {inp:?}"));
        let (inp, want) = as_case(&mut g);
        cs[3].eq(want, call(ref_as_run, &inp), || format!("case {i}: {inp:?}"));
    }
    if planted() {
        for c in cs {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    for c in cs {
        c.finish();
    }
}
