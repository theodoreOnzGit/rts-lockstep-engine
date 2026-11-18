//! UNTESTED DRAFT (#202, #205, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTreeDraw.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTreeDraw.h
//!
//! The `FXList` fields (`ToppleFX`, `BounceFX`) hold the FX list's name (`INI::parseFXList` resolves it against
//! `TheFXListStore` at parse time; the draft keeps the name for the caller to resolve).

use super::device_support::{draw_module_xfer, DrawableFacts};
use rts_lockstep_engine::common::game_common::LOGICFRAMES_PER_SECOND;
use rts_lockstep_engine::common::ini::{FieldParse, Ini, IniResult, UserData};
use rts_lockstep_engine::common::xfer::{Xfer, XferResult};

/// `class W3DTreeDrawModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct W3DTreeDrawModuleData {
    pub model_name: Vec<u8>,
    pub texture_name: Vec<u8>,
    // Push aside parameters. [5/29/2003]
    pub frames_to_move_outward: u32,
    pub frames_to_move_inward: u32,
    pub max_outward_movement: f32,
    pub darkening: f32,
    // Topple parameters. [7/7/2003]
    /// `m_toppleFX`, `m_bounceFX`: the FX lists' names (empty for `NULL`).
    pub topple_fx: Vec<u8>,
    pub bounce_fx: Vec<u8>,
    pub stump_name: Vec<u8>,
    pub initial_velocity_percent: f32,
    pub initial_accel_percent: f32,
    pub bounce_velocity_percent: f32,
    pub minimum_topple_speed: f32,
    pub kill_when_toppled: bool,
    pub do_topple: bool,
    /// How long it takes to sink after toppling. [7/11/2003]
    pub sink_frames: u32,
    /// How far it sinks.
    pub sink_distance: f32,
    pub do_shadow: bool,
}

impl Default for W3DTreeDrawModuleData {
    /// `W3DTreeDrawModuleData::W3DTreeDrawModuleData` (W3DTreeDraw.cpp:50).
    fn default() -> Self {
        const START_VELOCITY_PERCENT: f32 = 0.2;
        const START_ACCEL_PERCENT: f32 = 0.01;
        const VELOCITY_BOUNCE_PERCENT: f32 = 0.3; // multiply the velocity by this when you bounce
        const MINIMUM_TOPPLE_SPEED: f32 = 0.5; // Won't let trees fall slower than this
        W3DTreeDrawModuleData {
            model_name: Vec::new(),
            texture_name: Vec::new(),
            frames_to_move_inward: 1,
            frames_to_move_outward: 1,
            darkening: 0.0,
            max_outward_movement: 1.0,
            topple_fx: Vec::new(),
            bounce_fx: Vec::new(),
            stump_name: Vec::new(),
            kill_when_toppled: true,
            do_topple: false,
            do_shadow: false,
            initial_velocity_percent: START_VELOCITY_PERCENT,
            initial_accel_percent: START_ACCEL_PERCENT,
            bounce_velocity_percent: VELOCITY_BOUNCE_PERCENT,
            minimum_topple_speed: MINIMUM_TOPPLE_SPEED,
            sink_frames: (10 * LOGICFRAMES_PER_SECOND) as u32,
            sink_distance: 20.0,
        }
    }
}

