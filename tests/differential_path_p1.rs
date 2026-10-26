#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (PathNode, Path)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `PathNode` and `Path` (`logic::ai::path`) against the unchanged C++, `p1w13-sonnet`, layer 10 batch P1.
//!
//! # Method
//!
//! The C++ side is the real `AIPathfind.h` and AIPathfind.cpp's path code, cut verbatim by build.rs (`differential/wrap_pathfind.cpp`,
//! `ref_pf_p1_run`), on the stand-ins of `differential/shim_pathfind`: the pathfinder's answers to a path (`isLinePassable`,
//! `isGroundPathPassable`) are a hash of their arguments, with rates the case picks, and lines of the log; `getCell` is the header's
//! own inline code over a 24 x 16 map whose cells are cliffs and pinched by the hash; the terrain's `getLayerForDestination` is the
//! hash too. The Rust side is the real thing, on a [`PathHost`] that answers with the same hash and writes the same lines.
//!
//! Each case is a program of 1 to 60 operations on one path: nodes prepended and appended on a grid of cells (straight, diagonal and
//! odd steps, on the ground and two bridge layers), the last node moved, `optimize` and `optimizeGroundPath`, `computePointOnPath` and
//! `computeFlightDistToGoal` from positions near the path, a node made unoptimizable, the flags, the direction and optimized link of a
//! node, a fresh path, and the save, the load (into a fresh path) and the CRC. Compared, bit for bit, after every operation: its
//! results, the log of the pathfinder's and terrain's calls in order, the xfer status, bytes and CRC, and the whole path (every node's
//! position, layer, flag, id, links, optimized distance and direction; the flags and the cache of `computePointOnPath`).
//!
//! # Results (2026-10-26, `PF_P1_CASES=20000`)
//!
//! 20,000 cases, 615,816 operations (92,178 optimizations, 73,871 closest-point queries, 18,561 loads), 62,845,640 dump words:
//! **0 mismatches**, on the first run. Nothing is excluded: the operations upstream leaves undefined (an optimization of an empty path,
//! a node index beyond the path) are not run on either side.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 20,000 cases: 9 of 9 caught (3005 once the closest-point queries were also placed along
//! the path's own segments).
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3000 | a zero-length optimized segment counted as 0.1, not 0.01 | 7,691 |
//! | 3001 | `optimize`'s one-cell diagonal step not passable | 903 |
//! | 3002 | an unpinched cliff cell not passable | 726 |
//! | 3003 | the jig/jog threshold 4.1 cells squared, not 3.9 | 9,754 |
//! | 3004 | the cached closest point's countdown not decremented | 43 |
//! | 3005 | look ahead only past 0.6 of the segment, not 0.5 | 12 |
//! | 3006 | the save walks the nodes forward | 187,825 |
//! | 3007 | a flight segment just reached not counted | 5,199 |
//! | 3008 | an append on an optimized path dropped at the tail's x alone | 145,744 |

mod support;

use rts_lockstep_engine::common::base_type::{real_to_int_floor, Coord3D};
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer, LAYER_GROUND};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::ai::path::{Path, PathHost, CELL_CLIFF, PATHFIND_CELL_SIZE};
use rts_lockstep_engine::logic::locomotor::LocomotorSurfaceTypeMask;
use std::ffi::CString;
use support::Gen;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/pf_p1.bin");
const MAP_W: i32 = 24;
const MAP_H: i32 = 16;

/// The scene: the hash's seed, the rates (percent) of a passable line, a passable ground path, a cliff and a pinched cell, the object.
#[derive(Clone, Copy, Debug)]
struct Scene {
    seed: u32,
    line_rate: u32,
    ground_rate: u32,
    cliff_rate: u32,
    pinch_rate: u32,
    obj_layer: PathfindLayer,
    obj_id: ObjectId,
}

/// The driver's `p1Hash`.
fn p1_hash(seed: u32, tag: u32, words: &[u32]) -> u32 {
    let mut x = seed ^ tag.wrapping_mul(0x85eb_ca6b);
    for &w in words {
        x ^= w;
        x = x.wrapping_mul(0x9E37_79B1);
        x ^= x >> 15;
    }
    x
}

