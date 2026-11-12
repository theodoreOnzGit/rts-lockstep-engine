#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/View.h (WorldToScreenReturn, worldToScreen)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp (worldToScreenTriReturn,
//     iterateDrawablesInRegion)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/W3DConvert.cpp (W3DLogicalScreenToPixelScreen)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/camera.cpp (Get_View_Plane, Project, Update_Frustum, Get_Clip_Planes)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix4.h and matrix3d.cpp (Init_Perspective, operator *, Get_Inverse)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::tactical_view`'s world-to-screen mapping (GitHub #146) against the C++.
//!
//! # Method
//!
//! The C++ side is W3DView's `worldToScreenTriReturn` and `iterateDrawablesInRegion`, View's `worldToScreen`,
//! `W3DLogicalScreenToPixelScreen`, CameraClass's `Project` with its `Update_Frustum` and `Get_Clip_Planes`, cut verbatim
//! by build.rs, with WWMath's own `matrix4.h` and `matrix3d.cpp` (`differential/wrap_world_to_screen.cpp` lists the cuts,
//! the stand-ins and the words). **`Get_Inverse` calls the DirectX SDK's `D3DXMatrixInverse`, which is not in the
//! release**: both sides use the same stand-in (`d3dx_matrix_inverse_stand_in`, a cofactor inverse), so this test checks
//! everything but that SDK routine. Both sides take the same view (its size and origin, the camera's clip planes, view
//! plane and transform) and drawables (0 to 30 positions), run the same random sequence, and write every answer (the
//! projection's result and point as bits, the pixel and the return, the drawables called back in order and the count);
//! the words are compared whole.
//!
//! Each case: a view (a phone held upright, 390x844 and 844x390, the bar's 80 % of them, 800x600's 80 %, 1024x768,
//! 1920x1080, or random from 1 to 4,000 each way), an origin (mostly 0, 0), a near plane of 10 (`setCameraTransform`'s
//! `MAP_XY_FACTOR`) or random, a far plane of 1,200 or 12,000 or random, a view plane from `Set_View_Plane`'s 50 degrees
//! and the view's aspect (or random), and a transform: half `Matrix3D::look_at` from a random eye over a random ground
//! point, half random rows. The points: most near the ground point (on the screen), some anywhere within 5,000, some
//! behind the eye, some past the far plane. Then 1 to 40 steps: `worldToScreenTriReturn`, `worldToScreen`, `Project`,
//! `iterateDrawablesInRegion` over a region (a drag box anywhere on the view or past it; a point a quarter of the time,
//! with or without a picked drawable) or with none, the callback answering by a random mask, and a new size and origin.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches over 20,000 sequences: every operation is a `float` operation in upstream's order, `Init_Perspective`'s
//!   two `double` terms are `double` on both sides, and `REAL_TO_INT` is the x87 `fistp` of the truncated value on both.
//! - The plants all caught (9361-9377). The fewest: 9361 (the near test's `>=`, which needs a point exactly on the near
//!   plane: probably **missed**, as random floats almost never land there) and 9366 (the far test's `>=`, the same:
//!   probably missed). 9365 (the orthogonal inverse for the general one) caught only on the random-row transforms, where
//!   the two inverses differ; on a look-at's both are near the same but should still differ in the last bits often.
//!   9375 (the empty list quirk) only by cases with no drawables and a picked point: about one in 30 cases has none.
//!
//! # Results (2026-11-12, `WORLD_TO_SCREEN_CASES=20000`, the default)
//!
//! **First run** (without the exact cases below): 20,000 sequences, 412,544 steps, 137,697 `worldToScreen` (29,621
//! inside, 33,778 outside, 46,955 invalid), 206,133 region iterations (613,947 callbacks): **0 mismatches**, max ULP 0, as
//! predicted. Plants: 13 of 17 caught; **missed 9361 and 9366** (as predicted: no random point lies exactly on the near or
//! far plane), **9375** (prediction missed: a case with no drawables had no drawable to pick, since the picked one came
//! from the list; upstream's quirk needs `pickDrawable` to find a drawable that `TheGameClient`'s list does not hold) and
//! **9377** (prediction missed: no random point projected exactly onto a region's right edge). The protocol was then
//! fixed, not the plants: one case in eight is now exact (`gen_exact_case`), and the picked drawable may be one outside
//! the list.
//!
//! **Second run** (the protocol as it is now): 20,000 sequences, 383,892 steps, 123,884 `worldToScreen` (29,012 inside,
//! 29,135 outside, 41,493 invalid), 198,051 region iterations (552,197 callbacks): **0 mismatches**, max ULP 0, as
//! predicted. Plants, mismatching sequences of 20,000: 9361 1,674; 9362 5,864; 9363 3,522; 9364 230; 9365 12,832; 9366
//! 1,654; 9367 5,525; 9368 14,756; 9369 11,333; 9370 14,495; 9371 12,397; 9372 8,322; 9373 3,708; 9374 13,158; 9375 991;
//! 9376 15,989; 9377 997. **17 of 17 caught.** 9365's 12,832 shows that the inverse matters in the last bits even on a
//! look-at transform: the SDK's own `D3DXMatrixInverse` (unavailable) could differ from the stand-in by as much.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9361 | `Project`'s near test is `>=` |
//! | 9362 | `Project`'s frustum test misses y |
//! | 9363 | `Init_Perspective`'s `Row[1][2]` divides by the width |
//! | 9364 | `Init_Perspective`'s `Row[2][3]` in `float` |
//! | 9365 | `Update_Frustum` inverts by `Get_Orthogonal_Inverse` |
//! | 9366 | `Project`'s far test is `>=` |
//! | 9367 | `worldToScreenTriReturn` ignores the origin's x |
//! | 9368 | `W3DLogicalScreenToPixelScreen` does not flip y |
//! | 9369 | `worldToScreenTriReturn` calls a point outside the frustum inside |
//! | 9370 | the invalid return's pixel is (1, 1) |
//! | 9371 | `W3DLogicalScreenToPixelScreen` rounds (no truncation) |
//! | 9372 | the region's normalized bottom is from its top |
//! | 9373 | the region test takes points outside the frustum |
//! | 9374 | a region of one pixel is not a point |
//! | 9375 | the picked drawable is called back with no drawables |
//! | 9376 | the count counts every drawable called back |
//! | 9377 | the region's right edge is exclusive |