/// `W3DTreeDrawModuleData::buildFieldParse`'s table (W3DTreeDraw.cpp:77-98).
pub static W3D_TREE_DRAW_FIELDS: [FieldParse<W3DTreeDrawModuleData>; 19] = [
    FieldParse::new("ModelName", |i, d, _| i.parse_ascii_string().map(|v| d.model_name = v), UserData::None),
    FieldParse::new("TextureName", |i, d, _| i.parse_ascii_string().map(|v| d.texture_name = v), UserData::None),
    FieldParse::new("MoveOutwardTime", |i, d, _| i.parse_duration_unsigned_int().map(|v| d.frames_to_move_outward = v), UserData::None),
    FieldParse::new("MoveInwardTime", |i, d, _| i.parse_duration_unsigned_int().map(|v| d.frames_to_move_inward = v), UserData::None),
    FieldParse::new("MoveOutwardDistanceFactor", |i, d, _| i.parse_real().map(|v| d.max_outward_movement = v), UserData::None),
    FieldParse::new("DarkeningFactor", |i, d, _| i.parse_real().map(|v| d.darkening = v), UserData::None),
    FieldParse::new("ToppleFX", |i, d, _| i.parse_ascii_string().map(|v| d.topple_fx = v), UserData::None), // INI::parseFXList
    FieldParse::new("BounceFX", |i, d, _| i.parse_ascii_string().map(|v| d.bounce_fx = v), UserData::None), // INI::parseFXList
    FieldParse::new("StumpName", |i, d, _| i.parse_ascii_string().map(|v| d.stump_name = v), UserData::None),
    FieldParse::new("KillWhenFinishedToppling", |i, d, _| i.parse_bool().map(|v| d.kill_when_toppled = v), UserData::None),
    FieldParse::new("DoTopple", |i, d, _| i.parse_bool().map(|v| d.do_topple = v), UserData::None),
    FieldParse::new("InitialVelocityPercent", |i, d, _| i.parse_percent_to_real().map(|v| d.initial_velocity_percent = v), UserData::None),
    FieldParse::new("InitialAccelPercent", |i, d, _| i.parse_percent_to_real().map(|v| d.initial_accel_percent = v), UserData::None),
    FieldParse::new("BounceVelocityPercent", |i, d, _| i.parse_percent_to_real().map(|v| d.bounce_velocity_percent = v), UserData::None),
    FieldParse::new("MinimumToppleSpeed", |i, d, _| i.parse_positive_non_zero_real().map(|v| d.minimum_topple_speed = v), UserData::None),
    FieldParse::new("SinkDistance", |i, d, _| i.parse_positive_non_zero_real().map(|v| d.sink_distance = v), UserData::None),
    FieldParse::new("SinkTime", |i, d, _| i.parse_duration_unsigned_int().map(|v| d.sink_frames = v), UserData::None),
    FieldParse::new("DoShadow", |i, d, _| i.parse_bool().map(|v| d.do_shadow = v), UserData::None),
    FieldParse::end(),
];

impl W3DTreeDrawModuleData {
    /// Reads the `Draw = W3DTreeDraw` block's fields through [`W3D_TREE_DRAW_FIELDS`] (the caller opens the block, as
    /// [`crate::client::w3d_laser_draw`] does for its own).
    pub fn parse_fields(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        ini.init_from_ini(self, &W3D_TREE_DRAW_FIELDS)
    }
}

/// What `TheTerrainRenderObject->addTree(...)` (BaseHeightMap) is given.
#[derive(Clone, Debug, PartialEq)]
pub struct AddTreeRequest {
    pub drawable_id: u32,
    pub position: rts_lockstep_engine::common::base_type::Coord3D,
    pub scale: f32,
    pub orientation: f32,
    pub scale_randomness: f32,
}

/// `class W3DTreeDraw`.
#[derive(Clone, Debug, Default)]
pub struct W3DTreeDraw {
    /// `m_treeAdded`.
    pub tree_added: bool,
}

impl W3DTreeDraw {
    /// `W3DTreeDraw::W3DTreeDraw` (W3DTreeDraw.cpp:115): `m_treeAdded(false)`.
    pub fn new() -> Self {
        W3DTreeDraw { tree_added: false }
    }

    /// `W3DTreeDraw::reactToTransformChange` (W3DTreeDraw.cpp:129). The tree is given to the terrain once, the first
    /// time the drawable is anywhere but (0, 0); the request is what `TheTerrainRenderObject->addTree` is called with.
    /// `drawable_id`: `draw->getID()`; `orientation`: `draw->getOrientation()`; `has_module_data`:
    /// `getW3DTreeDrawModuleData() != NULL`.
    pub fn react_to_transform_change(&mut self, drawable: &DrawableFacts, drawable_id: u32, orientation: f32, has_module_data: bool) -> Option<AddTreeRequest> {
        if self.tree_added {
            return None;
        }
        if drawable.position.x == 0.0 && drawable.position.y == 0.0 {
            return None;
        }
        self.tree_added = true;
        if !has_module_data {
            return None;
        }
        let scale = drawable.scale;
        let mut scale_randomness = 0.0f32; // draw->getTemplate()->getInstanceScaleFuzziness()
        scale_randomness = 0.0; // We use the scale fuzziness inside WB to generate random scales, so they don't change at load time. jba. [4/22/2003]
        Some(AddTreeRequest { drawable_id, position: drawable.position, scale, orientation, scale_randomness })
    }

    /// `W3DTreeDraw::doDrawModule` (W3DTreeDraw.cpp:157): draws nothing; the terrain draws the tree.
    pub fn do_draw_module(&mut self) {}

    /// `W3DTreeDraw::crc` (W3DTreeDraw.cpp:168).
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `W3DTreeDraw::xfer` (W3DTreeDraw.cpp:180): version 1; no data of its own.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        draw_module_xfer(xfer)
    }

    /// `W3DTreeDraw::loadPostProcess` (W3DTreeDraw.cpp:197).
    pub fn load_post_process(&mut self) {}
}
