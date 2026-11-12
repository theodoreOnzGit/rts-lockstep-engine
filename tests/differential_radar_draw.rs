#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/System/W3DRadar.cpp (radarToPixel, drawHeroIcon)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (triggerRadarAttackGlow,
//     updateRadarAttackGlow, reset's glow lines)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindow.cpp (winEnable)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable/Update/BeaconClientUpdate.cpp and its header
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #162 (the drawing items of #57): the Rust against the upstream text compiled verbatim
//! (`differential/wrap_radar_draw.cpp`, which lists the cuts, the stand-ins and the words).
//!
//! # Methodology
//!
//! - **The hero icon** ([`w3d_radar::hero_icon_rect`]): 20,000 random cases (map extents of 1 to 5,000 and now and then
//!   tiny, zero or negative; positions on and off the map and now and then huge; radar areas of 1 to 400 pixels at 0 to
//!   300; icons of -2 to 40 pixels). The C++ is `W3DRadar::drawHeroIcon` itself, whose `TheDisplay->drawImage`
//!   arguments are compared with the Rust's rectangle. A case the Rust finds undefined upstream (a float to `Int`
//!   conversion out of range or of a NaN, an `Int` overflow) is not given to the C++ (counted).
//! - **The radar attack glow** ([`RadarAttackGlow`] on the translated window manager): 20,000 random sequences of 1 to
//!   400 steps (`triggerRadarAttackGlow` 3 %, reset's glow lines 2 %, the window enabled or disabled from elsewhere
//!   10 %, `updateRadarAttackGlow` the rest; in half the sequences the trigger 0.3 % and the reset 0.2 %, so that glows
//!   also run their 150 frames out) on a window with 0 to 3 children, sometimes no window, compared after each step:
//!   the glow's on flag and frames, the window's and each child's enabled bit.
//! - **`BeaconClientUpdate`** ([`BeaconClientUpdate`] through a test [`BeaconHost`] that mirrors the C++ stand-ins):
//!   20,000 random sequences of 1 to 60 steps (`clientUpdate` 50 %, the logic frame moving on 25 %, `hideBeacon` 4 %,
//!   stealth on or off 6 %, the manager losing the system 3 %, the object coming or going 3 %, a new indicator colour
//!   3 %) from random module data (frequencies and durations of 0 to 60 frames and now and then near 2^32, so the
//!   `UnsignedInt` sum wraps), colours (sometimes white or with the top byte set), the colour's template and the white
//!   one there or not, creation failing now and then, and start frames near 0 or 2^32. Compared: every template looked
//!   up (its name's bytes and whether it was found), system made, tint, stop, the drawable hidden and its shadows, each
//!   radar event (position, type and seconds as bits), the defaults of `BeaconClientUpdateModuleData()`, and the
//!   module's two members after each step.
//!
//! # Predictions (written before the first run, `issue162_predictions.md`)
//!
//! 0 mismatches in all three. Hero cases excluded as undefined: a few per cent (the huge positions and zero extents).
//!
//! # Results (2026-11-12)
//!
//! - First run: the three tests ran on parallel threads into the wrapper's one record buffer, so beacon and glow
//!   sequences mismatched (a defect of this harness, not of the translation); the tests now take a lock.
//! - **Hero icon:** 20,000 cases, 16,969 compared, **0 mismatches**; 3,031 undefined upstream (predicted 2-6 %: a miss,
//!   the tiny extents send every position out of `Int` range).
//! - **Radar attack glow:** 20,000 sequences, 4,015,618 steps (872,726 of them lit while on, 21,173 glows ended),
//!   **0 mismatches**.
//! - **`BeaconClientUpdate`:** 20,000 sequences, 606,745 steps (32,487 pulses, 16,026 systems made, 2,822 tints, 12,462
//!   stops), **0 mismatches**.
//!
//! Planted bugs, mismatching cases of 20,000 (the hero's of 16,969): 9581 10,075; 9582 810 (6 before the generator's
//! sparse sequences, whose glows run out on their own); 9583 11,625; 9584 8,002; 9585 17,181; 9586 2,822; 9587 2,255;
//! 9588 12,480; 9589 786; 9590 10,767; 9591 10,395; 9592 16,969. **12 of 12 caught.**
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9581 | `triggerRadarAttackGlow` disables the window only when it is already disabled |
//! | 9582 | `updateRadarAttackGlow` ends the glow one frame early (at 1) |
//! | 9583 | the glow flashes every 16 frames, not 15 |
//! | 9584 | reset leaves the frames count |
//! | 9585 | the beacon's template name in lower-case hexadecimal |
//! | 9586 | the failsafe system is not tinted |
//! | 9587 | `hideBeacon` makes no system when it has none |
//! | 9588 | `clientUpdate` pulses while the drawable is hidden |
//! | 9589 | a pulse at `>=` the due frame, not `>` |
//! | 9590 | the pulse lives `RadarPulseDuration` seconds, not frames |
//! | 9591 | `m_lastRadarPulse` is not moved by a pulse |
//! | 9592 | the hero icon is shifted by `iconWidth / 2`, not `iconWidth / 2 - 1` |

