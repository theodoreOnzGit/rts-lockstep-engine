#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::drawable` against `Drawable`'s pips, construct, caption, veterancy and health bar draws (#215, group lc3).
//!
//! # Method
//!
//! `computeHealthRegion`, `drawAmmo`, `drawContained`, `drawConstructPercent`, `drawCaption`, `drawVeterancy` and `drawHealthBar` are cut verbatim by build.rs into
//! `differential/wrap_draft_drawable.cpp` (stand-ins listed there: the object, its contain module, the images, the display, the tactical view, the display strings, all of
//! them logging or answering from the case). A case is a drawable (selected, moused over, controlled by the local player, with or without an object, ammo and container pips, a
//! health box, images present or missing, construction and health values, the damage state, a caption) and a program of up to 12 of the seven draws on it, with random health bar
//! regions. Compared after each op: every call the draw made (images with their four coordinates and colour, rectangles, display string texts and draws), the region, the construct
//! string's flag and `m_lastConstructDisplayed`. 20,000 cases from a fixed seed. EXCLUDED (upstream undefined or the draft does not model): `drawConstructPercent` and `drawCaption` with a drawable centre the view cannot project (upstream ignores `worldToScreen`'s result and reads
//! the uninitialised `screen`; the draft reads (0, 0)), a health ratio above 1 (a float above 255
//! into an `UnsignedByte`), `drawAmmo` / `drawContained` / `drawVeterancy` with no object (upstream dereferences NULL), `drawContained` with a missing container image (upstream reads `s_emptyContainer->getImageWidth()` without a NULL check; the
//! draft returns). NOT covered here: `drawBattlePlans`, `drawUIText`, `drawHealing`, `drawEnthusiastic`, `drawBombed`, `drawDisabled` (they need the Anim2D collection and
//! host state beyond this wrapper), the selection and tint functions, `drawsAnyUIText`, `imitateStealthLook`, and the module-list helpers.
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! One miss expected: `drawVeterancy` adds `healthBoxWidth * scale * 0.5f` to `screenCenter.x` as a float and truncates once; the draft truncates the product first, which differs for a
//! negative screen x. Everything else: 0 mismatches.
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches over 20,000 cases** after one draft fix. The predicted defect was found by reading upstream while the plants were being written and fixed before the first run, so there is
//! no "before" run; planted bug 19951 puts the old code back and is caught in 449 cases: `drawVeterancy` added `(healthBoxWidth * scale * 0.5) as i32` to `screenCenter.x`, where upstream adds as a float and
//! truncates once. Excluded inputs found by the first run (both upstream-undefined, now listed above): `drawContained` with a missing container image (a NULL dereference upstream; 415 cases of the first
//! run were `drawConstructPercent` reading upstream's uninitialised `screen` when the centre cannot be projected; the first run before that segfaulted in the C++). Planted bugs, 15 of 15 caught (of 20,000 cases):
//! 19951 449; 19952 2,057; 19953 1,810; 19954 1,472; 19955 983; 19956 1,003 (first an equivalent mutant at ratio 0.5, replaced); 19957 3,634; 19958 688; 19959 6,818; 19960 448; 19961 4,041; 19962 4,041; 19963 1,469;
//! 19964 790; 19965 752.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 19951 drawVeterancy truncates the product before adding (the old code); 19952 drawAmmo has no spacing between pips; 19953 drawAmmo draws the empty pips at posy; 19954 drawContained swaps the
//! two colours; 19955 drawContained counts one infantry too many; 19956 drawHealthBar's really-damaged green is quartered; 19957 the fill width is not reduced by 2; 19958 the blue bar's green rounds; 19959
//! computeHealthRegion uses 0.5 for 0.45; 19960 drawConstructPercent's `screen.x < 1` is `< 2`; 19961 drawCaption's background alpha is 128; 19962 its box starts at screen.x; 19963 drawHealthBar swaps the damage
//! flags; 19964 drawAmmo needs the drawable selected (mouse-over is not enough); 19965 drawConstructPercent rebuilds the text every call.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::anim2d::{Anim2DHost, AnimImage};
use rts_lockstep_engine::drafts::sweep_common_client::drawable::{compute_health_region, DrawGroupInfo, DrawableIconType, DrawableUi, DrawableUiHost, HealingFacts, IRegion2D, IconImage};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc3_dr_run(prog: *const i32, nprog: i32, out: *mut i32) -> i32;
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

