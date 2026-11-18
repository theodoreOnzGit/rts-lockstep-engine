//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DGameClient.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DGameClient.h
//!
//! `W3DGameClient` extends `GameClient` (init/update/reset only call the base: nothing of their own). What it adds is
//! the device hooks below, each translated against a host trait naming the object it touches
//! (`TheTerrainRenderObject`, `TheWaterRenderObj`, `TheW3DShadowManager`, `TheDisplay`, `TheRayEffects`,
//! `TheThingFactory`, `TheGameLODManager`). Already in the port: the texture reduction state
//! (`crate::client::game_lod`, whose `adjust_lod` is the manager's own), `RayEffectSystem` (`crate::client::fx_list`).

use rts_lockstep_engine::common::base_type::Coord3D;

/// `Scorches` (Scorch.h): the kind of scorch mark.
pub type Scorches = i32;
/// `TimeOfDay`.
pub type TimeOfDay = i32;

/// The objects `W3DGameClient`'s hooks call.
pub trait W3DGameClientHost {
    /// `TheTerrainRenderObject != NULL`.
    fn has_terrain_render_object(&self) -> bool;
    /// `TheTerrainRenderObject->addScorch(loc, radius, type)`.
    fn terrain_add_scorch(&mut self, loc: (f32, f32, f32), radius: f32, scorch_type: Scorches);
    /// `TheTerrainRenderObject->setTextureLOD(reduction)`.
    fn terrain_set_texture_lod(&mut self, reduction: i32);
    /// `TheTerrainRenderObject->unitMoved(obj)`.
    fn terrain_unit_moved(&mut self, object_id: u32);
    /// `TheThingFactory->newDrawable(tmpl)`: the new drawable's id, `None` for `NULL`.
    fn new_drawable(&mut self, template: &[u8]) -> Option<u32>;
    /// `draw->setPosition(&pos)`.
    fn drawable_set_position(&mut self, drawable: u32, pos: &Coord3D);
    /// `TheRayEffects->addRayEffect(draw, start, end)`.
    fn add_ray_effect(&mut self, drawable: u32, start: &Coord3D, end: &Coord3D);
    /// `GameClient::setTimeOfDay(tod)`.
    fn game_client_set_time_of_day(&mut self, tod: TimeOfDay);
    /// `TheWaterRenderObj->setTimeOfDay(tod)` when it exists.
    fn water_set_time_of_day(&mut self, tod: TimeOfDay);
    /// `TheW3DShadowManager->setTimeOfDay(tod)` when it exists.
    fn shadow_manager_set_time_of_day(&mut self, tod: TimeOfDay);
    /// `TheDisplay->setTimeOfDay(tod)`.
    fn display_set_time_of_day(&mut self, tod: TimeOfDay);
    /// `W3DStatusCircle::setColor(r, g, b)`.
    fn status_circle_set_color(&mut self, r: i32, g: i32, b: i32);
    /// `TheGlobalData != NULL`, `TheWritableGlobalData->m_textureReductionFactor` (get / set).
    fn has_global_data(&self) -> bool;
    fn texture_reduction_factor(&self) -> i32;
    fn set_texture_reduction_factor(&mut self, v: i32);
    /// `WW3D::Get_Texture_Reduction()`.
    fn ww3d_texture_reduction(&self) -> i32;
    /// `WW3D::Set_Texture_Reduction(v, 32)`.
    fn ww3d_set_texture_reduction(&mut self, v: i32, min_dim: i32);
    /// `TheGameLODManager->setCurrentTextureReduction(v)`.
    fn lod_manager_set_current_texture_reduction(&mut self, v: i32);
}

/// `class W3DGameClient`: holds no data of its own.
#[derive(Clone, Copy, Debug, Default)]
pub struct W3DGameClient;

impl W3DGameClient {
    /// `W3DGameClient::W3DGameClient` (W3DGameClient.cpp:63).
    pub fn new() -> Self {
        W3DGameClient
    }

    /// `W3DGameClient::init` (W3DGameClient.cpp:75): extends `GameClient::init()`, which is the caller's.
    pub fn init(&mut self) {}

