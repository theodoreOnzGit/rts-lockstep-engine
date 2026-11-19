#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/POWTruckAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/PrisonDockUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::pow_truck_ai_update::{PowTruckAiUpdate, prison_dock_update_action}` against `POWTruckAIUpdate` and
//! `PrisonDockUpdate::action` (#215).
//!
//! # Method
//!
//! The constructor to `crc` of `POWTruckAIUpdate.cpp` and `PrisonDockUpdate::action` are cut verbatim (build.rs) into
//! `differential/wrap_draft_pow_truck.cpp`, compiled as if `ALLOW_SURRENDER` were defined (the shipped game does not define it), on stand-ins
//! for the AI base, the objects, their contain modules, the action manager, the partition manager and the pathfinder (listed there). The
//! draft runs against a [`PowTruckWorld`] that does the same. A case is a scenario of 2 to 6 objects (the first is the truck, always with a
//! contain module; the others with or without contain, AI, a controlling player, KINDOF_COLLECTS_PRISON_BOUNTY; each with scripted answers
//! for "can pick up", "quick path available", "quick path exists", a position on a half-unit grid so that distances tie, a cost, some
//! contained in the truck) and 3 to 30 ops (set the frame near the `u32` wrap, update, `aiDoCommand` of each command from each source with
//! or without an object, `setTask`, `setAIMode`, `loadPrisoner`, `unloadPrisonersToPrison`, `PrisonDockUpdate::action`, toggle idle and
//! "allowed to respond"). Both sides log every call that changes something or whose answer the logic acts on, the result of `action`, and the
//! module's six fields and every object's contain count, container and list length after each op; the logs are compared whole.
//! 20,000 scenarios from a fixed seed.
//!
//! Not covered (the world's, in the draft's `PowTruckWorld`): `TheGlobalData->m_prisonBountyMultiplier * calcCostToBuild` (the test computes the
//! product on both sides), the money deposit and floating text (logged as the same words by both worlds), `canPickUpPrisoner` itself.
//! Excluded inputs: a truck with no contain module (`updateFindTarget`'s no-target branch and `loadPrisoner` call through it unchecked: a NULL
//! dereference upstream); `PrisonDockUpdate::action` with a docker that is not the truck (`getPOWTruckAIUpdateInterface()` is NULL for any
//! other AI and is dereferenced); a negative bounty (float to unsigned is undefined in C). Not covered: `HUGE_DIST`'s value is not observable
//! (the first target is accepted by the `closestTarget == NULL` test before the distance is compared).
//!
//! # Prediction (written before the first run, 2026-11-19)
//!
//! 0 mismatches in the logic, apart from one constant: the draft's `HUGE_DIST` is 1.0e10 and `PartitionManager.h` line 77 says
//! `1000000.0f`; it is not observable, and is corrected in the draft in the same change.
//!
//! # Results
//!
//! 2026-11-19: the first run gave **884 mismatches**, all a bug of the *wrapper*: the stand-in `POWTruckAIUpdate` never held the module data that
//! `getPOWTruckAIUpdateModuleData()` returns (a garbage `m_boredTimeInFrames`); fixed in the wrapper. Second run: **0 mismatches over 20,000
//! scenarios** (3,352 sent a pick-up, 11,364 moved a prisoner, 6,994 cleared a surrender, 3,670 paid a bounty, 11,290 docked), max ULP 0. The
//! prediction held (the one draft change, `HUGE_DIST`, is unobservable). Planted bugs, 14 of 14 caught (of 20,000): 15297 `updateWaiting`'s `>`
//! as `>=` 2,062; 15298 `updateFindTarget`'s `<` as `<=` 368; 15299 `m_lastFindFrame` not recorded 6,767; 15300 a distance tie goes to the later
//! target 24; 15301 `quickDoesPathExist` skipped 3,548; 15302 leaving collecting keeps the target id 6,383; 15303 entering waiting does not record
//! the frame 13,984; 15304 a command from the AI also sets automatic mode 523; 15305 the hang-around distance not squared 25; 15306 the bounty
//! overwritten, not accumulated 1,680 (the first 15306, `bounty + trunc(x)` for `trunc(bounty + x)`, was an equivalent mutant for these
//! magnitudes: 0 caught, replaced); 15307 a bounty of 0 awarded 3,429; 15308 the full-truck check in `loadPrisoner` omitted 3,222; 15309 a player
//! command does not set the waiting task 4,696; 15310 `updateCollectingTarget` ignores the AI mode 203.

