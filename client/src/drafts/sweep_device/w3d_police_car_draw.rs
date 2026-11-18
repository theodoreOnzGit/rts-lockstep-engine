//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DPoliceCarDraw.cpp
//!
//! Sweep 2 (#213): `W3DPoliceCarDraw::createDynamicLight`.

use super::w3d_scene::DynamicLightList;

/// The values `createDynamicLight` sets on a new light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoliceCarLight {
    /// `light->setEnabled(TRUE)` (the scene already did, in `getADynamicLight`).
    pub enabled: bool,
    /// `Set_Ambient` / `Set_Diffuse` / `Set_Position`: all black / the origin.
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub position: [f32; 3],
    /// `Set_Far_Attenuation_Range(5, 15)`: start and end.
    pub far_attenuation_range: (f32, f32),
}

/// `W3DPoliceCarDraw::createDynamicLight` (W3DPoliceCarDraw.cpp:47): takes a dynamic light from the scene
/// (`W3DDisplay::m_3DScene->getADynamicLight()`, which never returns NULL) and sets it up for a flashing police light.
/// Returns the light's index in the scene's list and its settings.
pub fn create_dynamic_light(lights: &mut DynamicLightList) -> (usize, PoliceCarLight) {
    let index = lights.get_a_dynamic_light();
    (index, PoliceCarLight { enabled: true, ambient: [0.0; 3], diffuse: [0.0; 3], position: [0.0; 3], far_attenuation_range: (5.0, 15.0) })
}