fn fb(f: f32) -> i32 {
    f.to_bits() as i32
}

#[derive(Clone, Debug, Default)]
struct Case {
    selected: bool,
    moused: bool,
    show: bool,
    has_object: bool,
    ctrl_local: bool,
    ammo: Option<(i32, i32)>,
    pips: Option<(i32, i32)>,
    has_contain: bool,
    n_items: i32,
    n_inf: i32,
    pos: [f32; 3],
    max_h: f32,
    radius: f32,
    ammo_world: [f32; 3],
    ammo_screen: [f32; 2],
    cont_world: [f32; 3],
    cont_screen: [f32; 2],
    zoom: f32,
    hb_pos: [f32; 3],
    hb_ok: bool,
    hb_h: f32,
    hb_w: f32,
    img_flags: i32,
    empty_ammo: (i32, i32),
    empty_cont: (i32, i32),
    vet: (i32, i32),
    has_exp: bool,
    under: bool,
    sold: bool,
    disabled: bool,
    held: bool,
    force_attackable: bool,
    health: f32,
    max_health: f32,
    construct_percent: f32,
    damaged: bool,
    really_damaged: bool,
    center: [f32; 3],
    has_caption: bool,
    cap: (i32, i32),
    caption_color: i32,
    construct_w: i32,
    has_construct_ds: bool,
    ev: Vec<i32>,
}

impl Case {
    fn put(&self, o: &mut Vec<i32>) {
        let f3 = |a: &[f32; 3], o: &mut Vec<i32>| o.extend(a.iter().map(|&v| fb(v)));
        o.extend([self.selected as i32, self.moused as i32, self.show as i32, self.has_object as i32, self.ctrl_local as i32]);
        o.extend([self.ammo.is_some() as i32, self.ammo.map_or(0, |a| a.0), self.ammo.map_or(0, |a| a.1)]);
        o.extend([self.pips.is_some() as i32, self.pips.map_or(0, |a| a.0), self.pips.map_or(0, |a| a.1)]);
        o.extend([self.has_contain as i32, self.n_items, self.n_inf]);
        f3(&self.pos, o);
        o.extend([fb(self.max_h), fb(self.radius)]);
        f3(&self.ammo_world, o);
        o.extend([fb(self.ammo_screen[0]), fb(self.ammo_screen[1])]);
        f3(&self.cont_world, o);
        o.extend([fb(self.cont_screen[0]), fb(self.cont_screen[1])]);
        o.push(fb(self.zoom));
        f3(&self.hb_pos, o);
        o.extend([self.hb_ok as i32, fb(self.hb_h), fb(self.hb_w), self.img_flags, self.empty_ammo.0, self.empty_ammo.1, self.empty_cont.0, self.empty_cont.1, self.vet.0, self.vet.1]);
        o.extend([self.has_exp as i32, self.under as i32, self.sold as i32, self.disabled as i32, self.held as i32, self.force_attackable as i32]);
        o.extend([fb(self.health), fb(self.max_health), fb(self.construct_percent), self.damaged as i32, self.really_damaged as i32]);
        f3(&self.center, o);
        o.extend([self.has_caption as i32, self.cap.0, self.cap.1, self.caption_color, self.construct_w, self.has_construct_ds as i32]);
    }
}

struct Host(Case);

impl Anim2DHost for Host {
    fn logic_frame(&self) -> u32 {
        0
    }
    fn game_client_random_value(&mut self, _lo: i32, _hi: i32) -> i32 {
        0
    }
    fn find_image_by_name(&self, _name: &[u8]) -> Option<AnimImage> {
        None
    }
    fn draw_image(&mut self, _image: &AnimImage, _x0: i32, _y0: i32, _x1: i32, _y1: i32, _color: u32) {}
}

fn img(id: u8, w: i32, h: i32) -> IconImage {
    IconImage { name: vec![id], width: w, height: h }
}

