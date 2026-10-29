// SPDX-License-Identifier: GPL-3.0-only
//! Differential test of the AI's collisions (`logic::ai::ai_collision`, Phase 1 layer 10, `p1w13-sonnet`, batch A22) against
//! AIUpdate.cpp's `hasHigherPathPriority`, `calculateMaxBlockedSpeed`, `blockedBy`, `needToRotate` and `processCollision`, cut
//! verbatim and compiled over a two-party scene (`differential/wrap_ai_collide.cpp`).
//!
//! Each case is one scene ([`ColScene`]: the two objects' positions, directions, kinds, formations, physics, crush answers and
//! angles; the two AIs' members and query answers) and one function asked of party 0 about party 1. Compared: the answer, each
//! party's `m_isBlocked`, `m_curMaxBlockedSpeed`, `m_blockedFrames` and `m_isBlockedAndStuck` after it, and the log of the
//! commands, the pathfinder's adjustments and the path queries. The scenes favour the branches: the two objects a cell or less apart
//! (sometimes on one spot), directions often parallel, angles about the 45- and 90-degree limits.
//!
//! # Results
//!
//! (filled in by the batch's docs commit)
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::kind_of::{KindOfType, KINDOF_DOZER, KINDOF_INFANTRY, KINDOF_VEHICLE};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_IS_USING_ABILITY};
use rts_lockstep_engine::common::state_machine::StateId;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::differential::{ColParty, ColScene};
use rts_lockstep_engine::logic::ai::ai_collision::{self as col, CollisionHost, Party};
use rts_lockstep_engine::logic::ai::ai_update::LocoGoalType;
use rts_lockstep_engine::logic::world::object_damage::CrushSquishTestType;
use std::os::raw::c_int;
use support::{Check, Gen};

const CASES: u64 = 200_000;

struct Host {
    s: ColScene,
    log: Vec<u32>,
}

impl Host {
    fn c(&self, p: Party) -> &ColParty {
        &self.s.p[idx(p)]
    }
    fn cm(&mut self, p: Party) -> &mut ColParty {
        &mut self.s.p[idx(p)]
    }
}

fn idx(p: Party) -> usize {
    match p {
        Party::Me => 0,
        Party::Other => 1,
    }
}

fn v3(a: [f32; 3]) -> Coord3D {
    Coord3D { x: a[0], y: a[1], z: a[2] }
}

