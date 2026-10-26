#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (PathfindCellInfo, PathfindCell)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `PathfindCellInfo` and `PathfindCell` (`logic::ai::cell`) against the unchanged C++, `p1w13-sonnet`, layer 10 batch P2.
//!
//! # Method
//!
//! The C++ side is the real `AIPathfind.h` and AIPathfind.cpp's cell code, cut by build.rs (one edit: MSVC's default-int `static warn`),
//! in the pathfinder build (`differential/wrap_pathfind.cpp`, `ref_pf_p2_run`): a grid of real `PathfindCell`s and the real pool of
//! 30,000 infos, allocated for the case; the obstacles an `Object` stand-in whose rubble and see-through kind the case gives. The Rust
//! side is a [`CellStore`] with the same grid.
//!
//! Each case is a grid of 2 to 6 by 2 to 5 cells and a program of 1 to 120 operations: infos allocated and released, unit goals,
//! positions and aircraft goals set and cleared, obstacles (rubble or not, fences, see-through) set and removed, types set, cells reset,
//! searches started, parents set and cleared, cells put on and taken off the open (sorted) and closed lists and the lists released, the
//! three costs, the setters and the getters. The program keeps to the calls upstream defines (it asks the Rust store, before each
//! operation, whether the cells it names have the infos the call dereferences, and the lists the flags it asserts). Compared, bit for
//! bit, after every operation: its results, the heads of the two lists, every cell (type, flags, zone, aircraft goal, pinched, layers,
//! its info's index), the head of the free list, and every info below the highest a cell has held (each field and link).
//!
//! A planted defect can make the Rust store judge a call defined that is not on the C++ state (a cell without the info the call
//! dereferences); the driver then stops that program with a marker, which counts as a mismatch, rather than crash.
//!
//! # Results (2026-10-26, `PF_P2_CASES=20000`)
//!
//! 20,000 cases, 1,210,531 operations (143,667 list operations, 78,016 costs): **0 mismatches**. The first run found one defect in the
//! port: `isObstaclePresent` read the cell's type only, not the info's obstacle id (612 mismatches in 2,000 cases), fixed. Nothing is
//! excluded: the calls upstream leaves undefined (a member on a cell without an info, a list operation on a cell not on that list, a
//! release of an info something still links to) are not run on either side.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 20,000 cases: 11 of 11 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3010 | the open list inserts before an equal cost | 340,107 |
//! | 3011 | an aircraft goal does not keep the info | 186,876 |
//! | 3012 | a unit's goal on its own position not fixed | 53,760 |
//! | 3013 | rubble keeps the info | 39,880 |
//! | 3014 | the type keeps 5 bits | 144,991 |
//! | 3015 | removing an obstacle leaves rubble | 61,340 |
//! | 3016 | the octile estimate's short axis a third | 68,955 |
//! | 3017 | a 90-degree turn costs as a 45 | 2,167 |
//! | 3018 | the hierarchical estimate not rounded | 902 |
//! | 3019 | the zone keeps 15 bits | 255,472 |
//! | 3020 | removal from the closed list keeps the back link | 15,612 |

mod support;

use rts_lockstep_engine::common::base_type::ICoord2D;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::ai::cell::{CellId, CellStore, ObstacleFacts, PathfindCell};
use support::Gen;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const NONE: u32 = u32::MAX;

struct Run {
    store: CellStore,
    w: u32,
    h: u32,
    open: Option<CellId>,
    closed: Option<CellId>,
    high_water: u32,
}

impl Run {
    fn new(w: u32, h: u32) -> Self {
        let mut store = CellStore::new();
        store.allocate_cell_infos();
        store.cells = vec![PathfindCell::default(); (w * h) as usize];
        Run { store, w, h, open: None, closed: None, high_water: 0 }
    }
    fn count(&self) -> u32 {
        self.w * self.h
    }
    fn cell(&self, k: u32) -> CellId {
        k % self.count()
    }
    fn pos(&self, k: u32) -> ICoord2D {
        let k = k % self.count();
        ICoord2D { x: (k / self.h) as i32, y: (k % self.h) as i32 }
    }
    fn info(&self, k: u32) -> bool {
        self.store.cells[self.cell(k) as usize].has_info()
    }

