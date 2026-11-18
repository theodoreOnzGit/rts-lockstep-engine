//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DProjectileStreamDraw.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DProjectileStreamDraw.h
//!
//! Reuses: [`SegmentedLine`] (`SegmentedLineClass`, formats::w3d::segline), `MAX_PROJECTILE_STREAM`
//! (logic::modules::projectile_stream_update), `PRESET_ADDITIVE_SPRITE_SHADER`, `TILED_TEXTURE_MAP`.

use super::device_support::draw_module_xfer;
use rts_lockstep_engine::common::ini::{FieldParse, Ini, IniResult, UserData};
use rts_lockstep_engine::common::xfer::{Xfer, XferResult};
use rts_lockstep_engine::formats::w3d::segline::SegmentedLine;
use rts_lockstep_engine::formats::w3d::seglinerenderer::TILED_TEXTURE_MAP;
use rts_lockstep_engine::formats::w3d::shader::PRESET_ADDITIVE_SPRITE_SHADER;
use rts_lockstep_engine::logic::modules::projectile_stream_update::MAX_PROJECTILE_STREAM;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `class W3DProjectileStreamDrawModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct W3DProjectileStreamDrawModuleData {
    pub texture_name: Vec<u8>,
    pub width: f32,
    pub tile_factor: f32,
    pub scroll_rate: f32,
    pub max_segments: i32,
}

impl Default for W3DProjectileStreamDrawModuleData {
    /// `W3DProjectileStreamDrawModuleData()` (W3DProjectileStreamDraw.cpp:47).
    fn default() -> Self {
        W3DProjectileStreamDrawModuleData { texture_name: Vec::new(), width: 0.0, tile_factor: 0.0, scroll_rate: 0.0, max_segments: 0 }
    }
}

/// `buildFieldParse`'s table (W3DProjectileStreamDraw.cpp:62-70).
pub static W3D_PROJECTILE_STREAM_DRAW_FIELDS: [FieldParse<W3DProjectileStreamDrawModuleData>; 6] = [
    FieldParse::new("Texture", |i, d, _| i.parse_ascii_string().map(|v| d.texture_name = v), UserData::None),
    FieldParse::new("Width", |i, d, _| i.parse_real().map(|v| d.width = v), UserData::None),
    FieldParse::new("TileFactor", |i, d, _| i.parse_real().map(|v| d.tile_factor = v), UserData::None),
    FieldParse::new("ScrollRate", |i, d, _| i.parse_real().map(|v| d.scroll_rate = v), UserData::None),
    FieldParse::new("MaxSegments", |i, d, _| i.parse_int().map(|v| d.max_segments = v), UserData::None),
    FieldParse::end(),
];

impl W3DProjectileStreamDrawModuleData {
    /// Reads the block's fields through [`W3D_PROJECTILE_STREAM_DRAW_FIELDS`].
    pub fn parse_fields(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        ini.init_from_ini(self, &W3D_PROJECTILE_STREAM_DRAW_FIELDS)
    }
}

/// `class W3DProjectileStreamDraw`.
#[derive(Clone, Debug)]
pub struct W3DProjectileStreamDraw {
    pub data: W3DProjectileStreamDrawModuleData,
    /// `m_texture != NULL` (`Get_Texture(textureName)`).
    pub has_texture: bool,
    /// `m_allLines[MAX_PROJECTILE_STREAM]`: a line, and whether it is in the scene (`Peek_Scene()`).
    pub all_lines: Vec<Option<(SegmentedLine, bool)>>,
    /// `m_linesValid`.
    pub lines_valid: i32,
}

impl W3DProjectileStreamDraw {
    /// `W3DProjectileStreamDraw::W3DProjectileStreamDraw` (W3DProjectileStreamDraw.cpp:99).
    pub fn new(data: W3DProjectileStreamDrawModuleData, has_texture: bool) -> Self {
        W3DProjectileStreamDraw { data, has_texture, all_lines: (0..MAX_PROJECTILE_STREAM).map(|_| None).collect(), lines_valid: 0 }
    }

    /// `W3DProjectileStreamDraw::~W3DProjectileStreamDraw` (W3DProjectileStreamDraw.cpp:80): every line out of the
    /// scene and released, then the texture.
    pub fn destroy(&mut self) {
        for line_index in 0..self.lines_valid as usize {
            // upstream: Remove_Render_Object(deadLine) when in the scene — device-only, not translated
            self.all_lines[line_index] = None;
        }
        self.has_texture = false;
    }

    /// `W3DProjectileStreamDraw::setFullyObscuredByShroud` (W3DProjectileStreamDraw.cpp:109): a hidden stream's lines
    /// leave the scene; a shown one's go back.
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        for line_index in 0..self.lines_valid as usize {
            if let Some((_, in_scene)) = self.all_lines[line_index].as_mut() {
                if fully_obscured {
                    //we need to remove all our lines from the scene because they are hidden
                    if *in_scene {
                        *in_scene = false; // deadLine->Remove()
                    }
                } else if !*in_scene {
                    //we need to restore lines into scene
                    *in_scene = true; // Add_Render_Object(deadLine)
                }
            }
        }
    }

    /// `W3DProjectileStreamDraw::doDrawModule` (W3DProjectileStreamDraw.cpp:135). `all_points`: what
    /// `ProjectileStreamUpdate::getAllPoints` gives (`MAX_PROJECTILE_STREAM` points, the first `points_used` valid);
    /// `has_object`: `getDrawable()->getObject() != NULL`.
    pub fn do_draw_module(&mut self, has_object: bool, all_points: &[Vector3; MAX_PROJECTILE_STREAM], points_used: i32) {
        // get object from logic
        if !has_object {
            return;
        }

        let mut staging_points = [Vector3::new(0.0, 0.0, 0.0); MAX_PROJECTILE_STREAM];
        let zero_vector = Vector3::new(0.0, 0.0, 0.0);

        let mut lines_made: i32 = 0;
        let mut current_master_point: i32 = 0;
        let mut current_staging_point: usize = 0;

        if self.data.max_segments != 0 {
            // If I have a drawing cap, I need to increase the start point in the array.  The furthest (oldest)
            // point from the tank is in spot zero.
            current_master_point = points_used - self.data.max_segments;
            current_master_point = current_master_point.max(0); // (but if they say to draw more than exists, draw all)
        }

        // Okay.  I have an array of ordered points that may have blanks in it.  I need to copy to the staging area
        // until I hit a blank or the end.  Then if I have a line made, I'll overwrite it, otherwise I'll make a new one.
        // I'll keep doing this until I run out of valid points.
        while current_master_point < points_used {
            while current_master_point < points_used && all_points[current_master_point as usize] != zero_vector {
                // While I am not looking at a bad point (off edge or zero)
                staging_points[current_staging_point] = all_points[current_master_point as usize]; // copy to the staging
                current_staging_point += 1; // increment how many I have
                current_master_point += 1; // increment what I am looking at
            }
            // Use or reuse a line
            if current_staging_point > 1 {
                // Don't waste a line on a double hole (0) or a one point line (1)
                self.make_or_update_line(&staging_points, current_staging_point as u32, lines_made);
                lines_made += 1; // keep track of how many are real this frame
            }
            current_master_point += 1; //I am either pointed off the edge anyway, or I am pointed at a zero I want to skip
            current_staging_point = 0; //start over in the staging area
        }

        let old_lines_valid = self.lines_valid;
        for line_index in lines_made..old_lines_valid {
            // Delete any line we aren't using anymore.
            // upstream: Remove_Render_Object(deadLine) when in the scene — device-only, not translated
            self.all_lines[line_index as usize] = None;
            self.lines_valid -= 1;
        }
    }

    /// `W3DProjectileStreamDraw::makeOrUpdateLine` (W3DProjectileStreamDraw.cpp:213).
    pub fn make_or_update_line(&mut self, points: &[Vector3], point_count: u32, line_index: i32) {
        let mut new_line = false;

        if self.all_lines[line_index as usize].is_none() {
            //Need a new one if this is blank, otherwise I'll reset the existing one
            self.all_lines[line_index as usize] = Some((SegmentedLine::new(0), false)); // NEW SegmentedLineClass (sync time: WW3D::Get_Sync_Time())
            self.lines_valid += 1;
            new_line = true;
        }

        let (line, in_scene) = self.all_lines[line_index as usize].as_mut().unwrap();

        line.set_points(&points[..point_count as usize]); //tell the line which points to use

        if new_line {
            // This is one time stuff we only need to do if this is a new and not a change
            line.set_texture(if self.has_texture { Some(self.data.texture_name.clone()) } else { None }); //set the texture
            line.set_shader(PRESET_ADDITIVE_SPRITE_SHADER); //pick the alpha blending mode you want - see shader.h for others.
            line.set_width(self.data.width); //set line width in world units
            line.set_texture_mapping_mode(TILED_TEXTURE_MAP); //this tiles the texture across the line
            line.set_texture_tile_factor(self.data.tile_factor); //number of times to tile texture across each segment
            line.set_uv_offset_rate(&Vector2::new(0.0, self.data.scroll_rate)); //amount to scroll texture on each draw
            *in_scene = true; // W3DDisplay::m_3DScene->Add_Render_Object(line)
        }
    }

    /// `W3DProjectileStreamDraw::crc` (W3DProjectileStreamDraw.cpp:247).
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `W3DProjectileStreamDraw::xfer` (W3DProjectileStreamDraw.cpp:259): version 1; no data to save.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        draw_module_xfer(xfer)
    }

    /// `W3DProjectileStreamDraw::loadPostProcess` (W3DProjectileStreamDraw.cpp:277).
    pub fn load_post_process(&mut self) {}
}
