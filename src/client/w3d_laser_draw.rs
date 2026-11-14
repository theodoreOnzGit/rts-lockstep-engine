// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DLaserDraw.cpp (W3DLaserDrawModuleData:
//     its constructor and buildFieldParse; W3DLaserDraw: its constructor, destructor, getLaserTemplateWidth,
//     doDrawModule, crc, xfer, loadPostProcess)
//   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DLaserDraw.h (the classes)
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/DrawModule.cpp (DrawModule::xfer, its version byte)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Color.cpp (GameGetColorComponentsReal)
//   (#169: the lines are formats::w3d::segline's SegmentedLineClass, translated there from WW3D2/segline.cpp, and
//     formats::w3d::seglinerenderer's SegLineRendererClass; nothing of segline.cpp or seglinerenderer.cpp is in this
//     file any more)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.cpp (_PresetAdditiveShader, formats::w3d::shader)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc. (Colin Day / Kris Morness)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The laser's draw module (GitHub #168): `W3DLaserDraw`, the `LaserDrawInterface` that `LaserUpdate` asks for its
//! width, and the beam geometry it hands its `SegmentedLineClass` lines (GitHub #169: the translated ones).
//!
//! # As upstream does it
//!
//! - **Its data** (`Draw = W3DLaserDraw`, W3DLaserDraw.cpp:65-111): `NumBeams` (1), `InnerBeamWidth` (0),
//!   `OuterBeamWidth` (1), `InnerColor` and `OuterColor` (`parseColorInt`; the constructor leaves them unset: 0 from the
//!   allocator's zero fill), `MaxIntensityLifetime` and `FadeLifetime` (`parseDurationUnsignedInt`, 0; read by nothing),
//!   `Texture`, `ScrollRate` (0), `Tile` (no), `Segments` (1), `ArcHeight` (0), `SegmentOverlapRatio` (0),
//!   `TilingScalar` (1).
//! - **Made** (W3DLaserDraw.cpp:116-220): the texture (`Get_Texture(name)`: none for an empty name, else a texture,
//!   the "missing" one of 128 x 128 for a file that is not there) and its aspect ratio (width / height, 1 without one);
//!   `numBeams * segments` lines, each segment's beams made from the last to the first: one beam takes the inner width
//!   and the inner colour times the inner alpha; several take `scale = i / (numBeams - 1)` of the way from inner to outer
//!   (the colour `inner + scale * (outer - inner) * innerAlpha`, the inner term not times alpha). Each line: the
//!   texture, the additive shader, the width, the colour, a UV offset rate of `(0, ScrollRate)`, the tiled mapping when
//!   there is a texture, added to the scene and hidden.
//! - **`getLaserTemplateWidth`** (W3DLaserDraw.cpp:244-248): `OuterBeamWidth * 0.5`.
//! - **`doDrawModule`** (W3DLaserDraw.cpp:252-424): the drawable's `LaserUpdate` (`findClientUpdateModule`); none, nothing
//!   (a debug assertion). When the update is dirty or the module has not drawn since it was made or loaded: both
//!   cleared, then per segment its two points: with an arc (`ArcHeight > 0` and more than one segment) the segment's share
//!   of the line from start to end (its ratios widened by the overlap ratio at the inner ends), each point raised by
//!   `cos(dist / halfLength * PI * 0.5) * ArcHeight` (its distance from the middle) and kept at least 2 above the ground;
//!   without, the start and the end. Then per beam, last to first: its width (as made, times the update's width scale),
//!   the tile factor (with a texture and `Tile`: `length / width * aspect * TilingScalar`), `Set_Width`, `Set_Points`.
//! - **The save** (`xfer`, version 1, W3DLaserDraw.cpp:442-456): `DrawModule::xfer` (version 1) and its
//!   `DrawableModule::xfer` and `Module::xfer` (a version byte each): 4 bytes, no data of its own. `crc` adds nothing;
//!   `loadPostProcess` marks the module to draw again.
//! - **The lines** (`SegmentedLineClass`, segline.cpp, and its `SegLineRendererClass`): `Set_Width` keeps `MAX(width,
//!   0)`; `Set_Texture_Tile_Factor` keeps at most 50 and `MAX(factor, 0)`; `Set_UV_Offset_Rate(rate)` keeps `rate *
//!   0.001` per millisecond. Each render advances the UV offset by the milliseconds since the last
//!   (`WW3D::Get_Sync_Time()`) and keeps its fraction; the tiled mapping's U is 0 on one edge and 1 on the other, V is
//!   the point's index times the tile factor, both plus the offset. The scene shows a line its camera does not cull: the
//!   constructor's `Set_Visible(0)` lasts until the first scene update, by which time the line has its points.
//!   **Corrected 2026-11-14 (#169):** these are the translated `SegmentedLineClass`'s
//!   ([`crate::formats::w3d::segline`]) and `SegLineRendererClass`'s ([`crate::formats::w3d::seglinerenderer`]), which
//!   the module now drives as upstream does. Two details the stand-in had wrong: a render object is made **not**
//!   visible (`RenderObjClass`'s `DEFAULT_BITS` has no `IS_VISIBLE`, so `Set_Visible(0)` changes nothing), and
//!   `SegmentedLineClass::Render` does not read the visibility bit at all (only `Is_Not_Hidden_At_All`): the bit is the
//!   scene's culling pass's, which decides whether `Render` is called.
//!
//! # Upstream quirks that are kept
//!
//! - `doDrawModule` computes the colours again each draw and uses none of them (nor the alpha it works out per beam): a
//!   line's colour is the constructor's for good. `Set_Opacity` is never called: the lines' opacity is 1.
//! - The constructor's several-beam colour multiplies only the difference by the inner alpha, the one-beam colour all
//!   of it.
//! - The arc's ground clamp is `MAX(z, 2 + getGroundHeight(x, y))`: the ground is asked twice when it wins (the same
//!   answer: asked once here).
//! - A line that is never drawn keeps no points: nothing is shown until the first `doDrawModule`.
//!
//! # Undefined upstream ([`LaserDrawUndefined`], the module is not made)
//!
//! - `numBeams * segments` wrapping an `UnsignedInt` (`NEW SegmentedLineClass *[n]` of the wrapped size, then the loops
//!   write past it).
//!
//! # Where this differs from upstream
//!
//! - ~~**The W3D lines** are [`SegmentedLine`]: what `W3DLaserDraw` gives them (as given, the clamps in their getters) and
//!   the renderer's UV state; `SegmentedLine::render_strip` is the part of `SegLineRendererClass::Render` the client
//!   draws (one quad per line: upstream's renderer subdivides, adds noise, merges intersections and builds a camera
//!   space strip with end caps; none of that applies to a two-point line with no subdivision or noise but the quad's
//!   orientation, which the Bevy glue makes).~~ **Corrected 2026-11-14 (#169):** the lines are the translated
//!   `SegmentedLineClass` ([`SegmentedLine`], `formats::w3d::segline`), set through its own setters as upstream sets
//!   them, and drawn by [`W3DLaserDraw::render_lines`] through `SegmentedLineClass::Render` and the translated
//!   `SegLineRendererClass::Render` into eye-space chunks (the stand-in and its `render_strip` are removed).
//! - **The scene** is a flag ([`W3DLaserDraw::in_scene`]): every line is added by the constructor and removed by the
//!   destructor together; its culling pass is not modelled (`render_lines`).
//! - **The ground** is the caller's ([`W3DLaserDraw::do_draw_module`]'s `ground`: `TheTerrainLogic->getGroundHeight`).
//! - **The drawable's `LaserUpdate`** is the caller's to find (`findClientUpdateModule`, which upstream never advances
//!   past the first client update module: [`crate::client::drawable`]).