mod support;

use rts_lockstep_engine::drafts::logic_common::pow_truck_ai_update::{
    prison_dock_update_action, CommandSource, PowTruckAiMode, PowTruckAiUpdate, PowTruckAiUpdateModuleData, PowTruckCommand, PowTruckTask, PowTruckWorld,
};
use std::cell::RefCell;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_pow_run(input: *const u32, ops: *const i32, nops: i32, out: *mut u32, cap: i32) -> i32;
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

struct Obj {
    can_pick: bool,
    q_avail: bool,
    q_exists: bool,
    x: f32,
    y: f32,
    producer: i32,
    has_player: bool,
    collects: bool,
    contain: Option<(i32, i32)>,
    list: Vec<i32>,
    has_ai: bool,
    cost: i32,
    height: f32,
    contained_by: i32,
}

struct W {
    objs: Vec<Obj>,
    frame: u32,
    mult: f32,
    idle: bool,
    allowed: bool,
    log: RefCell<Vec<u32>>,
}

impl W {
    fn put(&self, words: &[u32]) {
        self.log.borrow_mut().extend_from_slice(words);
    }
    fn obj(&self, id: i32) -> &Obj {
        &self.objs[(id - 1) as usize]
    }
}

impl PowTruckWorld for W {
    fn get_frame(&self) -> u32 {
        self.frame
    }
    fn set_ultra_accurate(&mut self, _truck: i32) {
        self.put(&[1]);
    }
    fn ai_update_base(&mut self, _truck: i32) {
        self.put(&[2]);
    }
    fn is_allowed_to_respond_to_ai_commands(&mut self, _truck: i32) -> bool {
        self.put(&[3, self.allowed as u32]);
        self.allowed
    }
    fn ai_idle(&mut self, _truck: i32) {
        self.put(&[4]);
    }
    fn ai_do_command_base(&mut self, _truck: i32) {
        self.put(&[5]);
    }
    fn can_pick_up_prisoner(&mut self, _truck: i32, target: i32) -> bool {
        if target < 1 || target as usize > self.objs.len() {
            self.put(&[6, 0, 0]);
            return false;
        }
        let r = self.obj(target).can_pick;
        self.put(&[6, target as u32, r as u32]);
        r
    }
    fn ignore_obstacle(&mut self, _truck: i32, obstacle: i32) {
        self.put(&[7, obstacle as u32]);
    }
    fn ai_move_to_object(&mut self, _truck: i32, target: i32) {
        self.put(&[8, target as u32]);
    }
    fn ai_dock(&mut self, _truck: i32, prison: i32, source: CommandSource) {
        self.put(&[9, prison as u32, (source == CommandSource::FromAi) as u32]);
    }
    fn is_idle(&self, _truck: i32) -> bool {
        self.put(&[10, self.idle as u32]);
        self.idle
    }
    fn ai_pick_up_prisoner(&mut self, _truck: i32, target: i32) {
        self.put(&[11, target as u32]);
    }
    fn ai_return_prisoners(&mut self, _truck: i32, prison: i32) {
        self.put(&[12, prison as u32]);
    }
    fn find_object_by_id(&self, id: i32) -> Option<i32> {
        if id >= 1 && id as usize <= self.objs.len() { Some(id) } else { None }
    }
    fn contain_count_and_max(&self, obj: i32) -> Option<(i32, i32)> {
        self.obj(obj).contain
    }
    fn add_to_contain(&mut self, container: i32, obj: i32) {
        // `destContain` is NULL for a prison without a contain module: the call is not made
        if self.obj(container).contain.is_none() {
            return;
        }
        self.put(&[15, container as u32, obj as u32]);
        let c = &mut self.objs[(container - 1) as usize];
        c.list.push(obj);
        if let Some(cm) = c.contain.as_mut() {
            cm.0 += 1;
        }
        self.objs[(obj - 1) as usize].contained_by = container;
    }
    fn remove_from_contain(&mut self, container: i32, obj: i32) {
        if self.obj(container).contain.is_none() {
            return;
        }
        self.put(&[16, container as u32, obj as u32]);
        let c = &mut self.objs[(container - 1) as usize];
        if let Some(pos) = c.list.iter().position(|&o| o == obj) {
            c.list.remove(pos);
        }
        if let Some(cm) = c.contain.as_mut() {
            cm.0 -= 1;
        }
        self.objs[(obj - 1) as usize].contained_by = 0;
    }
    fn clear_surrendered(&mut self, obj: i32) {
        if self.obj(obj).has_ai {
            self.put(&[17, obj as u32]);
        }
    }
    fn get_producer_id(&self, obj: i32) -> i32 {
        self.obj(obj).producer
    }
    fn has_controlling_player(&self, obj: i32) -> bool {
        self.obj(obj).has_player
    }
    fn get_distance_squared_center_2d(&self, a: i32, b: i32) -> f32 {
        let (a, b) = (self.obj(a), self.obj(b));
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        dx * dx + dy * dy
    }
    fn is_quick_path_available(&mut self, _truck: i32, other: i32) -> bool {
        let r = self.obj(other).q_avail;
        self.put(&[18, other as u32, r as u32]);
        r
    }
    fn quick_does_path_exist(&mut self, _truck: i32, other: i32) -> bool {
        let r = self.obj(other).q_exists;
        self.put(&[19, other as u32, r as u32]);
        r
    }
    fn all_objects(&self) -> Vec<i32> {
        (1..=self.objs.len() as i32).collect()
    }
    fn contained_by(&self, obj: i32) -> Option<i32> {
        let c = self.obj(obj).contained_by;
        if c == 0 { None } else { Some(c) }
    }
    fn iterate_contained(&self, truck: i32) -> Vec<i32> {
        self.obj(truck).list.clone()
    }
    fn collects_prison_bounty(&self, prison: i32) -> bool {
        self.obj(prison).collects
    }
    fn prisoner_bounty(&mut self, prisoner: i32) -> f32 {
        self.mult * self.obj(prisoner).cost as f32
    }
    fn award_bounty(&mut self, _truck: i32, prison: i32, bounty: u32) {
        let o = self.obj(prison);
        let z = prison as f32 + o.height;
        let (x, y) = (o.x, o.y);
        self.put(&[20, bounty, 21, bounty, 22, x.to_bits(), y.to_bits(), z.to_bits()]);
    }
}

