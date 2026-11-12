// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/Common/W3DRadar.h
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/System/W3DRadar.cpp (OVERLAY_REFRESH_RATE,
//     legalRadarPoint, reconstructViewBox, radarToPixel, drawHeroIcon, drawViewBox, drawSingleBeaconEvent,
//     drawSingleGenericEvent, drawEvents, drawIcons, renderObjectList, interpolateColorForHeight, reset, newMap,
//     buildTerrainTexture, clearShroud, setShroudLevel, draw, refreshTerrain)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DControlBar.cpp (W3DLeftHUDDraw's
//     radar branch)
//   GeneralsMD/Code/GameEngine/Source/Common/System/Radar.cpp (RadarObject::isTemporarilyHidden)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! What the radar looks like (GitHub #35): `W3DRadar`'s three 128 x 128 textures (the terrain, the objects' overlay and
//! the shroud), its event markers and the view box, drawn into an RGBA image of the radar window ([`RadarImage`]) that
//! the Bevy client shows in `ControlBar.wnd`'s `LeftHUD` (`client/src/ui.rs`, `app.rs`).
//!
//! # As upstream draws it
//!
//! - **The terrain** (`buildTerrainTexture`, on a new map and on `refreshTerrain`): each radar cell averages the 3 x 3
//!   cells around it. Water (`isUnderwater` at the cell) takes `TheWaterTransparency`'s radar colour, darkened by depth;
//!   land takes the terrain texture's colour (`getTerrainColorAt`: the tile mipped to one pixel) shaded by height; a
//!   working bridge (not rubble) its road type's radar colour at the bridge's height.
//! - **The objects** (`renderObjectList`, every `OVERLAY_REFRESH_RATE` frames): a 2 x 2 dot per listed object, in its
//!   radar colour, unless it is hidden by stealth (`isTemporarilyHidden`), fogged or shrouded for the local player, a
//!   `LOCAL_UNIT_ONLY` object not the local player's (unless the local player is an observer), or an enemy's stealthed
//!   object neither detected nor disguised; a stealthed object it draws "twinkles" (its alpha cycles over a second).
//! - **The shroud** (`setShroudLevel` for each partition cell): black, alpha 255 shrouded, 127 fogged, 0 clear.
//! - **Then** the icons (the heroes' `HeroReticle`), the events (`drawEvents`: a spinning, shrinking triangle in the
//!   event's two colours, fading over its last half second; the beacon's spins the other way and is smaller; the
//!   "RadarEvent" sound when an event is first drawn), and the view box (the tactical view's corners on the terrain's
//!   average height, yellow above, darker below).
//! - **Where**: `W3DLeftHUDDraw` draws the radar, 1 pixel inside `LeftHUD`, only when it is forced on, or not hidden and
//!   the local player `hasRadar()` (which a radar building gives and low power takes away: `Player::hasRadar`'s
//!   `m_radarDisabled`); `findDrawPositions` keeps the map's aspect ratio, with black bars and grey edge lines.
//!
//! # Where this differs from upstream
//!
//! - **`TheDisplay`'s drawing is new**: lines, rectangles and the three images are drawn into [`RadarImage`] here, each
//!   blended over what is there, where the D3D device drew them. The images are sampled nearest (D3D filtered them), and
//!   a line is clipped pixel by pixel to its region where upstream clipped it (`ClipLine2D`) before drawing.
//! - **The shroud** is read from the partition for the local player when the overlay is rebuilt (`RadarDrawHost`), rather
//!   than kept from `setShroudLevel` calls; each cell is written as `setShroudLevel` writes it.
//! - ~~**The hero icon** (`HeroReticle`, a mapped image) is drawn as a hollow 7 x 7 square in white, centred where upstream
//!   centres the image, until the radar draws mapped images.~~ **Corrected 2026-11-12 (#162):** the hero icon is the
//!   `HeroReticle` mapped image ([`W3dRadar::hero_reticle`], which the client sets from the art's mapped images and
//!   texture page), drawn by [`RadarImage::draw_image`] (nearest texel, blended), where upstream's `drawHeroIcon` puts
//!   it ([`hero_icon_rect`]); nothing without the image. Upstream looks the image up once (a function `static`) at the
//!   first hero drawn and keeps that answer; here it is whatever the client last set.
//! - **The view box** is given the view's four corners on the average height by the caller (`getScreenCornerWorldPointsAtZ`);
//!   it is rebuilt whenever they move, where upstream rebuilds its offsets when the zoom or angle change and moves the box
//!   with the view's origin.
//! - **The frame** of `OVERLAY_REFRESH_RATE` is the count of [`W3dRadar::draw`] calls (upstream's client frame).
//!
//! # Upstream quirks that are kept
//!
//! - `buildTerrainTexture` shades land with `interpolateColorForHeight(color, z, terrainAverageZ, mapHi, mapLo)`: the
//!   map's top as the middle height and the average as the top, so nearly every point darkens toward the map's floor.
//! - `drawSingleGenericEvent` and `drawSingleBeaconEvent` add a size in pixels (`width / 2`, `width / 10`) to a position in
//!   radar cells, then convert the corners to pixels.
//! - The stealth twinkle's alpha goes through `REAL_TO_UNSIGNEDBYTE` of a negative number for most of the cycle.
//! - The water colour of an INI with no `RadarWaterColor` is 140, 140, 255 as reals, clamped to 1 (`client::water`).

use crate::client::radar::{Radar, RadarHost, MAX_RADAR_EVENTS, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH, RADAR_EVENT_BEACON_PULSE, RADAR_EVENT_FAKE, RADAR_PRIORITY_LOCAL_UNIT_ONLY};
use crate::common::base_type::{real_to_int, real_to_unsigned_byte, Coord3D, ICoord2D, RGBColor};
use crate::common::game_common::{CellShroudStatus, ObjectShroudStatus, Relationship, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::ObjectId;
use crate::common::ini::game_make_color;
use crate::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED};

/// `OVERLAY_REFRESH_RATE`: "over updates once this many frames".
pub const OVERLAY_REFRESH_RATE: u32 = 6;

/// `legalRadarPoint(px, py)`.
pub fn legal_radar_point(px: i32, py: i32) -> bool {
    !(px < 0 || py < 0 || px >= RADAR_CELL_WIDTH || py >= RADAR_CELL_HEIGHT)
}

/// What drawing the radar reads besides the radar's own questions.
pub trait RadarDrawHost: RadarHost {
    /// `TheTerrainVisual->getTerrainColorAt(x, y, &color)`.
    fn radar_terrain_color_at(&self, x: f32, y: f32) -> RGBColor;
    /// `TheWaterTransparency->m_radarColor`.
    fn radar_water_color(&self) -> RGBColor;
    /// `TheTerrainLogic->findBridgeAt(&pos)`, when its bridge object is not rubble: the road type's radar colour
    /// (`TheTerrainRoads->findBridge(name)->getRadarColor()`, `None` for no such type) and the bridge's height (its four
    /// corners' average Z).
    fn radar_working_bridge_at(&self, pos: &Coord3D) -> Option<(Option<RGBColor>, f32)>;
    /// `RadarObject::isTemporarilyHidden()`: the drawable's stealth look invisible, or the drawable effectively hidden.
    fn radar_temporarily_hidden(&self, obj: ObjectId) -> bool;
    /// `obj->isHero()`.
    fn radar_is_hero(&mut self, obj: ObjectId) -> bool;
    /// `obj->getShroudedStatus(playerIndex)`.
    fn radar_object_shrouded_status(&self, obj: ObjectId, player: i32) -> ObjectShroudStatus;
    /// `obj->testStatus(status)`.
    fn radar_test_status(&self, obj: ObjectId, status: ObjectStatusTypes) -> bool;
    /// The partition's cells (`W3DShroud`'s cells are the partition's): their counts and width, `None` without a shroud.
    fn radar_shroud_grid(&self) -> Option<(i32, i32, f32)>;
    /// `ThePartitionManager->getShroudStatusForPlayer(localPlayer, x, y)`, as `setShroudLevel` is told it.
    fn radar_cell_shroud(&self, x: i32, y: i32) -> CellShroudStatus;
}

/// An RGBA image of the radar window, row 0 at the top.
#[derive(Clone, Debug, PartialEq)]
pub struct RadarImage {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
    /// How many events were drawn for the first time with their sound due: `static AudioEventRTS eventSound("RadarEvent");
    /// TheAudio->addAudioEvent(&eventSound)` once each.
    pub event_sounds: u32,
}

impl RadarImage {
    fn new(width: i32, height: i32) -> Self {
        RadarImage { width, height, rgba: vec![0; (width.max(0) * height.max(0) * 4) as usize], event_sounds: 0 }
    }

    /// The pixel's RGBA.
    pub fn pixel(&self, x: i32, y: i32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }

    /// One pixel of `color` (a `Color`, ARGB) blended over the image, if inside both the image and `clip` (`(lo, hi)`,
    /// both inclusive, as `ClipLine2D`'s region is).
    fn blend(&mut self, x: i32, y: i32, color: u32, clip: Option<(ICoord2D, ICoord2D)>) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        if let Some((lo, hi)) = clip {
            if x < lo.x || y < lo.y || x > hi.x || y > hi.y {
                return;
            }
        }
        let a = (color >> 24) & 0xff;
        if a == 0 {
            return;
        }
        let i = ((y * self.width + x) * 4) as usize;
        let src = [(color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff];
        let dst_a = u32::from(self.rgba[i + 3]);
        // "over": out = src * a + dst * (1 - a)
        for (k, &s) in src.iter().enumerate() {
            let d = u32::from(self.rgba[i + k]);
            self.rgba[i + k] = ((s * a + d * (255 - a) + 127) / 255) as u8;
        }
        self.rgba[i + 3] = (a + dst_a * (255 - a) / 255).min(255) as u8;
    }

    /// `TheDisplay->drawFillRect(x, y, width, height, color)`.
    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: i32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.blend(xx, yy, color as u32, None);
            }
        }
    }

    /// `TheDisplay->drawLine(x0, y0, x1, y1, 1, c0, c1)` clipped to `clip`: a one-pixel line, its colour going from
    /// `c0` to `c1` (each channel, alpha too).
    fn line(&mut self, a: ICoord2D, b: ICoord2D, c0: i32, c1: i32, clip: Option<(ICoord2D, ICoord2D)>) {
        let (dx, dy) = ((b.x - a.x).abs(), -(b.y - a.y).abs());
        let (sx, sy) = (if a.x < b.x { 1 } else { -1 }, if a.y < b.y { 1 } else { -1 });
        let steps = dx.max(-dy).max(1);
        let (mut x, mut y, mut err) = (a.x, a.y, dx + dy);
        let mut n = 0;
        loop {
            let t = n as f32 / steps as f32;
            let mix = |s: u32| {
                let ch0 = ((c0 as u32) >> s) & 0xff;
                let ch1 = ((c1 as u32) >> s) & 0xff;
                ((ch0 as f32 + (ch1 as f32 - ch0 as f32) * t) as u32) << s
            };
            self.blend(x, y, mix(24) | mix(16) | mix(8) | mix(0), clip);
            if x == b.x && y == b.y {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
            n += 1;
        }
    }

    /// (#162) `TheDisplay->drawImage(image, x0, y0, x1, y1)` of a mapped image (`DRAW_IMAGE_ALPHA`, white: the texels as
    /// they are), stretched over the rectangle `[x0, x1) x [y0, y1)` and blended over the image; each pixel takes the
    /// texel nearest its centre's UV (`W3DDisplay::drawImage`'s quad, or its two triangles for an image rotated 90
    /// degrees clockwise: the screen's upper left takes the UV's upper right, its lower left the upper left). No clip
    /// (the radar does not enable one).
    pub fn draw_image(&mut self, image: &MappedImage, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (w, h) = (x1 - x0, y1 - y0);
        if w <= 0 || h <= 0 || image.page_width == 0 || image.page_height == 0 {
            return;
        }
        let [ul, ut, ur, ub] = image.uv;
        for py in y0..y1 {
            for px in x0..x1 {
                let s = (px - x0) as f32 / w as f32 + 0.5 / w as f32;
                let t = (py - y0) as f32 / h as f32 + 0.5 / h as f32;
                let (u, v) = if image.rotated { (ur - t * (ur - ul), ut + s * (ub - ut)) } else { (ul + s * (ur - ul), ut + t * (ub - ut)) };
                let tx = (u as i32).clamp(0, image.page_width as i32 - 1) as usize;
                let ty = (v as i32).clamp(0, image.page_height as i32 - 1) as usize;
                let i = (ty * image.page_width as usize + tx) * 4;
                let c = &image.rgba[i..i + 4];
                let argb = (u32::from(c[3]) << 24) | (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]);
                self.blend(px, py, argb, None);
            }
        }
    }

    /// `TheDisplay->drawImage(image, ul.x, ul.y, lr.x, lr.y)` of a radar texture whose UVs flip it vertically
    /// (`W3DRadar::init`: "(0,0) in the upper left" to the world's), sampled nearest.
    fn image(&mut self, tex: &[u32], ul: ICoord2D, lr: ICoord2D) {
        let (w, h) = (lr.x - ul.x, lr.y - ul.y);
        if w <= 0 || h <= 0 {
            return;
        }
        for py in ul.y..lr.y {
            let v = ((py - ul.y) * RADAR_CELL_HEIGHT / h).clamp(0, RADAR_CELL_HEIGHT - 1);
            let row = RADAR_CELL_HEIGHT - 1 - v;
            for px in ul.x..lr.x {
                let u = ((px - ul.x) * RADAR_CELL_WIDTH / w).clamp(0, RADAR_CELL_WIDTH - 1);
                self.blend(px, py, tex[(row * RADAR_CELL_WIDTH + u) as usize], None);
            }
        }
    }
}