use std::sync::Arc;

use crate::client::laser_update::{drawable_module_base_xfer, LaserUpdate};
use crate::common::base_type::{Coord3D, PI};
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::common::thing_template::ClientModuleData;
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::formats::w3d::seglinerenderer::{SegLineChunk, TILED_TEXTURE_MAP};
use crate::formats::w3d::segline::SegmentedLine;
use crate::formats::w3d::shader::PRESET_ADDITIVE_SHADER;
use crate::formats::w3d::W3dResult;
use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::random::Random3;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// The module's name (`Draw = W3DLaserDraw`).
pub const W3D_LASER_DRAW: &[u8] = b"W3DLaserDraw";


/// `MissingTexture`'s size (missingtexture.cpp:25-26): what `Get_Texture` gives for a file that is not there.
pub const MISSING_TEXTURE_SIZE: (u32, u32) = (128, 128);

/// `W3DLaserDrawModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct W3DLaserDrawModuleData {
    /// `m_innerColor`, `m_outerColor` (`Color`, packed ARGB).
    pub inner_color: i32,
    pub outer_color: i32,
    /// `m_innerBeamWidth`, `m_outerBeamWidth`.
    pub inner_beam_width: f32,
    pub outer_beam_width: f32,
    /// `m_scrollRate`.
    pub scroll_rate: f32,
    /// `m_tile`.
    pub tile: bool,
    /// `m_numBeams`.
    pub num_beams: u32,
    /// `m_maxIntensityFrames`, `m_fadeFrames` (read by nothing).
    pub max_intensity_frames: u32,
    pub fade_frames: u32,
    /// `m_textureName`.
    pub texture_name: Vec<u8>,
    /// `m_segments`.
    pub segments: u32,
    /// `m_arcHeight`.
    pub arc_height: f32,
    /// `m_segmentOverlapRatio`.
    pub segment_overlap_ratio: f32,
    /// `m_tilingScalar`.
    pub tiling_scalar: f32,
    /// The module's tag (`ModuleData::getModuleTagNameKey`'s name), for the save's module list.
    pub module_tag: Vec<u8>,
}

