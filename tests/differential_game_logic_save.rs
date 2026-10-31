#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `GameLogic::xfer` (`logic::world::game_logic_xfer`) against the C++, layer 12 batch L2.
//!
//! # Method
//!
//! The C++ side is `GameLogic.cpp` from `findTOCEntryByName` to `loadPostProcess` (the table of contents,
//! `prepareLogicForObjectLoad`, `xfer`), cut verbatim by build.rs into batch L1's build
//! (`differential/wrap_game_logic_frame.cpp`), with the Xfer build's real `XferSave` and `XferLoad` on a file, and
//! logging stand-ins: objects made by a factory that prepends them to the list as `registerObject` does (templates
//! `Tank`, `Bridge` (a bridge), `WallWalker` (walks on walls), `Tower`), each a snapshot of an `Int`; bridges found by
//! an object's position; `destroyObject` and the destroy list; the neutral player; the pathfinder; the campaign
//! manager, the cave system, the polygon triggers and the build assistant's sell list as snapshots of an
//! `UnsignedInt`; the control bar's three buttons. The override tables are the stand-in `GameLogic`'s ordered maps
//! (upstream's are `hash_map`s: see the module's note on their order).
//!
//! A program makes objects (some on bridges, whose towers and bridge object it names), changes them, sets the members
//! the block saves and the overrides, configures the neutral player and the polygon triggers, saves, empties the
//! override tables (a load requires it) and loads the bytes (as saved, with a byte replaced, or cut), and destroys
//! objects; after each save or load it dumps the logic's members, the objects, the table of contents and the tables.
//!
//! An operation the Rust side reports as undefined ends the Rust run there; the C++ runs only the operations before
//! it (a load finding no bridge where a bridge object stands, or no neutral player; an invalid `Bool` loaded).
//!
//! # Results (2026-10-31, `GLSAVE_CASES=20000`, the default)
//!
//! 20,000 programs, 285,976 operations, 1,238,145 log lines (saves byte for byte; loads as saved, damaged and cut):
//! **0 mismatches**. 2,114 programs cut at an undefined operation: 2,010 at a null dereference (a bridge object with
//! no bridge where it stands, or no neutral player), 104 at an invalid `Bool`. Paths reached (log lines): 35,480
//! saves, 11,645 loads that complete, 3,111 loads throwing `SC_INVALID_DATA`, 22,801 loads running out of bytes,
//! 22,732 objects destroyed (bridges, their towers and wall walkers before a load, and by the program), 11,427 wall
//! pieces added, 13,725 pathfinder maps rebuilt.
//!
//! The first run found two defects: the test host dropped the bytes a short read had already copied into an object's
//! value, and **the port did the same for the frame and the rank limit** (upstream xfers into the members, so
//! `fread`'s partial read lands there); both now store the value before the error goes on.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 5 of 5 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 4011 | the table of contents keeps a template once per object | from program 0 on, until the diverged C++ reaches a step the Rust side no longer cuts (the run dies) |
//! | 4012 | a bridge's own object not destroyed before a load | 279 programs |
//! | 4013 | a loaded wall walker not added to the pathfinder | 4,168 programs |
//! | 4014 | a load into a buildable table that is not empty accepted | 435 programs |
//! | 4015 | the superweapon restriction not xferred | from program 0 on, as 4011 |

use rts_lockstep_engine::common::game_state::SaveError;
use rts_lockstep_engine::common::xfer::{take_invalid_bool_read, Xfer};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::game_logic_update::{set_buildable_status_override, set_control_bar_override, GameLogicState};
use rts_lockstep_engine::logic::world::game_logic_xfer::*;
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
type Cfg = [u32; 4];

const TNAMES: [&[u8]; 4] = [b"Tank", b"Bridge", b"WallWalker", b"Tower"];
const BAR_NAMES: [&[u8]; 6] = [b"Alpha", b"beta", b"", b"Gamma_Set", b"alpha", b"x"];
const BUTTONS: [&[u8]; 4] = [b"", b"Command_A", b"Command_B", b"Command_C"];

#[derive(Clone)]
struct Obj {
    id: u32,
    template: usize,
    value: i32,
    kind: u32,
    destroyed: bool,
    bridge: i32,
}