fn state(log: &mut Vec<u32>, ai: &PowTruckAiUpdate, w: &W) {
    log.extend([ai.ai_mode as u32, ai.current_task as u32, ai.target_id as u32, ai.prison_id as u32, ai.entered_waiting_frame, ai.last_find_frame]);
    for o in &w.objs {
        log.extend([o.contain.map(|c| 1 + c.0 as u32).unwrap_or(0), o.contained_by as u32, o.list.len() as u32]);
    }
}

fn task(i: i32) -> PowTruckTask {
    match i {
        0 => PowTruckTask::Waiting,
        1 => PowTruckTask::FindTarget,
        2 => PowTruckTask::CollectingTarget,
        _ => PowTruckTask::ReturningPrisoners,
    }
}

fn run_rust(input: &[u32], ops: &[i32]) -> Vec<u32> {
    let n = input[0] as usize;
    let mut w = W { objs: Vec::new(), frame: input[1], mult: f32::from_bits(input[4]), idle: input[6] != 0, allowed: input[5] != 0, log: RefCell::new(Vec::new()) };
    let mut f = &input[7..];
    for i in 0..n {
        let has_contain = f[8] != 0 || i == 0;
        w.objs.push(Obj {
            can_pick: f[0] != 0,
            q_avail: f[1] != 0,
            q_exists: f[2] != 0,
            x: f32::from_bits(f[3]),
            y: f32::from_bits(f[4]),
            producer: f[5] as i32,
            has_player: f[6] != 0,
            collects: f[7] != 0,
            contain: if has_contain { Some((f[9] as i32, f[10] as i32)) } else { None },
            list: Vec::new(),
            has_ai: f[11] != 0 || i == 0,
            cost: f[12] as i32,
            height: f32::from_bits(f[13]),
            contained_by: 0,
        });
        f = &f[15..];
    }
    let mut f = &input[7..];
    for i in 0..n {
        if i >= 1 && f[14] != 0 {
            w.objs[i].contained_by = 1;
            w.objs[0].list.push(i as i32 + 1);
        }
        f = &f[15..];
    }
    let mut ai = PowTruckAiUpdate::new(PowTruckAiUpdateModuleData { bored_time_in_frames: input[2], hang_around_prison_distance: f32::from_bits(input[3]) });
    let mut log: Vec<u32> = Vec::new();
    state(&mut log, &ai, &w);
    let opt = |w: &W, o: i32| if o == 0 { None } else { w.find_object_by_id(o) };
    let mut p = 0;
    while p < ops.len() {
        let op = ops[p];
        p += 1;
        log.push(1000 + op as u32);
        let mut result: Option<u32> = None;
        match op {
            0 => {
                w.frame = ops[p] as u32;
                p += 1;
            }
            1 => ai.update(&mut w, 1),
            2 => {
                let cmd = match ops[p] {
                    0 => PowTruckCommand::PickUpPrisoner,
                    1 => PowTruckCommand::ReturnPrisoners,
                    _ => PowTruckCommand::Other,
                };
                let src = if ops[p + 1] == 0 { CommandSource::FromPlayer } else { CommandSource::FromAi };
                let obj = opt(&w, ops[p + 2]);
                p += 3;
                ai.ai_do_command(&mut w, 1, cmd, src, obj);
            }
            3 => {
                let (t, o) = (ops[p], ops[p + 1]);
                p += 2;
                let obj = opt(&w, o);
                ai.set_task(&mut w, task(t), obj);
            }
            4 => {
                ai.set_ai_mode(if ops[p] == 0 { PowTruckAiMode::Automatic } else { PowTruckAiMode::Manual });
                p += 1;
            }
            5 => {
                let obj = opt(&w, ops[p]);
                p += 1;
                ai.load_prisoner(&mut w, 1, obj);
            }
            6 => {
                let obj = opt(&w, ops[p]);
                p += 1;
                ai.unload_prisoners_to_prison(&mut w, 1, obj);
            }
            7 => {
                let (d, pr) = (ops[p], ops[p + 1]);
                p += 2;
                let docker = opt(&w, d);
                result = Some(prison_dock_update_action(&mut w, &mut ai, pr, docker, None) as u32);
            }
            8 => {
                w.idle = ops[p] != 0;
                p += 1;
            }
            _ => {
                w.allowed = ops[p] != 0;
                p += 1;
            }
        }
        log.append(&mut w.log.borrow_mut());
        if let Some(r) = result {
            log.push(r);
        }
        state(&mut log, &ai, &w);
    }
    log
}