impl Default for W3DLaserDrawModuleData {
    /// `W3DLaserDrawModuleData()` (W3DLaserDraw.cpp:65-78); the colours unset (0, the allocator's fill).
    fn default() -> Self {
        W3DLaserDrawModuleData {
            inner_color: 0,
            outer_color: 0,
            inner_beam_width: 0.0,
            outer_beam_width: 1.0,
            scroll_rate: 0.0,
            tile: false,
            num_beams: 1,
            max_intensity_frames: 0,
            fade_frames: 0,
            texture_name: Vec::new(),
            segments: 1,
            arc_height: 0.0,
            segment_overlap_ratio: 0.0,
            tiling_scalar: 1.0,
            module_tag: Vec::new(),
        }
    }
}

/// `buildFieldParse`'s table (W3DLaserDraw.cpp:92-109; `ModuleData::buildFieldParse` adds none).
pub static W3D_LASER_DRAW_FIELDS: [FieldParse<W3DLaserDrawModuleData>; 15] = [
    FieldParse::new("NumBeams", |i, d, _| i.parse_unsigned_int().map(|v| d.num_beams = v), UserData::None),
    FieldParse::new("InnerBeamWidth", |i, d, _| i.parse_real().map(|v| d.inner_beam_width = v), UserData::None),
    FieldParse::new("OuterBeamWidth", |i, d, _| i.parse_real().map(|v| d.outer_beam_width = v), UserData::None),
    FieldParse::new("InnerColor", |i, d, _| i.parse_color_int().map(|v| d.inner_color = v), UserData::None),
    FieldParse::new("OuterColor", |i, d, _| i.parse_color_int().map(|v| d.outer_color = v), UserData::None),
    FieldParse::new("MaxIntensityLifetime", |i, d, _| i.parse_duration_unsigned_int().map(|v| d.max_intensity_frames = v), UserData::None),
    FieldParse::new("FadeLifetime", |i, d, _| i.parse_duration_unsigned_int().map(|v| d.fade_frames = v), UserData::None),
    FieldParse::new("Texture", |i, d, _| i.parse_ascii_string().map(|v| d.texture_name = v), UserData::None),
    FieldParse::new("ScrollRate", |i, d, _| i.parse_real().map(|v| d.scroll_rate = v), UserData::None),
    FieldParse::new("Tile", |i, d, _| i.parse_bool().map(|v| d.tile = v), UserData::None),
    FieldParse::new("Segments", |i, d, _| i.parse_unsigned_int().map(|v| d.segments = v), UserData::None),
    FieldParse::new("ArcHeight", |i, d, _| i.parse_real().map(|v| d.arc_height = v), UserData::None),
    FieldParse::new("SegmentOverlapRatio", |i, d, _| i.parse_real().map(|v| d.segment_overlap_ratio = v), UserData::None),
    FieldParse::new("TilingScalar", |i, d, _| i.parse_real().map(|v| d.tiling_scalar = v), UserData::None),
    FieldParse::end(),
];

fn parse_draw_block(ini: &mut Ini<'_>, d: &mut W3DLaserDrawModuleData) -> IniResult<()> {
    ini.init_from_ini(d, &W3D_LASER_DRAW_FIELDS)
}

impl W3DLaserDrawModuleData {
    /// The module data from the `Draw` block the logic keeps as tokens, read through [`W3D_LASER_DRAW_FIELDS`].
    pub fn parse(block: &ClientModuleData) -> IniResult<Self> {
        let text = crate::client::w3d_model_draw::client_block_text(block);
        let mut d = Self::default();
        let blocks = [BlockParse { token: "Draw", parse: parse_draw_block }];
        Ini::load(b"Draw", &text, IniLoadType::Overwrite, None, &blocks, &mut d)?;
        Ok(d)
    }
}