mod support;

use rts_lockstep_engine::client::tactical_view::{TacticalView, ViewCamera};
use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D, IRegion2D};
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use support::{Check, Gen};

fn bits(f: f32) -> i32 {
    f.to_bits() as i32
}

fn real(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    let mut v = TacticalView::new(&GlobalData::new());
    v.camera = ViewCamera::new();
    v.width = input[0];
    v.height = input[1];
    v.origin = ICoord2D { x: input[2], y: input[3] };
    v.camera.clip = (real(input[4]), real(input[5]));
    v.camera.view_plane = (Vector2::new(real(input[6]), real(input[7])), Vector2::new(real(input[8]), real(input[9])));
    let r = |k: usize| Vector4::new(real(input[10 + k * 4]), real(input[11 + k * 4]), real(input[12 + k * 4]), real(input[13 + k * 4]));
    v.camera.transform = Matrix3D { row: [r(0), r(1), r(2)] };
    let n = input[22] as usize;
    let drawables: Vec<(i32, Coord3D)> = (0..n).map(|i| (i as i32, Coord3D { x: real(input[23 + i * 3]), y: real(input[24 + i * 3]), z: real(input[25 + i * 3]) })).collect();
    for w in input[23 + n * 3..].chunks(7) {
        let p = Coord3D { x: real(w[1]), y: real(w[2]), z: real(w[3]) };
        match w[0] {
            0 => {
                let (r, s) = v.world_to_screen_tri_return(&p);
                out.extend([0, r as i32, s.x, s.y]);
            }
            1 => {
                let (b, s) = v.world_to_screen(&p);
                out.extend([1, i32::from(b), s.x, s.y]);
            }
            2 => {
                let (r, d) = v.camera.project(&Vector3::new(p.x, p.y, p.z));
                out.extend([2, r as i32, bits(d.x), bits(d.y), bits(d.z)]);
            }
            3 | 4 => {
                let region = IRegion2D { lo: ICoord2D { x: w[1], y: w[2] }, hi: ICoord2D { x: w[3], y: w[4] } };
                let picked = (w[5] >= 0 && (w[5] as usize) <= n).then_some(w[5]);
                let mask = w[6] as u32;
                out.push(3);
                let mut called = Vec::new();
                let count = v.iterate_drawables_in_region((w[0] == 3).then_some(&region), drawables.iter().copied(), |_| picked, |d| {
                    called.push(d);
                    (mask >> (d % 31)) & 1 != 0
                });
                out.extend(called);
                out.extend([-1, count]);
            }
            _ => {
                v.width = w[1];
                v.height = w[2];
                v.origin = ICoord2D { x: w[3], y: w[4] };
            }
        }
    }
    out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 200_000];
    let n = unsafe { cpp::ref_world_to_screen_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn view_size(g: &mut Gen) -> (i32, i32) {
    match g.below(8) {
        0 => (390, 844),
        1 => (390, (844.0f32 * 0.8f32) as i32),
        2 => (844, 390),
        3 => (800, 480),
        4 => (1024, 768),
        5 => (1920, 1080),
        _ => (1 + g.below(4000) as i32, 1 + g.below(4000) as i32),
    }
}

/// A view whose numbers are exact (one case in eight): the identity transform (the camera at the origin looking down
/// -z), a 400x400 view, the view plane -1 .. 1, a near plane of 1, 2 or 4 and a far plane twice it, and points on a grid
/// at the near plane, the far plane and between them; the regions' edges on multiples of 100 pixels. Points then land
/// exactly on the near and far planes and on a region's edges, which random floats never do.
fn gen_exact_case(g: &mut Gen) -> Vec<i32> {
    let near = [1.0f32, 2.0, 4.0][g.below(3) as usize];
    let far = near * 2.0;
    let mut v = vec![400, 400, 0, 0, bits(near), bits(far), bits(-1.0), bits(-1.0), bits(1.0), bits(1.0)];
    for row in Matrix3D::identity().row {
        v.extend([bits(row.x), bits(row.y), bits(row.z), bits(row.w)]);
    }
    let point = |g: &mut Gen| -> [f32; 3] {
        let z = [-near, -far, -near * 1.5f32, -near * 0.5f32][g.below(4) as usize];
        [(g.below(9) as f32 - 4.0) * 0.25f32 * -z, (g.below(9) as f32 - 4.0) * 0.25f32 * -z, z]
    };
    let n = if g.below(4) == 0 { 0 } else { 1 + g.below(12) };
    v.push(n as i32);
    for _ in 0..n {
        let p = point(g);
        v.extend([bits(p[0]), bits(p[1]), bits(p[2])]);
    }
    for _ in 0..1 + g.below(20) {
        let edge = |g: &mut Gen| g.below(5) as i32 * 100;
        match g.below(4) {
            0 => {
                let p = point(g);
                v.extend([g.below(3) as i32, bits(p[0]), bits(p[1]), bits(p[2]), 0, 0, 0]);
            }
            1 => {
                let (a, b) = (edge(g), edge(g));
                v.extend([3, a, b, a, b, g.below(n + 1) as i32, g.u32() as i32]);
            }
            _ => {
                let (ax, ay, bx, by) = (edge(g), edge(g), edge(g), edge(g));
                v.extend([3, ax.min(bx), ay.min(by), ax.max(bx), ay.max(by), -1, g.u32() as i32]);
            }
        }
    }
    v
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    if g.below(8) == 0 {
        return gen_exact_case(g);
    }
    let (w, h) = view_size(g);
    let (ox, oy) = if g.below(5) == 0 { (g.below(300) as i32, g.below(300) as i32) } else { (0, 0) };
    let near = if g.below(3) == 0 { g.uniform(0.5, 50.0) } else { 10.0 };
    let far = [1200.0f32, 12000.0, g.uniform(60.0, 20000.0)][g.below(3) as usize];
    let (vmin, vmax) = if g.below(4) == 0 {
        (Vector2::new(g.uniform(-2.0, -0.05), g.uniform(-2.0, -0.05)), Vector2::new(g.uniform(0.05, 2.0), g.uniform(0.05, 2.0)))
    } else {
        // Set_View_Plane(DEG_TO_RADF(50)) then Set_Aspect_Ratio(width / height)
        let mut c = ViewCamera::new();
        c.set_aspect_ratio(w as f32 / h as f32);
        c.view_plane
    };
    let mut v = vec![w, h, ox, oy, bits(near), bits(far), bits(vmin.x), bits(vmin.y), bits(vmax.x), bits(vmax.y)];
    let target = Vector3::new(g.uniform(-500.0, 3000.0), g.uniform(-500.0, 3000.0), g.uniform(-20.0, 60.0));
    let eye = Vector3::new(target.x + g.uniform(-300.0, 300.0), target.y + g.uniform(-400.0, 50.0), target.z + g.uniform(50.0, 600.0));
    let m = if g.below(2) == 0 {
        let mut m = Matrix3D::identity();
        m.look_at(&eye, &target, 0.0);
        m
    } else {
        let mut r = || Vector4::new(g.uniform(-2.0, 2.0), g.uniform(-2.0, 2.0), g.uniform(-2.0, 2.0), g.uniform(-1000.0, 1000.0));
        Matrix3D { row: [r(), r(), r()] }
    };
    for row in m.row {
        v.extend([bits(row.x), bits(row.y), bits(row.z), bits(row.w)]);
    }
    let point = |g: &mut Gen| -> [f32; 3] {
        match g.below(10) {
            0..=5 => [target.x + g.uniform(-250.0, 250.0), target.y + g.uniform(-250.0, 250.0), target.z + g.uniform(-10.0, 40.0)],
            6 | 7 => [g.uniform(-5000.0, 5000.0), g.uniform(-5000.0, 5000.0), g.uniform(-100.0, 400.0)],
            // behind the eye
            8 => [eye.x - (target.x - eye.x) * g.uniform(0.0, 2.0), eye.y - (target.y - eye.y) * g.uniform(0.0, 2.0), eye.z - (target.z - eye.z) * g.uniform(0.0, 2.0)],
            // past the far plane
            _ => [target.x + (target.x - eye.x) * g.uniform(5.0, 60.0), target.y + (target.y - eye.y) * g.uniform(5.0, 60.0), target.z],
        }
    };
    let n = if g.below(30) == 0 { 0 } else { 1 + g.below(30) };
    v.push(n as i32);
    for _ in 0..n {
        let p = point(g);
        v.extend([bits(p[0]), bits(p[1]), bits(p[2])]);
    }
    let (mut w, mut h) = (w, h);
    let steps = 1 + g.below(40);
    for _ in 0..steps {
        let px = |g: &mut Gen, d: i32| g.below((d + 401) as u32) as i32 - 200;
        match g.below(12) {
            0..=2 => {
                let p = point(g);
                v.extend([0, bits(p[0]), bits(p[1]), bits(p[2]), 0, 0, 0]);
            }
            3 => {
                let p = point(g);
                v.extend([1, bits(p[0]), bits(p[1]), bits(p[2]), 0, 0, 0]);
            }
            4 => {
                let p = point(g);
                v.extend([2, bits(p[0]), bits(p[1]), bits(p[2]), 0, 0, 0]);
            }
            5..=9 => {
                let (ax, ay) = (px(g, w), px(g, h));
                let (bx, by) = if g.below(4) == 0 { (ax, ay) } else { (px(g, w), px(g, h)) };
                let (lo, hi) = ((ax.min(bx), ay.min(by)), (ax.max(bx), ay.max(by)));
                // (n: a drawable outside the list, as pickDrawable could give; see the wrapper)
                let pick = if g.below(3) == 0 { -1 } else { g.below(n + 1) as i32 };
                v.extend([3, lo.0, lo.1, hi.0, hi.1, pick, g.u32() as i32]);
            }
            10 => v.extend([4, 0, 0, 0, 0, -1, g.u32() as i32]),
            _ => {
                (w, h) = view_size(g);
                let (ox, oy) = if g.below(3) == 0 { (g.below(300) as i32, g.below(300) as i32) } else { (0, 0) };
                v.extend([5, w, h, ox, oy, 0, 0]);
            }
        }
    }
    v
}

#[test]
fn the_view_maps_the_world_to_the_screen_as_upstream() {
    let cases: u64 = std::env::var("WORLD_TO_SCREEN_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x146_2021);
    let mut whole = Check::new("the view's world-to-screen mapping");
    let (mut steps, mut tri, mut inside, mut outside, mut invalid, mut regions, mut called) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        let n = input[22] as usize;
        steps += (input[23 + n * 3..].len() / 7) as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        // (the C++ side's answers, counted)
        let mut k = 0;
        while k < c.len() {
            match c[k] {
                0 | 1 => {
                    tri += 1;
                    match (c[k], c[k + 1]) {
                        (0, 0) | (1, 1) => inside += 1,
                        (0, 1) => outside += 1,
                        (0, _) => invalid += 1,
                        _ => {}
                    }
                    k += 4;
                }
                2 => k += 5,
                _ => {
                    regions += 1;
                    k += 1;
                    while c[k] != -1 {
                        called += 1;
                        k += 1;
                    }
                    k += 2;
                }
            }
        }
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(6);
            println!("sequence {i} first difference at word {k}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 8).min(r.len())], &c[k0.min(c.len())..(k + 8).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps} steps: {tri} worldToScreen ({inside} inside, {outside} outside, {invalid} invalid), {regions} region iterations ({called} callbacks)");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