mod support;

use rts_lockstep_engine::client::beacon_client_update::{BeaconClientUpdate, BeaconClientUpdateModuleData, BeaconHost};
use rts_lockstep_engine::client::control_bar_glow::RadarAttackGlow;
use rts_lockstep_engine::client::particle_sys::ParticleSystemId;
use rts_lockstep_engine::client::radar::RadarEventType;
use rts_lockstep_engine::client::w3d_radar;
use rts_lockstep_engine::client::window_manager::{GameWindowManager, NoGuiHost, SystemFunc};
use rts_lockstep_engine::client::window_script::{win_status, WinInstanceData};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

/// The wrapper's records are one global buffer: one test at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}

fn cases() -> u64 {
    std::env::var("RADAR_DRAW_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

// ---------------------------------------------------------------------------
// drawHeroIcon
// ---------------------------------------------------------------------------

#[test]
fn the_hero_icon_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x162_0001);
    let mut c = Check::new("W3DRadar::drawHeroIcon's rectangle");
    let mut undefined = 0u64;
    for k in 0..cases() {
        let ext = |g: &mut Gen| match g.below(30) {
            0 => 0.0,
            1 => -g.uniform(1.0, 100.0),
            2 => g.uniform(0.0001, 0.01),
            _ => g.uniform(1.0, 5000.0),
        };
        let (ew, eh) = (ext(&mut g), ext(&mut g));
        let coord = |g: &mut Gen, e: f32| match g.below(25) {
            0 => g.uniform(-1.0e12, 1.0e12),
            1 => g.mixed(),
            _ => g.uniform(-100.0, e.abs() + 100.0),
        };
        let (x, y) = (coord(&mut g, ew), coord(&mut g, eh));
        let area = (g.below(301) as i32, g.below(301) as i32, 1 + g.below(400) as i32, 1 + g.below(400) as i32);
        let icon = (g.below(43) as i32 - 2, g.below(43) as i32 - 2);
        let pos = Coord3D { x, y, z: 0.0 };
        let Some(r) = w3d_radar::hero_icon_rect((ew, eh), &pos, area, icon) else {
            undefined += 1;
            continue;
        };
        let f = [ew, eh, x, y];
        let i = [area.0, area.1, area.2, area.3, icon.0, icon.1];
        let mut out = [0i32; 5];
        unsafe { cpp::ref_radar_hero_icon(f.as_ptr(), i.as_ptr(), out.as_mut_ptr()) };
        c.eq([1, r.0, r.1, r.2, r.3], out, || format!("case {k}: extent {ew} x {eh}, pos ({x}, {y}), area {area:?}, icon {icon:?}"));
    }
    println!("hero icon: {} cases, {undefined} undefined upstream (not given to the C++)", cases());
    finish(c);
}

// ---------------------------------------------------------------------------
// The radar attack glow
// ---------------------------------------------------------------------------

fn glow_rust(input: &[i32]) -> Vec<i32> {
    let mut wm = GameWindowManager::new((800, 600));
    let w = wm.win_create(&mut NoGuiHost, None, 0, 0, 0, 10, 10, SystemFunc::Default, Some(WinInstanceData::new()));
    let en = |b: bool| if b { win_status::ENABLED } else { 0 };
    wm.tree.windows[w].status = en(input[1] != 0);
    let nk = input[2] as usize;
    let mut kids = Vec::new();
    for k in 0..nk {
        let c = wm.win_create(&mut NoGuiHost, Some(w), 0, 0, 0, 5, 5, SystemFunc::Default, Some(WinInstanceData::new()));
        wm.tree.windows[c].status = en((input[3] >> k) & 1 != 0);
        kids.push(c);
    }
    let mut glow = RadarAttackGlow::new();
    if input[0] != 0 {
        glow.window = Some(w);
    }
    let mut out = Vec::new();
    for st in input[4..].chunks(2) {
        match st[0] {
            0 => glow.trigger(&mut wm),
            1 => glow.update(&mut wm),
            2 => glow.reset(&mut wm),
            _ => wm.win_enable(w, st[1] != 0),
        }
        let kb = kids.iter().enumerate().fold(0, |a, (k, &c)| a | (i32::from(wm.tree.windows[c].status & win_status::ENABLED != 0) << k));
        out.extend([100, i32::from(glow.on), glow.remaining_frames, i32::from(wm.tree.windows[w].status & win_status::ENABLED != 0), kb, 0, 0, 0]);
    }
    out
}