fn pick(h: u32, rate: u32) -> bool {
    (h >> 8) % 100 < rate
}

struct Host {
    s: Scene,
    log: Vec<String>,
}

impl PathHost for Host {
    fn is_line_passable(&mut self, obj: ObjectId, surfaces: LocomotorSurfaceTypeMask, layer: PathfindLayer, start: &Coord3D, end: &Coord3D, blocked: bool, allow_pinched: bool) -> bool {
        let b = |f: f32| f.to_bits();
        let h = p1_hash(self.s.seed, 2, &[obj as u32, surfaces as u32, layer as u32, b(start.x), b(start.y), b(end.x), b(end.y), u32::from(blocked), u32::from(allow_pinched)]);
        let r = pick(h, self.s.line_rate);
        self.log.push(format!(
            "linepassable {obj} {} {layer} {:08x} {:08x} {:08x} {:08x} {} {} -> {}",
            surfaces as u32,
            b(start.x),
            b(start.y),
            b(end.x),
            b(end.y),
            i32::from(blocked),
            i32::from(allow_pinched),
            i32::from(r)
        ));
        r
    }
    fn is_ground_path_passable(&mut self, crusher: bool, start: &Coord3D, layer: PathfindLayer, end: &Coord3D, diameter: i32) -> bool {
        let b = |f: f32| f.to_bits();
        let h = p1_hash(self.s.seed, 3, &[u32::from(crusher), b(start.x), b(start.y), layer as u32, b(end.x), b(end.y), diameter as u32]);
        let r = pick(h, self.s.ground_rate);
        self.log.push(format!(
            "groundpassable {} {:08x} {:08x} {layer} {:08x} {:08x} {diameter} -> {}",
            i32::from(crusher),
            b(start.x),
            b(start.y),
            b(end.x),
            b(end.y),
            i32::from(r)
        ));
        r
    }
    fn cell_type_and_pinched(&mut self, _layer: PathfindLayer, pos: &Coord3D) -> Option<(i32, bool)> {
        // Pathfinder::getCell(layer, pos), inline: worldToCell, NULL on an overflow; a bridge layer's cell is none (the ground's)
        let x = real_to_int_floor(pos.x / PATHFIND_CELL_SIZE as f32);
        let y = real_to_int_floor(pos.y / PATHFIND_CELL_SIZE as f32);
        if x < 0 || y < 0 || x > MAP_W - 1 || y > MAP_H - 1 {
            return None;
        }
        let h = p1_hash(self.s.seed, 5, &[x as u32, y as u32]);
        let ty = if pick(h, self.s.cliff_rate) { CELL_CLIFF } else { 0 };
        Some((ty, pick(p1_hash(self.s.seed, 6, &[h]), self.s.pinch_rate)))
    }
    fn layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        let b = |f: f32| f.to_bits();
        let h = p1_hash(self.s.seed, 1, &[b(pos.x), b(pos.y), b(pos.z)]);
        let layer = (1 + (h >> 8) % 3) as PathfindLayer;
        self.log.push(format!("layerfor {:08x} {:08x} {:08x} {layer}", b(pos.x), b(pos.y), b(pos.z)));
        layer
    }
    fn object_layer(&mut self, _obj: ObjectId) -> PathfindLayer {
        self.s.obj_layer
    }
}

fn put_str(v: &mut Vec<u32>, s: &str) {
    v.push(s.len() as u32);
    for c in s.as_bytes().chunks(4) {
        let mut w = 0u32;
        for (k, &b) in c.iter().enumerate() {
            w |= u32::from(b) << (8 * k);
        }
        v.push(w);
    }
}