struct Host {
    frame: u32,
    objects: Vec<Obj>,
    next_id: u32,
    has_neutral: bool,
    /// The polygon triggers' ids, in order; their values by slot (the stand-in's six triggers keep theirs).
    poly_ids: Vec<i32>,
    poly_values: [u32; 6],
    campaign: u32,
    caves: u32,
    sell: u32,
    bridges: [(u32, [u32; 4]); 4],
    rank_level_limit: i32,
    log: String,
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

impl Host {
    fn line(&mut self, s: String) {
        self.log.push_str(&s);
        self.log.push('\n');
    }
    fn obj(&mut self, id: u32) -> &mut Obj {
        self.objects.iter_mut().find(|o| o.id == id).expect("object")
    }
}

impl GameLogicSaveHost for Host {
    fn x_frame(&self) -> u32 {
        self.frame
    }
    fn x_objects(&self) -> Vec<u32> {
        self.objects.iter().map(|o| o.id).collect()
    }
    fn x_process_destroy_list(&mut self) {
        self.line("destroylist".into());
        self.objects.retain(|o| !o.destroyed);
    }
    fn x_set_frame(&mut self, frame: u32) {
        self.frame = frame;
    }
    fn x_object_template_name(&self, id: u32) -> Vec<u8> {
        TNAMES[self.objects.iter().find(|o| o.id == id).expect("object").template].to_vec()
    }
    fn x_object_is_bridge(&self, id: u32) -> bool {
        self.objects.iter().any(|o| o.id == id && o.kind & 1 != 0)
    }
    fn x_object_walks_on_walls(&self, id: u32) -> bool {
        self.objects.iter().any(|o| o.id == id && o.kind >> 1 & 1 != 0)
    }
    fn x_bridge_at_object(&self, id: u32) -> Option<(u32, [u32; BRIDGE_MAX_TOWERS])> {
        let b = self.objects.iter().find(|o| o.id == id).expect("object").bridge;
        (0..4).contains(&b).then(|| self.bridges[b as usize])
    }
    fn x_object_exists(&self, id: u32) -> bool {
        self.objects.iter().any(|o| o.id == id)
    }
    fn x_destroy_object(&mut self, id: u32) {
        let o = self.obj(id);
        if o.destroyed {
            return;
        }
        o.destroyed = true;
        self.line(format!("destroy {id}"));
    }
    fn x_xfer_object(&mut self, id: u32, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        let mut v = self.obj(id).value;
        // a short read keeps the bytes it found (fread's), so the value is stored before the error goes on
        let r = xfer.xfer_int(&mut v);
        self.obj(id).value = v;
        r?;
        self.line(format!("object {id} xfer {v}"));
        Ok(())
    }
    fn x_has_neutral_player(&self) -> bool {
        self.has_neutral
    }
    fn x_template_exists(&self, name: &[u8]) -> bool {
        TNAMES.contains(&name)
    }
    fn x_new_object(&mut self, template_name: &[u8]) -> Result<u32, SaveError> {
        let t = TNAMES.iter().position(|n| *n == template_name).expect("a known template");
        let id = self.next_id;
        self.next_id += 1;
        let kind = match t {
            1 => 1,
            2 => 2,
            _ => 0,
        };
        // a made bridge stands on bridge 0
        self.objects.insert(0, Obj { id, template: t, value: 0, kind, destroyed: false, bridge: if t == 1 { 0 } else { -1 } });
        self.line(format!("new object {id} [{}]", lossy(template_name)));
        Ok(id)
    }
    fn x_add_wall_piece(&mut self, id: u32) {
        self.line(format!("wall piece {id}"));
    }
    fn x_pathfinder_new_map(&mut self) {
        self.line("pathfinder newmap".into());
    }
    fn x_xfer_campaign_manager(&mut self, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        xfer.xfer_unsigned_int(&mut self.campaign)?;
        let v = self.campaign;
        self.line(format!("campaign xfer {v}"));
        Ok(())
    }
    fn x_xfer_cave_system(&mut self, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        xfer.xfer_unsigned_int(&mut self.caves)?;
        let v = self.caves;
        self.line(format!("caves xfer {v}"));
        Ok(())
    }
    fn x_polygon_triggers(&self) -> Vec<i32> {
        self.poly_ids.clone()
    }
    fn x_xfer_polygon_trigger(&mut self, i: usize, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        let id = self.poly_ids[i];
        xfer.xfer_unsigned_int(&mut self.poly_values[i])?;
        let v = self.poly_values[i];
        self.line(format!("poly {id} xfer {v}"));
        Ok(())
    }
    fn x_rank_level_limit(&self) -> i32 {
        self.rank_level_limit
    }
    fn x_set_rank_level_limit(&mut self, limit: i32) {
        self.rank_level_limit = limit;
    }
    fn x_xfer_sell_list(&mut self, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        xfer.xfer_unsigned_int(&mut self.sell)?;
        let v = self.sell;
        self.line(format!("selllist xfer {v}"));
        Ok(())
    }
    fn x_find_command_button(&self, name: &[u8]) -> Option<u32> {
        (1..4).find(|&i| BUTTONS[i as usize] == name)
    }
    fn x_command_button_name(&self, button: u32) -> Vec<u8> {
        BUTTONS[button as usize].to_vec()
    }
}

fn dump(h: &mut Host, s: &GameLogicState) {
    let line = format!(
        "logic frame {} scoring {} markers {} icon {} lod {} hulk {} rankpts {} sw {} ranklimit {} campaign {} caves {} sell {}",
        h.frame,
        i32::from(s.scoring_enabled),
        i32::from(s.show_behind_building_markers),
        i32::from(s.draw_icon_ui),
        i32::from(s.show_dynamic_lod),
        s.script_hulk_max_lifetime_override,
        s.rank_points_to_add_at_game_start,
        s.superweapon_restriction,
        h.rank_level_limit,
        h.campaign,
        h.caves,
        h.sell
    );
    h.line(line);
    let o: String = h.objects.iter().map(|o| format!(" {}:{}:{}:{}", o.id, lossy(TNAMES[o.template]), o.value, o.kind)).collect();
    h.line(format!("objects{o}"));
    let toc: String = s.object_toc.iter().map(|e| format!(" {}:{}", lossy(&e.name), e.id)).collect();
    h.line(format!("toc{toc}"));
    let bo: String = s.buildable_overrides.iter().map(|(n, v)| format!(" {}:{v}", lossy(n))).collect();
    let cb: String = s.control_bar_overrides.iter().map(|(n, b)| format!(" {}:{}", lossy(n), b.unwrap_or(0))).collect();
    h.line(format!("buildable{bo} bar{cb}"));
}

fn result_line(what: &str, r: &Result<(), SaveError>) -> Result<String, Cut> {
    Ok(match r {
        Ok(()) => what.to_string(),
        Err(SaveError::Xfer(st)) => format!("{what} threw xfer {}", *st as i32),
        Err(SaveError::InvalidData) => format!("{what} threw save 6"),
        Err(SaveError::Undefined(e)) => return Err(Cut(format!("{e:?}"))),
    })
}

struct Cut(String);

fn nth_object(h: &Host, n: u32) -> Option<u32> {
    if h.objects.is_empty() {
        None
    } else {
        Some(h.objects[n as usize % h.objects.len()].id)
    }
}

fn op(s: &mut GameLogicState, h: &mut Host, saved: &mut Vec<u8>, w: &Op) -> Result<(), Cut> {
    let (a, b, c, d, e) = (w[1], w[2], w[3], w[4], w[5]);
    match w[0] {
        0 => {
            let name = TNAMES[(a % 4) as usize];
            let id = h.x_new_object(name).map_err(|_| Cut("new".into()))?;
            h.obj(id).value = b as i32;
            h.obj(id).bridge = (c % 6) as i32 - 1;
            h.line(format!("made {id}"));
        }
        1 => {
            if let Some(id) = nth_object(h, a) {
                h.obj(id).value = b as i32;
            }
            h.line("value".into());
        }
        2 => {
            let mut x = XferSave::new();
            x.open(b"glsave").expect("open");
            let r = xfer_game_logic(s, &mut x, h);
            h.line(result_line("save", &r)?);
            *saved = x.bytes().to_vec();
            let hex: String = saved.iter().map(|b| format!("{b:02x}")).collect();
            h.line(format!("bytes {hex}"));
            dump(h, s);
        }
        3 => {
            let mut bytes = saved.clone();
            if b & 1 != 0 && !bytes.is_empty() {
                let n = bytes.len();
                bytes[c as usize % n] = d as u8;
            }
            if b & 2 != 0 && !bytes.is_empty() {
                let n = bytes.len();
                bytes.truncate(n - 1 - (e as usize % n));
            }
            let mut x = XferLoad::new(bytes);
            x.open(b"glsave").expect("open");
            let r = xfer_game_logic(s, &mut x, h);
            if take_invalid_bool_read() {
                return Err(Cut("InvalidBoolLoaded".into()));
            }
            h.line(result_line("load", &r)?);
            dump(h, s);
        }
        4 => {
            s.scoring_enabled = a & 1 != 0;
            s.show_behind_building_markers = a >> 1 & 1 != 0;
            s.draw_icon_ui = a >> 2 & 1 != 0;
            s.show_dynamic_lod = a >> 3 & 1 != 0;
            s.script_hulk_max_lifetime_override = b as i32;
            s.rank_points_to_add_at_game_start = c as i32;
            s.superweapon_restriction = d as u16;
            h.rank_level_limit = e as i32;
            h.line("members".into());
        }
        5 => {
            if a & 1 != 0 {
                set_buildable_status_override(s, Some(TNAMES[(b % 4) as usize]), (c % 5) as i32);
            } else {
                let v = d % 4;
                set_control_bar_override(s, BAR_NAMES[(b % 6) as usize], (c % 4) as i32, (v != 0).then_some(v)).map_err(|e| Cut(format!("{e:?}")))?;
            }
            h.line("override".into());
        }
        6 => {
            s.buildable_overrides.clear();
            s.control_bar_overrides.clear();
            h.line("cleared".into());
        }
        7 => {
            h.has_neutral = a & 1 != 0;
            h.poly_ids = (0..b % 5).map(|i| ((c >> (3 * i)) & 7) as i32).collect();
            h.line("config".into());
        }
        8 => dump(h, s),
        9 => {
            h.frame = a;
            h.line("frame".into());
        }
        10 => {
            if let Some(id) = nth_object(h, a) {
                h.x_destroy_object(id);
            }
            h.x_process_destroy_list();
            h.line("destroyed".into());
        }
        11 => {
            let mut towers = [0u32; 4];
            for (i, t) in towers.iter_mut().enumerate() {
                *t = (c >> (5 * i)) & 31;
            }
            h.bridges[(a % 4) as usize] = (b % 24, towers);
            h.line("bridge".into());
        }
        12 => {
            h.campaign = a;
            h.caves = b;
            h.sell = c;
            for (i, v) in h.poly_values.iter_mut().enumerate() {
                *v = d.wrapping_add(i as u32);
            }
            h.line("snapshots".into());
        }
        _ => h.line("op?".into()),
    }
    Ok(())
}

fn rust_run(cfg: &Cfg, ops: &[Op]) -> (String, Option<(usize, String)>) {
    let mut h = Host {
        frame: cfg[0],
        objects: Vec::new(),
        next_id: 1,
        has_neutral: true,
        poly_ids: Vec::new(),
        poly_values: [0; 6],
        campaign: cfg[1],
        caves: cfg[2],
        sell: cfg[3],
        bridges: [(0, [0; 4]); 4],
        rank_level_limit: 1000,
        log: String::new(),
    };
    let mut s = GameLogicState { scoring_enabled: true, draw_icon_ui: true, show_dynamic_lod: true, script_hulk_max_lifetime_override: -1, ..Default::default() };
    let mut saved = Vec::new();
    take_invalid_bool_read();
    for (i, w) in ops.iter().enumerate() {
        let before = h.log.len();
        if let Err(Cut(why)) = op(&mut s, &mut h, &mut saved, w) {
            h.log.truncate(before);
            return (h.log, Some((i, why)));
        }
    }
    (h.log, None)
}

fn cpp_run(cfg: &Cfg, ops: &[Op]) -> String {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut cap = 1 << 18;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_glsave_run(cfg.as_ptr(), flat.as_ptr(), ops.len() as i32, out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

fn gen_program(g: &mut Gen) -> (Cfg, Vec<Op>) {
    let cfg: Cfg = [g.below(1000), g.next(), g.next(), g.next()];
    let mut ops: Vec<Op> = Vec::new();
    for _ in 0..g.below(6) {
        let t = g.below(4);
        let slot = if t == 1 && !g.chance(6) || g.chance(3) { 1 + g.below(4) } else { 0 };
        ops.push([0, t, g.next(), slot, 0, 0]);
    }
    if g.chance(2) {
        ops.push([7, u32::from(!g.chance(6)), g.below(5), g.next(), 0, 0]);
    }
    for _ in 0..2 + g.below(18) {
        let k = match g.below(100) {
            0..=11 => 0,
            12..=16 => 1,
            17..=34 => 2,
            35..=54 => 3,
            55..=59 => 4,
            60..=69 => 5,
            70..=74 => 6,
            75..=78 => 7,
            79..=80 => 8,
            81..=82 => 9,
            83..=87 => 10,
            88..=93 => 11,
            _ => 12,
        };
        // a load mostly follows an emptying of the tables
        if k == 3 && !g.chance(4) {
            ops.push([6, 0, 0, 0, 0, 0]);
        }
        let o: Op = match k {
            0 => {
                let t = g.below(4);
                // a bridge mostly stands on a bridge the terrain knows
                let slot = if t == 1 && !g.chance(6) || g.chance(3) { 1 + g.below(4) } else { 0 };
                [k, t, g.next(), slot, 0, 0]
            }
            3 => [k, 0, if g.chance(2) { 0 } else { g.below(4) }, g.next(), g.below(256), g.next()],
            5 => [k, g.below(2), g.below(6), g.below(8), g.below(4), 0],
            7 => [k, u32::from(!g.chance(6)), g.below(5), g.next(), 0, 0],
            11 => [k, g.below(4), g.below(24), g.next(), 0, 0],
            _ => [k, g.next(), g.next(), g.next(), g.next(), g.next()],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[test]
fn game_logic_save_matches_upstream() {
    let cases: usize = std::env::var("GLSAVE_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x474c_5332_cafe_0001);
    let (mut mismatches, mut cut_n, mut ops_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut cut_why = BTreeMap::<String, usize>::new();
    let mut by_op = [0usize; 13];
    let mut marks = BTreeMap::<String, usize>::new();
    for case in 0..cases {
        let (cfg, ops) = gen_program(&mut g);
        let (r, undefined_at) = rust_run(&cfg, &ops);
        let n = undefined_at.as_ref().map_or(ops.len(), |u| u.0);
        if let Some((_, why)) = &undefined_at {
            cut_n += 1;
            *cut_why.entry(why.clone()).or_insert(0) += 1;
        }
        let c = cpp_run(&cfg, &ops[..n]);
        ops_run += n;
        for o in &ops[..n] {
            by_op[o[0] as usize] += 1;
        }
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let (rl, cl): (Vec<&str>, Vec<&str>) = (r.lines().collect(), c.lines().collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                println!("case {case}: cfg {cfg:?} ops {:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}", &ops[..n], rl.get(at), cl.get(at));
            }
        } else {
            lines += r.lines().count();
            for l in r.lines() {
                for m in ["load threw", "save threw", "destroy ", "wall piece", "pathfinder newmap", "new object"] {
                    if l.starts_with(m) {
                        let key = if m.ends_with("threw") { l.to_string() } else { m.to_string() };
                        *marks.entry(key).or_insert(0) += 1;
                    }
                }
                if l == "load" || l == "save" {
                    *marks.entry(l.to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    println!("L2: {cases} programs ({cut_n} cut at an undefined operation), {ops_run} operations, {lines} lines, {mismatches} mismatches; by kind {by_op:?}");
    println!("reached: {marks:?}");
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