/// `GameGetColorComponentsReal(color, &r, &g, &b, &a)` (Color.cpp:125-131): `(red, green, blue, alpha)`, each byte /
/// 255.
pub fn game_get_color_components_real(color: i32) -> (f32, f32, f32, f32) {
    let c = color as u32;
    let alpha = ((c & 0xFF00_0000) >> 24) as f32 / 255.0;
    let red = ((c & 0x00FF_0000) >> 16) as f32 / 255.0;
    let green = if plant(9653) { (c & 0x0000_00FF) as f32 / 255.0 } else { ((c & 0x0000_FF00) >> 8) as f32 / 255.0 };
    let blue = (c & 0x0000_00FF) as f32 / 255.0;
    (red, green, blue, alpha)
}

/// What upstream's `W3DLaserDraw` leaves undefined (module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaserDrawUndefined {
    /// `numBeams * segments` wraps an `UnsignedInt`: the lines array is smaller than the loops index.
    LinesWrap,
}

/// `class W3DLaserDraw`.
#[derive(Clone, Debug, PartialEq)]
pub struct W3DLaserDraw {
    pub data: Arc<W3DLaserDrawModuleData>,
    /// `m_line3D`: `numBeams * segments` lines, segment by segment.
    pub lines: Vec<SegmentedLine>,
    /// `m_texture != NULL`.
    pub has_texture: bool,
    /// `m_textureAspectRatio`.
    pub texture_aspect_ratio: f32,
    /// `m_selfDirty` (not saved).
    pub self_dirty: bool,
    /// (#169) The lines are in the scene (`W3DDisplay::m_3DScene->Add_Render_Object`, every line by the constructor;
    /// `Remove_Render_Object` by the destructor): the scene renders them ([`W3DLaserDraw::render_lines`]).
    pub in_scene: bool,
}

impl W3DLaserDraw {
    /// `W3DLaserDraw(thing, moduleData)` (W3DLaserDraw.cpp:116-220). `texture_size`: the texture's top level's
    /// `(Width, Height)` when `Get_Texture` gives one (an empty name gives none; a missing file gives
    /// [`MISSING_TEXTURE_SIZE`]); `sync_time` the lines' `LastUsedSyncTime`.
    pub fn new(data: Arc<W3DLaserDrawModuleData>, texture_size: Option<(u32, u32)>, sync_time: u32) -> Result<Self, LaserDrawUndefined> {
        let d = &*data;
        let has_texture = !d.texture_name.is_empty();
        let mut texture_aspect_ratio = 1.0f32;
        if has_texture {
            let (w, h) = texture_size.unwrap_or(MISSING_TEXTURE_SIZE);
            texture_aspect_ratio = if plant(9636) { h as f32 / w as f32 } else { w as f32 / h as f32 };
        }
        // Get the color components for calculation purposes.
        let (inner_red, inner_green, inner_blue, inner_alpha) = game_get_color_components_real(d.inner_color);
        let (outer_red, outer_green, outer_blue, outer_alpha) = game_get_color_components_real(d.outer_color);
        // Allocate an array of lines equal to the number of beams * segments
        let count = d.num_beams.checked_mul(d.segments).ok_or(LaserDrawUndefined::LinesWrap)?;
        let mut lines = vec![SegmentedLine::new(sync_time); count as usize];
        for segment in 0..d.segments {
            // create all the lines we need at the right transparency level
            let mut i = d.num_beams as i32 - 1;
            while i >= 0 {
                let index = (segment.wrapping_mul(d.num_beams).wrapping_add(i as u32)) as usize;
                let (red, green, blue, _alpha, width);
                if d.num_beams == 1 {
                    width = d.inner_beam_width;
                    _alpha = inner_alpha;
                    red = if plant(9631) { inner_red } else { inner_red * inner_alpha };
                    green = inner_green * inner_alpha;
                    blue = inner_blue * inner_alpha;
                } else {
                    // Calculate the scale between min and max values
                    let scale = if plant(9632) { i as f32 / d.num_beams as f32 } else { i as f32 / (d.num_beams as f32 - 1.0) };
                    width = d.inner_beam_width + scale * (d.outer_beam_width - d.inner_beam_width);
                    _alpha = inner_alpha + scale * (outer_alpha - inner_alpha);
                    red = if plant(9633) { (inner_red + scale * (outer_red - inner_red)) * inner_alpha } else { inner_red + scale * (outer_red - inner_red) * inner_alpha };
                    green = inner_green + scale * (outer_green - inner_green) * inner_alpha;
                    blue = inner_blue + scale * (outer_blue - inner_blue) * inner_alpha;
                }
                let line = &mut lines[index];
                line.set_texture(has_texture.then(|| d.texture_name.clone()));
                //pick the alpha blending mode you want - see shader.h for others.
                line.set_shader(PRESET_ADDITIVE_SHADER);
                line.set_width(width);
                line.set_color(&Vector3::new(red, green, blue));
                //amount to scroll texture on each draw
                line.set_uv_offset_rate(&if plant(9634) { Vector2::new(d.scroll_rate, 0.0) } else { Vector2::new(0.0, d.scroll_rate) });
                if has_texture || plant(9635) {
                    //this tiles the texture across the line
                    line.set_texture_mapping_mode(TILED_TEXTURE_MAP);
                }
                // add to scene (`in_scene`, below)
                // hide the render object until the first time we come to draw it and set the correct position
                line.set_visible(plant(9656));
                i -= 1;
            }
        }
        Ok(W3DLaserDraw { data, lines, has_texture, texture_aspect_ratio, self_dirty: true, in_scene: true })
    }