/// The driver's `putPath`.
fn put_path(v: &mut Vec<u32>, p: &Path) {
    let list = p.list();
    let index = |n: Option<usize>| -> u32 {
        match n {
            None => u32::MAX,
            Some(i) => list.iter().position(|&x| x == i).map_or(1000, |k| k as u32),
        }
    };
    v.push(list.len() as u32);
    for &i in &list {
        let n = p.node(i);
        let pos = n.position();
        v.extend([pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), n.layer() as u32, u32::from(n.can_optimize()), n.id as u32]);
        let (opt, dir, dist) = n.next_optimized();
        v.extend([index(opt), dist.to_bits(), dir.x.to_bits(), dir.y.to_bits(), index(n.previous())]);
    }
    v.push(index(p.last_node()));
    v.push(u32::from(p.is_optimized()));
    v.push(u32::from(p.blocked_by_ally()));
    let (valid, countdown, cin, cout, recent) = p.cpop_state();
    v.extend([u32::from(valid), countdown as u32, cin.x.to_bits(), cin.y.to_bits(), cin.z.to_bits(), cout.dist_along_path.to_bits()]);
    v.extend([cout.pos_on_path.x.to_bits(), cout.pos_on_path.y.to_bits(), cout.pos_on_path.z.to_bits(), cout.layer as u32, index(recent)]);
}

fn rust_run(s: Scene, ops: &[[u32; 6]]) -> Vec<u32> {
    let mut v = Vec::new();
    let mut host = Host { s, log: Vec::new() };
    let mut path = Path::new();
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        host.log.clear();
        let mut res: Vec<u32> = Vec::new();
        let (mut xst, mut bytes, mut crc) = (-2i32, Vec::new(), 0u32);
        let pos = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) };
        let node_at = |p: &Path, k: u32| p.list().get(k as usize).copied();
        match o[0] {
            0 => path.prepend_node(&pos, o[4] as PathfindLayer),
            1 => path.append_node(&pos, o[4] as PathfindLayer),
            2 => path.update_last_node(&mut host, &pos),
            3 => path.optimize(&mut host, s.obj_id, o[4] as LocomotorSurfaceTypeMask, o[5] != 0),
            4 => path.optimize_ground_path(&mut host, o[4] != 0, o[5] as i32),
            5 => {
                let out = path.compute_point_on_path(&mut host, s.obj_id, o[4] as LocomotorSurfaceTypeMask, &pos);
                res.extend([out.dist_along_path.to_bits(), out.pos_on_path.x.to_bits(), out.pos_on_path.y.to_bits(), out.pos_on_path.z.to_bits(), out.layer as u32]);
            }
            6 => {
                let (d, goal) = path.compute_flight_dist_to_goal(&pos);
                res.extend([d.to_bits(), goal.x.to_bits(), goal.y.to_bits(), goal.z.to_bits()]);
            }
            7 => {
                if let Some(n) = node_at(&path, o[1]) {
                    // (PathNode::setCanOptimize)
                    set_can_optimize(&mut path, n, o[2] != 0);
                }
            }
            8 => path.set_blocked_by_ally(o[1] != 0),
            9 => path.mark_optimized(),
            10 => {
                let mut x = XferSave::new();
                x.open(b"pf_p1.bin").unwrap();
                xst = match x.xfer_snapshot(&mut path) {
                    Ok(()) => 0,
                    Err(e) => e as i32,
                };
                let _ = x.close();
                bytes = x.bytes().to_vec();
                saved = bytes.clone();
            }
            11 => {
                path = Path::new();
                let mut x = XferLoad::new(saved.clone());
                x.open(b"pf_p1.bin").unwrap();
                xst = match x.xfer_snapshot(&mut path) {
                    Ok(()) => 0,
                    Err(e) => e as i32,
                };
                let _ = x.close();
            }
            12 => {
                let mut x = XferCrc::new();
                x.open(b"crc").unwrap();
                xst = match x.xfer_snapshot(&mut path) {
                    Ok(()) => 0,
                    Err(e) => e as i32,
                };
                crc = x.get_crc();
                let _ = x.close();
            }
            13 => {
                if let Some(n) = node_at(&path, o[1]) {
                    let d = path.compute_direction_vector(n);
                    res.extend([d.x.to_bits(), d.y.to_bits(), d.z.to_bits()]);
                }
            }
            14 => {
                if let Some(n) = node_at(&path, o[1]) {
                    let list = path.list();
                    let (opt, dir, dist) = path.node(n).next_optimized();
                    let idx = opt.map_or(u32::MAX, |i| list.iter().position(|&x| x == i).map_or(1000, |k| k as u32));
                    res.extend([idx, dir.x.to_bits(), dir.y.to_bits(), dist.to_bits()]);
                }
            }
            15 => path = Path::new(),
            _ => unreachable!(),
        }
        v.push(res.len() as u32);
        v.extend(&res);
        v.push(host.log.len() as u32);
        for l in &host.log {
            put_str(&mut v, l);
        }
        v.push(xst as u32);
        v.push(bytes.len() as u32);
        v.extend(bytes.iter().map(|&b| u32::from(b)));
        v.push(crc);
        put_path(&mut v, &path);
    }
    v
}