/// (#162) A mapped image as `TheDisplay->drawImage` draws it: its texture page's texels and its rectangle on the page.
#[derive(Clone, Debug, PartialEq)]
pub struct MappedImage {
    /// `getImageWidth()`, `getImageHeight()`.
    pub image_size: (i32, i32),
    /// The page's size in texels and its RGBA texels, rows top first (shared: one page holds many images).
    pub page_width: u32,
    pub page_height: u32,
    pub rgba: std::sync::Arc<Vec<u8>>,
    /// `getUV()` in texels of the page: left, top, right, bottom.
    pub uv: [f32; 4],
    /// `IMAGE_STATUS_ROTATED_90_CLOCKWISE`.
    pub rotated: bool,
}

impl MappedImage {
    /// The image `image` of the collection on its loaded page `rgba` (`width` x `height`): its UVs over the page's size
    /// (UVs given before the INI's `TextureWidth` stay texels, as the client's other mapped images, `client/src/app.rs`).
    pub fn new(image: &crate::client::image::Image, width: u32, height: u32, rgba: std::sync::Arc<Vec<u8>>) -> Self {
        let to_px = |u: f32, size: u32, given: i32| if given != 0 { u * size as f32 } else { u };
        MappedImage {
            image_size: image.image_size,
            page_width: width,
            page_height: height,
            rgba,
            uv: [
                to_px(image.uv[0], width, image.texture_size.0),
                to_px(image.uv[1], height, image.texture_size.1),
                to_px(image.uv[2], width, image.texture_size.0),
                to_px(image.uv[3], height, image.texture_size.1),
            ],
            rotated: image.is_rotated(),
        }
    }
}