    /// Whether `op` is a call upstream defines on the current state (the program keeps to those).
    fn defined(&self, o: &[u32; 6]) -> bool {
        let (a, b) = (self.cell(o[1]), self.cell(o[2]));
        let ia = self.info(o[1]);
        let flags = |c: CellId| {
            let i = self.store.cells[c as usize].info.map(|i| *self.store.info(i));
            i.map(|i| (i.open, i.closed))
        };
        match o[0] {
            // these can release the info whatever it is linked to (upstream's callers clear the lists first): only an unlinked one
            1..=8 => match self.store.cells[a as usize].info {
                None => true,
                Some(i) => {
                    let f = self.store.info(i);
                    !f.open && !f.closed && f.next_open.is_none() && f.prev_open.is_none() && !self.is_referenced(i)
                }
            },
            // a search starts on a cell in neither list (it marks the cell open without linking it)
            9 => ia && flags(a) == Some((false, false)) && !self.in_list(self.open, a) && (o[3] != 0 || self.info(o[2])),
            10 | 11 => ia && self.info(o[2]),
            12 | 24 => ia,
            13 | 15 => ia && flags(a) == Some((false, false)),
            14 => ia && self.in_list(self.open, a),
            16 => ia && self.in_list(self.closed, a),
            19 => ia && self.info(o[2]),
            20 => self.info(o[2]),
            21 => {
                ia && (o[3] != 0 || (self.info(o[2]) && self.store.get_parent_cell(b).is_none_or(|p| self.store.cells[p as usize].has_info())))
            }
            _ => true,
        }
    }

    /// Whether cell `c` is on the list headed by `head`.
    fn in_list(&self, head: Option<CellId>, c: CellId) -> bool {
        let mut cur = head;
        while let Some(x) = cur {
            if x == c {
                return true;
            }
            cur = self.store.info(self.store.cells[x as usize].info.unwrap()).next_open.and_then(|n| self.store.info(n).cell);
        }
        false
    }

    /// Whether some cell's info, or a list, still points at info `i` (a reset would leave it dangling).
    fn is_referenced(&self, i: u32) -> bool {
        self.store.cells.iter().filter_map(|c| c.info).any(|x| {
            let f = self.store.info(x);
            x != i && (f.path_parent == Some(i) || f.next_open == Some(i) || f.prev_open == Some(i))
        }) || self.open.and_then(|c| self.store.cells[c as usize].info) == Some(i)
            || self.closed.and_then(|c| self.store.cells[c as usize].info) == Some(i)
    }