    /// `~W3DLaserDraw()` (W3DLaserDraw.cpp:224-240): each line out of the scene and released.
    pub fn destroy(&mut self) {
        // remove line from scene, delete line
        self.in_scene = false;
        self.lines.clear();
    }

    /// (#169) The scene's render of the module's lines (each `SegmentedLineClass::Render`, in the order the constructor
    /// made them: segment by segment, a segment's beams from the last to the first, as `Add_Render_Object` added them to
    /// the scene's list) at the WW3D sync time `sync_time`, with the camera's view `view` and the randomizers' generator
    /// `v3` (a laser's lines have no subdivision levels and draw nothing from it): their chunks, in that order. Nothing
    /// out of the scene. The scene's culling of each line by its bounding sphere is not modelled (the client draws every
    /// line; Bevy culls).
    pub fn render_lines(&mut self, sync_time: u32, view: &Matrix3D, v3: &mut Random3) -> W3dResult<Vec<SegLineChunk>> {
        let mut out = Vec::new();
        if !self.in_scene {
            return Ok(out);
        }
        let d = self.data.clone();
        for segment in 0..d.segments {
            let mut i = d.num_beams as i32 - 1;
            while i >= 0 {
                let index = segment.wrapping_mul(d.num_beams).wrapping_add(i as u32) as usize;
                out.extend(self.lines[index].render(sync_time, view, v3)?);
                i -= 1;
            }
        }
        Ok(out)
    }

    /// `getLaserTemplateWidth()` (W3DLaserDraw.cpp:244-248), the `LaserDrawInterface`.
    pub fn get_laser_template_width(&self) -> f32 {
        if plant(9637) {
            self.data.outer_beam_width
        } else {
            self.data.outer_beam_width * 0.5
        }
    }