fn glow_cpp(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; (input.len() / 2 + 4) * 8];
    let n = unsafe { cpp::ref_radar_glow_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

#[test]
fn the_radar_attack_glow_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x162_0002);
    let mut c = Check::new("ControlBar's radar attack glow");
    let (mut steps, mut lit_steps, mut ends) = (0u64, 0u64, 0u64);
    for k in 0..cases() {
        let mut input = vec![i32::from(g.below(10) != 0), g.below(2) as i32, g.below(4) as i32, g.below(8) as i32];
        // (half the sequences trigger and reset rarely, so that glows also run out on their own)
        let sparse = g.below(2) == 0;
        for _ in 0..1 + g.below(400) {
            let op = match (sparse, g.below(1000)) {
                (false, 0..=29) | (true, 0..=2) => 0,
                (false, 30..=49) | (true, 3..=4) => 2,
                (_, 900..) => 3,
                _ => 1,
            };
            input.extend([op, g.below(2) as i32]);
        }
        steps += ((input.len() - 4) / 2) as u64;
        let r = glow_rust(&input);
        let cc = glow_cpp(&input);
        for (a, b) in cc.chunks(8).zip(cc.chunks(8).skip(1)) {
            if b[1] == 1 {
                lit_steps += u64::from(b[3] == 0);
            }
            if a[1] == 1 && b[1] == 0 {
                ends += 1;
            }
        }
        c.eq(r, cc, || format!("sequence {k}: {input:?}"));
    }
    println!("radar attack glow: {} sequences, {steps} steps, {lit_steps} steps lit while on, {ends} glows ended", cases());
    finish(c);
}

// ---------------------------------------------------------------------------
// BeaconClientUpdate
// ---------------------------------------------------------------------------

/// The C++ stand-ins' world, in Rust: the drawable, its object, the manager's one system, the logic and the radar,
/// recording what the module calls as the wrapper does.
struct World {
    has_object: bool,
    color: i32,
    color_template: bool,
    white_template: bool,
    create_fails: bool,
    hidden: bool,
    hidden_by_stealth: bool,
    lost: bool,
    frame: u32,
    next_id: u32,
    current: u32,
    out: Vec<i32>,
}

impl World {
    fn rec(&mut self, w: &[i32]) {
        let mut r = [0i32; 8];
        r[..w.len()].copy_from_slice(w);
        self.out.extend(r);
    }
}

