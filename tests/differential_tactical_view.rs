#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/View.h (the size and origin accessors)
//   GeneralsMD/Code/GameEngine/Source/GameClient/View.cpp (getScreenCornerWorldPointsAtZ)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp (setHeight, setWidth, setOrigin,
//     getPickRay, screenToWorldAtZ)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/W3DConvert.cpp (PixelScreenToW3DLogicalScreen)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/camera.h and camera.cpp (the viewport, the view plane, the aspect,
//     Un_Project)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::tactical_view`'s height, origin and screen-to-world mapping (GitHub #138) against the C++.
//!
//! # Method
//!
//! The C++ side is W3DView's `setHeight`, `setWidth`, `setOrigin`, `getPickRay` and `screenToWorldAtZ`, View's
//! `getScreenCornerWorldPointsAtZ` and accessors, `PixelScreenToW3DLogicalScreen`, and CameraClass's viewport, view
//! plane, aspect and `Un_Project`, cut verbatim by build.rs (`differential/wrap_tactical_view.cpp` lists the cuts and
//! the words). Both sides start a camera as CameraClass's constructor does, give it a transform and a far clip plane,
//! then run the same random sequence and write every answer and, after each step, the view's size and origin and the
//! camera's aspect, view plane and viewport, floats as bits; the words are compared whole.
//!
//! Each case: a display (a phone held upright, 390x844 and 844x390, the 800x600 layout, 1024x768, 1920x1080, or random
//! from 1 to 4,000 each way), the view's size (the display's, or random), a far plane of 1,200 or 12,000 or random, and
//! a transform: half of them `Matrix3D::look_at` from a random eye over a random ground point (a real view's), half
//! random rows. Then 1 to 40 steps: `setHeight` (the control bar's 80 % of the display, the whole display, or random),
//! `setWidth`, `setOrigin` (mostly 0, 0), a new display size, the pick ray, `screenToWorldAtZ` and the four corners at
//! heights from -50 to 300, at pixels inside the display and up to 200 past its edges.
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - 0 mismatches over 20,000 sequences: every operation is a `float` operation in upstream's order, and the one `tan`
//!   (the view plane, in `double`) is the interposed petir one on both sides.
//! - Plants 9271-9278 all caught. 9273 (`setOrigin` skips its `setHeight`) the fewest: it needs an origin with a y
//!   other than the last `setHeight`'s, which the generator makes in about one step in twelve.
//!
//! # Results (2026-11-11, `TACTICAL_VIEW_CASES=20000`, the default)
//!
//! 20,000 sequences, 412,481 steps (68,424 `setHeight`, 68,720 pick rays, 102,973 `screenToWorldAtZ`, 68,996 corner
//! sets): **0 mismatches** on the first run, max ULP 0, as predicted.
//!
//! Planted bugs (same sequences), mismatching sequences of 20,000: 9271 17,011; 9272 14,181; 9273 13,183; 9274 9,546;
//! 9275 15,857; 9276 19,326; 9277 13,059; 9278 17,513. **8 of 8 caught.** **Prediction missed:** 9273 was not the
//! fewest (9274 was). `setOrigin`'s `setHeight` also recomputes the viewport's bottom from the display's current height,
//! which no earlier step had applied when the display changed or the setup's view height was not the display's, so even
//! an origin of (0, 0) shows the plant; 9274 needs an origin with a y other than 0 and a pick ray after it.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9271 | `setHeight`'s aspect divides by the display's height, not the view's |
//! | 9272 | `setWidth`'s field of view scales by the display's width, not the view's |
//! | 9273 | `setOrigin` does not call `setHeight` |
//! | 9274 | the pick ray ignores the origin's y |
//! | 9275 | the pick ray maps y over the display's height, not the view's |
//! | 9276 | `screenToWorldAtZ`'s y is `Find_X_At_Z` |
//! | 9277 | the corners' height is the display's, not the view's |
//! | 9278 | the corners' lower left is at the right edge |

mod support;