    /// `doDrawModule(transformMtx)` (W3DLaserDraw.cpp:252-424); the transform is not read. `update`: the drawable's
    /// `findClientUpdateModule("LaserUpdate")` (`None`: the debug assertion, nothing done). `ground`:
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    pub fn do_draw_module(&mut self, update: Option<&mut LaserUpdate>, ground: &mut dyn FnMut(f32, f32) -> f32) {
        let Some(update) = update else {
            // DEBUG_ASSERTCRASH: "expects its owner drawable to have a ClientUpdate = LaserUpdate module"
            return;
        };
        // If the update has moved the laser, it requires a reset of the laser.
        if !(update.is_dirty() || self.self_dirty) {
            return;
        }
        if !plant(9639) {
            update.set_dirty(false);
        }
        if !plant(9638) {
            self.self_dirty = false;
        }
        let d = self.data.clone();
        let mut laser_points = [Vector3::default(); 2];
        for segment in 0..d.segments {
            let arc = if plant(9640) { d.arc_height > 0.0 && d.segments >= 1 } else { d.arc_height > 0.0 && d.segments > 1 };
            if arc {
                // CALCULATE A CURVED LINE BASED ON TOTAL LENGTH AND DESIRED HEIGHT INCREASE
                let line_start = *update.get_start_pos();
                let line_end = *update.get_end_pos();
                // Get the length of the line
                let mut line_vector = line_end;
                line_vector.sub(&line_start);
                let line_length = line_vector.length();
                // Get the middle point (the highest)
                let mut line_middle = line_start;
                if !plant(9655) {
                    line_middle.add(&line_end);
                    line_middle.scale(0.5);
                }
                let half_length = if plant(9643) { line_length } else { line_length * 0.5 };
                // Now calculate which segment we will use.
                let mut start_segment_ratio = segment as f32 / d.segments as f32;
                let mut end_segment_ratio = if plant(9642) { segment as f32 / d.segments as f32 } else { (segment as f32 + 1.0) / d.segments as f32 };
                // Offset the segment ever-so-slightly to minimize overlap -- only apply to segments that are not the
                // start/end point
                if segment > 0 || plant(9641) {
                    start_segment_ratio -= d.segment_overlap_ratio;
                }
                if segment < d.segments.wrapping_sub(1) {
                    if plant(9657) {
                        end_segment_ratio -= d.segment_overlap_ratio;
                    } else {
                        end_segment_ratio += d.segment_overlap_ratio;
                    }
                }
                // Calculate our start and end segment positions on the *ground*.
                let mut vector = line_vector;
                vector.scale(start_segment_ratio);
                let mut segment_start = line_start;
                segment_start.add(&vector);
                vector = line_vector;
                vector.scale(end_segment_ratio);
                let mut segment_end = line_start;
                segment_end.add(&vector);
                // Calculate the raised height for the start/end segment positions using cosine.
                segment_start.z += arc_raise(&line_middle, &segment_start, half_length, d.arc_height);
                let from = if plant(9658) { line_end } else { line_middle };
                segment_end.z += arc_raise(&from, &segment_end, half_length, d.arc_height);
                // This makes the laser skim the ground rather than penetrate it!
                laser_points[0] = Vector3::new(segment_start.x, segment_start.y, skim(segment_start.z, ground(segment_start.x, segment_start.y)));
                laser_points[1] = Vector3::new(segment_end.x, segment_end.y, skim(segment_end.z, ground(segment_end.x, segment_end.y)));
            } else {
                // No arc -- way simpler!
                let s = update.get_start_pos();
                let e = update.get_end_pos();
                laser_points[0] = Vector3::new(s.x, s.y, s.z);
                laser_points[1] = Vector3::new(e.x, e.y, if plant(9646) { s.z } else { e.z });
            }
            // (the colour components are worked out again here and not used)
            let mut i = d.num_beams as i32 - 1;
            while i >= 0 {
                let index = segment.wrapping_mul(d.num_beams).wrapping_add(if plant(9654) { d.num_beams - 1 - i as u32 } else { i as u32 }) as usize;
                let width = if d.num_beams == 1 {
                    if plant(9647) {
                        d.inner_beam_width
                    } else {
                        d.inner_beam_width * update.get_width_scale()
                    }
                } else {
                    let scale = i as f32 / (d.num_beams as f32 - 1.0);
                    let ultimate_scale = update.get_width_scale();
                    let mut w = if plant(9648) { d.outer_beam_width } else { d.inner_beam_width + scale * (d.outer_beam_width - d.inner_beam_width) };
                    w *= ultimate_scale;
                    w
                };
                // Calculate the number of times to tile the line based on the height of the texture used.
                if (self.has_texture || plant(9650)) && d.tile {
                    let mut line_vector = Vector3::subtract_vectors(&laser_points[1], &laser_points[0]);
                    if plant(9660) {
                        line_vector.z = 0.0;
                    }
                    let length = line_vector.length();
                    // Adjust tile factor so texture is NOT stretched but tiled equally in both width and length.
                    let tile_factor = if plant(9649) { length / width * d.tiling_scalar } else { length / width * self.texture_aspect_ratio * d.tiling_scalar };
                    //number of times to tile texture across each segment
                    self.lines[index].set_texture_tile_factor(tile_factor);
                }
                self.lines[index].set_width(width);
                self.lines[index].set_points(&laser_points);
                i -= 1;
            }
        }
    }