    fn op(&mut self, o: &[u32; 6], v: &mut Vec<u32>) {
        let (a, b, pa) = (self.cell(o[1]), self.cell(o[2]), self.pos(o[1]));
        let s = &mut self.store;
        let mut res = Vec::new();
        let ci = |c: Option<CellId>| c.unwrap_or(NONE);
        match o[0] {
            0 => res.push(u32::from(s.allocate_info(a, pa))),
            1 => s.release_info(a),
            2 => s.set_goal_unit(a, o[2] as ObjectId, pa),
            3 => s.set_goal_aircraft(a, o[2] as ObjectId, pa),
            4 => s.set_pos_unit(a, o[2] as ObjectId, pa),
            5 => {
                let ob = ObstacleFacts { id: o[2] as ObjectId, rubble: o[3] != 0, see_through: o[4] != 0 };
                res.push(u32::from(s.set_type_as_obstacle(a, &ob, o[5] != 0, pa)));
            }
            6 => s.set_type(a, o[2] as u8),
            7 => res.push(u32::from(s.remove_obstacle(a, o[2] as ObjectId))),
            8 => s.reset(a),
            9 => res.push(u32::from(s.start_pathfind(a, (o[3] == 0).then_some(b)))),
            10 => s.set_parent_cell(a, b),
            11 => s.set_parent_cell_hierarchical(a, b),
            12 => s.clear_parent_cell(a),
            13 => self.open = s.put_on_sorted_open_list(a, self.open),
            14 => self.open = s.remove_from_open_list(a, self.open),
            15 => self.closed = s.put_on_closed_list(a, self.closed),
            16 => self.closed = s.remove_from_closed_list(a, self.closed),
            17 => {
                res.push(s.release_open_list(self.open) as u32);
                self.open = None;
            }
            18 => {
                res.push(s.release_closed_list(self.closed) as u32);
                self.closed = None;
            }
            19 => res.push(s.cost_to_goal(a, b)),
            20 => res.push(s.cost_to_hier_goal(a, b)),
            21 => res.push(s.cost_so_far(a, (o[3] == 0).then_some(b))),
            22 => s.set_cost_so_far(a, o[2]),
            23 => s.set_total_cost(a, o[2]),
            24 => s.set_blocked_by_ally(a, o[2] != 0),
            25 => s.cells[a as usize].set_pinched(o[2] != 0),
            26 => s.cells[a as usize].set_zone(o[2] as u16),
            27 => s.cells[a as usize].set_layer(o[2] as i32),
            28 => s.cells[a as usize].set_connect_layer(o[2] as i32),
            29 => {
                res.extend([s.get_goal_unit(a) as u32, s.get_goal_aircraft(a) as u32, s.get_pos_unit(a) as u32, s.get_obstacle_id(a) as u32]);
                res.extend([u32::from(s.is_obstacle_transparent(a)), u32::from(s.is_obstacle_fence(a)), u32::from(s.is_obstacle_present(a, o[2] as ObjectId))]);
                res.push(ci(s.get_parent_cell(a)));
                if s.cells[a as usize].has_info() {
                    res.extend([ci(s.get_next_open(a)), u32::from(s.x_index(a)), u32::from(s.y_index(a))]);
                }
            }
            _ => unreachable!(),
        }
        v.push(res.len() as u32);
        v.extend(&res);
        v.push(ci(self.open));
        v.push(ci(self.closed));
        let s = &self.store;
        for c in &s.cells {
            v.extend([u32::from(c.get_type()), u32::from(c.get_flags()), u32::from(c.get_zone()), u32::from(c.is_aircraft_goal())]);
            v.extend([u32::from(c.get_pinched()), c.get_layer() as u32, c.get_connect_layer() as u32, c.info.unwrap_or(NONE)]);
            if let Some(i) = c.info {
                self.high_water = self.high_water.max(i + 1);
            }
        }
        v.push(s.first_free().unwrap_or(NONE));
        v.push(self.high_water);
        for k in 0..self.high_water {
            let f = s.info(k);
            v.extend([u32::from(f.is_free), f.next_open.unwrap_or(NONE), f.prev_open.unwrap_or(NONE), f.path_parent.unwrap_or(NONE), f.cell.unwrap_or(NONE)]);
            v.extend([u32::from(f.total_cost), u32::from(f.cost_so_far), f.pos.x as u32, f.pos.y as u32]);
            v.extend([f.goal_unit_id as u32, f.pos_unit_id as u32, f.goal_aircraft_id as u32, f.obstacle_id as u32]);
            v.extend([u32::from(f.blocked_by_ally), u32::from(f.obstacle_is_fence), u32::from(f.obstacle_is_transparent), u32::from(f.open), u32::from(f.closed)]);
        }
    }
}

fn cpp_run(w: u32, h: u32, ops: &[[u32; 6]]) -> Vec<u32> {
    let cfg = [w, h];
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut buf = vec![0u32; 1 << 22];
    let mut len = 0i32;
    let st = unsafe { cpp::ref_pf_p2_run(cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len) };
    assert_eq!(st, 0, "the dump fits");
    buf.truncate(len as usize);
    buf
}

/// One op's part of a dump (`cells` cells).
fn split_ops(b: &[u32], n: usize, cells: usize) -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    let mut p = 0;
    for _ in 0..n {
        if p >= b.len() || b[p] == 0xbadc_0de5 {
            break; // the C++ stopped at a call undefined on its own state (see the driver)
        }
        let start = p;
        p += 1 + b[p] as usize + 2 + 8 * cells + 1;
        let hw = b[p] as usize;
        p += 1 + 18 * hw;
        out.push(b[start..p].to_vec());
    }
    out
}

