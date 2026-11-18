#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the drafts src/drafts/wwlib/{cullsys, aabtreecull, gridcull}.rs against (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/cullsys.h, cullsys.cpp, aabtreecull.h, aabtreecull.cpp,
//   gridcull.h, gridcull.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! The culling systems (`AABTreeCullSystemClass`, `GridCullSystemClass`, `CullableClass`) against the C++ (#215).
//!
//! # Method
//!
//! `differential/wrap_draft_cull.cpp` compiles cullsys.cpp unchanged and aabtreecull.cpp and gridcull.cpp with the
//! mechanical edits `build.rs` (`drafts_wwmath`) lists (the pool definition spelled as clang wants it; the split plane's
//! `rand()` going to `draft_crt_rand`, the MSVC C runtime's generator, which the Rust closure reproduces), plus
//! colmath*.cpp unchanged for the overlap tests the culling calls. A case is a PROGRAM: objects (`CullableClass`es with
//! boxes) and then 8 to 70 operations on one system, run on both sides from the same float stream. After EVERY
//! operation both sides dump the system's whole structure as 32-bit words, floats by their bits, and after a
//! `Collect_Objects` the collected ids IN COLLECTION ORDER; the dumps are compared whole.
//!
//! Tree dump: `ObjectCount`, `NodeCount`, `Partition_Node_Count()`, `Partition_Tree_Depth()`, `Get_Bounding_Box()`, then
//! every node in pre-order (index, box, parent/front/back index, user data, object count, the objects in list order),
//! then `Get_Node_Bounds(i)` for every index. Grid dump: `ObjCount`, `CellCount`, `Origin`, `CellDim`, `OOCellDim`,
//! `MaxObjExtent`, termination count, `MinCellSize`, the statistics, every non-empty cell's list and the no-grid list,
//! each object with `GridAddress`, `Prev`, `Next`.
//!
//! Operations: add (tree: dropped into the tree, or into a given node), remove, `Set_Cull_Box` (with and without
//! `just_loaded`), `Update_Culling`, `Collect_Objects` for a point, an `AABoxClass`, an `OBBoxClass` and a
//! `FrustumClass` (and a sphere for the tree), the tree's `Re_Partition()` (random seed) and `Re_Partition(bounds,
//! boxes)`, `Update_Bounding_Boxes`, `AABTreeIterator` walks, the grid's `Re_Partition(min, max, objdim)`,
//! `Set_Min_Cell_Size`, `Set_Termination_Count`, and destroying the system (which releases its objects) and making
//! a new one. The grid's geometry helpers (`compute_box`, `init_volume`, `map_point_to_address`) are compared directly.
//! Not covered: `Save`, `Load`, `Load_Object_Linkage`, `Save_Object_Linkage` (chunk I/O) and the `Get_*_Collected_Object`
//! typed wrappers (the same as `Peek_*`: no `Add_Ref`).
//!
//! # Predictions (written before the first run, 2026-11-18)
//!
//! Not 0. The draft stores nodes in an arena and objects in `Arc` lists, which is a large departure; I expect at
//! least: the tree's `Re_Partition(bounds, boxes)` order of re-insertion; `Update_Culling` after `Set_Cull_Box`; and the
//! grid's `Re_Partition` re-linking order. Everything simpler (`Add`, `Remove`, point and box collection) I expect to match.
//!
//! # Results, plants
//!
//! see the comment on #215.

mod support;

use rts_lockstep_engine::drafts::wwlib::aabox::AABoxClass;
use rts_lockstep_engine::drafts::wwlib::aabtreecull::{AABTreeCullSystemClass, AABTreeIterator, AABTreeNodeClass, NodeId};
use rts_lockstep_engine::drafts::wwlib::cullsys::{CullableClass, CullableHandle};
use rts_lockstep_engine::drafts::wwlib::frustum::FrustumClass;
use rts_lockstep_engine::drafts::wwlib::gridcull::GridCullSystemClass;
use rts_lockstep_engine::drafts::wwlib::matrix3::Matrix3x3;
use rts_lockstep_engine::drafts::wwlib::obbox::OBBoxClass;
use rts_lockstep_engine::drafts::wwlib::sphere::SphereClass;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::{Arc, Mutex};
use support::{Check, Gen};