fn set_can_optimize(path: &mut Path, n: usize, b: bool) {
    path.node_mut(n).set_can_optimize(b);
}

fn cpp_run(s: Scene, ops: &[[u32; 6]]) -> Vec<u32> {
    let cfg = [s.seed, s.line_rate, s.ground_rate, s.cliff_rate, 0, s.pinch_rate, s.obj_layer as u32, s.obj_id as u32];
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut buf = vec![0u32; 1 << 22];
    let mut len = 0i32;
    let path = CString::new(XFER_FILE).unwrap();
    let st = unsafe { cpp::ref_pf_p1_run(path.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len) };
    assert_eq!(st, 0, "the dump fits");
    buf.truncate(len as usize);
    buf
}

/// One op's part of a dump.
fn split_ops(b: &[u32], n: usize) -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    let mut p = 0;
    for _ in 0..n {
        let start = p;
        let take = |p: &mut usize| -> u32 {
            let x = b[*p];
            *p += 1;
            x
        };
        let nr = take(&mut p) as usize;
        p += nr;
        let nl = take(&mut p);
        for _ in 0..nl {
            let len = take(&mut p) as usize;
            p += len.div_ceil(4);
        }
        p += 1; // xst
        let nb = take(&mut p) as usize;
        p += nb + 1;
        let nn = take(&mut p) as usize;
        p += 11 * nn + 3 + 11;
        out.push(b[start..p].to_vec());
    }
    out
}

/// A position on the grid of cell centres, or off it a little.
fn gen_pos(g: &mut Gen) -> [u32; 3] {
    let cell = |g: &mut Gen, n: u32| (g.below(n) * 10 + 5) as f32;
    let (mut x, mut y) = (cell(g, MAP_W as u32 + 2) - 10.0, cell(g, MAP_H as u32 + 2) - 10.0);
    if g.below(4) == 0 {
        x += g.uniform(-4.0, 4.0);
        y += g.uniform(-4.0, 4.0);
    }
    let z = if g.below(5) == 0 { g.uniform(0.0, 20.0) } else { 0.0 };
    [x.to_bits(), y.to_bits(), z.to_bits()]
}

fn gen_layer(g: &mut Gen) -> u32 {
    if g.below(4) == 0 {
        2 + g.below(2)
    } else {
        LAYER_GROUND as u32
    }
}