fn gen_op(g: &mut Gen, count: u32) -> [u32; 6] {
    let c = |g: &mut Gen| g.below(count);
    let id = |g: &mut Gen| if g.below(4) == 0 { 0 } else { 1 + g.below(4) };
    match g.below(100) {
        0..=5 => [0, c(g), 0, 0, 0, 0],
        6..=8 => [1, c(g), 0, 0, 0, 0],
        9..=14 => [2, c(g), id(g), 0, 0, 0],
        15..=17 => [3, c(g), id(g), 0, 0, 0],
        18..=23 => [4, c(g), id(g), 0, 0, 0],
        24..=28 => [5, c(g), 1 + g.below(4), u32::from(g.below(4) == 0), g.below(2), g.below(2)],
        29..=31 => [6, c(g), g.below(8) + if g.below(10) == 0 { 16 } else { 0 }, 0, 0, 0],
        32..=34 => [7, c(g), 1 + g.below(4), 0, 0, 0],
        35..=36 => [8, c(g), 0, 0, 0, 0],
        37..=42 => [9, c(g), c(g), u32::from(g.below(4) == 0), 0, 0],
        43..=47 => [10, c(g), c(g), 0, 0, 0],
        48..=49 => [11, c(g), c(g), 0, 0, 0],
        50..=51 => [12, c(g), 0, 0, 0, 0],
        52..=61 => [13, c(g), 0, 0, 0, 0],
        62..=65 => [14, c(g), 0, 0, 0, 0],
        66..=70 => [15, c(g), 0, 0, 0, 0],
        71..=73 => [16, c(g), 0, 0, 0, 0],
        74 => [17, 0, 0, 0, 0, 0],
        75 => [18, 0, 0, 0, 0, 0],
        76..=78 => [19, c(g), c(g), 0, 0, 0],
        79..=80 => [20, c(g), c(g), 0, 0, 0],
        81..=84 => [21, c(g), c(g), u32::from(g.below(5) == 0), 0, 0],
        85..=87 => [22, c(g), *[0u32, 7, 40, 70000].get(g.below(4) as usize).unwrap(), 0, 0, 0],
        88..=90 => [23, c(g), *[0u32, 9, 30, 65537].get(g.below(4) as usize).unwrap(), 0, 0, 0],
        91 => [24, c(g), g.below(2), 0, 0, 0],
        92 => [25, c(g), g.below(2), 0, 0, 0],
        93 => [26, c(g), *[0u32, 5, 0x3fff, 0x4003, 0xffff].get(g.below(5) as usize).unwrap(), 0, 0, 0],
        94 => [27, c(g), g.below(20), 0, 0, 0],
        95 => [28, c(g), g.below(20), 0, 0, 0],
        _ => [29, c(g), g.below(5), 0, 0, 0],
    }
}

#[test]
fn cells_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("PF_P2_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen::new(0x0010_C311);
    let mut check = support::Check::new("P2 PathfindCell ops");
    let (mut ops_run, mut lists, mut costs) = (0u64, 0u64, 0u64);
    for case in 0..cases {
        let (w, h) = (2 + g.below(5), 2 + g.below(4));
        let n = 1 + g.below(120) as usize;
        let mut run = Run::new(w, h);
        let mut ops = Vec::new();
        let mut r = Vec::new();
        while ops.len() < n {
            let o = gen_op(&mut g, w * h);
            if !run.defined(&o) {
                continue;
            }
            run.op(&o, &mut r);
            ops.push(o);
        }
        let c = cpp_run(w, h, &ops);
        let (rs, cs) = (split_ops(&r, n, (w * h) as usize), split_ops(&c, n, (w * h) as usize));
        for (i, a) in rs.iter().enumerate() {
            let b = cs.get(i).cloned().unwrap_or_else(|| vec![0xbadc_0de5]);
            check.eq(a.clone(), b, || format!("case {case} op {i} {:?} grid {w}x{h} ops {:?}", ops[i], &ops[..=i]));
        }
        ops_run += n as u64;
        lists += ops.iter().filter(|o| (13..=18).contains(&o[0])).count() as u64;
        costs += ops.iter().filter(|o| (19..=21).contains(&o[0])).count() as u64;
    }
    println!("P2: {cases} cases, {ops_run} ops ({lists} list operations, {costs} costs)");
    check.finish();
}