/// (#162) `drawHeroIcon`'s rectangle for `pos` (the arguments of its `TheDisplay->drawImage`): `pos` in radar cells
/// (`Int` truncations of `pos / (extent / RADAR_CELL_*)`), to pixels of the area `(px, py, w, h)` (`radarToPixel`),
/// shifted "from an upper left to a center focus" by `iconWidth / 2 - 1` and `iconHeight / 2`, `iconWidth` by
/// `iconHeight` large. `None` where upstream is undefined: a float to `Int` conversion out of range (or of a NaN), or
/// an `Int` overflow.
pub fn hero_icon_rect(map_extent: (f32, f32), pos: &Coord3D, area: (i32, i32, i32, i32), icon: (i32, i32)) -> Option<(i32, i32, i32, i32)> {
    let (px, py, w, h) = area;
    let to_int = |v: f32| if v.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&v) { None } else { Some(v as i32) };
    let ulx = to_int(pos.x / (map_extent.0 / RADAR_CELL_WIDTH as f32))?;
    let uly = to_int(pos.y / (map_extent.1 / RADAR_CELL_HEIGHT as f32))?;
    // radarToPixel
    let x = ulx.checked_mul(w)?.checked_div(RADAR_CELL_WIDTH)?.checked_add(px)?;
    let y = (RADAR_CELL_HEIGHT - 1).checked_sub(uly)?.checked_mul(h)?.checked_div(RADAR_CELL_HEIGHT)?.checked_add(py)?;
    let (iw, ih) = icon;
    let shift = if crate::logic::modules::plant(9592) { iw / 2 } else { (iw / 2) - 1 };
    let x = x.checked_sub(shift)?;
    let y = y.checked_sub(ih / 2)?;
    Some((x, y, x.checked_add(iw)?, y.checked_add(ih)?))
}

/// `class W3DRadar`'s textures and view box.
#[derive(Clone, Debug, PartialEq)]
pub struct W3dRadar {
    /// `m_terrainTexture`, ARGB, row `y` is radar row `y` (south at row 0).
    pub terrain_texture: Vec<u32>,
    /// `m_overlayTexture`.
    pub overlay_texture: Vec<u32>,
    /// `m_shroudTexture`.
    pub shroud_texture: Vec<u32>,
    /// `m_viewBox`: the first corner (0, 0), then each corner's offset in radar cells from the one before.
    pub view_box: [ICoord2D; 4],
    /// The corners the view box was made from (see the module docs).
    pub view_corners: Option<[Coord3D; 4]>,
    /// `m_reconstructViewBox`.
    pub reconstruct_view_box: bool,
    /// `m_cachedHeroPosList`.
    pub cached_hero_pos_list: Vec<Coord3D>,
    /// `TheGameClient->getFrame()`, as counted by [`W3dRadar::draw`].
    pub client_frame: u32,
    /// (#162) `drawHeroIcon`'s `static const Image *image = TheMappedImageCollection->findImageByName("HeroReticle")`:
    /// the image with its page, set by the client once the art's mapped images and the page are read (`None`: not
    /// found, nothing drawn, as upstream with a null image).
    pub hero_reticle: Option<MappedImage>,
}

impl Default for W3dRadar {
    fn default() -> Self {
        Self::new()
    }
}

const TEXELS: usize = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;

impl W3dRadar {
    /// `W3DRadar()` and `init()`: the three textures, cleared.
    pub fn new() -> Self {
        W3dRadar {
            terrain_texture: vec![0; TEXELS],
            overlay_texture: vec![0; TEXELS],
            shroud_texture: vec![0; TEXELS],
            view_box: [ICoord2D::default(); 4],
            view_corners: None,
            reconstruct_view_box: true,
            cached_hero_pos_list: Vec::new(),
            client_frame: 0,
            hero_reticle: None,
        }
    }

    /// `W3DRadar::reset()`: `Radar::reset`, the terrain and overlay cleared, the shroud cleared (`clearShroud`).
    pub fn reset(&mut self, radar: &mut Radar) {
        radar.reset();
        self.terrain_texture.fill(0);
        self.overlay_texture.fill(0);
        self.clear_shroud();
    }

    /// `W3DRadar::newMap(terrain)`: `Radar::newMap` (whose reset clears the textures), then `buildTerrainTexture`.
    pub fn new_map(&mut self, radar: &mut Radar, host: &impl RadarDrawHost) {
        self.terrain_texture.fill(0);
        self.overlay_texture.fill(0);
        self.clear_shroud();
        radar.new_map(host);
        self.build_terrain_texture(radar, host);
    }

    /// `W3DRadar::refreshTerrain(terrain)`: `Radar::refreshTerrain`, then the terrain texture rebuilt.
    pub fn refresh_terrain(&mut self, radar: &mut Radar, host: &impl RadarDrawHost) {
        radar.refresh_terrain();
        self.build_terrain_texture(radar, host);
    }