    /// `crc(xfer)`: `DrawModule::crc`, nothing of its own.
    pub fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer(xfer)` (version 1, W3DLaserDraw.cpp:442-456): its version, then `DrawModule::xfer` (version 1,
    /// DrawModule.cpp:52-64) and `DrawableModule::xfer` and `Module::xfer`. "Kris says there is no data to save".
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        if !plant(9651) {
            let mut draw_version: XferVersion = 1;
            xfer.xfer_version(&mut draw_version, 1)?;
        }
        drawable_module_base_xfer(xfer)
    }

    /// `loadPostProcess()` (W3DLaserDraw.cpp:461-469): drawn again the first time after a load.
    pub fn load_post_process(&mut self) {
        if !plant(9652) {
            self.self_dirty = true;
        }
    }
}

/// The arc's raise of `point` (W3DLaserDraw.cpp:341-357): `cos(dist / halfLength * PI * 0.5) * arcHeight`, `dist` its
/// distance from `middle`. `cos` of a `Real` is the C library's double `cos` (the headers' C semantics), rounded back.
fn arc_raise(middle: &Coord3D, point: &Coord3D, half_length: f32, arc_height: f32) -> f32 {
    let mut vector = *middle;
    vector.sub(point);
    let dist = vector.length();
    let scaled_radians = dist / half_length * PI * 0.5;
    let mut height = if plant(9644) { petir::Real::sin(f64::from(scaled_radians)) as f32 } else { petir::Real::cos(f64::from(scaled_radians)) as f32 };
    height *= arc_height;
    height
}

/// `MAX(z, 2.0f + ground)` (the macro: the second when the first is not greater).
fn skim(z: f32, ground: f32) -> f32 {
    let floor = if plant(9645) { 1.0 + ground } else { 2.0 + ground };
    if plant(9659) {
        return z;
    }
    if z > floor {
        z
    } else {
        floor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::laser_update::LaserUpdateModuleData;
    use crate::common::thing_template::ClientLine;

    fn data(f: impl FnOnce(&mut W3DLaserDrawModuleData)) -> Arc<W3DLaserDrawModuleData> {
        let mut d = W3DLaserDrawModuleData::default();
        f(&mut d);
        Arc::new(d)
    }

    /// Predicted: the fields parse (a colour packed ARGB with alpha 255 when none is given; durations in frames, 1000 ms
    /// = 30); an empty block gives the constructor's defaults.
    #[test]
    fn module_data_parses() {
        let line = |f: &str, a: &str| ClientLine { depth: 0, field: f.as_bytes().to_vec(), args: a.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() };
        let block = ClientModuleData {
            module_tag_name_key: Default::default(),
            lines: vec![
                line("NumBeams", "3"),
                line("InnerBeamWidth", "0.5"),
                line("OuterBeamWidth", "4"),
                line("InnerColor", "R:255 G:0 B:0 A:128"),
                line("MaxIntensityLifetime", "1000"),
                line("Texture", "EXLaser.tga"),
                line("Tile", "Yes"),
                line("Segments", "5"),
                line("ArcHeight", "30"),
            ],
        };
        let d = W3DLaserDrawModuleData::parse(&block).unwrap();
        assert_eq!((d.num_beams, d.inner_beam_width, d.outer_beam_width, d.inner_color as u32), (3, 0.5, 4.0, 0x80FF_0000));
        assert_eq!((d.max_intensity_frames, d.texture_name.as_slice(), d.tile, d.segments, d.arc_height), (30, &b"EXLaser.tga"[..], true, 5, 30.0));
        let empty = ClientModuleData { module_tag_name_key: Default::default(), lines: vec![] };
        assert_eq!(W3DLaserDrawModuleData::parse(&empty).unwrap(), W3DLaserDrawModuleData::default());
    }

    /// Predicted: two beams of one segment: beam 0 the inner width and colour, beam 1 the outer width and `inner + (outer
    /// - inner) * innerAlpha`; the first draw (self dirty) gives both the start and end and the width scale; a second draw
    /// with nothing dirty changes nothing; the template width is half the outer width.
    #[test]
    fn two_beams_draw_once_until_dirty() {
        let d = data(|d| {
            d.num_beams = 2;
            d.inner_beam_width = 2.0;
            d.outer_beam_width = 6.0;
            d.inner_color = 0xFF00_FF00u32 as i32;
            d.outer_color = 0xFFFF_0000u32 as i32;
        });
        let mut m = W3DLaserDraw::new(d, None, 0).unwrap();
        assert_eq!(m.lines.len(), 2);
        // (#169: read through the translated line's getters)
        assert_eq!((m.lines[0].get_width(), m.lines[0].get_color()), (2.0, Vector3::new(0.0, 1.0, 0.0)));
        assert_eq!((m.lines[1].get_width(), m.lines[1].get_color()), (6.0, Vector3::new(1.0, 0.0, 0.0)));
        assert!(m.in_scene);
        assert!(m.lines.iter().all(|l| !l.is_visible() && l.point_locations.is_empty() && l.get_texture_mapping_mode() != TILED_TEXTURE_MAP));
        assert!(m.lines.iter().all(|l| l.get_shader() == PRESET_ADDITIVE_SHADER));
        let mut u = LaserUpdate::new(LaserUpdateModuleData::default());
        u.start_pos = Coord3D { x: 1.0, y: 2.0, z: 3.0 };
        u.end_pos = Coord3D { x: 4.0, y: 5.0, z: 6.0 };
        u.current_width_scalar = 0.5;
        m.do_draw_module(Some(&mut u), &mut |_, _| 0.0);
        assert_eq!((m.lines[1].get_width(), m.lines[1].point_locations[1]), (3.0, Vector3::new(4.0, 5.0, 6.0)));
        assert!(!m.self_dirty && !u.dirty);
        u.current_width_scalar = 1.0;
        m.do_draw_module(Some(&mut u), &mut |_, _| 0.0);
        assert_eq!(m.lines[1].get_width(), 3.0, "not dirty: not drawn again");
        assert_eq!(m.get_laser_template_width(), 3.0);
        m.load_post_process();
        m.do_draw_module(Some(&mut u), &mut |_, _| 0.0);
        assert_eq!(m.lines[1].get_width(), 6.0);
        m.destroy();
        assert!(!m.in_scene && m.lines.is_empty());
        assert!(m.render_lines(0, &Matrix3D::identity(), &mut Random3::default()).unwrap().is_empty());
    }

    /// Predicted: an arc of two segments over flat ground 0 from (0,0,0) to (100,0,0) of height 10: the middle point is
    /// raised by the full 10 (cos 0), the ends by cos(PI/2) (about 0) but kept 2 above the ground; with a texture of 64 x 32
    /// and `Tile`, the tile factor is length / width * 2.
    #[test]
    fn an_arc_rises_to_the_middle_and_skims_the_ground() {
        let d = data(|d| {
            d.segments = 2;
            d.arc_height = 10.0;
            d.inner_beam_width = 5.0;
            d.texture_name = b"Beam.tga".to_vec();
            d.tile = true;
        });
        let mut m = W3DLaserDraw::new(d, Some((64, 32)), 0).unwrap();
        let mut u = LaserUpdate::new(LaserUpdateModuleData::default());
        u.end_pos = Coord3D { x: 100.0, y: 0.0, z: 0.0 };
        m.do_draw_module(Some(&mut u), &mut |_, _| 0.0);
        let p = |k: usize| m.lines[k].point_locations.clone();
        assert_eq!(p(0)[0].z, 2.0);
        assert_eq!(p(0)[1], Vector3::new(50.0, 0.0, 10.0));
        assert_eq!(p(1)[1].z, 2.0);
        let len = Vector3::subtract_vectors(&p(0)[1], &p(0)[0]).length();
        assert_eq!(m.lines[0].get_texture_tile_factor(), len / 5.0 * 2.0 * 1.0);
        assert_eq!(m.lines[0].get_texture_mapping_mode(), TILED_TEXTURE_MAP);
    }

    /// Predicted: the save is 4 version bytes and loads back; the line's strip scrolls its V by the rate per second over
    /// the milliseconds since its last render and keeps the fraction. (First run: a miss of the test's, not the code's:
    /// it expected exactly 0.5, and `0.5 * 0.001` in a float gives 0.5000001 after 3000 ms.) **(#169)** Now through
    /// [`W3DLaserDraw::render_lines`] and the translated renderer: no chunk before the first draw (no points); after it,
    /// one chunk of 4 vertices whose V is the offset (the uniform-width mapping, no texture) and whose U is 0 and 1 plus
    /// the U offset (0).
    #[test]
    fn the_save_and_the_scroll() {
        let mut m = W3DLaserDraw::new(data(|d| d.scroll_rate = 0.5), None, 1000).unwrap();
        let mut s = crate::common::xfer_save::XferSave::new();
        s.open(b"t").unwrap();
        m.xfer(&mut s).unwrap();
        assert_eq!(s.bytes(), &[1, 1, 1, 1]);
        let mut ld = crate::common::xfer_load::XferLoad::new(vec![1, 1, 1, 1]);
        ld.open(b"t").unwrap();
        m.xfer(&mut ld).unwrap();
        let mut view = Matrix3D::identity();
        view.translate(0.0, 0.0, -20.0);
        let mut g = Random3::default();
        assert!(m.render_lines(3000, &view, &mut g).unwrap().is_empty(), "no points yet");
        m.lines[0].set_points(&[Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        m.lines[0].set_width(0.5);
        // a line of fewer than two points never reaches the renderer (`Render_Seg_Line`), so the render at 3000 did not
        // advance the offset: 3000 ms from the made time 1000. (First run: a miss of the test's: it had the first render
        // advance it.)
        let chunks = m.render_lines(4000, &view, &mut g).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].vertices.len(), 4);
        let off = 0.5f32 * 0.001 * 3000.0;
        let off = off - crate::ww::wwmath::floor(off);
        assert!(chunks[0].vertices.iter().all(|v| v.v == off), "{:?}", chunks[0].vertices);
        assert!((off - 0.5).abs() < 1e-6);
        let mut us: Vec<f32> = chunks[0].vertices.iter().map(|v| v.u).collect();
        us.sort_by(f32::total_cmp);
        assert_eq!(us, vec![0.0, 0.0, 1.0, 1.0]);
    }
}