fn half(g: &mut Gen) -> f32 {
    g.below(21) as f32 * 0.5
}

fn frame(g: &mut Gen) -> u32 {
    match g.below(6) {
        0 => 0,
        1 => u32::MAX - g.below(70),
        2 => g.below(40),
        3 => 30 + g.below(5),
        _ => g.below(400),
    }
}

#[test]
fn pow_truck_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_90B7);
    let mut c = Check::new("POWTruckAIUpdate");
    let (mut found, mut unloaded, mut loaded, mut awarded, mut docked) = (0u32, 0u32, 0u32, 0u32, 0u32);
    for case in 0..20_000 {
        let n = 2 + g.below(5) as usize;
        let mults = [0.0f32, 0.5, 1.0, 1.5, 2.25, 10.0];
        let mut input: Vec<u32> = vec![
            n as u32,
            frame(&mut g),
            [0, 1, 5, 29, 30, 31, 100][g.below(7) as usize],
            [0.0f32, 1.0, 2.5, 5.0][g.below(4) as usize].to_bits(),
            mults[g.below(6) as usize].to_bits(),
            (g.below(5) != 0) as u32,
            (g.below(2) == 0) as u32,
        ];
        for i in 0..n {
            input.extend([
                (g.below(3) != 0) as u32,
                (g.below(4) != 0) as u32,
                (g.below(4) != 0) as u32,
                half(&mut g).to_bits(),
                half(&mut g).to_bits(),
                if g.below(4) == 0 { 0 } else if g.below(8) == 0 { n as u32 + 1 } else { 1 + g.below(n as u32) },
                (g.below(5) != 0) as u32,
                (g.below(2) == 0) as u32,
                (g.below(3) != 0) as u32,
                g.below(4),
                g.below(4),
                (g.below(4) != 0) as u32,
                g.below(500),
                [0.0f32, 1.5, 4.0][g.below(3) as usize].to_bits(),
                (i >= 1 && g.below(3) == 0) as u32,
            ]);
        }
        let mut ops: Vec<i32> = Vec::new();
        let any = |g: &mut Gen| if g.below(6) == 0 { 0 } else if g.below(10) == 0 { n as i32 + 1 } else { 1 + g.below(n as u32) as i32 };
        for _ in 0..3 + g.below(28) {
            match g.below(20) {
                0..=5 => ops.push(1),
                6 | 7 => ops.extend([0, frame(&mut g) as i32]),
                8 | 9 => ops.extend([2, g.below(3) as i32, g.below(2) as i32, any(&mut g)]),
                10 | 11 => ops.extend([3, g.below(4) as i32, any(&mut g)]),
                12 => ops.extend([4, g.below(2) as i32]),
                13 | 14 => ops.extend([5, any(&mut g)]),
                15 => ops.extend([6, any(&mut g)]),
                16 => ops.extend([7, if g.below(5) == 0 { 0 } else { 1 }, 1 + g.below(n as u32) as i32]),
                17 | 18 => ops.extend([8, g.below(2) as i32]),
                _ => ops.extend([9, g.below(2) as i32]),
            }
        }
        let mut out = vec![0u32; 200_000];
        let len = unsafe { ref_pow_run(input.as_ptr(), ops.as_ptr(), ops.len() as i32, out.as_mut_ptr(), out.len() as i32) };
        assert!(len >= 0, "the log did not fit");
        out.truncate(len as usize);
        let rust = run_rust(&input, &ops);
        found += out.windows(2).any(|w| w[0] == 11) as u32;
        unloaded += out.windows(2).any(|w| w[0] == 15) as u32;
        loaded += out.windows(2).any(|w| w[0] == 17) as u32;
        awarded += out.windows(2).any(|w| w[0] == 20) as u32;
        docked += (ops.windows(1).any(|w| w[0] == 7)) as u32;
        let first = rust.iter().zip(out.iter()).position(|(a, b)| a != b).unwrap_or(rust.len().min(out.len()));
        c.eq(rust == out, true, || format!("case {case}: input {input:?} ops {ops:?}; first difference at word {first}: rust {:?} c++ {:?}", rust.get(first.saturating_sub(8)..(first + 4).min(rust.len())), out.get(first.saturating_sub(8)..(first + 4).min(out.len()))));
    }
    println!("(pow_truck: {found} scenarios sent a pick-up, {unloaded} moved a prisoner, {loaded} cleared a surrender, {awarded} paid a bounty, {docked} docked)");
    finish(c);
}