    /// `interpolateColorForHeight(color, height, hiZ, midZ, loZ)`: lighter toward 95 % white above `mid_z`, darker
    /// toward 60 % black below it, in proportion to the height's place between `mid_z` and the bound; clamped to [0, 1].
    pub fn interpolate_color_for_height(color: &mut RGBColor, height: f32, mut hi_z: f32, mid_z: f32, mut lo_z: f32) {
        let how_bright = 0.95f32; // bigger is brighter (0.0 to 1.0)
        let how_dark = 0.60f32; // bigger is darker (0.0 to 1.0)
        // "sanity on map height (flat maps bomb)"
        if hi_z == mid_z {
            hi_z = mid_z + 0.1;
        }
        if mid_z == lo_z {
            lo_z = mid_z - 0.1;
        }
        if hi_z == lo_z {
            hi_z = lo_z + 0.2;
        }
        let t;
        let target;
        if height >= mid_z {
            t = (height - mid_z) / (hi_z - mid_z);
            target = RGBColor {
                red: color.red + (1.0 - color.red) * how_bright,
                green: color.green + (1.0 - color.green) * how_bright,
                blue: color.blue + (1.0 - color.blue) * how_bright,
            };
        } else {
            t = (mid_z - height) / (mid_z - lo_z);
            target = RGBColor {
                red: color.red + (0.0 - color.red) * how_dark,
                green: color.green + (0.0 - color.green) * how_dark,
                blue: color.blue + (0.0 - color.blue) * how_dark,
            };
        }
        color.red += (target.red - color.red) * t;
        color.green += (target.green - color.green) * t;
        color.blue += (target.blue - color.blue) * t;
        color.red = color.red.clamp(0.0, 1.0);
        color.green = color.green.clamp(0.0, 1.0);
        color.blue = color.blue.clamp(0.0, 1.0);
    }

    /// `buildTerrainTexture(terrain)`: every radar cell's colour (see the module docs). The view box is rebuilt next.
    pub fn build_terrain_texture(&mut self, radar: &Radar, host: &impl RadarDrawHost) {
        self.reconstruct_view_box = true;
        let water_color = host.radar_water_color();
        let (tw, th) = (RADAR_CELL_WIDTH, RADAR_CELL_HEIGHT);
        let lo_z = radar.map_extent.lo.z;
        let hi_z = radar.map_extent.hi.z;
        for y in 0..th {
            for x in 0..tw {
                let world_point = radar.radar_to_world_2d(&ICoord2D { x, y });
                // "check to see if this point is part of a working bridge"
                let bridge = host.radar_working_bridge_at(&world_point);
                let centre = host.radar_is_underwater(world_point.x, world_point.y);
                let mut sample = RGBColor::default();
                let mut samples = 0i32;
                if bridge.is_none() && centre.underwater {
                    // (`waterZ` is written when the point is under water)
                    let water_z = centre.water_z.unwrap_or(0.0);
                    for j in y - 1..=y + 1 {
                        if j < 0 || j >= th {
                            continue;
                        }
                        for i in x - 1..=x + 1 {
                            if i < 0 || i >= tw {
                                continue;
                            }
                            let p = radar.radar_to_world_2d(&ICoord2D { x: i, y: j });
                            let u = host.radar_is_underwater(p.x, p.y);
                            if u.underwater {
                                let mut c = water_color;
                                Self::interpolate_color_for_height(&mut c, u.terrain_z, water_z, water_z, lo_z);
                                sample.red += c.red;
                                sample.green += c.green;
                                sample.blue += c.blue;
                                samples += 1;
                            }
                        }
                    }
                } else {
                    for j in y - 1..=y + 1 {
                        if j < 0 || j >= th {
                            continue;
                        }
                        for i in x - 1..=x + 1 {
                            if i < 0 || i >= tw {
                                continue;
                            }
                            let p = radar.radar_to_world(&ICoord2D { x: i, y: j }, host);
                            let mut c;
                            if let Some((road_color, bridge_height)) = bridge {
                                c = road_color.unwrap_or_else(|| {
                                    let mut w = RGBColor::default();
                                    w.set_from_int(-1);
                                    w
                                });
                                // "interpolate the color, but use the bridge height, not the terrain height"
                                Self::interpolate_color_for_height(&mut c, bridge_height, radar.terrain_average_z, hi_z, lo_z);
                            } else {
                                c = host.radar_terrain_color_at(p.x, p.y);
                                Self::interpolate_color_for_height(&mut c, p.z, radar.terrain_average_z, hi_z, lo_z);
                            }
                            sample.red += c.red;
                            sample.green += c.green;
                            sample.blue += c.blue;
                            samples += 1;
                        }
                    }
                }
                if samples == 0 {
                    samples = 1;
                }
                let n = samples as f32;
                let c = RGBColor { red: sample.red / n, green: sample.green / n, blue: sample.blue / n };
                // (`GameMakeColor(color.red * 255, ...)`: each a Real converted to an UnsignedByte)
                self.terrain_texture[(y * tw + x) as usize] = game_make_color((c.red * 255.0) as u8, (c.green * 255.0) as u8, (c.blue * 255.0) as u8, 255) as u32;
            }
        }
    }

    /// `clearShroud()`: every texel transparent ("shroud will make black").
    pub fn clear_shroud(&mut self) {
        self.shroud_texture.fill(game_make_color(0, 0, 0, 0) as u32);
    }