impl CollisionHost for Host {
    fn object_id(&mut self, p: Party) -> ObjectId {
        self.c(p).id
    }
    fn position(&mut self, p: Party) -> Coord3D {
        v3(self.c(p).pos)
    }
    fn unit_direction_2d(&mut self, p: Party) -> Coord3D {
        v3(self.c(p).dir)
    }
    fn is_kind_of(&mut self, p: Party, k: KindOfType) -> bool {
        let bit = if k == KINDOF_INFANTRY {
            0
        } else if k == KINDOF_VEHICLE {
            1
        } else if k == KINDOF_DOZER {
            2
        } else {
            unreachable!("a kind the collisions do not ask")
        };
        (self.c(p).kinds >> bit) & 1 != 0
    }
    fn test_status(&mut self, p: Party, s: ObjectStatusTypes) -> bool {
        assert_eq!(s, OBJECT_STATUS_IS_USING_ABILITY);
        self.c(p).using_ability != 0
    }
    fn formation_id(&mut self, p: Party) -> u32 {
        self.c(p).formation as u32
    }
    fn physics_velocity(&mut self, p: Party) -> Option<Coord3D> {
        (self.c(p).has_physics != 0).then(|| v3(self.c(p).vel))
    }
    fn can_crush_or_squish(&mut self, p: Party, _victim: Party, test: CrushSquishTestType) -> bool {
        let t = match test {
            CrushSquishTestType::CrushOnly => 0,
            CrushSquishTestType::SquishOnly => 1,
            CrushSquishTestType::CrushOrSquish => 2,
        };
        self.c(p).crush[t] != 0
    }
    fn relative_angle_2d(&mut self, p: Party, pos: &Coord3D) -> f32 {
        let c = self.c(p);
        if pos.x == c.path_pos[0] && pos.y == c.path_pos[1] && pos.z == c.path_pos[2] {
            c.angle_to_path
        } else {
            c.angle_to_other
        }
    }
    fn frame(&mut self) -> u32 {
        self.s.frame
    }
    fn has_ai(&mut self, p: Party) -> bool {
        self.c(p).has_ai != 0
    }
    fn ignore_collisions_until(&mut self, p: Party) -> u32 {
        self.c(p).ignore_until
    }
    fn can_path_through_units(&mut self, p: Party) -> bool {
        self.c(p).can_path_through != 0
    }
    fn locomotor_goal_type(&mut self, p: Party) -> LocoGoalType {
        match self.c(p).loco_goal_type {
            0 => LocoGoalType::None,
            1 => LocoGoalType::PositionOnPath,
            2 => LocoGoalType::PositionExplicit,
            _ => LocoGoalType::Angle,
        }
    }
    fn blocked_frames(&mut self, p: Party) -> i32 {
        self.c(p).blocked_frames
    }
    fn set_blocked_frames(&mut self, p: Party, v: i32) {
        self.cm(p).blocked_frames = v;
    }
    fn cur_max_blocked_speed(&mut self, p: Party) -> f32 {
        self.c(p).cur_max_blocked_speed
    }
    fn set_cur_max_blocked_speed(&mut self, p: Party, v: f32) {
        self.cm(p).cur_max_blocked_speed = v;
    }
    fn set_is_blocked(&mut self, p: Party, v: bool) {
        self.cm(p).is_blocked = i32::from(v);
    }
    fn set_is_blocked_and_stuck(&mut self, p: Party, v: bool) {
        self.cm(p).is_blocked_and_stuck = i32::from(v);
    }
    fn machine_goal_position(&mut self, p: Party) -> Coord3D {
        v3(self.c(p).goal_pos)
    }
    fn pathfind_goal_cell(&mut self, p: Party) -> (i32, i32) {
        (self.c(p).goal_cell[0], self.c(p).goal_cell[1])
    }
    fn is_moving(&mut self, p: Party) -> bool {
        self.c(p).moving != 0
    }
    fn is_doing_ground_movement(&mut self, p: Party) -> bool {
        self.c(p).ground != 0
    }
    fn is_waiting_for_path(&mut self, p: Party) -> bool {
        self.c(p).waiting != 0
    }
    fn is_moving_away_from(&mut self, p: Party, _from: Party) -> bool {
        self.c(p).moving_away != 0
    }
    fn is_busy(&mut self, p: Party) -> bool {
        self.c(p).busy != 0
    }
    fn is_idle(&mut self, p: Party) -> bool {
        self.c(p).idle != 0
    }
    fn is_ai_in_dead_state(&mut self, p: Party) -> bool {
        self.c(p).dead != 0
    }
    fn current_state_id(&mut self, p: Party) -> StateId {
        self.c(p).state_id as StateId
    }
    fn cur_locomotor_moving_backwards(&mut self, p: Party) -> Option<bool> {
        (self.c(p).has_loco != 0).then(|| self.c(p).backwards != 0)
    }
    fn cur_locomotor_wander_width_factor(&mut self, p: Party) -> Option<f32> {
        (self.c(p).has_loco != 0).then(|| self.c(p).wander)
    }
    fn compute_point_on_path(&mut self, p: Party) -> Option<Coord3D> {
        if self.c(p).has_path == 0 {
            return None;
        }
        self.log.extend([3, idx(p) as u32]);
        Some(v3(self.c(p).path_pos))
    }
    fn ai_move_away_from_unit(&mut self, p: Party, from: Party) {
        let id = self.c(from).id as u32;
        self.log.extend([0, idx(p) as u32, id, 2]);
    }
    fn adjust_to_possible_destination(&mut self, p: Party, pos: &mut Coord3D) {
        self.log.extend([2, idx(p) as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        let c = self.c(p);
        pos.x += c.adjust[0];
        pos.y += c.adjust[1];
    }
    fn ai_move_to_position(&mut self, p: Party, pos: &Coord3D) {
        self.log.extend([1, idx(p) as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), 2]);
    }
}

#[allow(clippy::field_reassign_with_default)]
fn gen_party(g: &mut Gen, i: usize, near: [f32; 3]) -> ColParty {
    let mut c = ColParty::default();
    c.id = if g.below(8) == 0 { 7 } else { 1 + g.below(30) as i32 + 40 * i as i32 };
    c.pos = if i == 0 {
        [g.uniform(0.0, 200.0), g.uniform(0.0, 200.0), 0.0]
    } else if g.below(10) == 0 {
        near
    } else if g.below(10) == 0 {
        [near[0] + g.uniform(-0.05, 0.05), near[1] + g.uniform(-0.05, 0.05), 0.0]
    } else {
        [near[0] + g.uniform(-12.0, 12.0), near[1] + g.uniform(-12.0, 12.0), 0.0]
    };
    let a = if g.below(4) == 0 { [0.0f32, 0.5, 1.0, 1.5, std::f32::consts::PI][g.below(5) as usize] } else { g.uniform(-3.2, 3.2) };
    c.dir = if g.below(15) == 0 { [0.0, 0.0, 0.0] } else { [a.cos(), a.sin(), 0.0] };
    c.kinds = g.below(8) as i32;
    c.using_ability = i32::from(g.below(6) == 0);
    c.formation = if g.below(3) == 0 { g.below(3) as i32 } else { 0 };
    c.has_physics = i32::from(g.below(5) != 0);
    c.vel = [g.uniform(-3.0, 3.0), g.uniform(-3.0, 3.0), g.uniform(-1.0, 1.0)];
    c.crush = [i32::from(g.below(4) == 0), i32::from(g.below(4) == 0), i32::from(g.below(5) == 0)];
    let angle = |g: &mut Gen| {
        if g.below(3) == 0 {
            let lim = [std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_4 * 0.75, std::f32::consts::FRAC_PI_2, std::f32::consts::PI / 30.0][g.below(4) as usize];
            lim * if g.below(2) == 0 { 1.0 } else { -1.0 } + g.uniform(-0.01, 0.01)
        } else {
            g.uniform(-3.2, 3.2)
        }
    };
    c.angle_to_other = angle(g);
    c.angle_to_path = angle(g);
    c.has_ai = i32::from(i == 0 || g.below(10) != 0);
    c.ignore_until = if g.below(5) == 0 { 100 + g.below(20) } else { 0 };
    c.can_path_through = i32::from(g.below(10) == 0);
    c.is_blocked = i32::from(g.below(3) == 0);
    c.cur_max_blocked_speed = if g.below(3) == 0 { 1.0e9 } else { g.uniform(0.0, 4.0) };
    c.blocked_frames = [0, 1, 5, 30, 31, 60][g.below(6) as usize];
    c.is_blocked_and_stuck = i32::from(g.below(4) == 0);
    c.loco_goal_type = g.below(4) as i32;
    c.goal_pos = if g.below(3) == 0 {
        [c.pos[0] + g.uniform(-12.0, 12.0), c.pos[1] + g.uniform(-12.0, 12.0), 0.0]
    } else {
        [g.uniform(0.0, 200.0), g.uniform(0.0, 200.0), 0.0]
    };
    c.goal_cell = [g.below(5) as i32 - 1, g.below(5) as i32 - 1];
    c.moving = i32::from(g.below(4) != 0);
    c.ground = i32::from(g.below(8) != 0);
    c.waiting = i32::from(g.below(5) == 0);
    c.moving_away = i32::from(g.below(3) == 0);
    c.busy = i32::from(g.below(4) == 0);
    c.idle = i32::from(g.below(3) == 0);
    c.dead = i32::from(g.below(6) == 0);
    c.state_id = [0, 1, 19, 41, 9][g.below(5) as usize];
    c.has_loco = i32::from(g.below(5) != 0);
    c.backwards = i32::from(g.below(6) == 0);
    c.wander = if g.below(4) == 0 { g.uniform(0.0, 1.0) } else { 0.0 };
    c.has_path = i32::from(g.below(4) != 0);
    c.path_pos = [c.pos[0] + g.uniform(-30.0, 30.0), c.pos[1] + g.uniform(-30.0, 30.0), g.uniform(0.0, 2.0)];
    c.adjust = [g.uniform(-5.0, 5.0), g.uniform(-5.0, 5.0)];
    c
}

fn rust_run(op: i32, s: &ColScene, bug: u32) -> (Vec<u32>, Vec<u32>) {
    let mut h = Host { s: *s, log: Vec::new() };
    let mut r = vec![match op {
        0 => u32::from(col::process_collision(&mut h)),
        1 => u32::from(col::blocked_by(&mut h, Party::Me)),
        2 => u32::from(col::need_to_rotate(&mut h, Party::Me)),
        3 => u32::from(col::has_higher_path_priority(&mut h, Party::Me)),
        _ => col::calculate_max_blocked_speed(&mut h, Party::Me).to_bits(),
    }];
    if bug == 1 {
        r[0] ^= 1;
    }
    for c in &h.s.p {
        r.extend([c.is_blocked as u32, c.cur_max_blocked_speed.to_bits(), c.blocked_frames as u32, c.is_blocked_and_stuck as u32]);
    }
    (r, h.log)
}

#[test]
fn ai_collisions_match_upstream() {
    let cases: u64 = std::env::var("AI_COL_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(CASES);
    let mut results = Check::new("AI collision results (answer, the parties' blocked members)");
    let mut logs = Check::new("AI collision calls (commands, adjustments, path points)");
    let mut planted = Check::new("PLANTED AI collision results");
    let mut by_op = [[0u64; 2]; 5];
    for case in 0..cases {
        let mut g = Gen::new(0xA22_0000 + case);
        let mut s = ColScene::default();
        s.p[0] = gen_party(&mut g, 0, [0.0; 3]);
        let near = s.p[0].pos;
        s.p[1] = gen_party(&mut g, 1, near);
        s.frame = 100 + g.below(20);
        let op = [0, 0, 0, 0, 1, 1, 2, 3, 4][g.below(9) as usize];
        // (blockedBy asks the other's AI before checking for one: undefined without, which processCollision never asks)
        if op == 1 {
            s.p[1].has_ai = 1;
        }
        let (r, rl) = rust_run(op, &s, 0);
        let mut out = vec![0u32; 64];
        let mut log = vec![0u32; 256];
        let mut log_len: c_int = 0;
        let n = unsafe { cpp::ref_ai_collide(op, &s, out.as_mut_ptr(), log.as_mut_ptr(), log.len() as c_int, &mut log_len) };
        out.truncate(n as usize);
        assert!(log_len as usize <= log.len(), "log too long");
        log.truncate(log_len as usize);
        let tag = || format!("case {case} op {op} scene {s:?}");
        results.eq(r.clone(), out.clone(), tag);
        logs.eq(rl.clone(), log.clone(), tag);
        planted.eq(rust_run(op, &s, 1).0, out.clone(), tag);
        by_op[op as usize][usize::from(out[0] != 0)] += 1;
        if !rl.is_empty() && std::env::var("AI_COL_DEBUG").is_ok() && rl != log {
            println!("case {case}: rust log {rl:?} cpp {log:?}");
        }
    }
    println!("COVERAGE: {cases} cases; per op (processCollision, blockedBy, needToRotate, hasHigherPathPriority, calculateMaxBlockedSpeed) answers zero/non-zero {by_op:?}");
    results.finish();
    logs.finish();
    planted.finish_planted();
}
