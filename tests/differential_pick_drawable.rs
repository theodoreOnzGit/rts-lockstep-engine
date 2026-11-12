#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/View.h (enum PickType)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp (W3DView::pickDrawable)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp (RTS3DScene::castRay)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.h, hlod.cpp, boxrobj.cpp (the collision types; Cast_Ray)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathline.cpp (the box collides, Test_Aligned_Box)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp (collision types,
//     setSelectable)
//   GeneralsMD/Code/GameEngine/Source/GameClient/SelectionInfo.cpp (getPickTypesForContext)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::pick_drawable` (GitHub #148) against the C++.
//!
//! # Method
//!
//! The C++ side is the cuts `differential/wrap_pick_drawable.cpp` lists, verbatim, on the real WWMath line segment, cast
//! result, boxes and sphere and WW3D2's `RayCollisionTestClass`, with stand-in render objects (an HLod of box models),
//! scene list, windows and drawables. Both sides get the same random scene: 0 to 8 render objects (a drawable or none;
//! collision bits from the pick types, `COLL_TYPE_ALL` alone, 0, or a random byte; really visible or not; a bounding
//! sphere that holds the models or a random one, so the quick sphere test both passes and rejects), each with 0 to 3
//! top-LOD models and 0 to 2 additional ones (axis-aligned or oriented boxes, a rotation about z, or about z then x, or a
//! random matrix; random collision bits; sometimes animation-hidden). Then steps: `pickDrawable` (rays through the scene
//! from far away, from inside a box, zero-length; force attack; every pick type and random ones; 0 to 3 windows under the
//! cursor, see-through or not), `castRay` (the clipped ray, its direction and length and the object hit compared bit for
//! bit), the two `Collide`s on any model's box with a random starting result, the model draw's collision types and
//! `setSelectable` on random bits, and `getPickTypesForContext` on random commands and selections.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches over 20,000 sequences: the same `float` operations in the same order (the slab test, the
//!   transpose product, `Normalize` through the portable `Inv_Sqrt`, `Compute_Point`).
//! - Every plant caught: 9391 (invisible objects tested) and 9393 (the sphere test skipped) by the picks; 9392 (the ray not
//!   shortened) whenever two boxes lie on one ray in list order far-then-near; 9394 (the smallest `maxt`) by most
//!   collides; 9395 (the basis not transposed) by the rotated boxes; 9396 (StartBad ignored) by the rays starting inside a
//!   box; 9397 (only the top window checked) by the window chains; 9398 (`COLL_TYPE_ALL` not forced) by kinds 3 and 4;
//!   9399 (the dead keep their type), 9406 (setSelectable clears everything) and 9400, 9407 (the pick types) by kinds 3 to
//!   5.
//!
//! # Results (2026-11-12, `PICK_DRAWABLE_CASES=20000`, the default)
//!
//! 20,000 sequences: 51,181 picks (7,244 found a drawable), 51,461 casts (11,545 hit), 51,595 collides (5,084 true,
//! 9,434 with `StartBad` after): **0 mismatches** on the first run, as predicted. Plants, mismatching sequences of
//! 20,000: 9391 1,482; 9392 6,712; 9393 2,915; 9394 10,198; 9395 4,954; 9396 447; 9397 604; 9398 17,116; 9399 845; 9400
//! 13,142; 9406 8,202; 9407 2,824. **12 of 12 caught**, as predicted.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 9391 | `castRay` tests objects that are not really visible |
//! | 9392 | `castRay` does not shorten the ray after a hit (the last hit wins, not the nearest) |
//! | 9393 | `castRay` skips the quick sphere test |
//! | 9394 | `Test_Aligned_Box` takes the smallest `maxt` |
//! | 9395 | the OBBox collide multiplies by the basis, not its transpose |
//! | 9396 | a box's `Cast_Ray` ignores `StartBad` |
//! | 9397 | `pickDrawable` checks only the window under the cursor, not its parents |
//! | 9398 | `Set_Collision_Type` does not force `COLL_TYPE_ALL` |
//! | 9399 | the model draw keeps a dead object's collision type |
//! | 9400 | `getPickTypesForContext` does not add `FORCEATTACKABLE` in force-attack mode |
//! | 9406 | `setSelectable(FALSE)` clears every collision bit |
//! | 9407 | `getPickTypesForCurrentSelection` adds shrubbery without force-attack mode |

mod support;

use rts_lockstep_engine::client::pick_drawable::*;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