    /// `W3DGameClient::update` (W3DGameClient.cpp:85): extends `GameClient::update()`.
    pub fn update(&mut self) {}

    /// `W3DGameClient::reset` (W3DGameClient.cpp:96): extends `GameClient::reset()`.
    pub fn reset(&mut self) {}

    /// `W3DGameClient::friend_createDrawable` (W3DGameClient.cpp:111): `newInstance(Drawable)(tmplate, statusBits)`;
    /// `None` for a `NULL` template. The drawable itself is `crate::client::drawable`'s; the caller makes it.
    pub fn friend_create_drawable(&self, template: Option<&[u8]>) -> bool {
        template.is_some()
    }

    /// `W3DGameClient::addScorch` (W3DGameClient.cpp:128).
    pub fn add_scorch(&mut self, host: &mut dyn W3DGameClientHost, pos: &Coord3D, radius: f32, scorch_type: Scorches) {
        if host.has_terrain_render_object() {
            host.terrain_add_scorch((pos.x, pos.y, pos.z), radius, scorch_type);
        }
    }

    /// `W3DGameClient::createRayEffectByTemplate` (W3DGameClient.cpp:144): the drawable sits at the midpoint of the
    /// laser.
    pub fn create_ray_effect_by_template(&mut self, host: &mut dyn W3DGameClientHost, start: &Coord3D, end: &Coord3D, tmpl: &[u8]) {
        if let Some(draw) = host.new_drawable(tmpl) {
            // add to world, the location of the drawable is at the midpoint of laser
            let pos = Coord3D { x: (end.x - start.x) * 0.5 + start.x, y: (end.y - start.y) * 0.5 + start.y, z: (end.z - start.z) * 0.5 + start.z };
            host.drawable_set_position(draw, &pos);

            // add this ray effect to the list of ray effects
            host.add_ray_effect(draw, start, end);
        }
    }

    /// `W3DGameClient::setTimeOfDay` (W3DGameClient.cpp:171): tell all the drawables what time of day it is now.
    pub fn set_time_of_day(&mut self, host: &mut dyn W3DGameClientHost, tod: TimeOfDay, has_water: bool, has_shadow_manager: bool) {
        host.game_client_set_time_of_day(tod);

        //tell cloud/water plane to update its lighting/texture
        if has_water {
            host.water_set_time_of_day(tod);
        }
        if has_shadow_manager {
            host.shadow_manager_set_time_of_day(tod);
        }

        //tell the display to update its lighting
        host.display_set_time_of_day(tod);
    }

    /// `W3DGameClient::setTeamColor` (W3DGameClient.cpp:192).
    pub fn set_team_color(&mut self, host: &mut dyn W3DGameClientHost, red: i32, green: i32, blue: i32) {
        host.status_circle_set_color(red, green, blue);
    }

    /// `W3DGameClient::adjustLOD` (W3DGameClient.cpp:200): temporary entry point for adjusting LOD for development
    /// testing.
    pub fn adjust_lod(&mut self, host: &mut dyn W3DGameClientHost, adj: i32) {
        if !host.has_global_data() {
            return;
        }
        let mut v = host.texture_reduction_factor() + adj;
        if v > 4 {
            v = 4; //16x less resolution is probably enough.
        }
        if v < 0 {
            v = 0;
        }
        host.set_texture_reduction_factor(v);
        if host.ww3d_texture_reduction() != v {
            host.ww3d_set_texture_reduction(v, 32);
            host.lod_manager_set_current_texture_reduction(v);
            if host.has_terrain_render_object() {
                host.terrain_set_texture_lod(v);
            }
        }
    }

    /// `W3DGameClient::notifyTerrainObjectMoved` (W3DGameClient.cpp:224): so the terrain can knock down trees or crush
    /// grass or whatever is appropriate. jba.
    pub fn notify_terrain_object_moved(&mut self, host: &mut dyn W3DGameClientHost, object_id: u32) {
        if host.has_terrain_render_object() {
            host.terrain_unit_moved(object_id);
        }
    }
}
