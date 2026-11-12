// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DControlBar.cpp
//     (W3DDrawMapPreview, drawSkinnyBorder)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The map preview's draw callback (GitHub #94, #157): `W3DDrawMapPreview`, which the skirmish screens' map windows
//! name in their WND files, with its frame (`drawSkinnyBorder`).
//!
//! # As upstream reads it
//!
//! A window without its `MapMetaData *` user data draws as a plain window (`W3DGameWinDefaultDraw`) in the frame.
//! Otherwise the map's area keeps the map's aspect ratio inside the window (`findDrawPositions`): the bands left over
//! are filled black, edged by a grey (50, 50, 50) line; the area holds the window's enabled image 0 (the preview
//! `getMapPreviewImage` set, or `UnknownMap`), or is filled grey without one; over it go the tech buildings'
//! (`TecBuilding`) and supply docks' (`Cash`) markers, `SUPPLY_TECH_SIZE` square, where
//! `positionAdditionalImages` placed them (the start-spot buttons are child windows, drawn by themselves). Then the
//! frame: `FrameT` / `FrameB` along the top and bottom in 5-pixel pieces (a half and an adjusted half piece to finish),
//! `FrameL` / `FrameR` down the sides, and the four corners.
//!
//! # Upstream quirks that are kept
//!
//! - The band test compares `extent.width() / width` with `extent.height() / height`, `Real` over `Int`.
//! - The band rectangles are one pixel short of the area (`ul.y - pixelY - 1`) and can be of negative size.
//! - The markers are drawn whatever the map; `TheSupplyAndTechImageLocations` is the last `positionAdditionalImages`'.
//!
//! # Verification
//!
//! With `positionStartSpots` (the start spots, the markers), `getMapPreviewImage` (the copy and the new image) and
//! `findDrawPositions`, compared with the C++ (tests/differential_map_preview.rs, 2026-11-12): 20,000 cases, 42,461
//! start spots, 8,797 preview images, 2,063,560 draw calls, **0 mismatches**; planted bugs 9529 .. 9536, 9539, 9540
//! all caught.
//!
//! # Where this differs from upstream
//!
//! `TheDisplay`'s calls are [`DrawItem`]s for the client to render (as every window's draw,
//! [`crate::client::window_draw`]); an image the collection lacks (`NULL`) draws nothing, as `W3DDisplay::drawImage`.

use crate::client::map_util::{find_draw_positions, SUPPLY_TECH_SIZE};
use crate::client::window_draw::w3d_game_win_default_draw;
use crate::client::window_manager::{DrawItem, GameWindowManager};
use crate::client::window_script::{win_status, WinId};
use crate::common::base_type::{ICoord2D, Region3D};
use crate::common::ini::game_make_color;
use crate::logic::modules::plant;

/// `TheDisplay->drawImage(image, x0, y0, x1, y1)` (white, no grey; `NULL` draws nothing).
fn draw_image(out: &mut Vec<DrawItem>, wm: &GameWindowManager, img: &Option<Vec<u8>>, x0: i32, y0: i32, x1: i32, y1: i32) {
    if let Some(i) = img {
        out.push(DrawItem::Image { image: i.clone(), x0, y0, x1, y1, color: 0xFFFF_FFFF, grayscale: false, clip: wm.clip });
    }
}

/// `W3DDrawMapPreview(window, instData)`: `extent` is the user data's map's (`None`: no user data), `supply` and
/// `tech` `TheSupplyAndTechImageLocations`' lists, `find` `TheMappedImageCollection->findImageByName`.
pub fn w3d_draw_map_preview(wm: &GameWindowManager, window: WinId, extent: Option<&Region3D>, supply: &[ICoord2D], tech: &[ICoord2D], find: &dyn Fn(&[u8]) -> Option<Vec<u8>>, out: &mut Vec<DrawItem>) {
    let (pixel_x, pixel_y) = wm.tree.screen_position(window);
    let (width, height) = wm.tree.windows[window].size;
    let Some(extent) = extent else {
        w3d_game_win_default_draw(wm, window, out);
        draw_skinny_border(wm, pixel_x - 1, pixel_y - 1, width + 2, height + 2, find, out);
        return;
    };
    let (ul, lr) = find_draw_positions(pixel_x, pixel_y, width, height, extent);
    let fill_color = game_make_color(0, 0, 0, 255);
    let line_color = game_make_color(50, 50, 50, 255);
    let ew = extent.hi.x - extent.lo.x;
    let eh = extent.hi.y - extent.lo.y;
    // (plant 9530, differential builds only: `>` for `>=`)
    let horizontal = if plant(9530) { ew / width as f32 > eh / height as f32 } else { ew / width as f32 >= eh / height as f32 };
    if horizontal {
        out.push(DrawItem::FillRect { x: pixel_x, y: pixel_y, w: width, h: ul.y - pixel_y - 1, color: fill_color });
        // (plant 9531, differential builds only: the bottom band from the area's edge, not a pixel below)
        let by = if plant(9531) { lr.y } else { lr.y + 1 };
        out.push(DrawItem::FillRect { x: pixel_x, y: by, w: width, h: pixel_y + height - lr.y - 1, color: fill_color });
        out.push(DrawItem::Line { x0: pixel_x, y0: ul.y, x1: pixel_x + width, y1: ul.y, color: line_color });
        out.push(DrawItem::Line { x0: pixel_x, y0: lr.y + 1, x1: pixel_x + width, y1: lr.y + 1, color: line_color });
    } else {
        out.push(DrawItem::FillRect { x: pixel_x, y: pixel_y, w: ul.x - pixel_x - 1, h: height, color: fill_color });
        out.push(DrawItem::FillRect { x: lr.x + 1, y: pixel_y, w: width - (lr.x - pixel_x) - 1, h: height, color: fill_color });
        out.push(DrawItem::Line { x0: ul.x, y0: pixel_y, x1: ul.x, y1: pixel_y + height, color: line_color });
        out.push(DrawItem::Line { x0: lr.x + 1, y0: pixel_y, x1: lr.x + 1, y1: pixel_y + height, color: line_color });
    }
    let w = &wm.tree.windows[window];
    let enabled0 = w.inst.enabled_draw_data[0].image.clone();
    if w.status & win_status::IMAGE == 0 || enabled0.is_none() {
        out.push(DrawItem::FillRect { x: ul.x, y: ul.y, w: lr.x - ul.x, h: lr.y - ul.y, color: line_color });
    } else {
        draw_image(out, wm, &enabled0, ul.x, ul.y, lr.x, lr.y);
    }
    let image = find(b"TecBuilding");
    if image.is_some() {
        for it in tech {
            draw_image(out, wm, &image, pixel_x + it.x, pixel_y + it.y, pixel_x + it.x + SUPPLY_TECH_SIZE, pixel_y + it.y + SUPPLY_TECH_SIZE);
        }
    }
    let image = find(b"Cash");
    if image.is_some() {
        for it in supply {
            draw_image(out, wm, &image, pixel_x + it.x, pixel_y + it.y, pixel_x + it.x + SUPPLY_TECH_SIZE, pixel_y + it.y + SUPPLY_TECH_SIZE);
        }
    }
    draw_skinny_border(wm, pixel_x - 1, pixel_y - 1, width + 2, height + 2, find, out);
}