fn rb(i: i32) -> f32 {
    f32::from_bits(i as u32)
}
fn bits(f: f32) -> i32 {
    f.to_bits() as i32
}
fn v3(w: &[i32]) -> Vector3 {
    Vector3::new(rb(w[0]), rb(w[1]), rb(w[2]))
}
fn push_v(out: &mut Vec<i32>, v: &Vector3) {
    out.extend([bits(v.x), bits(v.y), bits(v.z)]);
}

/// The scene from the input words, and the flat list of its models; returns the index of the first step.
fn scene(input: &[i32]) -> (Vec<SceneRenderObj<i32>>, Vec<BoxRenderObj>, usize) {
    let mut k = 0;
    let n = input[k] as usize;
    k += 1;
    let (mut list, mut flat) = (Vec::new(), Vec::new());
    for _ in 0..n {
        let w = &input[k..k + 9];
        k += 9;
        let mut models = Vec::new();
        for _ in 0..(w[7] + w[8]) {
            let b = &input[k..k + 18];
            k += 18;
            let basis = (b[0] != 0).then(|| {
                let mut m = [[0.0f32; 3]; 3];
                for i in 0..3 {
                    for j in 0..3 {
                        m[i][j] = rb(b[9 + 3 * i + j]);
                    }
                }
                m
            });
            let m = BoxRenderObj { collision_type: b[1] & COLL_TYPE_MASK, animation_hidden: b[2] != 0, center: v3(&b[3..6]), extent: v3(&b[6..9]), basis };
            models.push(m);
            flat.push(m);
        }
        list.push(SceneRenderObj { drawable: (w[0] >= 0).then_some(w[0]), collision_type: w[1] & COLL_TYPE_MASK, really_visible: w[2] != 0, sphere_center: v3(&w[3..6]), sphere_radius: rb(w[6]), models });
    }
    (list, flat, k)
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    let (list, flat, k) = scene(input);
    for s in input[k..].chunks(20) {
        if s.len() < 20 {
            break;
        }
        let w = &s[1..];
        match s[0] {
            0 => {
                let chain: Vec<bool> = (0..w[8] as usize).map(|i| w[9 + i] != 0).collect();
                let d = pick_drawable_on_ray(chain, v3(&w[0..3]), v3(&w[3..6]), w[6] != 0, w[7], &list);
                out.extend([0, d.unwrap_or(-2)]);
            }
            1 => {
                let mut rt = RayTest::new(LineSeg::new(v3(&w[0..3]), v3(&w[3..6])), CastResult::default(), coll_type::ALL, false, false);
                let hit = cast_ray(&list, &mut rt, w[6] != 0, w[7]);
                out.extend([1, i32::from(hit), rt.collided.map_or(-1, |i| i as i32)]);
                push_v(&mut out, &rt.ray.p1);
                push_v(&mut out, &rt.ray.dir);
                out.push(bits(rt.ray.length));
            }
            2 => {
                let line = LineSeg::new(v3(&w[1..4]), v3(&w[4..7]));
                let mut res = CastResult { fraction: rb(w[8]), start_bad: w[9] != 0, compute_contact_point: w[10] != 0, ..CastResult::default() };
                let r = if flat.is_empty() {
                    false
                } else {
                    let m = &flat[w[7] as u32 as usize % flat.len()];
                    if w[0] == 0 {
                        collide_aabox(&line, &m.center, &m.extent, &mut res)
                    } else {
                        let basis = m.basis.unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
                        collide_obbox(&line, &m.center, &m.extent, &basis, &mut res)
                    }
                };
                out.extend([2, i32::from(r), i32::from(res.start_bad), bits(res.fraction)]);
                push_v(&mut out, &res.normal);
                push_v(&mut out, &res.contact_point);
            }
            3 => {
                let t = CollisionKinds { selectable: w[1] & 1 != 0, shrubbery: w[1] & 2 != 0, mine: w[1] & 4 != 0, forceattackable: w[1] & 8 != 0, click_through: w[1] & 16 != 0 };
                let o = CollisionObject { bridge: w[3] & 1 != 0, bridge_tower: w[3] & 2 != 0, structure: w[3] & 4 != 0, rubble: w[4] != 0, effectively_dead: w[5] != 0 };
                out.extend([3, model_draw_collision_type(w[0] & COLL_TYPE_MASK, &t, (w[2] != 0).then_some(&o))]);
            }
            4 => out.extend([4, set_selectable(w[0] & COLL_TYPE_MASK, w[1] != 0)]),
            _ => {
                let n = (w[4].clamp(0, 4)) as usize;
                let flags: Vec<Option<bool>> = (0..n).map(|i| (w[5 + i] >= 0).then_some(w[5 + i] == 1)).collect();
                let t = get_pick_types_for_context(w[0] != 0, (w[1] != 0).then_some(w[2] as u32), |f| get_pick_types_for_current_selection(f, w[3] != 0, flags.iter().copied()));
                out.extend([5, t as i32]);
            }
        }
    }
    out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 200_000];
    let n = unsafe { cpp::ref_pick_drawable_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn coll_bits(g: &mut Gen) -> i32 {
    match g.below(9) {
        0 => coll_type::ALL,
        1 => 0,
        2 => g.u32() as i32,
        3 => coll_type::ALL | pick_type::SHRUBBERY,
        4 => coll_type::ALL | pick_type::MINES,
        5 => coll_type::ALL | pick_type::FORCEATTACKABLE,
        _ => coll_type::ALL | pick_type::SELECTABLE,
    }
}

fn basis(g: &mut Gen) -> [[f32; 3]; 3] {
    use rts_lockstep_engine::ww::wwmath::{cos, sin};
    let a = g.uniform(-3.2, 3.2);
    let (s, c) = (sin(a), cos(a));
    let z = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
    match g.below(4) {
        0 => {
            let b = g.uniform(-3.2, 3.2);
            let (s2, c2) = (sin(b), cos(b));
            let x = [[1.0, 0.0, 0.0], [0.0, c2, -s2], [0.0, s2, c2]];
            let mut m = [[0.0f32; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    m[i][j] = z[i][0] * x[0][j] + z[i][1] * x[1][j] + z[i][2] * x[2][j];
                }
            }
            m
        }
        1 => [[g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)], [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)], [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)]],
        _ => z,
    }
}