    /// `setShroudLevel(shroudX, shroudY, setting)`: the radar cells the shroud cell covers (its corners through
    /// `worldToRadar`), black at alpha 255 shrouded, 127 fogged, 0 clear.
    pub fn set_shroud_level(&mut self, radar: &Radar, shroud_x: i32, shroud_y: i32, cell_width: f32, setting: CellShroudStatus) {
        let map_min_x = (shroud_x as f32 * cell_width) as i32;
        let map_min_y = (shroud_y as f32 * cell_width) as i32;
        let map_max_x = ((shroud_x + 1) as f32 * cell_width) as i32;
        let map_max_y = ((shroud_y + 1) as f32 * cell_width) as i32;
        let lo = radar.world_to_radar(&Coord3D { x: map_min_x as f32, y: map_min_y as f32, z: 0.0 });
        let hi = radar.world_to_radar(&Coord3D { x: map_max_x as f32, y: map_max_y as f32, z: 0.0 });
        let alpha = match setting {
            CellShroudStatus::Shrouded => 255,
            CellShroudStatus::Fogged => 127,
            CellShroudStatus::Clear => 0,
        };
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                if legal_radar_point(x, y) {
                    self.shroud_texture[(y * RADAR_CELL_WIDTH + x) as usize] = game_make_color(0, 0, 0, alpha) as u32;
                }
            }
        }
    }

    /// The shroud, as the partition's `setShroudLevel` calls would have left it for the local player (see the module
    /// docs).
    pub fn rebuild_shroud(&mut self, radar: &Radar, host: &impl RadarDrawHost) {
        let Some((cx, cy, width)) = host.radar_shroud_grid() else { return };
        self.clear_shroud();
        for y in 0..cy {
            for x in 0..cx {
                self.set_shroud_level(radar, x, y, width, host.radar_cell_shroud(x, y));
            }
        }
    }

    /// `renderObjectList(listHead, texture, calcHero)`: the list's dots into the overlay (see the module docs).
    fn render_object_list(&mut self, radar: &Radar, local: bool, host: &mut impl RadarDrawHost) {
        let list = if local { &radar.local_object_list } else { &radar.object_list };
        let player = host.radar_local_player();
        let player_index = player.unwrap_or(0);
        if local {
            // "clear all entries from the cached hero object list"
            self.cached_hero_pos_list.clear();
        }
        let frame = host.radar_frame();
        for r in list {
            let obj = r.object;
            if host.radar_temporarily_hidden(obj) {
                continue;
            }
            let Some(pos) = host.radar_object_position(obj) else { continue };
            if local && host.radar_is_hero(obj) {
                self.cached_hero_pos_list.push(pos);
            }
            let mut skip = false;
            // "object is fogged or shrouded, don't render it."
            if (host.radar_object_shrouded_status(obj, player_index) as i32) > (ObjectShroudStatus::PartialClear as i32) {
                skip = true;
            }
            if host.radar_priority(obj) == RADAR_PRIORITY_LOCAL_UNIT_ONLY
                && host.radar_controlling_player(obj) != player
                && player.is_some_and(|p| host.radar_is_player_active(p))
            {
                skip = true;
            }
            let mut radar_point = ICoord2D {
                x: (pos.x / (radar.map_extent.width() / RADAR_CELL_WIDTH as f32)) as i32,
                y: (pos.y / (radar.map_extent.height() / RADAR_CELL_HEIGHT as f32)) as i32,
            };
            if skip {
                continue;
            }
            let mut c = r.color as u32;
            if host.radar_test_status(obj, OBJECT_STATUS_STEALTHED) {
                if player.is_some_and(|p| host.radar_player_relationship_to_object(p, obj) == Relationship::Enemies)
                    && !host.radar_test_status(obj, OBJECT_STATUS_DETECTED)
                    && !host.radar_test_status(obj, OBJECT_STATUS_DISGUISED)
                {
                    continue;
                }
                let frames_for_transition = LOGICFRAMES_PER_SECOND as u32;
                let min_alpha = 32.0f32;
                let alpha_scale = (frame % frames_for_transition) as f32 / (frames_for_transition as f32 / 2.0);
                let a = if alpha_scale > 0.0 {
                    real_to_unsigned_byte(((alpha_scale - 1.0) * (255.0 - min_alpha)) + min_alpha)
                } else {
                    real_to_unsigned_byte((alpha_scale * (255.0 - min_alpha)) + min_alpha)
                };
                c = (c & 0x00ff_ffff) | (u32::from(a) << 24);
            }
            // "draw the blip, but make sure the points are legal"
            for step in 0..4 {
                if legal_radar_point(radar_point.x, radar_point.y) {
                    self.overlay_texture[(radar_point.y * RADAR_CELL_WIDTH + radar_point.x) as usize] = c;
                }
                match step {
                    0 => radar_point.y += 1,
                    1 => radar_point.x += 1,
                    2 => radar_point.y -= 1,
                    _ => {}
                }
            }
        }
    }

    /// The overlay rebuilt (`surface->Clear()`, then the other list, then the local list with the heroes), and the shroud.
    pub fn refresh_overlay(&mut self, radar: &Radar, host: &mut impl RadarDrawHost) {
        self.overlay_texture.fill(0);
        self.render_object_list(radar, false, host);
        self.render_object_list(radar, true, host);
        self.rebuild_shroud(radar, host);
    }

    /// `radarToPixel(radar, pixel, ulX, ulY, width, height)`: y inverted ("+x=right and -y=down").
    pub fn radar_to_pixel(radar: &ICoord2D, ul_x: i32, ul_y: i32, width: i32, height: i32) -> ICoord2D {
        ICoord2D { x: (radar.x * width / RADAR_CELL_WIDTH) + ul_x, y: ((RADAR_CELL_HEIGHT - 1 - radar.y) * height / RADAR_CELL_HEIGHT) + ul_y }
    }

    /// `reconstructViewBox()`: the four corners (upper left, upper right, lower right, lower left) in radar cells, as the
    /// first and each one's offset from the one before.
    fn reconstruct_view_box(&mut self, radar: &Radar, corners: &[Coord3D; 4]) {
        let mut cells = [ICoord2D::default(); 4];
        for i in 0..4 {
            cells[i] = ICoord2D {
                x: (corners[i].x / (radar.map_extent.width() / RADAR_CELL_WIDTH as f32)) as i32,
                y: (corners[i].y / (radar.map_extent.height() / RADAR_CELL_HEIGHT as f32)) as i32,
            };
            self.view_box[i] = if i == 0 { ICoord2D::default() } else { ICoord2D { x: cells[i].x - cells[i - 1].x, y: cells[i].y - cells[i - 1].y } };
        }
        self.view_corners = Some(*corners);
        self.reconstruct_view_box = false;
    }

    /// `drawViewBox(pixelX, pixelY, width, height)`: the box from the view's upper left corner, clipped to the whole
    /// radar window (`window` its top left and size in the image).
    fn draw_view_box(&self, img: &mut RadarImage, radar: &Radar, ul_world: &Coord3D, area: (i32, i32, i32, i32), window: (ICoord2D, ICoord2D)) {
        let (px, py, w, h) = area;
        let top_color = game_make_color(225, 225, 0, 255);
        let bottom_color = game_make_color(158, 158, 0, 255);
        let clip = Some((window.0, ICoord2D { x: window.0.x + window.1.x, y: window.0.y + window.1.y }));
        let ul_radar = ICoord2D {
            x: (ul_world.x / (radar.map_extent.width() / RADAR_CELL_WIDTH as f32)) as i32,
            y: (ul_world.y / (radar.map_extent.height() / RADAR_CELL_HEIGHT as f32)) as i32,
        };
        let ul_start = Self::radar_to_pixel(&ul_radar, px, py, w, h);
        // top line
        let mut r = ICoord2D { x: ul_radar.x + self.view_box[1].x, y: ul_radar.y + self.view_box[1].y };
        let mut end = Self::radar_to_pixel(&r, px, py, w, h);
        img.line(ul_start, end, top_color, top_color, clip);
        // right line
        let mut start = end;
        r.x += self.view_box[2].x;
        r.y += self.view_box[2].y;
        end = Self::radar_to_pixel(&r, px, py, w, h);
        img.line(start, end, top_color, bottom_color, clip);
        // bottom line
        start = end;
        r.x += self.view_box[3].x;
        r.y += self.view_box[3].y;
        end = Self::radar_to_pixel(&r, px, py, w, h);
        img.line(start, end, bottom_color, bottom_color, clip);
        // left line
        img.line(end, ul_start, bottom_color, top_color, clip);
    }

    /// The triangle of `drawSingleGenericEvent` (`beacon` false) or `drawSingleBeaconEvent` and its two colours, at
    /// `current_frame`, in pixels of the area `(px, py, w, h)`.
    pub fn event_triangle(radar: &Radar, index: usize, beacon: bool, current_frame: u32, area: (i32, i32, i32, i32)) -> ([ICoord2D; 3], i32, i32) {
        let (px, py, w, h) = area;
        let e = &radar.event[index];
        let max_event_size = if beacon { w as f32 / 10.0 } else { w as f32 / 2.0 };
        let min_event_size = 6;
        let time_from_full_size_to_small_size = LOGICFRAMES_PER_SECOND as f32 * 1.5;
        let total_angles_to_spin = 2.0 * std::f32::consts::PI;
        let frame_diff = current_frame.wrapping_sub(e.create_frame);
        let mut event_size = real_to_int(max_event_size * (1.0 - frame_diff as f32 / time_from_full_size_to_small_size));
        if event_size < min_event_size {
            event_size = min_event_size;
        }
        let spin = total_angles_to_spin * (frame_diff as f32 / time_from_full_size_to_small_size);
        let add_angle = if beacon { -spin } else { spin };
        let corner = |angle: f32| {
            let r = ICoord2D {
                x: real_to_int((crate::ww::wwmath::cos(angle) * event_size as f32) + e.radar_loc.x as f32),
                y: real_to_int((crate::ww::wwmath::sin(angle) * event_size as f32) + e.radar_loc.y as f32),
            };
            Self::radar_to_pixel(&r, px, py, w, h)
        };
        let pi = std::f32::consts::PI;
        let tri = [corner(0.0 - add_angle), corner(2.0 * pi / 3.0 - add_angle), corner(-2.0 * pi / 3.0 - add_angle)];
        let fade = |c: &crate::common::base_type::RGBAColorInt| {
            let mut a = c.alpha as u8;
            if current_frame > e.fade_frame {
                a = real_to_unsigned_byte(f32::from(a) * (1.0 - (current_frame - e.fade_frame) as f32 / e.die_frame.wrapping_sub(e.fade_frame) as f32));
            }
            game_make_color(c.red as u8, c.green as u8, c.blue as u8, a)
        };
        (tri, fade(&e.color1), fade(&e.color2))
    }

    /// `drawEvents(pixelX, pixelY, width, height)`: each active event but a fake one, its sound due the first time.
    fn draw_events(&self, img: &mut RadarImage, radar: &mut Radar, current_frame: u32, area: (i32, i32, i32, i32)) {
        let (px, py, w, h) = area;
        let clip = Some((ICoord2D { x: px, y: py }, ICoord2D { x: px + w, y: py + h }));
        for i in 0..MAX_RADAR_EVENTS {
            let e = radar.event[i];
            if !(e.active && e.event_type != RADAR_EVENT_FAKE) {
                continue;
            }
            // "if we haven't played the sound for this event, do it now that we can see it"
            if !e.sound_played && e.event_type != RADAR_EVENT_BEACON_PULSE {
                img.event_sounds += 1;
            }
            radar.event[i].sound_played = true;
            let (tri, c0, c1) = Self::event_triangle(radar, i, e.event_type == RADAR_EVENT_BEACON_PULSE, current_frame, area);
            img.line(tri[0], tri[1], c0, c1, clip);
            img.line(tri[1], tri[2], c0, c1, clip);
            img.line(tri[2], tri[0], c0, c1, clip);
        }
    }

    /// `drawHeroIcon(pixelX, pixelY, width, height, pos)`: the `HeroReticle` image ([`W3dRadar::hero_reticle`]; nothing
    /// without it) at [`hero_icon_rect`] (nothing where that is undefined upstream).
    fn draw_hero_icon(&self, img: &mut RadarImage, radar: &Radar, pos: &Coord3D, area: (i32, i32, i32, i32)) {
        let Some(image) = &self.hero_reticle else { return };
        let extent = (radar.map_extent.width(), radar.map_extent.height());
        if let Some((x0, y0, x1, y1)) = hero_icon_rect(extent, pos, area, image.image_size) {
            img.draw_image(image, x0, y0, x1, y1);
        }
    }

    /// `W3DRadar::draw(pixelX, pixelY, width, height)` into `img`: nothing when the local player has no radar and it is
    /// not forced; else the bars, the terrain, the overlay (rebuilt every `OVERLAY_REFRESH_RATE` calls), the shroud, the
    /// icons, the events and the view box (from `view`, the tactical view's corners at the terrain's average height:
    /// upper left, upper right, lower right, lower left).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, img: &mut RadarImage, radar: &mut Radar, host: &mut impl RadarDrawHost, pixel_x: i32, pixel_y: i32, width: i32, height: i32, view: Option<[Coord3D; 4]>, window: (ICoord2D, ICoord2D)) {
        if radar.terrain_refresh_wanted {
            radar.terrain_refresh_wanted = false;
            self.build_terrain_texture(radar, host);
        }
        let local = host.radar_local_player();
        if !local.is_some_and(|p| host.radar_player_has_radar(p)) && !radar.radar_force_on {
            return;
        }
        if width <= 0 || height <= 0 {
            return;
        }
        let (ul, lr) = radar.find_draw_positions(pixel_x, pixel_y, width, height);
        let (sw, sh) = (lr.x - ul.x, lr.y - ul.y);
        // "draw black border areas where we need map"
        let fill_color = game_make_color(0, 0, 0, 255);
        let line_color = game_make_color(50, 50, 50, 255);
        if radar.map_extent.width() / width as f32 >= radar.map_extent.height() / height as f32 {
            img.fill_rect(pixel_x, pixel_y, width, ul.y - pixel_y - 1, fill_color);
            img.fill_rect(pixel_x, lr.y + 1, width, pixel_y + height - lr.y - 1, fill_color);
            img.line(ICoord2D { x: pixel_x, y: ul.y }, ICoord2D { x: pixel_x + width, y: ul.y }, line_color, line_color, None);
            img.line(ICoord2D { x: pixel_x, y: lr.y + 1 }, ICoord2D { x: pixel_x + width, y: lr.y + 1 }, line_color, line_color, None);
        } else {
            img.fill_rect(pixel_x, pixel_y, ul.x - pixel_x - 1, height, fill_color);
            img.fill_rect(lr.x + 1, pixel_y, width - (lr.x - pixel_x) - 1, height, fill_color);
            img.line(ICoord2D { x: ul.x, y: pixel_y }, ICoord2D { x: ul.x, y: pixel_y + height }, line_color, line_color, None);
            img.line(ICoord2D { x: lr.x + 1, y: pixel_y }, ICoord2D { x: lr.x + 1, y: pixel_y + height }, line_color, line_color, None);
        }
        img.image(&self.terrain_texture, ul, lr);
        // "refresh the overlay texture once every so many frames"
        if self.client_frame.is_multiple_of(OVERLAY_REFRESH_RATE) {
            self.refresh_overlay(radar, host);
        }
        self.client_frame = self.client_frame.wrapping_add(1);
        img.image(&self.overlay_texture, ul, lr);
        img.image(&self.shroud_texture, ul, lr);
        let area = (ul.x, ul.y, sw, sh);
        for p in self.cached_hero_pos_list.clone() {
            self.draw_hero_icon(img, radar, &p, area);
        }
        let frame = host.radar_frame();
        self.draw_events(img, radar, frame, area);
        if let Some(corners) = view {
            if self.reconstruct_view_box || self.view_corners != Some(corners) {
                self.reconstruct_view_box(radar, &corners);
            }
            self.draw_view_box(img, radar, &corners[0], area, window);
        }
    }

    /// `W3DLeftHUDDraw`'s radar: in a window of `size`, the radar 1 pixel inside it when it is forced on, or not hidden
    /// and the local player has one (`None`: nothing drawn, the window shows what is behind it).
    pub fn draw_left_hud(&mut self, radar: &mut Radar, host: &mut impl RadarDrawHost, size: ICoord2D, view: Option<[Coord3D; 4]>) -> Option<RadarImage> {
        if !radar.is_on_for(host) {
            return None;
        }
        let mut img = RadarImage::new(size.x, size.y);
        self.draw(&mut img, radar, host, 1, 1, size.x - 2, size.y - 2, view, (ICoord2D::default(), size));
        Some(img)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::radar::tests::Host;
    use crate::client::radar::RADAR_EVENT_UNDER_ATTACK;
    use std::collections::BTreeMap;

    /// The radar test host with the drawing's questions: grass-green terrain, blue water, no bridge; a 4 x 2 partition
    /// of 250-unit cells, the west half shrouded and the cell (2, 0) fogged.
    struct DrawHost {
        base: Host,
        hidden: Vec<ObjectId>,
        status: BTreeMap<ObjectId, Vec<ObjectStatusTypes>>,
    }

    impl RadarHost for DrawHost {
        fn radar_frame(&self) -> u32 {
            self.base.radar_frame()
        }
        fn radar_local_player(&self) -> Option<i32> {
            self.base.radar_local_player()
        }
        fn radar_priority(&mut self, obj: ObjectId) -> u8 {
            self.base.radar_priority(obj)
        }
        fn radar_object_position(&self, obj: ObjectId) -> Option<Coord3D> {
            self.base.radar_object_position(obj)
        }
        fn radar_is_locally_controlled(&self, obj: ObjectId) -> bool {
            self.base.radar_is_locally_controlled(obj)
        }
        fn radar_controlling_player(&self, obj: ObjectId) -> Option<i32> {
            self.base.radar_controlling_player(obj)
        }
        fn radar_indicator_color(&self, obj: ObjectId) -> i32 {
            self.base.radar_indicator_color(obj)
        }
        fn radar_is_kind_of(&self, obj: ObjectId, kind: crate::common::kind_of::KindOfType) -> bool {
            self.base.radar_is_kind_of(obj, kind)
        }
        fn radar_disguised_player(&mut self, obj: ObjectId) -> Option<i32> {
            self.base.radar_disguised_player(obj)
        }
        fn radar_contain_apparent_player(&mut self, obj: ObjectId, observer: Option<i32>) -> Option<Option<i32>> {
            self.base.radar_contain_apparent_player(obj, observer)
        }
        fn radar_player_color(&self, player: i32) -> i32 {
            self.base.radar_player_color(player)
        }
        fn radar_player_exists(&self, player: i32) -> bool {
            self.base.radar_player_exists(player)
        }
        fn radar_player_relationship_to_player(&self, player: i32, other: i32) -> Relationship {
            self.base.radar_player_relationship_to_player(player, other)
        }
        fn radar_player_relationship_to_object(&self, player: i32, obj: ObjectId) -> Relationship {
            self.base.radar_player_relationship_to_object(player, obj)
        }
        fn radar_is_player_active(&self, player: i32) -> bool {
            self.base.radar_is_player_active(player)
        }
        fn radar_player_has_radar(&self, player: i32) -> bool {
            self.base.radar_player_has_radar(player)
        }
        fn radar_map_extent(&self) -> crate::common::base_type::Region3D {
            self.base.radar_map_extent()
        }
        fn radar_ground_height(&self, x: f32, y: f32) -> f32 {
            self.base.radar_ground_height(x, y)
        }
        fn radar_is_underwater(&self, x: f32, y: f32) -> crate::client::radar::RadarUnderwater {
            self.base.radar_is_underwater(x, y)
        }
    }

    impl RadarDrawHost for DrawHost {
        fn radar_terrain_color_at(&self, _x: f32, _y: f32) -> RGBColor {
            RGBColor { red: 0.25, green: 0.5, blue: 0.125 }
        }
        fn radar_water_color(&self) -> RGBColor {
            RGBColor { red: 0.2, green: 0.4, blue: 0.8 }
        }
        fn radar_working_bridge_at(&self, _pos: &Coord3D) -> Option<(Option<RGBColor>, f32)> {
            None
        }
        fn radar_temporarily_hidden(&self, obj: ObjectId) -> bool {
            self.hidden.contains(&obj)
        }
        fn radar_is_hero(&mut self, _obj: ObjectId) -> bool {
            false
        }
        fn radar_object_shrouded_status(&self, obj: ObjectId, _player: i32) -> ObjectShroudStatus {
            match self.radar_object_position(obj) {
                Some(p) if p.x < 500.0 => ObjectShroudStatus::Shrouded,
                _ => ObjectShroudStatus::Clear,
            }
        }
        fn radar_test_status(&self, obj: ObjectId, status: ObjectStatusTypes) -> bool {
            self.status.get(&obj).is_some_and(|s| s.contains(&status))
        }
        fn radar_shroud_grid(&self) -> Option<(i32, i32, f32)> {
            Some((4, 2, 250.0))
        }
        fn radar_cell_shroud(&self, x: i32, y: i32) -> CellShroudStatus {
            match (x, y) {
                (0 | 1, _) => CellShroudStatus::Shrouded,
                (2, 0) => CellShroudStatus::Fogged,
                _ => CellShroudStatus::Clear,
            }
        }
    }

    fn host() -> DrawHost {
        let mut base = Host::new();
        base.has_radar = true;
        DrawHost { base, hidden: Vec::new(), status: BTreeMap::new() }
    }

    /// #35: `interpolateColorForHeight`. Predicted before the run: at the middle height the colour stays; at the top it is
    /// 95 % of the way to white (0.5 -> 0.975); at the bottom 60 % of the way to black (0.5 -> 0.2); halfway down, 30 %
    /// (0.5 -> 0.35); a flat map (all three equal) does not divide by zero.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn colours_shade_with_height() {
        let grey = RGBColor { red: 0.5, green: 0.5, blue: 0.5 };
        let shade = |h: f32, hi: f32, mid: f32, lo: f32| {
            let mut c = grey;
            W3dRadar::interpolate_color_for_height(&mut c, h, hi, mid, lo);
            c.red
        };
        assert_eq!(shade(10.0, 20.0, 10.0, 0.0), 0.5);
        assert!((shade(20.0, 20.0, 10.0, 0.0) - 0.975).abs() < 1e-6);
        assert!((shade(0.0, 20.0, 10.0, 0.0) - 0.2).abs() < 1e-6);
        assert!((shade(5.0, 20.0, 10.0, 0.0) - 0.35).abs() < 1e-6);
        assert!(shade(7.0, 7.0, 7.0, 7.0).is_finite());
    }

    /// #35: the terrain, overlay and shroud textures. Predicted before the run, with the test map (1000 x 500, ground 10,
    /// the map's Z from 0 to 40, a lake west of x 200 with its floor 2 under the water at 5): land is the terrain colour
    /// shaded by `interpolate(10, hi = average 10, mid = 40, lo = 0)`, i.e. darkened by 0.6 * 30/40 = 45 % (green 0.5 ->
    /// 0.275 -> byte 70); deep in the lake the water colour is darkened by 0.6 * (5-2)/5 = 36 % (blue 0.8 -> 0.512 ->
    /// byte 130). The enemy's dot at (700, 100) is drawn as 2 x 2 texels from cell (89, 25); the one at (300, 100) is
    /// shrouded for the local player and not drawn; a hidden one is not drawn. The shroud's west half is black, cell
    /// (2, 0) at alpha 127, the rest 0.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn the_textures_are_built_as_upstream_builds_them() {
        let mut h = host();
        h.base.put(20, 700.0, 100.0, 2, 3, &[]);
        h.base.put(21, 300.0, 100.0, 2, 3, &[]);
        h.base.put(22, 800.0, 300.0, 2, 3, &[]);
        h.hidden.push(22);
        let mut radar = Radar::new();
        let mut w = W3dRadar::new();
        w.new_map(&mut radar, &h);
        for id in [20, 21, 22] {
            radar.add_object(id, &mut h);
        }
        let land = w.terrain_texture[(64 * 128 + 64) as usize];
        assert_eq!(((land >> 16) & 0xff, (land >> 8) & 0xff, land & 0xff, land >> 24), (35, 70, 17, 255));
        let lake = w.terrain_texture[(64 * 128 + 5) as usize];
        assert_eq!(lake & 0xff, 130);
        w.refresh_overlay(&radar, &mut h);
        let c = h.radar_player_color(2) as u32;
        let at = |x: i32, y: i32| w.overlay_texture[(y * 128 + x) as usize];
        assert_eq!((at(89, 25), at(90, 25), at(89, 26), at(90, 26)), (c, c, c, c));
        assert_eq!(w.overlay_texture.iter().filter(|&&t| t != 0).count(), 4);
        let s = |x: i32, y: i32| w.shroud_texture[(y * 128 + x) as usize] >> 24;
        assert_eq!((s(10, 10), s(70, 10), s(100, 10), s(100, 100)), (255, 127, 0, 0));
    }

    /// #35: the drawing at a phone's width. Predicted before the run: a 120 x 92 window draws the 2:1 map 118 x 60 from
    /// (1, 16) to (119, 76) (`findDrawPositions` of the 118 x 90 area inside the 1-pixel edge: `(90 - 59) / 2` truncates
    /// to 15); the bars above and below
    /// are opaque black; the window's corners stay transparent; a fresh under-attack event draws red, and its sound is due
    /// once (the second draw does not ask again); with the radar not forced and the player without one, nothing is drawn;
    /// the view box over the middle of the map is drawn in yellow.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn the_radar_draws_at_a_phones_width() {
        let mut h = host();
        let mut radar = Radar::new();
        let mut w = W3dRadar::new();
        w.new_map(&mut radar, &h);
        h.base.frame = 50;
        radar.create_event(&Coord3D { x: 500.0, y: 250.0, z: 0.0 }, RADAR_EVENT_UNDER_ATTACK, 4.0, &h);
        let size = ICoord2D { x: 120, y: 92 };
        assert_eq!(radar.find_draw_positions(1, 1, 118, 90), (ICoord2D { x: 1, y: 16 }, ICoord2D { x: 119, y: 76 }));
        let view = [
            Coord3D { x: 400.0, y: 300.0, z: 10.0 },
            Coord3D { x: 600.0, y: 300.0, z: 10.0 },
            Coord3D { x: 600.0, y: 200.0, z: 10.0 },
            Coord3D { x: 400.0, y: 200.0, z: 10.0 },
        ];
        let img = w.draw_left_hud(&mut radar, &mut h, size, Some(view)).expect("the player has a radar");
        assert_eq!((img.width, img.height), (120, 92));
        assert_eq!(img.pixel(0, 0)[3], 0);
        assert_eq!(img.pixel(60, 5), [0, 0, 0, 255]);
        assert_eq!(img.pixel(60, 85), [0, 0, 0, 255]);
        assert_eq!(img.event_sounds, 1);
        let red = (0..img.height).flat_map(|y| (0..img.width).map(move |x| (x, y))).filter(|&(x, y)| {
            let p = img.pixel(x, y);
            p[0] > 200 && p[1] < 80 && p[2] < 80
        });
        assert!(red.count() > 10);
        let yellow = (0..img.height).flat_map(|y| (0..img.width).map(move |x| (x, y))).filter(|&(x, y)| {
            let p = img.pixel(x, y);
            p[0] > 150 && p[1] > 150 && p[2] < 40
        });
        assert!(yellow.count() > 20);
        let again = w.draw_left_hud(&mut radar, &mut h, size, Some(view)).expect("drawn");
        assert_eq!(again.event_sounds, 0);
        h.base.has_radar = false;
        assert!(w.draw_left_hud(&mut radar, &mut h, size, None).is_none());
    }

    /// #162: the hero icon's rectangle (upstream's arithmetic: cells by truncation, `radarToPixel`, the centring shift
    /// of `w / 2 - 1` and `h / 2`) and the mapped image stretched into it. Predicted: a 7 x 5 icon at the middle of a
    /// 1000 x 1000 map in a 128 x 128 area at (0, 0) covers x 62..69, y 61..66; the image's red texels fill it, its
    /// rotated drawing takes the page's other column first.
    #[test]
    fn the_hero_icon_is_the_mapped_image() {
        let pos = Coord3D { x: 500.0, y: 500.0, z: 0.0 };
        assert_eq!(hero_icon_rect((1000.0, 1000.0), &pos, (0, 0, 128, 128), (7, 5)), Some((62, 61, 69, 66)));
        assert_eq!(hero_icon_rect((0.0, 1000.0), &pos, (0, 0, 128, 128), (7, 5)), None, "inf to Int: undefined");
        // a 2 x 1 page: red, then blue; the image is the whole page
        let page = std::sync::Arc::new(vec![255, 0, 0, 255, 0, 0, 255, 255]);
        let mut im = crate::client::image::Image::new(b"HeroReticle");
        im.texture_size = (2, 1);
        im.uv = [0.0, 0.0, 1.0, 1.0];
        im.image_size = (2, 1);
        let image = MappedImage::new(&im, 2, 1, page);
        let mut img = RadarImage::new(4, 4);
        img.draw_image(&image, 0, 0, 4, 2);
        assert_eq!(img.pixel(0, 0), [255, 0, 0, 255]);
        assert_eq!(img.pixel(3, 1), [0, 0, 255, 255]);
        assert_eq!(img.pixel(0, 3), [0, 0, 0, 0]);
        let mut rotated = image.clone();
        rotated.rotated = true;
        let mut img = RadarImage::new(4, 4);
        img.draw_image(&rotated, 0, 0, 2, 2);
        // the screen's upper left takes the UV's right edge (blue); its lower left the left edge (red)
        assert_eq!(img.pixel(0, 0), [0, 0, 255, 255]);
        assert_eq!(img.pixel(0, 1), [255, 0, 0, 255]);
    }
}
