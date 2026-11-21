// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/w3d_main_menu.rs` (`advance_position`, `w3d_shell_menu_scheme_draw`, `w3d_main_menu_draw`,
//! `w3d_main_menu_four_draw`, `w3d_main_menu_map_border`, `w3d_thin_border_draw`) against upstream `W3DMainMenu.cpp`'s `advancePosition`, `W3DShellMenuSchemeDraw`, `W3DMainMenuDraw`,
//! `W3DMainMenuFourDraw`, `W3DMainMenuMapBorder` and `W3DThinBorderDraw`, cut verbatim by build.rs (`drafts_ui3b`; `differential/wrap_draft_w3d_main_menu.cpp`, whose stand-ins and the
//! forked process per case, for `advancePosition`'s function statics, are listed there). NOT covered: `W3DMetalBarMenuDraw`, `W3DClockDraw` (the clock's `strftime` and font),
//! `W3DMainMenuButtonDropShadowDraw` and `drawText` (the push button's draw data and the display strings' measure), `W3DMainMenuRandomTextDraw`, `W3DMainMenuInit`
//! (one call), `W3DCreditsMenuDraw` (one call): the first four need the draw data and the display string of the window, which this wrapper does not stand in for.
//!
//! # Method
//!
//! 20,000 cases (fixed seed, `DRAFT_CASES` overrides), each in its own forked C++ process. 1 to 3 windows (one may be the parent of another) of position -60 to 1,400 and size 0 to
//! 1,300 (sizes that are multiples of 10 and 25 are frequent, where a product with 0.1, 0.12, 0.9, 0.92, 0.225 ... is near an integer); a display height; the seven images
//! (`MainMenuPulse`, the border pieces) present or not with sizes 1 to 400; an enabled image and an offset for the thin border; 1 to 6 draw calls (the main menu's two frames, the map border, the
//! thin border, the shell scheme draw) with a clock that starts anywhere (the unsigned wrap included), mostly advances by up to 15 s (so the pulse crosses both directions, and the
//! `percentDone >= 1` edge) and sometimes goes backwards. Every `drawLine` (coordinates and colour; the draft has no width field, so the width is not compared) and `drawImage`
//! (image, corners, colour) is compared in order. The draft runs on the verified `GameWindowManager` (`screen_position` against the C++ `winGetScreenPosition`).
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! A miss is likely in the frame: the draft multiplies the size by `0.1`, `0.12`, `0.9`, `0.92`, `0.225`, ... in `f32`, upstream in `double` (`size.y * .1` is `Int * double`, truncated on the
//! store), and the two disagree where the product is within a rounding of an integer. Everything else: 0 mismatches.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21261 | the frame's `pos + size * k` is made in `f32` and truncated before the position is added (the draft's original defect) |
//! | 21262 | the vertical lines use the height for the horizontal offset |
//! | 21263 | the lower drop line is two pixels below the lower line |
//! | 21264 | the four-line variant's last vertical is at 0.6662 |
//! | 21265 | the pulse's backward leg uses the forward leg's formula |
//! | 21266 | the pulse's period is 1,000 ms, not 10,000 |
//! | 21267 | the map border's half piece is drawn full width |
//! | 21268 | the map border's remainder piece misses the `+ 1` before `& ~1` |
//! | 21269 | the lower right corner uses the lower left image |
//! | 21270 | the thin border ignores the image offset |
//! | 21271 | the pulse's backward `x` truncates the product before the subtraction (the draft's original defect) |
//! | 21272 | the pulse's forward `x` truncates the product before the subtraction (the draft's original defect) |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 cases** after the fixes noted here, max ULP 0 (nothing is floating point in the comparison, which is of integer words). The first run showed **4,716 mismatches**, two draft defects, both a float expression truncated too early. (a) `advancePosition`: upstream's `x = (percentDone * Width) - image->getImageWidth()` and `x = size.x - (percentDone * Width)` are single-precision expressions truncated once on the store; the draft truncated the product first and then subtracted. (b) the frames' `pos.y + (size.y * .1)`, `.12`, `.9`, `.92` and `pos.x + (size.x * .225)` ...: `Int + (Int * double)` is a double truncated once when stored in the `Int` field; the draft multiplied in `f32`, truncated, then added the position (wrong for a negative position and where the `f32` product rounds across an integer). **Both fixed**, then **0 mismatches**. Prediction was right about (b), missed (a).
//! Planted bugs (of 20,000): 21261 965; 21262 18,136; 21263 18,545; 21264 9,922; 21265 3,223; 21266 9,264; 21267 2,040; 21268 1,585; 21269 5,928; 21270 4,106; 21271 1,188; 21272 2,934. (21267's first version, a threshold off by one, showed 0 mismatches: at the one length where the two paths differ both draw the same piece, an equivalent mutant, replaced by a half piece drawn full width.)
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::window_manager::{DrawItem, GameWindowManager, GuiHost, SystemFunc};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::w3d_main_menu::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_w3dmm_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

const NAMES: [&[u8]; 7] = [b"MainMenuPulse", b"FrameCornerHorizontal", b"FrameCornerVertical", b"FrameCornerUL", b"FrameCornerUR", b"FrameCornerLL", b"FrameCornerLR"];

struct Host {
    log: Vec<u32>,
    now: u32,
    height: i32,
    shell_active: bool,
    adv: AdvancePositionState,
    images: [Option<(i32, i32)>; 7],
}

impl GuiHost for Host {
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        NAMES.iter().position(|n| *n == image).and_then(|i| self.images[i])
    }
    fn has_image(&self, image: &[u8]) -> bool {
        self.image_size(image).is_some()
    }
}