extern "C" {
    fn draft_cull_run(prog: *const f32, n: i32, out: *mut u32) -> i32;
}

/// The C++ wrapper's objects and rand state are global.
static CPP: Mutex<()> = Mutex::new(());

const TREE_CASES: usize = 12_000;
const GRID_CASES: usize = 12_000;

fn plant_id() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

fn fb(f: f32) -> u32 {
    f.to_bits()
}

fn v3(a: &[f32]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

fn box_of(a: &[f32]) -> AABoxClass {
    AABoxClass::new(v3(&a[0..3]), v3(&a[3..6]))
}

// ---------------------------------------------------------------------------------------------------------------------
// The Rust side: the same program interpreter, over the drafts.

enum Sys {
    Tree(AABTreeCullSystemClass),
    Grid(GridCullSystemClass),
}

struct MsvcRand {
    holdrand: u32,
}

impl MsvcRand {
    fn next(&mut self) -> i32 {
        self.holdrand = self.holdrand.wrapping_mul(214013).wrapping_add(2531011);
        ((self.holdrand >> 16) & 0x7fff) as i32
    }
}

fn id_of(objs: &[Option<CullableHandle>], h: &CullableHandle) -> i32 {
    for (i, o) in objs.iter().enumerate() {
        if let Some(o) = o {
            if Arc::ptr_eq(o, h) {
                return i as i32;
            }
        }
    }
    -2
}

fn push_box(out: &mut Vec<u32>, b: &AABoxClass) {
    out.extend_from_slice(&[fb(b.center.x), fb(b.center.y), fb(b.center.z), fb(b.extent.x), fb(b.extent.y), fb(b.extent.z)]);
}

fn dump_collected(out: &mut Vec<u32>, base: &rts_lockstep_engine::drafts::wwlib::cullsys::CullSystemClass, objs: &[Option<CullableHandle>]) {
    let mut list = Vec::new();
    let mut c = base.peek_first_collected_object_internal();
    while let Some(h) = c {
        list.push(id_of(objs, &h));
        c = base.peek_next_collected_object_internal(Some(&h));
    }
    out.push(list.len() as u32);
    for i in list {
        out.push(i as u32);
    }
}

fn next_object_tree(h: &CullableHandle) -> Option<CullableHandle> {
    use rts_lockstep_engine::drafts::wwlib::cullsys::CullLinkClass;
    match h.read().unwrap().get_cull_link() {
        Some(CullLinkClass::AABTree(l)) => l.next_object.clone(),
        _ => None,
    }
}

fn dump_tree_node(out: &mut Vec<u32>, nodes: &[AABTreeNodeClass], id: NodeId, objs: &[Option<CullableHandle>]) {
    let n = &nodes[id];
    out.push(n.index);
    push_box(out, &n.box_);
    out.push(n.parent.map(|p| nodes[p].index as i32).unwrap_or(-1) as u32);
    out.push(n.front.map(|p| nodes[p].index as i32).unwrap_or(-1) as u32);
    out.push(n.back.map(|p| nodes[p].index as i32).unwrap_or(-1) as u32);
    out.push(n.user_data);
    out.push(n.object_count() as u32);
    let mut c = n.object.clone();
    while let Some(h) = c {
        out.push(id_of(objs, &h) as u32);
        c = next_object_tree(&h);
    }
    if let Some(f) = n.front {
        dump_tree_node(out, nodes, f, objs);
    }
    if let Some(b) = n.back {
        dump_tree_node(out, nodes, b, objs);
    }
}

fn dump_tree(out: &mut Vec<u32>, t: &AABTreeCullSystemClass, objs: &[Option<CullableHandle>]) {
    out.push(t.object_count() as u32);
    let (nodes, root) = t.debug_arena();
    // NodeCount is the count of IndexedNodes, which Re_Index_Nodes sets; Partition_Node_Count counts the tree
    out.push(t.debug_node_count() as u32);
    out.push(t.partition_node_count() as u32);
    out.push(t.partition_tree_depth() as u32);
    push_box(out, t.get_bounding_box());
    dump_tree_node(out, nodes, root, objs);
    for i in 0..t.debug_node_count() {
        push_box(out, &t.get_node_bounds(i));
    }
}

fn walk(out: &mut Vec<u32>, t: &AABTreeCullSystemClass, it: &mut AABTreeIterator) {
    let b = it.get_current_box(t);
    out.push(it.get_current_node_index() as u32);
    push_box(out, &b);
    let hf = it.has_front_child(t);
    let hb = it.has_back_child(t);
    out.push(hf as u32);
    out.push(hb as u32);
    if it.enter_front_child(t) {
        out.push(1);
        walk(out, t, it);
        let sib = it.enter_sibling(t);
        out.push(sib as u32);
        if sib {
            walk(out, t, it);
            let s2 = it.enter_sibling(t);
            out.push(s2 as u32);
        }
        let p = it.enter_parent(t);
        out.push(p as u32);
    } else {
        out.push(0);
        if it.enter_back_child(t) {
            out.push(1);
            walk(out, t, it);
            let p = it.enter_parent(t);
            out.push(p as u32);
        } else {
            out.push(0);
        }
    }
}

fn dump_grid_list(out: &mut Vec<u32>, head: &Option<CullableHandle>, objs: &[Option<CullableHandle>]) {
    use rts_lockstep_engine::drafts::wwlib::cullsys::CullLinkClass;
    let mut list = Vec::new();
    let mut c = head.clone();
    while let Some(h) = c {
        let (addr, prev, next) = match h.read().unwrap().get_cull_link() {
            Some(CullLinkClass::Grid(l)) => (l.grid_address, l.prev.clone(), l.next.clone()),
            _ => panic!("no grid link"),
        };
        list.push((id_of(objs, &h), addr, prev.map(|p| id_of(objs, &p)).unwrap_or(-1), next.clone().map(|p| id_of(objs, &p)).unwrap_or(-1)));
        c = next;
    }
    out.push(list.len() as u32);
    for (a, b, c, d) in list {
        out.extend_from_slice(&[a as u32, b as u32, c as u32, d as u32]);
    }
}

fn dump_grid(out: &mut Vec<u32>, g: &GridCullSystemClass, objs: &[Option<CullableHandle>]) {
    let s = g.debug_state();
    out.push(g.get_object_count() as u32);
    out.extend_from_slice(&[s.cell_count[0] as u32, s.cell_count[1] as u32, s.cell_count[2] as u32]);
    for v in [s.origin, s.cell_dim, s.oo_cell_dim] {
        out.extend_from_slice(&[fb(v.x), fb(v.y), fb(v.z)]);
    }
    out.push(fb(s.max_obj_extent));
    out.push(g.get_termination_count() as u32);
    let m = g.get_min_cell_size();
    out.extend_from_slice(&[fb(m.x), fb(m.y), fb(m.z)]);
    let st = g.get_statistics();
    out.extend_from_slice(&[st.node_count as u32, st.nodes_accepted as u32, st.nodes_trivially_accepted as u32, st.nodes_rejected as u32]);
    let nonempty = s.cells.iter().filter(|c| c.is_some()).count();
    out.push(nonempty as u32);
    for (c, head) in s.cells.iter().enumerate() {
        if head.is_some() {
            out.push(c as u32);
            dump_grid_list(out, head, objs);
        }
    }
    dump_grid_list(out, &s.no_grid_list, objs);
}

fn run_rust(prog: &[f32]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let kind = prog[0] as i32;
    let mut objs: Vec<Option<CullableHandle>> = vec![None; 128];
    let mut sys = if kind == 0 { Sys::Tree(AABTreeCullSystemClass::new()) } else { Sys::Grid(GridCullSystemClass::new()) };
    let mut rng = MsvcRand { holdrand: 1 };
    let mut i = 1;
    while i < prog.len() {
        let op = prog[i] as i32;
        i += 1;
        out.push((-1000 - op) as u32);
        let mut collected = false;
        match op {
            1 => {
                let id = prog[i] as usize;
                let h = CullableClass::new_handle();
                h.write().unwrap().set_cull_box(&box_of(&prog[i + 1..i + 7]), true);
                objs[id] = Some(h);
                i += 7;
                continue;
            }
            2 => {
                let id = prog[i] as usize;
                let spec = prog[i + 1];
                i += 2;
                let h = objs[id].clone().unwrap();
                match &mut sys {
                    Sys::Tree(t) => {
                        let mut ni = -1;
                        if spec >= 0.0 {
                            let cnt = t.debug_node_count();
                            ni = (spec * cnt as f32) as i32;
                            if ni >= cnt {
                                ni = cnt - 1;
                            }
                        }
                        t.add_object_internal(&h, ni);
                    }
                    Sys::Grid(g) => g.add_object_internal(&h),
                }
            }
            3 => {
                let id = prog[i] as usize;
                i += 1;
                let h = objs[id].clone().unwrap();
                match &mut sys {
                    Sys::Tree(t) => t.remove_object_internal(&h),
                    Sys::Grid(g) => g.remove_object_internal(&h),
                }
            }
            4 => {
                let id = prog[i] as usize;
                let b = box_of(&prog[i + 1..i + 7]);
                let just_loaded = prog[i + 7] != 0.0;
                i += 8;
                let h = objs[id].clone().unwrap();
                let s = h.write().unwrap().set_cull_box(&b, just_loaded);
                if s.is_some() {
                    match &mut sys {
                        Sys::Tree(t) => t.update_culling(&h),
                        Sys::Grid(g) => g.update_culling(&h),
                    }
                }
            }
            5..=9 => {
                match &mut sys {
                    Sys::Tree(t) => t.base.reset_collection(),
                    Sys::Grid(g) => g.base.reset_collection(),
                }
                if op == 5 {
                    let p = v3(&prog[i..i + 3]);
                    i += 3;
                    match &mut sys {
                        Sys::Tree(t) => t.collect_objects_point(&p),
                        Sys::Grid(g) => g.collect_objects_point(&p),
                    }
                } else if op == 6 {
                    let b = box_of(&prog[i..i + 6]);
                    i += 6;
                    match &mut sys {
                        Sys::Tree(t) => t.collect_objects_aabox(&b),
                        Sys::Grid(g) => g.collect_objects_aabox(&b),
                    }
                } else if op == 7 {
                    let q = &prog[i..i + 15];
                    let bs = Matrix3x3::from_elements(q[0], q[1], q[2], q[3], q[4], q[5], q[6], q[7], q[8]);
                    let b = OBBoxClass::with_basis(&v3(&q[9..12]), &v3(&q[12..15]), &bs);
                    i += 15;
                    match &mut sys {
                        Sys::Tree(t) => t.collect_objects_obbox(&b),
                        Sys::Grid(g) => g.collect_objects_obbox(&b),
                    }
                } else if op == 8 {
                    let q = &prog[i..i + 18];
                    let mut cam = [0.0f32; 12];
                    cam.copy_from_slice(&q[0..12]);
                    let mut f = FrustumClass::default();
                    f.init(&Matrix3D::from_array(cam), &Vector2::new(q[12], q[13]), &Vector2::new(q[14], q[15]), q[16], q[17]);
                    i += 18;
                    match &mut sys {
                        Sys::Tree(t) => t.collect_objects_frustum(&f),
                        Sys::Grid(g) => g.collect_objects_frustum(&f),
                    }
                } else {
                    let s = SphereClass::new(&v3(&prog[i..i + 3]), prog[i + 3]);
                    i += 4;
                    if let Sys::Tree(t) = &mut sys {
                        t.collect_objects_sphere(&s);
                    }
                }
                collected = true;
            }
            10 => {
                rng.holdrand = prog[i] as u32;
                i += 1;
                if let Sys::Tree(t) = &mut sys {
                    t.re_partition(&mut || rng.next());
                }
            }
            11 => {
                rng.holdrand = prog[i] as u32;
                let bounds = box_of(&prog[i + 1..i + 7]);
                let nb = prog[i + 7] as usize;
                let mut boxes: Vec<AABoxClass> = (0..nb).map(|b| box_of(&prog[i + 8 + b * 6..i + 14 + b * 6])).collect();
                i += 8 + nb * 6;
                if let Sys::Tree(t) = &mut sys {
                    t.re_partition_boxes(&bounds, &mut boxes, &mut || rng.next());
                }
            }
            12 => {
                if let Sys::Tree(t) = &mut sys {
                    t.update_bounding_boxes();
                }
            }
            13 => {
                let mn = v3(&prog[i..i + 3]);
                let mx = v3(&prog[i + 3..i + 6]);
                let od = prog[i + 6];
                i += 7;
                if let Sys::Grid(g) = &mut sys {
                    g.re_partition(&mn, &mx, od);
                }
            }
            14 => {
                let s = v3(&prog[i..i + 3]);
                i += 3;
                if let Sys::Grid(g) = &mut sys {
                    g.set_min_cell_size(&s);
                }
            }
            15 => {
                let n = prog[i] as i32;
                i += 1;
                if let Sys::Grid(g) = &mut sys {
                    g.set_termination_count(n);
                }
            }
            16 => {
                let id = prog[i] as usize;
                i += 1;
                let h = objs[id].clone().unwrap();
                match &mut sys {
                    Sys::Tree(t) => t.update_culling(&h),
                    Sys::Grid(g) => g.update_culling(&h),
                }
            }
            17 => {
                if let Sys::Tree(t) = &sys {
                    let mut it = AABTreeIterator::new();
                    walk(&mut out, t, &mut it);
                }
            }
            18 => {
                match &mut sys {
                    Sys::Tree(t) => {
                        t.shutdown();
                        *t = AABTreeCullSystemClass::new();
                    }
                    // upstream's ~GridCullSystemClass only frees the cell array (it leaves the objects linked), so the
                    // program removes them first
                    Sys::Grid(g) => *g = GridCullSystemClass::new(),
                }
            }
            19 => {}
            20 => {
                if let Sys::Grid(g) = &sys {
                    let p = &prog[i..i + 12];
                    i += 12;
                    let s = g.debug_state();
                    let (ci, cj, ck) = (p[0] as i32, p[1] as i32, p[2] as i32);
                    if ci >= 0 && cj >= 0 && ck >= 0 && ci < s.cell_count[0] && cj < s.cell_count[1] && ck < s.cell_count[2] {
                        push_box(&mut out, &g.compute_box(ci, cj, ck));
                    }
                    let v = g.init_volume(&v3(&p[3..6]), &v3(&p[6..9]));
                    for a in 0..3 {
                        out.push(v.min[a] as u32);
                        out.push(v.max[a] as u32);
                    }
                    push_box(&mut out, &g.compute_box_volume(&v));
                    let (addr, inside) = g.debug_map_point_to_address(&v3(&p[9..12]));
                    out.push(inside as u32);
                    out.push(addr as u32);
                }
            }
            21 => {
                let ni = prog[i] as i32;
                let ud = prog[i + 1] as u32;
                i += 2;
                if let Sys::Tree(t) = &mut sys {
                    t.debug_set_user_data(ni, ud);
                }
            }
            _ => unreachable!(),
        }
        if collected {
            match &sys {
                Sys::Tree(t) => dump_collected(&mut out, &t.base, &objs),
                Sys::Grid(g) => dump_collected(&mut out, &g.base, &objs),
            }
        }
        match &sys {
            Sys::Tree(t) => dump_tree(&mut out, t, &objs),
            Sys::Grid(g) => dump_grid(&mut out, g, &objs),
        }
    }
    // tear down: the system releases the objects it holds
    if let Sys::Tree(t) = &mut sys {
        t.shutdown();
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// The generator.

struct Program {
    p: Vec<f32>,
}

impl Program {
    fn op(&mut self, code: i32, args: &[f32]) {
        self.p.push(code as f32);
        self.p.extend_from_slice(args);
    }
}

fn rand_box(g: &mut Gen, world: f32, clustered: bool, centers: &[[f32; 3]]) -> [f32; 6] {
    let c = if clustered && !centers.is_empty() {
        let b = centers[g.below(centers.len() as u32) as usize];
        [b[0] + g.uniform(-8.0, 8.0), b[1] + g.uniform(-8.0, 8.0), b[2] + g.uniform(-8.0, 8.0)]
    } else {
        [g.uniform(-world, world), g.uniform(-world, world), g.uniform(-world, world)]
    };
    let e = match g.below(20) {
        0 => [g.uniform(10.0, 30.0), g.uniform(10.0, 30.0), g.uniform(10.0, 30.0)], // bigger than the grid's max extent
        1 => [0.0, 0.0, 0.0],
        _ => [g.uniform(0.2, 6.0), g.uniform(0.2, 6.0), g.uniform(0.2, 6.0)],
    };
    [c[0], c[1], c[2], e[0], e[1], e[2]]
}

fn rotation(g: &mut Gen) -> [f32; 9] {
    let (a, b, c) = (g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0));
    let (sa, ca, sb, cb, sc, cc) = (petir::Real::sin(a), petir::Real::cos(a), petir::Real::sin(b), petir::Real::cos(b), petir::Real::sin(c), petir::Real::cos(c));
    [
        cb * cc, -cb * sc, sb,
        sa * sb * cc + ca * sc, -sa * sb * sc + ca * cc, -sa * cb,
        -ca * sb * cc + sa * sc, ca * sb * sc + sa * cc, ca * cb,
    ]
}

fn gen_program(g: &mut Gen, kind: i32) -> Vec<f32> {
    let mut pr = Program { p: vec![kind as f32] };
    let world = [40.0f32, 100.0, 180.0][g.below(3) as usize];
    let nobj = 2 + g.below(36) as usize;
    let nclusters = 1 + g.below(4) as usize;
    let centers: Vec<[f32; 3]> = (0..nclusters).map(|_| [g.uniform(-world, world), g.uniform(-world, world), g.uniform(-world, world)]).collect();
    let clustered = g.below(2) == 0;
    let mut in_sys = vec![false; nobj];
    let mut boxes: Vec<[f32; 6]> = Vec::new();
    for id in 0..nobj {
        let b = rand_box(g, world, clustered, &centers);
        pr.op(1, &[id as f32, b[0], b[1], b[2], b[3], b[4], b[5]]);
        boxes.push(b);
    }
    // add a good share of the objects first
    for id in 0..nobj {
        if g.below(10) < 7 {
            let spec = if kind == 0 && g.below(8) == 0 { g.uniform(0.0, 1.0) } else { -1.0 };
            pr.op(2, &[id as f32, spec]);
            in_sys[id] = true;
        }
    }
    let nops = 8 + g.below(63);
    for _ in 0..nops {
        let r = g.below(100);
        let pick_obj = |g: &mut Gen, want_in: bool, in_sys: &Vec<bool>| -> Option<usize> {
            let cands: Vec<usize> = (0..in_sys.len()).filter(|&i| in_sys[i] == want_in).collect();
            if cands.is_empty() { None } else { Some(cands[g.below(cands.len() as u32) as usize]) }
        };
        match r {
            0..=11 => {
                if let Some(id) = pick_obj(g, false, &in_sys) {
                    let spec = if kind == 0 && g.below(6) == 0 { g.uniform(0.0, 1.0) } else { -1.0 };
                    pr.op(2, &[id as f32, spec]);
                    in_sys[id] = true;
                }
            }
            12..=21 => {
                if let Some(id) = pick_obj(g, true, &in_sys) {
                    pr.op(3, &[id as f32]);
                    in_sys[id] = false;
                }
            }
            22..=33 => {
                let id = g.below(nobj as u32) as usize;
                let mut b = rand_box(g, world, clustered, &centers);
                if g.below(3) == 0 {
                    // a small move of the same box
                    let o = boxes[id];
                    b = [o[0] + g.uniform(-3.0, 3.0), o[1] + g.uniform(-3.0, 3.0), o[2] + g.uniform(-3.0, 3.0), o[3], o[4], o[5]];
                }
                boxes[id] = b;
                pr.op(4, &[id as f32, b[0], b[1], b[2], b[3], b[4], b[5], if g.below(6) == 0 { 1.0 } else { 0.0 }]);
            }
            34..=62 => {
                let near = |g: &mut Gen| -> [f32; 3] {
                    let b = boxes[g.below(nobj as u32) as usize];
                    [b[0] + g.uniform(-4.0, 4.0), b[1] + g.uniform(-4.0, 4.0), b[2] + g.uniform(-4.0, 4.0)]
                };
                match g.below(5) {
                    0 => {
                        let p = near(g);
                        pr.op(5, &p);
                    }
                    1 => {
                        let p = near(g);
                        pr.op(6, &[p[0], p[1], p[2], g.uniform(0.5, 40.0), g.uniform(0.5, 40.0), g.uniform(0.5, 40.0)]);
                    }
                    2 => {
                        let p = near(g);
                        let r = rotation(g);
                        let mut a = r.to_vec();
                        a.extend_from_slice(&p);
                        a.extend_from_slice(&[g.uniform(0.5, 40.0), g.uniform(0.5, 40.0), g.uniform(0.5, 40.0)]);
                        pr.op(7, &a);
                    }
                    3 => {
                        // a camera looking at a cluster from a distance
                        let target = near(g);
                        let r = rotation(g);
                        let dist = g.uniform(10.0, 120.0);
                        let mut a = vec![0.0f32; 18];
                        for row in 0..3 {
                            a[row * 4] = r[row * 3];
                            a[row * 4 + 1] = r[row * 3 + 1];
                            a[row * 4 + 2] = r[row * 3 + 2];
                            // camera looks down its -Z axis: place it dist along +Z from the target
                            a[row * 4 + 3] = target[row] + r[row * 3 + 2] * dist;
                        }
                        let h = g.uniform(0.3, 2.0);
                        let w = h * g.uniform(0.7, 1.8);
                        a[12] = -w;
                        a[13] = -h;
                        a[14] = w;
                        a[15] = h;
                        a[16] = g.uniform(0.5, 5.0);
                        a[17] = dist + g.uniform(20.0, 200.0);
                        pr.op(8, &a);
                    }
                    _ => {
                        if kind == 0 {
                            let p = near(g);
                            pr.op(9, &[p[0], p[1], p[2], g.uniform(1.0, 50.0)]);
                        } else {
                            let p = near(g);
                            pr.op(5, &p);
                        }
                    }
                }
            }
            63..=67 => {
                if kind == 0 {
                    pr.op(10, &[g.below(32768) as f32]);
                } else {
                    let w = g.uniform(30.0, 220.0);
                    let c = [g.uniform(-40.0, 40.0), g.uniform(-40.0, 40.0), g.uniform(-40.0, 40.0)];
                    pr.op(13, &[c[0] - w, c[1] - w, c[2] - w, c[0] + w, c[1] + w, c[2] + w, g.uniform(2.0, 20.0)]);
                }
            }
            68..=70 => {
                if kind == 0 {
                    let bounds = [g.uniform(-20.0, 20.0), g.uniform(-20.0, 20.0), g.uniform(-20.0, 20.0), world, world, world];
                    let nb = g.below(25) as usize;
                    let mut a = vec![g.below(32768) as f32];
                    a.extend_from_slice(&bounds);
                    a.push(nb as f32);
                    for _ in 0..nb {
                        let b = rand_box(g, world, clustered, &centers);
                        a.extend_from_slice(&b);
                    }
                    pr.op(11, &a);
                } else {
                    pr.op(14, &[g.uniform(2.0, 20.0), g.uniform(2.0, 20.0), g.uniform(2.0, 20.0)]);
                }
            }
            71..=73 => {
                if kind == 0 {
                    pr.op(12, &[]);
                } else {
                    pr.op(15, &[[2.0f32, 8.0, 64.0, 16384.0][g.below(4) as usize]]);
                }
            }
            74..=79 => {
                if let Some(id) = pick_obj(g, true, &in_sys) {
                    pr.op(16, &[id as f32]);
                }
            }
            80..=84 => {
                if kind == 0 {
                    pr.op(17, &[]);
                } else {
                    let p = [g.uniform(-3.0, 12.0), g.uniform(-3.0, 12.0), g.uniform(-3.0, 12.0)];
                    let a = [g.uniform(-150.0, 150.0), g.uniform(-150.0, 150.0), g.uniform(-150.0, 150.0)];
                    let b = [a[0] + g.uniform(0.0, 40.0), a[1] + g.uniform(0.0, 40.0), a[2] + g.uniform(0.0, 40.0)];
                    let q = [g.uniform(-150.0, 150.0), g.uniform(-150.0, 150.0), g.uniform(-150.0, 150.0)];
                    pr.op(20, &[p[0], p[1], p[2], a[0], a[1], a[2], b[0], b[1], b[2], q[0], q[1], q[2]]);
                }
            }
            85..=86 => {
                if kind == 1 {
                    for id in 0..nobj {
                        if in_sys[id] {
                            pr.op(3, &[id as f32]);
                        }
                    }
                }
                pr.op(18, &[]);
                for s in in_sys.iter_mut() {
                    *s = false;
                }
            }
            87..=88 => {
                if kind == 0 {
                    pr.op(21, &[g.below(6) as f32, g.below(1000) as f32]);
                }
            }
            _ => pr.op(19, &[]),
        }
    }
    pr.p
}

fn compare(c: &mut Check, prog: &[f32], r: &[u32], x: &[u32]) {
    if r == x {
        c.eq(0usize, 0usize, || String::new());
        return;
    }
    // locate the first difference and the operation it is in
    let k = r.iter().zip(x.iter()).position(|(a, b)| a != b).unwrap_or(r.len().min(x.len()));
    let mut marker = 0i32;
    for w in r[..k.min(r.len())].iter().rev() {
        let s = *w as i32;
        if (-1100..=-1000).contains(&s) {
            marker = -s - 1000;
            break;
        }
    }
    c.eq(r.len(), x.len() + 1_000_000, || {
        format!(
            "first difference at word {k} (after op {marker}); rust len {} c++ len {}; rust {:?} c++ {:?}; program {:?}",
            r.len(),
            x.len(),
            &r[k.saturating_sub(4)..(k + 8).min(r.len())],
            &x[k.saturating_sub(4)..(k + 8).min(x.len())],
            &prog[..prog.len().min(400)]
        )
    });
}

fn run_cases(name: &str, kind: i32, cases: usize, seed: u64) {
    let mut g = Gen::new(seed);
    let mut c = Check::new(name);
    let mut words = 0usize;
    for _ in 0..cases {
        let prog = gen_program(&mut g, kind);
        let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let r = run_rust(&prog);
        let mut out = vec![0u32; 4_000_000];
        let n = unsafe { draft_cull_run(prog.as_ptr(), prog.len() as i32, out.as_mut_ptr()) };
        words += r.len();
        compare(&mut c, &prog, &r, &out[..n as usize]);
    }
    println!("{name}: {cases} programs, {words} dump words compared");
    if plant_id() != 0 {
        let bad = c.report();
        println!("PLANTED {bad} mismatching programs of {cases}");
        assert!(bad > 0, "plant {} not caught", plant_id());
    } else {
        c.finish();
    }
}

#[test]
fn aabtree_cull_system_matches_the_cpp() {
    run_cases("AABTreeCullSystemClass programs", 0, TREE_CASES, 0x215_AAB0);
}

#[test]
fn grid_cull_system_matches_the_cpp() {
    run_cases("GridCullSystemClass programs", 1, GRID_CASES, 0x215_6D10);
}