impl DrawableUiHost for Host {
    fn show_object_health(&self) -> bool {
        self.0.show
    }
    fn is_moused_over(&self) -> bool {
        self.0.moused
    }
    fn has_object(&self) -> bool {
        self.0.has_object
    }
    fn is_controlled_by_local_player(&self) -> bool {
        self.0.ctrl_local
    }
    fn is_locally_controlled(&self) -> bool {
        false
    }
    fn squad_number_for_object(&self) -> i32 {
        0
    }
    fn formation_id(&self) -> i32 {
        0
    }
    fn ammo_pip_showing_info(&self) -> Option<(i32, i32)> {
        self.0.ammo
    }
    fn container_pips_to_show(&self) -> Option<(i32, i32)> {
        if self.0.has_contain {
            self.0.pips
        } else {
            None
        }
    }
    fn contained_infantry_count(&self) -> i32 {
        if self.0.has_contain {
            self.0.n_inf
        } else {
            0
        }
    }
    fn object_position_and_geometry(&self) -> ((f32, f32, f32), f32, f32) {
        ((self.0.pos[0], self.0.pos[1], self.0.pos[2]), self.0.max_h, self.0.radius)
    }
    fn ammo_pip_offsets(&self) -> ((f32, f32, f32), (f32, f32)) {
        ((self.0.ammo_world[0], self.0.ammo_world[1], self.0.ammo_world[2]), (self.0.ammo_screen[0], self.0.ammo_screen[1]))
    }
    fn container_pip_offsets(&self) -> ((f32, f32, f32), (f32, f32)) {
        ((self.0.cont_world[0], self.0.cont_world[1], self.0.cont_world[2]), (self.0.cont_screen[0], self.0.cont_screen[1]))
    }
    fn world_to_screen(&self, p: (f32, f32, f32)) -> Option<(i32, i32)> {
        if p.2 > 500.0 {
            None
        } else {
            Some(((p.0 * 1.7f32) as i32, (p.1 * 2.3f32 - 40.0f32) as i32))
        }
    }
    fn view_zoom(&self) -> f32 {
        self.0.zoom
    }
    fn health_box_position(&self) -> (f32, f32, f32) {
        (self.0.hb_pos[0], self.0.hb_pos[1], self.0.hb_pos[2])
    }
    fn health_box_dimensions(&self) -> Option<(f32, f32)> {
        if self.0.hb_ok {
            Some((self.0.hb_h, self.0.hb_w))
        } else {
            None
        }
    }
    fn display_draw_image(&mut self, image: &IconImage, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
        self.0.ev.extend([1, image.name[0] as i32, x0, y0, x1, y1, color as i32]);
    }
    fn display_draw_open_rect(&mut self, x: i32, y: i32, w: i32, h: i32, line_width: f32, color: u32) {
        self.0.ev.extend([2, x, y, w, h, fb(line_width), color as i32]);
    }
    fn display_draw_fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        self.0.ev.extend([3, x, y, w, h, color as i32]);
    }
    fn ammo_images(&self) -> (Option<IconImage>, Option<IconImage>) {
        ((self.0.img_flags & 1 != 0).then(|| img(1, 4, 4)), (self.0.img_flags & 2 != 0).then(|| img(2, self.0.empty_ammo.0, self.0.empty_ammo.1)))
    }
    fn container_images(&self) -> (Option<IconImage>, Option<IconImage>) {
        ((self.0.img_flags & 4 != 0).then(|| img(3, 4, 4)), (self.0.img_flags & 8 != 0).then(|| img(4, self.0.empty_cont.0, self.0.empty_cont.1)))
    }
    fn veterancy_image(&self) -> Option<IconImage> {
        if self.0.has_exp && self.0.img_flags & 16 != 0 {
            Some(img(5, self.0.vet.0, self.0.vet.1))
        } else {
            None
        }
    }
    fn animation_template(&self, _t: DrawableIconType) -> Option<usize> {
        None
    }
    fn battle_plans_apply(&self) -> bool {
        false
    }
    fn battle_plan_active(&self, _plan: i32) -> bool {
        false
    }
    fn healing_facts(&self) -> HealingFacts {
        HealingFacts { no_heal_icon: false, sold: false, health_not_max: false, frame: 0, last_healing_timestamp: 0, structure: false, vehicle: false }
    }
    fn weapon_bonus_enthusiastic(&self) -> (bool, bool) {
        (false, false)
    }
    fn kind_of_structure_hugevehicle_vehicle(&self) -> (bool, bool, bool) {
        (false, false, false)
    }
    fn weapon_set_carbomb(&self) -> bool {
        false
    }
    fn sticky_bomb(&self) -> Option<(bool, u32)> {
        None
    }
    fn disabled_for_icon(&self) -> bool {
        false
    }
    fn construction_facts(&self) -> (bool, bool, f32) {
        (self.0.under, self.0.sold, self.0.construct_percent)
    }
    fn is_force_attackable(&self) -> bool {
        self.0.force_attackable
    }
    fn health_and_max(&self) -> (f32, f32) {
        (self.0.health, self.0.max_health)
    }
    fn health_bar_blue(&self) -> bool {
        self.0.under || (self.0.disabled && !self.0.held)
    }
    fn model_condition_damaged(&self) -> (bool, bool) {
        (self.0.really_damaged, self.0.damaged)
    }
    fn set_construct_text(&mut self, percent: f32) {
        self.0.ev.extend([4, fb(percent)]);
    }
    fn construct_text_width(&self) -> i32 {
        self.0.construct_w
    }
    fn draw_construct_text(&mut self, x: i32, y: i32, color: u32, drop_color: u32) {
        self.0.ev.extend([5, x, y, color as i32, drop_color as i32]);
    }
    fn caption_size(&self) -> Option<(i32, i32)> {
        if self.0.has_caption {
            Some(self.0.cap)
        } else {
            None
        }
    }
    fn draw_caption_text(&mut self, x: i32, y: i32, color: u32, drop_color: u32) {
        self.0.ev.extend([5, x, y, color as i32, drop_color as i32]);
    }
    fn drawable_caption_color(&self) -> u32 {
        self.0.caption_color as u32
    }
    fn drawable_center_position(&self) -> (f32, f32, f32) {
        (self.0.center[0], self.0.center[1], self.0.center[2])
    }
    fn draw_group_numeral(&mut self, _group_num: i32, _x: i32, _y: i32, _color: u32) {}
    fn draw_formation_letter(&mut self, _x: i32, _y: i32, _color: u32) {}
    fn draw_group_info(&self) -> DrawGroupInfo {
        DrawGroupInfo { use_player_color: false, color_for_text: 0, using_pixel_offset_x: false, pixel_offset_x: 0, percent_offset_x: 0.0, using_pixel_offset_y: false, pixel_offset_y: 0, percent_offset_y: 0.0 }
    }
    fn player_color(&self) -> u32 {
        0
    }
    fn contain_flash_as_selected(&mut self) {}
    fn is_night(&self) -> bool {
        false
    }
}