impl W3DMainMenuHost for Host {
    fn time_get_time(&self) -> u32 {
        self.now
    }
    fn display_height(&self) -> i32 {
        self.height
    }
    fn advance_position_state(&mut self) -> &mut AdvancePositionState {
        &mut self.adv
    }
    fn shell_menu_scheme_draw(&mut self, _out: &mut Vec<DrawItem>) {
        if self.shell_active {
            self.log.push(50);
        }
    }
    fn credits_draw(&mut self, _out: &mut Vec<DrawItem>) {}
    fn local_time_hh_mm_ss(&self) -> String {
        String::new()
    }
    fn main_menu_init(&mut self, _wm: &mut GameWindowManager, _layout: usize) {}
}

fn image_token(name: &[u8]) -> u32 {
    if let Some(i) = NAMES.iter().position(|n| *n == name) {
        return 1000 + i as u32;
    }
    std::str::from_utf8(&name[1..]).unwrap().parse().unwrap()
}

fn items_to_words(items: &[DrawItem], out: &mut Vec<u32>) {
    for it in items {
        match it {
            DrawItem::Line { x0, y0, x1, y1, color } => out.extend([60, *x0 as u32, *y0 as u32, *x1 as u32, *y1 as u32, *color as u32]),
            DrawItem::Image { image, x0, y0, x1, y1, color, grayscale, clip } => {
                out.extend([61, image_token(image), *x0 as u32, *y0 as u32, *x1 as u32, *y1 as u32, *color]);
                if *grayscale || clip.is_some() {
                    out.push(0xDEAD_0001);
                }
            }
            _ => out.push(0xDEAD_0002),
        }
    }
}

fn size(g: &mut Gen) -> i32 {
    match g.below(6) {
        0 => 0,
        1 => 10 * g.below(120) as i32,
        2 => 25 * g.below(50) as i32,
        3 => g.below(60) as i32,
        _ => g.below(1301) as i32,
    }
}

#[test]
fn w3d_main_menu_draws_match_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_0E);
    let mut chk = Check::new("W3DMainMenu draws");
    for _ in 0..cases(20_000) {
        let height = 1 + g.below(1200) as i32;
        let shell_active = g.below(2) == 0;
        let nwin = 1 + g.below(3) as usize;
        let mut input: Vec<u32> = vec![height as u32, u32::from(shell_active), nwin as u32];
        let mut wm = GameWindowManager::new((800, 600));
        let mut nh = NoHost;
        let mut ids: Vec<WinId> = Vec::new();
        for i in 0..nwin {
            let parent = if i > 0 && g.below(3) == 0 { Some(g.below(i as u32) as usize) } else { None };
            let (x, y) = (g.below(1461) as i32 - 60, g.below(1461) as i32 - 60);
            let (w, hh) = (size(&mut g), size(&mut g));
            let id = wm.win_create(&mut nh, parent, 0, x, y, w, hh, SystemFunc::Default, None);
            let tok = if g.below(3) == 0 { 0 } else { 1 + g.below(500) };
            if tok != 0 {
                wm.tree.windows[id].inst.enabled_draw_data[0].image = Some(format!("E{tok}").into_bytes());
            }
            let off = (g.below(41) as i32 - 20, g.below(41) as i32 - 20);
            wm.tree.windows[id].inst.image_offset = off;
            input.extend([parent.map_or(0, |p| p as u32 + 1), x as u32, y as u32, w as u32, hh as u32, tok, off.0 as u32, off.1 as u32]);
            ids.push(id);
        }
        let mut images = [None; 7];
        input.push(7);
        for im in images.iter_mut() {
            let exists = g.below(8) != 0;
            let (w, hh) = (1 + g.below(400) as i32, 1 + g.below(400) as i32);
            if exists {
                *im = Some((w, hh));
            }
            input.extend([u32::from(exists), w as u32, hh as u32]);
        }
        let mut host = Host { log: Vec::new(), now: 0, height, shell_active, adv: AdvancePositionState::default(), images };
        let ncalls = 1 + g.below(6);
        input.push(ncalls);
        let mut clock = g.u32();
        let mut rust = Vec::new();
        for _ in 0..ncalls {
            let kind = match g.below(10) {
                0..=3 => 0,
                4..=5 => 1,
                6 => 2,
                7 => 3,
                8 => 4,
                _ => g.below(2),
            };
            let w = g.below(nwin as u32);
            clock = match g.below(8) {
                0 => clock.wrapping_sub(g.below(3000)),
                1 => clock.wrapping_add(10_000),
                2 => clock.wrapping_add(9_999),
                _ => clock.wrapping_add(g.below(15_000)),
            };
            input.extend([kind, w, clock]);
            host.now = clock;
            host.log.clear();
            let mut items: Vec<DrawItem> = Vec::new();
            let win = ids[w as usize];
            match kind {
                0 => w3d_main_menu_draw(&wm, &mut host, win, &mut items),
                1 => w3d_main_menu_four_draw(&wm, &mut host, win, &mut items),
                2 => w3d_main_menu_map_border(&wm, &host, win, &mut items),
                3 => w3d_thin_border_draw(&wm, win, &mut items),
                _ => w3d_shell_menu_scheme_draw(&mut host, &mut items),
            }
            // the shell draw logs, the item draws are the words: in the C++ order (the shell word is the only log of kind 4)
            let mut words = Vec::new();
            words.extend(host.log.iter().copied());
            items_to_words(&items, &mut words);
            rust.extend(words);
            rust.push(100);
        }
        trace(&input);
        let mut buf = vec![0u32; 1 << 16];
        let len = unsafe { ref_ui3b_w3dmm_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "the forked C++ failed or the output buffer was too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