fn point(g: &mut Gen, r: f32) -> [f32; 3] {
    [g.uniform(-r, r), g.uniform(-r, r), g.uniform(-r, r)]
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let n = g.below(9);
    let mut v = vec![n as i32];
    let mut centres: Vec<([f32; 3], [f32; 3])> = Vec::new();
    for o in 0..n {
        let id = if g.below(6) == 0 { -1 } else { 10 + o as i32 };
        let ntop = g.below(4);
        let nadd = if g.below(3) == 0 { g.below(3) } else { 0 };
        let mut models = Vec::new();
        let mut words = Vec::new();
        for _ in 0..ntop + nadd {
            let oriented = g.below(2) == 0;
            let c = point(g, 40.0);
            let e = [g.uniform(0.3, 8.0), g.uniform(0.3, 8.0), g.uniform(0.3, 8.0)];
            let b = if oriented { basis(g) } else { [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] };
            let cb = match g.below(6) {
                0 => coll_bits(g),
                _ => coll_type::ALL,
            };
            let hidden = g.below(12) == 0;
            words.extend([i32::from(oriented), cb, i32::from(hidden)]);
            words.extend(c.iter().chain(e.iter()).map(|&f| bits(f)));
            words.extend(b.iter().flatten().map(|&f| bits(f)));
            models.push(BoxRenderObj { collision_type: cb, animation_hidden: hidden, center: Vector3::new(c[0], c[1], c[2]), extent: Vector3::new(e[0], e[1], e[2]), basis: oriented.then_some(b) });
            centres.push((c, e));
        }
        let (sc, sr) = if g.below(5) == 0 {
            (Vector3::new(g.uniform(-40.0, 40.0), g.uniform(-40.0, 40.0), g.uniform(-40.0, 40.0)), g.uniform(0.0, 20.0))
        } else {
            bounding_sphere(&models)
        };
        v.extend([id, coll_bits(g), i32::from(g.below(7) != 0), bits(sc.x), bits(sc.y), bits(sc.z), bits(sr), ntop as i32, nadd as i32]);
        v.extend(words);
    }
    let ray = |g: &mut Gen, centres: &[([f32; 3], [f32; 3])]| -> ([f32; 3], [f32; 3]) {
        let target = if !centres.is_empty() && g.below(3) != 0 { centres[g.below(centres.len() as u32) as usize].0 } else { point(g, 40.0) };
        match g.below(8) {
            0 if !centres.is_empty() => {
                // from inside (or near) a box
                let (c, e) = centres[g.below(centres.len() as u32) as usize];
                let s = [c[0] + g.uniform(-e[0], e[0]), c[1] + g.uniform(-e[1], e[1]), c[2] + g.uniform(-e[2], e[2])];
                (s, [s[0] + (target[0] - s[0]) * 3.0, s[1] + (target[1] - s[1]) * 3.0, s[2] + (target[2] - s[2]) * 3.0])
            }
            1 => {
                let s = point(g, 60.0);
                (s, s)
            }
            _ => {
                let s = point(g, 150.0);
                let f = g.uniform(0.5, 4.0);
                (s, [s[0] + (target[0] - s[0]) * f, s[1] + (target[1] - s[1]) * f, s[2] + (target[2] - s[2]) * f])
            }
        }
    };
    for _ in 0..1 + g.below(30) {
        let mut s = [0i32; 20];
        let kind = g.below(6);
        s[0] = kind as i32;
        match kind {
            0 | 1 => {
                let (a, b) = ray(g, &centres);
                for i in 0..3 {
                    s[1 + i] = bits(a[i]);
                    s[4 + i] = bits(b[i]);
                }
                s[7] = i32::from(g.below(2) == 0);
                s[8] = match g.below(6) {
                    0 => g.u32() as i32,
                    1 => -1,
                    2 => pick_type::ALL_DRAWABLES,
                    3 => pick_type::SHRUBBERY | pick_type::SELECTABLE,
                    _ => pick_type::SELECTABLE,
                };
                if kind == 0 {
                    s[9] = if g.below(3) == 0 { 1 + g.below(3) as i32 } else { 0 };
                    for i in 0..3 {
                        s[10 + i] = i32::from(g.below(3) != 0);
                    }
                }
            }
            2 => {
                let (a, b) = ray(g, &centres);
                s[1] = g.below(2) as i32;
                for i in 0..3 {
                    s[2 + i] = bits(a[i]);
                    s[5 + i] = bits(b[i]);
                }
                s[8] = g.u32() as i32;
                s[9] = bits(match g.below(3) {
                    0 => g.uniform(0.0, 1.0),
                    1 => g.uniform(0.0, 4.0),
                    _ => 1.0,
                });
                s[10] = i32::from(g.below(6) == 0);
                s[11] = i32::from(g.below(2) == 0);
            }
            3 => {
                s[1] = coll_bits(g);
                s[2] = g.below(32) as i32;
                s[3] = g.below(2) as i32;
                s[4] = g.below(8) as i32;
                s[5] = i32::from(g.below(3) == 0);
                s[6] = i32::from(g.below(3) == 0);
            }
            4 => {
                s[1] = coll_bits(g);
                s[2] = g.below(2) as i32;
            }
            _ => {
                s[1] = g.below(2) as i32;
                s[2] = g.below(2) as i32;
                s[3] = match g.below(3) {
                    0 => 0x10,
                    1 => 0x800,
                    _ => g.u32() as i32,
                };
                s[4] = g.below(2) as i32;
                s[5] = g.below(5) as i32;
                for i in 0..4 {
                    s[6 + i] = g.below(3) as i32 - 1;
                }
            }
        }
        v.extend(s);
    }
    v
}

#[test]
fn pick_drawable_matches_upstream() {
    let cases: u64 = std::env::var("PICK_DRAWABLE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x148_2207);
    let mut whole = Check::new("pickDrawable, castRay and the collides");
    let (mut picks, mut picked, mut casts, mut hits, mut collides, mut collided, mut start_bad) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        // (tallies from the C++'s words)
        let mut k = 0;
        while k < c.len() {
            match c[k] {
                0 => {
                    picks += 1;
                    picked += u64::from(c[k + 1] != -2);
                    k += 2;
                }
                1 => {
                    casts += 1;
                    hits += u64::from(c[k + 1] != 0);
                    k += 10;
                }
                2 => {
                    collides += 1;
                    collided += u64::from(c[k + 1] != 0);
                    start_bad += u64::from(c[k + 2] != 0);
                    k += 10;
                }
                _ => k += 2,
            }
        }
        if r != c && !planted {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(4);
            println!("sequence {i} first difference at word {k}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 8).min(r.len())], &c[k0.min(c.len())..(k + 8).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences: {picks} picks ({picked} found a drawable), {casts} casts ({hits} hit), {collides} collides ({collided} true, {start_bad} StartBad)");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