/// `drawSkinnyBorder(x, y, width, height)`.
pub fn draw_skinny_border(wm: &GameWindowManager, x: i32, y: i32, width: i32, height: i32, find: &dyn Fn(&[u8]) -> Option<Vec<u8>>, out: &mut Vec<DrawItem>) {
    const BORDER_LINE_SIZE: i32 = 5;
    let offset = 2;
    let offset_lower = 5;
    let (original_x, original_y) = (x, y);
    let max_x = x + width;
    let max_y = y + height;
    let size = 5;
    let half_size = size / 2;
    // the horizontal lines
    let mut y = original_y - offset;
    let y2 = max_y - offset_lower;
    let mut x2 = max_x - (offset_lower + BORDER_LINE_SIZE);
    let image1 = find(b"FrameT");
    let image2 = find(b"FrameB");
    let mut x = original_x + 3;
    while x <= x2 {
        draw_image(out, wm, &image1, x, y, x + size, y + size);
        draw_image(out, wm, &image2, x, y2, x + size, y2 + size);
        x += BORDER_LINE_SIZE;
    }
    x2 = max_x - 5;
    if (x2 - x) >= (BORDER_LINE_SIZE / 2) {
        draw_image(out, wm, &image1, x, y, x + half_size, y + size);
        draw_image(out, wm, &image2, x, y2, x + half_size, y2 + size);
        x += BORDER_LINE_SIZE / 2;
    }
    if x < x2 {
        // (plant 9529, differential builds only: the adjusted half piece without the even rounding)
        x -= (BORDER_LINE_SIZE / 2) - if plant(9529) { (x2 - x) + 1 } else { ((x2 - x) + 1) & !1 };
        draw_image(out, wm, &image1, x, y, x + half_size, y + size);
        draw_image(out, wm, &image2, x, y2, x + half_size, y2 + size);
    }
    // the vertical lines
    let image1 = find(b"FrameL");
    let image2 = find(b"FrameR");
    let x = original_x - offset;
    let x2 = max_x - offset_lower;
    let mut y2 = max_y - (offset_lower + BORDER_LINE_SIZE);
    y = original_y + 3;
    while y <= y2 {
        draw_image(out, wm, &image1, x, y, x + size, y + size);
        draw_image(out, wm, &image2, x2, y, x2 + size, y + size);
        y += BORDER_LINE_SIZE;
    }
    y2 = max_y - offset_lower;
    if (y2 - y) >= (BORDER_LINE_SIZE / 2) {
        draw_image(out, wm, &image1, x, y, x + size, y + half_size);
        draw_image(out, wm, &image2, x2, y, x2 + size, y + half_size);
        y += BORDER_LINE_SIZE / 2;
    }
    if y < y2 {
        y -= (BORDER_LINE_SIZE / 2) - (((y2 - y) + 1) & !1);
        draw_image(out, wm, &image1, x, y, x + size, y + half_size);
        draw_image(out, wm, &image2, x2, y, x2 + size, y + half_size);
    }
    // the corners
    for (name, cx, cy) in [(&b"FrameCornerUL"[..], original_x - 2, original_y - 2), (b"FrameCornerUR", max_x - 5, original_y - 2), (b"FrameCornerLL", original_x - 2, max_y - 5), (b"FrameCornerLR", max_x - 5, max_y - 5)] {
        let image1 = find(name);
        draw_image(out, wm, &image1, cx, cy, cx + size, cy + size);
    }
}
