//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DPropDraw.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DPropDraw.h

use super::device_support::{draw_module_xfer, DrawableFacts};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{FieldParse, Ini, IniResult, UserData};
use rts_lockstep_engine::common::xfer::{Xfer, XferResult};

/// `class W3DPropDrawModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct W3DPropDrawModuleData {
    /// `m_modelName`.
    pub model_name: Vec<u8>,
}

/// `W3DPropDrawModuleData::buildFieldParse`'s table (W3DPropDraw.cpp:50-55).
pub static W3D_PROP_DRAW_FIELDS: [FieldParse<W3DPropDrawModuleData>; 2] = [
    FieldParse::new("ModelName", |i, d, _| i.parse_ascii_string().map(|v| d.model_name = v), UserData::None),
    FieldParse::end(),
];

impl W3DPropDrawModuleData {
    /// Reads the block's fields through [`W3D_PROP_DRAW_FIELDS`].
    pub fn parse_fields(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        ini.init_from_ini(self, &W3D_PROP_DRAW_FIELDS)
    }
}

/// What `TheTerrainRenderObject->addProp(...)` (BaseHeightMap) is given.
#[derive(Clone, Debug, PartialEq)]
pub struct AddPropRequest {
    pub drawable_id: i32,
    pub position: Coord3D,
    pub orientation: f32,
    pub scale: f32,
    pub model_name: Vec<u8>,
}

/// `class W3DPropDraw`.
#[derive(Clone, Debug, Default)]
pub struct W3DPropDraw {
    /// `m_propAdded`.
    pub prop_added: bool,
}

impl W3DPropDraw {
    /// `W3DPropDraw::W3DPropDraw` (W3DPropDraw.cpp:68): `m_propAdded(false)`.
    pub fn new() -> Self {
        W3DPropDraw { prop_added: false }
    }

    /// `W3DPropDraw::reactToTransformChange` (W3DPropDraw.cpp:84): the prop is given to the terrain once, the first
    /// time the drawable is anywhere but (0, 0).
    pub fn react_to_transform_change(&mut self, drawable: &DrawableFacts, drawable_id: u32, orientation: f32, module_data: Option<&W3DPropDrawModuleData>) -> Option<AddPropRequest> {
        if self.prop_added {
            return None;
        }
        if drawable.position.x == 0.0 && drawable.position.y == 0.0 {
            return None;
        }
        self.prop_added = true;
        let module_data = module_data?;
        let scale = drawable.scale;
        Some(AddPropRequest { drawable_id: drawable_id as i32, position: drawable.position, orientation, scale, model_name: module_data.model_name.clone() })
    }

    /// `W3DPropDraw::doDrawModule` (W3DPropDraw.cpp:108): draws nothing; the terrain draws the prop.
    pub fn do_draw_module(&mut self) {}

    /// `W3DPropDraw::crc` (W3DPropDraw.cpp:118).
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `W3DPropDraw::xfer` (W3DPropDraw.cpp:130): version 1; no data of its own.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        draw_module_xfer(xfer)
    }

    /// `W3DPropDraw::loadPostProcess` (W3DPropDraw.cpp:147).
    pub fn load_post_process(&mut self) {}
}