fn pick(g: &mut Gen, set: &[f32]) -> f32 {
    set[g.below(set.len() as u32) as usize]
}

#[test]
fn drawable_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_D2A);
    let mut c = Check::new("Drawable pips, construct, caption, veterancy, health bar");
    for case in 0..20_000 {
        let mut k = Case::default();
        k.selected = g.below(2) == 0;
        k.moused = g.below(2) == 0;
        k.show = g.below(6) != 0;
        k.has_object = g.below(8) != 0;
        k.ctrl_local = g.below(5) != 0;
        let t = g.below(9) as i32;
        k.ammo = if g.below(4) == 0 { None } else { Some((t, g.below(t as u32 + 1) as i32)) };
        let t = g.below(9) as i32;
        k.pips = if g.below(4) == 0 { None } else { Some((t, g.below(t as u32 + 1) as i32)) };
        k.has_contain = g.below(5) != 0;
        k.n_items = g.below(7) as i32;
        k.n_inf = g.below(k.n_items as u32 + 1) as i32;
        k.pos = [g.uniform(-60.0, 300.0), g.uniform(-60.0, 300.0), if g.below(12) == 0 { 600.0 } else { g.uniform(0.0, 100.0) }];
        k.max_h = g.uniform(0.0, 30.0);
        k.radius = g.uniform(0.0, 40.0);
        for i in 0..3 {
            k.ammo_world[i] = g.uniform(-5.0, 5.0);
            k.cont_world[i] = g.uniform(-5.0, 5.0);
        }
        for i in 0..2 {
            k.ammo_screen[i] = g.uniform(-3.0, 3.0);
            k.cont_screen[i] = g.uniform(-3.0, 3.0);
        }
        k.zoom = g.uniform(0.3, 3.0);
        k.hb_pos = [g.uniform(-60.0, 300.0), g.uniform(-60.0, 300.0), if g.below(12) == 0 { 600.0 } else { g.uniform(0.0, 100.0) }];
        k.hb_ok = g.below(8) != 0;
        k.hb_h = g.uniform(0.0, 20.0);
        k.hb_w = g.uniform(5.0, 80.0);
        k.img_flags = g.below(32) as i32 | if g.below(3) != 0 { 15 } else { 0 };
        k.empty_ammo = (1 + g.below(30) as i32, 1 + g.below(30) as i32);
        k.empty_cont = (1 + g.below(30) as i32, 1 + g.below(30) as i32);
        k.vet = (1 + g.below(40) as i32, 1 + g.below(40) as i32);
        k.has_exp = g.below(5) != 0;
        k.under = g.below(5) == 0;
        k.sold = g.below(8) == 0;
        k.disabled = g.below(5) == 0;
        k.held = g.below(3) == 0;
        k.force_attackable = g.below(10) == 0;
        k.max_health = if g.below(15) == 0 { 0.0 } else { g.uniform(1.0, 500.0) };
        k.health = match g.below(8) {
            0 => 0.0,
            1 => k.max_health,
            2 => k.max_health * 0.5,
            _ => g.uniform(0.0, 1.0) * k.max_health,
        };
        if k.health > k.max_health {
            k.health = k.max_health;
        }
        k.construct_percent = [0.0, 25.5, 50.0, 99.9, g.uniform(0.0, 100.0)][g.below(5) as usize];
        k.damaged = g.below(2) == 0;
        k.really_damaged = g.below(3) == 0;
        k.center = [pick(&mut g, &[0.0, 0.5, 0.59, 0.6, 1.0, 1.17, 1.18, 4.0]), g.uniform(-60.0, 300.0), g.uniform(0.0, 100.0)];
        if g.below(2) == 0 {
            k.center[0] = g.uniform(-60.0, 300.0);
        }
        k.has_caption = g.below(3) == 0;
        k.cap = (1 + g.below(80) as i32, 1 + g.below(20) as i32);
        k.caption_color = g.u32() as i32;
        k.construct_w = g.below(60) as i32;
        k.has_construct_ds = g.below(3) == 0;
        let nops = 1 + g.below(12);
        let mut prog: Vec<i32> = Vec::new();
        k.put(&mut prog);
        prog.push(nops as i32);
        let mut ops: Vec<[i32; 6]> = Vec::new();
        for _ in 0..nops {
            let mut kind;
            loop {
                kind = g.below(7) as i32;
                if (k.has_object || !matches!(kind, 1 | 2 | 5)) && (kind != 2 || k.img_flags & 12 == 12) {
                    break;
                }
            }
            let lo_x = g.below(360) as i32 - 60;
            let lo_y = g.below(360) as i32 - 60;
            let op = [kind, lo_x, lo_y, lo_x + g.below(120) as i32, lo_y + g.below(14) as i32 - 2, (g.below(6) != 0) as i32];
            prog.extend(op);
            ops.push(op);
        }
        let mut cpp = vec![0i32; 20_000];
        let len = unsafe { ref_lc3_dr_run(prog.as_ptr(), prog.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        let mut ui = DrawableUi { selected: k.selected, last_construct_displayed: -1.0, has_construct_display_string: k.has_construct_ds, ..Default::default() };
        let mut host = Host(k.clone());
        let mut rust: Vec<i32> = Vec::new();
        for op in &ops {
            host.0.ev.clear();
            let r = IRegion2D { lo_x: op[1], lo_y: op[2], hi_x: op[3], hi_y: op[4] };
            let mut ans = 0;
            match op[0] {
                0 => {
                    if let Some(rr) = compute_health_region(&host) {
                        ans = 1;
                        host.0.ev.extend([rr.lo_x, rr.lo_y, rr.hi_x, rr.hi_y]);
                    }
                }
                1 => ui.draw_ammo(&r, &mut host),
                2 => ui.draw_contained(&r, &mut host),
                3 => ui.draw_construct_percent(&mut host),
                4 => ui.draw_caption(&mut host),
                5 => ui.draw_veterancy(&mut host),
                _ => ui.draw_health_bar(if op[5] != 0 { Some(&r) } else { None }, &mut host),
            }
            rust.push(host.0.ev.len() as i32);
            rust.extend(host.0.ev.iter().copied());
            rust.push(ans);
            rust.push(ui.has_construct_display_string as i32);
            rust.push(fb(ui.last_construct_displayed));
        }
        c.eq(rust, cpp, || format!("case {case}: {k:?} ops {ops:?}"));
    }
    finish(c);
}