use rts_lockstep_engine::client::tactical_view::{TacticalView, ViewCamera};
use rts_lockstep_engine::common::base_type::ICoord2D;
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
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
    let (mut dw, mut dh) = (input[0], input[1]);
    let mut v = TacticalView::new(&GlobalData::new());
    // CameraClass::CameraClass, then the far plane and the transform
    v.camera = ViewCamera::new();
    v.camera.clip.1 = real(input[4]);
    let r = |k: usize| Vector4::new(real(input[5 + k * 4]), real(input[6 + k * 4]), real(input[7 + k * 4]), real(input[8 + k * 4]));
    v.camera.transform = Matrix3D { row: [r(0), r(1), r(2)] };
    v.width = input[2];
    v.height = input[3];
    for w in input[18..].chunks(4) {
        match w[0] {
            0 => v.set_width(w[1], dw),
            1 => v.set_height(w[1], dh),
            2 => v.set_origin(w[1], w[2], dw, dh),
            3 => (dw, dh) = (w[1], w[2]),
            4 => {
                let (a, b) = v.get_pick_ray(ICoord2D { x: w[1], y: w[2] });
                out.extend([4, bits(a.x), bits(a.y), bits(a.z), bits(b.x), bits(b.y), bits(b.z)]);
            }
            5 => {
                let p = v.screen_to_world_at_z(ICoord2D { x: w[1], y: w[2] }, real(w[3]));
                out.extend([5, bits(p.x), bits(p.y), bits(p.z)]);
            }
            _ => {
                out.push(6);
                for p in v.get_screen_corner_world_points_at_z(real(w[1])) {
                    out.extend([bits(p.x), bits(p.y), bits(p.z)]);
                }
            }
        }
        let c = &v.camera;
        out.extend([100, v.width, v.height, v.origin.x, v.origin.y, bits(c.aspect_ratio)]);
        out.extend([bits(c.view_plane.0.x), bits(c.view_plane.0.y), bits(c.view_plane.1.x), bits(c.view_plane.1.y)]);
        out.extend([bits(c.viewport.0.x), bits(c.viewport.0.y), bits(c.viewport.1.x), bits(c.viewport.1.y)]);
    }
    out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 100_000];
    let n = unsafe { cpp::ref_tactical_view_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn display(g: &mut Gen) -> (i32, i32) {
    match g.below(7) {
        0 => (390, 844),
        1 => (844, 390),
        2 => (800, 600),
        3 => (1024, 768),
        4 => (1920, 1080),
        _ => (1 + g.below(4000) as i32, 1 + g.below(4000) as i32),
    }
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let (dw, dh) = display(g);
    let (vw, vh) = if g.below(3) == 0 { (1 + g.below(4000) as i32, 1 + g.below(4000) as i32) } else { (dw, dh) };
    let far = [1200.0f32, 12000.0, g.uniform(50.0, 20000.0)][g.below(3) as usize];
    let mut v = vec![dw, dh, vw, vh, bits(far)];
    let m = if g.below(2) == 0 {
        // a real view: an eye above and behind a ground point, looking at it
        let target = Vector3::new(g.uniform(-500.0, 3000.0), g.uniform(-500.0, 3000.0), g.uniform(-20.0, 60.0));
        let eye = Vector3::new(target.x + g.uniform(-300.0, 300.0), target.y + g.uniform(-400.0, 50.0), target.z + g.uniform(50.0, 600.0));
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
    v.resize(18, 0);
    let (mut dw, mut dh) = (dw, dh);
    let steps = 1 + g.below(40);
    for _ in 0..steps {
        let px = |g: &mut Gen, d: i32| g.below((d + 401) as u32) as i32 - 200;
        let z = |g: &mut Gen| bits(g.uniform(-50.0, 300.0));
        let s = match g.below(12) {
            0..=1 => [1, [(dh as f32 * 0.80f32) as i32, dh, 1 + g.below(4000) as i32][g.below(3) as usize], 0, 0],
            2 => [0, if g.below(2) == 0 { dw } else { 1 + g.below(4000) as i32 }, 0, 0],
            3 => {
                if g.below(2) == 0 {
                    [2, 0, 0, 0]
                } else {
                    [2, g.below(300) as i32, g.below(300) as i32, 0]
                }
            }
            4 => {
                (dw, dh) = display(g);
                [3, dw, dh, 0]
            }
            5..=6 => [4, px(g, dw), px(g, dh), 0],
            7..=9 => [5, px(g, dw), px(g, dh), z(g)],
            _ => [6, z(g), 0, 0],
        };
        v.extend(s);
    }
    v
}

#[test]
fn the_tactical_view_maps_the_screen_as_upstream() {
    let cases: u64 = std::env::var("TACTICAL_VIEW_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x138_1380);
    let mut whole = Check::new("the tactical view's size, pick ray and corners");
    let (mut steps, mut picks, mut at_z, mut corners, mut heights) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        for s in input[18..].chunks(4) {
            steps += 1;
            match s[0] {
                1 => heights += 1,
                4 => picks += 1,
                5 => at_z += 1,
                6 => corners += 1,
                _ => {}
            }
        }
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(6);
            println!("sequence {i} first difference at word {k}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 12).min(r.len())], &c[k0.min(c.len())..(k + 12).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps} steps: {heights} setHeight, {picks} pick rays, {at_z} screenToWorldAtZ, {corners} corner sets");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
