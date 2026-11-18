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
//   GeneralsMD/Code/GameEngine/Source/GameClient/Display.cpp (#200: Display::setDisplayMode, setWidth, setHeight)
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
//!
//! # `Display::setDisplayMode` (GitHub #200)
//!
//! The second test, [`set_display_mode_rescales_the_view_as_upstream`], adds step kind 7, `TheDisplay->setDisplayMode(a,
//! b, 32, TRUE)` (Display.cpp:146, with its `setWidth` and `setHeight`, cut verbatim; `TheMouse` NULL), against
//! [`Display::set_display_mode`]. Its own generator (seed `0x200_2000`, so the first test's sequences are unchanged): the
//! first test's setup, then 1 to 40 steps, a third of them a new display size (a listed display, random from 1 to 4,000
//! each way, the same size, doubled or halved), the rest `setHeight` (80 % or 77 % of the display truncated as the bar
//! asks, the whole display, random), `setWidth`, `setOrigin` (random within the display) and pick rays. The words after
//! each step are the first test's (the view's size and origin, the camera's aspect, view plane and viewport), so the
//! rescale is compared through all three of `setWidth`, `setHeight` and `setOrigin`. The C++ calls are serialised (the
//! wrapper's state is global and the two tests run on two threads).
//!
//! *Predictions (2026-11-18, before the first run):* 0 mismatches: the same `float` operations in the same order, and
//! the `float`-to-`Int` truncation is `cvttss2si` on both sides for these in-range values. All plants caught;
//! 10446, 10449, 10450, 10453 and 10454 in thousands of sequences; 10447, 10451 and 10452 (a different rounding of the
//! same proportion) the fewest, in the hundreds; 10448 in thousands.
//!
//! *Results (2026-11-18, `TACTICAL_VIEW_CASES=20000`):* 20,000 sequences, 408,901 steps, 136,175 `setDisplayMode`:
//! **0 mismatches**, max ULP 0. 640 of the rescales gave a view height a pixel short of an exact integer product (e.g.
//! `483.0f / 604.0f * 604.0f` = 482.99997, so 482), so the truncation edge is exercised. Plants, mismatching sequences
//! of 20,000: 10446 14,167; 10447 2,729; 10448 15,289; 10449 18,957; 10450 18,499; 10451 4,322; 10452 4,828; 10453
//! 18,499; 10454 14,518. **9 of 9 caught.** **Prediction missed:** 10447, 10451 and 10452 were caught in thousands, not
//! hundreds: besides the truncations short of an integer, `float`'s rounding also lands a product just above an integer
//! that the exact quotient falls short of, and a halved or doubled display makes many proportions non-integer.
//!
//! | id | planted bug |
//! |---|---|
//! | 10446 | the origin is not rescaled |
//! | 10447 | the proportion in exact integers (`old * res / oldDisplay`) |
//! | 10448 | rounded to the nearest, not truncated |
//! | 10449 | the width scaled by the new height |
//! | 10450 | the old display size read after the display's `setWidth`/`setHeight` |
//! | 10451 | `res / oldDisplay * old`, the other order |
//! | 10452 | in `double` |
//! | 10453 | the display's `setWidth` skipped |
//! | 10454 | the origin's y scaled over the old display's width |

mod support;

use rts_lockstep_engine::client::tactical_view::{Display, TacticalView, ViewCamera};
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
            7 => {
                let mut d = Display { width: dw as u32, height: dh as u32 };
                let ok = d.set_display_mode(w[1] as u32, w[2] as u32, 32, true, &mut v);
                (dw, dh) = (d.width as i32, d.height as i32);
                out.extend([7, ok as i32]);
            }
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

/// The wrapper's state is global (`g_out`, the camera, the view, the display): one sequence at a time (#200: the two
/// tests of this binary run on two threads).
static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
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

/// GitHub #200: a sequence for [`Display::set_display_mode`]: the cases of [`gen_case`]'s setup, then 1 to 40 steps, of
/// which about a third are `setDisplayMode` to a new size (a listed display, a random one from 1 to 4,000 each way, the
/// same size, or the same size doubled or halved), the rest `setHeight` (the bar's 80 %, the whole display, the old view
/// height rounded through `float` as a bar would ask, or random), `setWidth`, `setOrigin` (random, up to the display's
/// size), and a pick ray, so the rescaled viewport and view plane are read through `getPickRay` too.
fn gen_case_mode(g: &mut Gen) -> Vec<i32> {
    let mut v = gen_case(g);
    v.truncate(18);
    let (mut dw, mut dh) = (v[0], v[1]);
    let steps = 1 + g.below(40);
    for _ in 0..steps {
        let px = |g: &mut Gen, d: i32| g.below((d + 401) as u32) as i32 - 200;
        let s = match g.below(9) {
            0..=2 => {
                let (nw, nh) = match g.below(5) {
                    0 | 1 => display(g),
                    2 => (dw, dh),
                    3 => (dw * 2, dh * 2),
                    _ => (max1(dw / 2), max1(dh / 2)),
                };
                (dw, dh) = (nw, nh);
                [7, nw, nh, 0]
            }
            3 | 4 => [1, [(dh as f32 * 0.80f32) as i32, dh, (dh as f32 * 0.77f32) as i32, 1 + g.below(4000) as i32][g.below(4) as usize], 0, 0],
            5 => [0, if g.below(2) == 0 { dw } else { 1 + g.below(4000) as i32 }, 0, 0],
            6 => [2, g.below((dw + 1) as u32) as i32, g.below((dh + 1) as u32) as i32, 0],
            _ => [4, px(g, dw), px(g, dh), 0],
        };
        v.extend(s);
    }
    v
}

fn max1(x: i32) -> i32 {
    if x < 1 { 1 } else { x }
}

#[test]
fn set_display_mode_rescales_the_view_as_upstream() {
    let cases: u64 = std::env::var("TACTICAL_VIEW_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x200_2000);
    let mut whole = Check::new("setDisplayMode's rescaled view");
    let (mut steps, mut modes, mut short) = (0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case_mode(&mut g);
        let mut dh = input[1];
        let mut vh = input[3];
        for s in input[18..].chunks(4) {
            steps += 1;
            match s[0] {
                1 => vh = s[1],
                7 => {
                    modes += 1;
                    // a truncation that a real-number product would not make: count them
                    let exact = vh as i64 * s[2] as i64 % dh as i64 == 0;
                    let got = (vh as f32 / dh as f32 * s[2] as f32) as i32;
                    if exact && got as i64 != vh as i64 * s[2] as i64 / dh as i64 {
                        short += 1;
                    }
                    vh = got;
                    dh = s[2];
                }
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
    println!("{cases} sequences, {steps} steps: {modes} setDisplayMode, {short} of them a view height a pixel short of the exact product");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