impl BeaconHost for World {
    fn has_drawable(&self) -> bool {
        true
    }
    fn object_indicator_color(&self) -> Option<i32> {
        self.has_object.then_some(self.color)
    }
    fn find_template(&self, name: &[u8]) -> bool {
        let expect = format!("BeaconSmoke{:06X}", 0xffffff & self.color);
        let found = if name == b"BeaconSmokeFFFFFF" && self.white_template { true } else { name == expect.as_bytes() && self.color_template };
        let mut w = [0u8; 20];
        let n = name.len().min(20);
        w[..n].copy_from_slice(&name[..n]);
        let words: Vec<i32> = w.chunks(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect();
        // (find_template is `&self`: the record goes through a cell)
        RECORDS.with(|r| {
            let mut v = vec![1, name.len() as i32];
            v.extend(&words);
            v.push(i32::from(found));
            r.borrow_mut().push(v);
        });
        found
    }
    fn create_attached_particle_system(&mut self, _name: &[u8]) -> Option<ParticleSystemId> {
        self.flush();
        if self.create_fails {
            return None;
        }
        self.next_id += 1;
        self.current = self.next_id;
        self.lost = false;
        self.rec(&[2, self.current as i32]);
        Some(self.current)
    }
    fn tint_all_colors(&mut self, id: ParticleSystemId, color: i32) {
        self.flush();
        self.rec(&[3, id as i32, color]);
    }
    fn stop_particle_system(&mut self, id: ParticleSystemId) {
        self.flush();
        if id != 0 && !self.lost && id == self.current {
            self.rec(&[4, id as i32]);
        }
    }
    fn hide_drawable(&mut self) {
        self.flush();
        self.rec(&[5]);
        self.hidden = true;
        self.rec(&[6, 0]);
    }
    fn is_drawable_effectively_hidden(&self) -> bool {
        self.hidden || self.hidden_by_stealth
    }
    fn drawable_position(&self) -> Coord3D {
        Coord3D { x: 12.5, y: -3.25, z: 7.0 }
    }
    fn logic_frame(&self) -> u32 {
        self.frame
    }
    fn radar_create_event(&mut self, pos: &Coord3D, event_type: RadarEventType, seconds_to_live: f32) {
        self.flush();
        self.rec(&[7, pos.x.to_bits() as i32, pos.y.to_bits() as i32, pos.z.to_bits() as i32, event_type, seconds_to_live.to_bits() as i32]);
    }
}

thread_local! {
    static RECORDS: std::cell::RefCell<Vec<Vec<i32>>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl World {
    /// The template lookups recorded so far, into the output in their place.
    fn flush(&mut self) {
        let recs = RECORDS.with(|r| std::mem::take(&mut *r.borrow_mut()));
        for v in recs {
            self.rec(&v);
        }
    }
}

fn beacon_rust(input: &[i32]) -> Vec<i32> {
    let d = BeaconClientUpdateModuleData::default();
    let mut w = World {
        has_object: input[2] != 0,
        color: input[3],
        color_template: input[4] != 0,
        white_template: input[5] != 0,
        create_fails: input[6] != 0,
        hidden: false,
        hidden_by_stealth: false,
        lost: false,
        frame: input[7] as u32,
        next_id: 0,
        current: 0,
        out: Vec::new(),
    };
    RECORDS.with(|r| r.borrow_mut().clear());
    w.rec(&[200, d.frames_between_radar_pulses as i32, d.radar_pulse_duration as i32]);
    let data = BeaconClientUpdateModuleData { frames_between_radar_pulses: input[0] as u32, radar_pulse_duration: input[1] as u32 };
    let mut b = BeaconClientUpdate::new(data, w.frame);
    for st in input[8..].chunks(2) {
        let arg = st[1];
        match st[0] {
            0 => b.client_update(&mut w),
            1 => b.hide_beacon(&mut w),
            2 => w.frame = arg as u32,
            3 => w.hidden_by_stealth = arg != 0,
            4 => w.lost = true,
            5 => w.has_object = arg != 0,
            _ => w.color = arg,
        }
        w.flush();
        w.rec(&[100, b.particle_system_id as i32, b.last_radar_pulse as i32]);
    }
    w.out
}

fn beacon_cpp(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 16];
    let n = unsafe { cpp::ref_beacon_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

#[test]
fn beacon_client_update_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x162_0003);
    let mut c = Check::new("BeaconClientUpdate");
    let (mut steps, mut pulses, mut made, mut tints, mut stops) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for k in 0..cases() {
        let frames = |g: &mut Gen| -> i32 {
            match g.below(10) {
                0 => (u32::MAX - g.below(100)) as i32,
                _ => g.below(61) as i32,
            }
        };
        let color = match g.below(8) {
            0 => 0x00FF_FFFF,
            1 => g.u32() as i32,
            2 => (0xFF00_0000u32 | g.below(0x0100_0000)) as i32,
            _ => g.below(0x0100_0000) as i32,
        };
        let start = if g.below(6) == 0 { u32::MAX - g.below(200) } else { g.below(1000) };
        let mut input = vec![frames(&mut g), frames(&mut g), i32::from(g.below(10) != 0), color, i32::from(g.below(4) != 0), i32::from(g.below(4) != 0), i32::from(g.below(20) == 0), start as i32];
        let mut frame = start;
        for _ in 0..1 + g.below(60) {
            let (op, arg) = match g.below(100) {
                0..=49 => (0, 0),
                50..=74 => {
                    frame = frame.wrapping_add(1 + g.below(40));
                    (2, frame as i32)
                }
                75..=78 => (1, 0),
                79..=84 => (3, g.below(2) as i32),
                85..=87 => (4, 0),
                88..=90 => (5, g.below(2) as i32),
                91..=93 => (6, g.u32() as i32),
                _ => (0, 0),
            };
            input.extend([op, arg]);
        }
        steps += ((input.len() - 8) / 2) as u64;
        let r = beacon_rust(&input);
        let cc = beacon_cpp(&input);
        for rec in cc.chunks(8) {
            match rec[0] {
                7 => pulses += 1,
                2 => made += 1,
                3 => tints += 1,
                4 => stops += 1,
                _ => {}
            }
        }
        if r != cc {
            let at = r.iter().zip(&cc).position(|(a, b)| a != b).unwrap_or(r.len().min(cc.len())) / 8 * 8;
            println!("sequence {k}: rust {:?} c++ {:?}", &r[at.min(r.len())..(at + 16).min(r.len())], &cc[at.min(cc.len())..(at + 16).min(cc.len())]);
        }
        c.eq(r, cc, || format!("sequence {k}: {input:?}"));
    }
    println!("BeaconClientUpdate: {} sequences, {steps} steps: {pulses} pulses, {made} systems made, {tints} tints, {stops} stops", cases());
    finish(c);
}