fn gen_case(g: &mut Gen) -> (Scene, Vec<[u32; 6]>) {
    let s = Scene {
        seed: g.u32(),
        line_rate: *[0u32, 30, 60, 90, 100].get(g.below(5) as usize).unwrap(),
        ground_rate: *[0u32, 30, 60, 90].get(g.below(4) as usize).unwrap(),
        cliff_rate: *[0u32, 10, 40].get(g.below(3) as usize).unwrap(),
        pinch_rate: *[0u32, 50].get(g.below(2) as usize).unwrap(),
        obj_layer: if g.below(5) == 0 { 2 } else { LAYER_GROUND },
        obj_id: 1 + g.below(9) as ObjectId,
    };
    let n = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    // a walk of grid steps, appended mostly
    let (mut cx, mut cy) = (g.below(MAP_W as u32) as i32, g.below(MAP_H as u32) as i32);
    // (the positions placed, for the closest-point queries along the path)
    let mut placed: Vec<(f32, f32)> = Vec::new();
    while ops.len() < n {
        let o = match g.below(100) {
            0..=39 => {
                let (dx, dy) = [(1, 0), (1, 1), (0, 1), (-1, 1), (1, 0), (1, 0), (2, 1), (0, -1)][g.below(8) as usize];
                cx += dx;
                cy += dy;
                let (x, y) = ((cx * 10 + 5) as f32, (cy * 10 + 5) as f32);
                let op = if g.below(6) == 0 { 0 } else { 1 };
                placed.push((x, y));
                [op, x.to_bits(), y.to_bits(), 0f32.to_bits(), gen_layer(g), 0]
            }
            40..=43 => {
                let p = gen_pos(g);
                [g.below(2), p[0], p[1], p[2], gen_layer(g), 0]
            }
            44..=46 => {
                let p = gen_pos(g);
                [2, p[0], p[1], p[2], 0, 0]
            }
            47..=55 => [3, 0, 0, 0, g.below(16), g.below(2)],
            56..=61 => [4, 0, 0, 0, g.below(2), *[1u32, 2, 3, 4].get(g.below(4) as usize).unwrap()],
            62..=73 => {
                let mut p = gen_pos(g);
                if placed.len() >= 2 && g.below(2) == 0 {
                    // a point between two positions placed one after the other, a little off the line
                    let i = g.below(placed.len() as u32 - 1) as usize;
                    let ((ax, ay), (bx, by)) = (placed[i], placed[i + 1]);
                    let t = g.uniform(0.0, 1.0);
                    p[0] = (ax + t * (bx - ax) + g.uniform(-2.0, 2.0)).to_bits();
                    p[1] = (ay + t * (by - ay) + g.uniform(-2.0, 2.0)).to_bits();
                }
                [5, p[0], p[1], p[2], g.below(16), 0]
            }
            74..=79 => {
                let p = gen_pos(g);
                [6, p[0], p[1], p[2], 0, 0]
            }
            80..=82 => [7, g.below(12), g.below(2), 0, 0, 0],
            83 => [8, g.below(2), 0, 0, 0, 0],
            84 => [9, 0, 0, 0, 0, 0],
            85..=88 => [10, 0, 0, 0, 0, 0],
            89..=91 => [11, 0, 0, 0, 0, 0],
            92 => [12, 0, 0, 0, 0, 0],
            93..=94 => [13, g.below(12), 0, 0, 0, 0],
            95..=97 => [14, g.below(12), 0, 0, 0, 0],
            _ => [15, 0, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (s, ops)
}

#[test]
fn path_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("PF_P1_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen::new(0x0010_9A71);
    let mut check = support::Check::new("P1 Path ops");
    let (mut ops_run, mut optimizes, mut cpops, mut loads, mut lines) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for case in 0..cases {
        let (s, ops) = gen_case(&mut g);
        let r = rust_run(s, &ops);
        let c = cpp_run(s, &ops);
        let (rs, cs) = (split_ops(&r, ops.len()), split_ops(&c, ops.len()));
        for (i, (a, b)) in rs.iter().zip(cs.iter()).enumerate() {
            check.eq(a.clone(), b.clone(), || format!("case {case} op {i} {:?} scene {s:?} ops {:?}", ops[i], &ops[..=i]));
        }
        ops_run += ops.len() as u64;
        optimizes += ops.iter().filter(|o| o[0] == 3 || o[0] == 4).count() as u64;
        cpops += ops.iter().filter(|o| o[0] == 5).count() as u64;
        loads += ops.iter().filter(|o| o[0] == 11).count() as u64;
        lines += r.len() as u64;
    }
    println!("P1: {cases} cases, {ops_run} ops ({optimizes} optimizations, {cpops} closest points, {loads} loads), {lines} dump words");
    check.finish();
}
