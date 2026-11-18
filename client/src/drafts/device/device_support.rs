//! UNTESTED DRAFT (#202, #205, #214): compiles; not yet tested against the upstream C++.
//!
//! Not a translation of one upstream file: the small stand-ins the W3DDevice drafts of this directory share, so that
//! each draft can keep upstream's control flow without a render object, a scene or a drawable to call. Each is named
//! for the upstream thing it stands for and carries only what the drafts read. Replace them with the port's own types
//! (`crate::formats::w3d::render`, `crate::client::drawable`) when the drafts are compiled.
//!
//! Upstream classes stood for (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DScene.h (`RTS3DScene`'s Add/Remove_Render_Object)
//!   GeneralsMD/Code/GameEngine/Include/Common/DrawModule.h (`DrawModule::xfer`)
//!   GeneralsMD/Code/GameEngine/Include/GameClient/Drawable.h (the getters the draw modules call)

use rts_lockstep_engine::client::laser_update::drawable_module_base_xfer;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::xfer::{Xfer, XferResult, XferVersion};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;

/// `RenderObjClass*` (device-only object; the id the scene knows it by). 0 stands for `NULL`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RenderObjHandle(pub u32);

impl RenderObjHandle {
    /// `m_renderObject == NULL`.
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

/// `HAnimClass*`. 0 stands for `NULL`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HAnimHandle(pub u32);

/// `Shadow*`. 0 stands for `NULL`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShadowHandle(pub u32);

/// `TextureClass*`. 0 stands for `NULL`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

/// What the draw modules read of their `Drawable` (Drawable.h): `getScale`, `getInstanceScale`,
/// `getTransformMatrix`, `getPosition`, and of its `Object` the one question `W3DDebrisDraw` asks.
#[derive(Clone, Debug)]
pub struct DrawableFacts {
    /// `getScale()`.
    pub scale: f32,
    /// `getInstanceScale()`.
    pub instance_scale: f32,
    /// `getTransformMatrix()`.
    pub transform: Matrix3D,
    /// `getPosition()`.
    pub position: Coord3D,
    /// `getObject() != NULL`.
    pub has_object: bool,
    /// `getObject()->isAboveTerrain()` (read only when `has_object`).
    pub object_is_above_terrain: bool,
}

/// `DrawModule::xfer` (DrawModule.cpp): its version byte (1), then `DrawableModule::xfer` and `Module::xfer` (a version
/// byte each): three bytes, no data of its own. Used by every draw module of this directory after its own version.
pub fn draw_module_xfer(xfer: &mut dyn Xfer) -> XferResult<()> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)?;
    drawable_module_base_xfer(xfer)
}
