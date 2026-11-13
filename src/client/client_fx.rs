// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp (WeaponTemplate::fireWeaponTemplate's client half:
//     `handleWeaponFireFX`, then `FXList::doFXPos` at the target or the drawable when not handled)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp
//     (W3DModelDraw::handleWeaponFireFX's `FXList::doFXPos` at the FX bone, or at the logic object when hidden)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp (GameClient::update's
//     `TheParticleSystemManager->setLocalPlayerIndex`)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (W3DDisplay::draw's
//     `TheParticleSystemManager->update()`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The client's effects (GitHub #43, #44): the FX lists and particle systems the logic's client events start, run with
//! the client's random, reading the World and the drawables only.
//!
//! # As upstream does it
//!
//! - **The logic plays FX lists** while it updates: `FXList::doFXObj`/`doFXPos` (deaths, upgrades, damage
//!   transitions, ...), and a weapon's fire FX through `Drawable::handleWeaponFireFX`, whose `W3DModelDraw` plays the
//!   list at the barrel's FX bone (or at the logic object when its render object is hidden); when no module handles
//!   it, `fireWeaponTemplate` plays it at the target (a contact weapon) or the drawable.
//! - **The logic makes particle systems** for some modules (`createAttachedParticleSystemID`).
//! - **The particle systems run** once per logic frame (`W3DDisplay::draw`'s `TheParticleSystemManager->update()`).
//!
//! # Where this differs from upstream
//!
//! - The logic emits client events; the client replays them after the frame ([`ClientFx::apply_event`], called in
//!   event order by `ClientDrawables::apply_events`, so the client random's draws keep the order they have among the
//!   drawables' own).
//! - **The FX bone's place** is the bone's pose as made (the skeleton's) times the object's transform: the animated
//!   pose, ~~the turret's turn and the barrel's recoil are~~ is not applied yet. **Corrected 2026-11-13 (#174):** the
//!   bone is chained from the render object's transform with its captured bones (the turret's turn and the barrel's
//!   recoil), `crate::client::w3d_model_draw::RenderObj::get_bone_transform`; ~~the animated pose is still not
//!   applied~~ **Corrected 2026-11-13 (#179):** with the animation's current pose at the sync time. The FX lists' bones
//!   (`doFxAtBones`) are read without writing the render objects (this host holds the drawables only to read), where
//!   upstream's query also moves the animation on (#179's gaps).
//! - **The local player** is the client's (`ClientDrawables::local_player_index`), else the World's; with none, the
//!   shroud is taken as clear.
//! - **The particle limit** is `TheGlobalData->m_maxParticleCount`, which upstream's static game LOD sets
//!   (`GameLODManager::applyStaticLODLevel`, ~~not translated~~ **Corrected 2026-11-12 (#160):** translated in
//!   `crate::client::game_lod`: with the shell's LOD manager, [`ClientFx::lod`] carries the cap it applied to the
//!   client's global data, and the dynamic level's particle priorities and skip mask and count); when the game data
//!   leaves it 0, the client uses [`DEFAULT_MAX_PARTICLE_COUNT`] (new), and never more than [`MAX_CLIENT_PARTICLES`]
//!   (new: the UI thread's cap). With no LOD manager (no shell, or its INI missing), the World's game data's cap and no
//!   dynamic limit, as before. A dynamic level never applied leaves upstream's priorities uninitialised (undefined); the
//!   client then takes the lowest priority (no limit).
//! - ~~`FXListAtBonePos` finds no bones yet~~ **Corrected 2026-11-13 (#174):** `FXListAtBonePos` finds the drawable's
//!   current bones (`getCurrentClientBonePositions`, [`WorldFxHost::bone_positions`]); `CreateAtGroundHeight` takes the
//!   ground layer (`getLayerForDestination`'s bridges are not read).
//! - The requests of nuggets not drawn yet (sounds, light pulses, shakes, scorches, tracers, rays) are kept in
//!   [`ClientFx::requests`] for the app, which may drop them.

use crate::client::drawable::Drawable;
use crate::client::fx_list::{FxHost, FxListStore, FxPlay, FxRequest};
use crate::client::particle_sys::{Attachment, ParticleHost, ParticleSettings, ParticleSystemManager};
use crate::common::base_type::Coord3D;
use crate::common::game_common::{CellShroudStatus, ObjectShroudStatus};
use crate::common::game_type::{ObjectId, INVALID_ID, LAYER_GROUND};
use crate::common::ini::IniResult;
use crate::common::random_value::RandomValues;
use crate::logic::client_event::ClientEvent;
use crate::logic::weapon_host::WeaponFx;
use crate::logic::world::game_logic::World;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::ww::matrix3d::Matrix3D;
use std::collections::BTreeMap;

/// The particle limit when the game data leaves `MaxParticleCount` at 0 (new; see the module docs).
pub const DEFAULT_MAX_PARTICLE_COUNT: i32 = 2500;
/// The most particles the client keeps (new: the UI thread's cap).
pub const MAX_CLIENT_PARTICLES: i32 = 4000;

/// A weapon's fire FX as `ClientDrawables` hands it over: the event's facts and, when a draw module handled it, where.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponFireFx {
    pub source: ObjectId,
    pub fx: Vec<u8>,
    pub speed: f32,
    pub target: Coord3D,
    pub radius: f32,
    pub contact_weapon: bool,
    /// `Some` when handled: the FX bone's transform ~~in model space~~ **Corrected 2026-11-12 (#172):** in the world,
    /// as the render object's `Get_Bone_Transform` gives it (`None` inside: no skeleton) and whether the render object
    /// is hidden.
    pub handled: Option<(Option<Matrix3D>, bool)>,
}

/// The FX lists, the particle systems and what the app is asked to do.
#[derive(Clone, Debug, Default)]
pub struct ClientFx {
    pub fx_lists: FxListStore,
    pub particles: ParticleSystemManager,
    /// Requests of nuggets the app may act on (drained by it).
    pub requests: Vec<FxRequest>,
    /// The INI files that did not load: name, why.
    pub errors: Vec<(Vec<u8>, String)>,
    /// The logic's particle system ids (`ObjectClientEvent::ParticleSystemAtBone`'s `id`) to the client's.
    pub logic_ids: BTreeMap<u32, u32>,
    /// (#160) `TheGameLODManager`'s particle settings, when the client has the manager (the owner copies them in each
    /// frame and the count back: [`ParticleLod`]).
    pub lod: Option<ParticleLod>,
}

/// (#160) What the particle systems read of `TheGameLODManager` and the global data its static level wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleLod {
    /// `TheGlobalData->m_maxParticleCount` as the static level set it (the client's copy of the global data).
    pub max_particle_count: i32,
    /// `getMinDynamicParticlePriority()`, `getMinDynamicParticleSkipPriority()` (`None`: no dynamic level applied).
    pub min_dynamic_particle_priority: Option<i32>,
    pub min_dynamic_particle_skip_priority: Option<i32>,
    /// `m_dynamicParticleSkipMask`.
    pub particle_skip_mask: i32,
    /// `m_numParticleGenerations`, advanced by `isParticleSkipped` during the update.
    pub particle_generations: i32,
}

/// The World and the drawables as [`FxHost`] and [`ParticleHost`] read them (all `&`: nothing is written).
pub struct WorldFxHost<'a> {
    pub world: &'a World,
    pub drawables: &'a BTreeMap<ObjectId, Drawable>,
    pub local_player_index: Option<i32>,
    /// (#179) `WW3D::Get_Sync_Time()`, at which the drawables' animated bones are posed.
    pub sync_time: u32,
}

impl WorldFxHost<'_> {
    fn object(&self, id: ObjectId) -> Option<(Coord3D, Matrix3D)> {
        self.world.find_object(id).map(|o| (*o.thing.position(), *o.thing.transform_matrix()))
    }
}

impl FxHost for WorldFxHost<'_> {
    fn cell_clear_for_local_player(&self, pos: &Coord3D) -> bool {
        self.local_player_index.is_none_or(|p| self.world.partition.get_shroud_status_for_player(p, pos) == CellShroudStatus::Clear)
    }
    fn object_fogged_for_local_player(&self, obj: ObjectId) -> bool {
        self.local_player_index.is_some_and(|p| self.world.get_shrouded_status(obj, p) > ObjectShroudStatus::PartialClear)
    }
    fn object_position_and_transform(&self, obj: ObjectId) -> Option<(Coord3D, Matrix3D)> {
        self.object(obj)
    }
    fn object_bounding_circle_radius(&self, obj: ObjectId) -> f32 {
        self.world.find_object(obj).map_or(0.0, |o| o.geometry_info.get_bounding_circle_radius())
    }
    fn layer_height_for_destination(&self, pos: &Coord3D) -> f32 {
        self.world.peek_layer_height_and_normal(pos.x, pos.y, LAYER_GROUND).0
    }
    /// ~~No bones yet (see the module docs).~~ **Corrected 2026-11-13 (#174):** `FXListAtBonePosFXNugget::doFxAtBones`
    /// (FXList.cpp:736-753): the object's drawable's `getCurrentClientBonePositions(boneName, start, bonePos, boneMtx,
    /// MAX_BONE_POINTS)` (40; [`Drawable::get_current_client_bone_positions`], the render objects' current bones in model
    /// space) and each through `obj->convertBonePosToWorldPos` (the object's transform). None without a drawable or an
    /// object.
    fn bone_positions(&self, obj: ObjectId, bone_name: &[u8], start: i32) -> Vec<(Coord3D, Matrix3D)> {
        const MAX_BONE_POINTS: usize = 40;
        let (Some(o), Some(draw)) = (self.world.find_object(obj), self.drawables.get(&obj)) else { return Vec::new() };
        let mut bone_pos = [Coord3D::default(); MAX_BONE_POINTS];
        let mut bone_mtx = [Matrix3D::IDENTITY; MAX_BONE_POINTS];
        let count = draw.get_current_client_bone_positions(bone_name, start, Some(&mut bone_pos), Some(&mut bone_mtx), MAX_BONE_POINTS as i32, self.sync_time);
        (0..count.max(0) as usize).filter(|&i| i < MAX_BONE_POINTS).map(|i| o.thing.convert_bone_pos_to_world_pos(&bone_pos[i], &bone_mtx[i])).collect()
    }
}

impl ParticleHost for WorldFxHost<'_> {
    fn drawable(&self, id: ObjectId) -> Option<Attachment> {
        let d = self.drawables.get(&id)?;
        let (position, transform) = self.object(id)?;
        Some(Attachment { transform, position, shrouded: d.get_fully_obscured_by_shroud() })
    }
    fn object(&self, id: ObjectId, local_player_index: i32) -> Option<Attachment> {
        let (position, transform) = self.object(id)?;
        let shrouded = self.local_player_index.is_some() && self.world.get_shrouded_status(id, local_player_index) >= ObjectShroudStatus::Fogged;
        Some(Attachment { transform, position, shrouded })
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        self.world.peek_layer_height_and_normal(x, y, LAYER_GROUND).0
    }
}

impl ClientFx {
    pub fn new() -> Self {
        Self::default()
    }

    /// The client's INI files (`ParticleSystemManager::init`, then the FX list store's), each if `open` has it:
    /// `Data\INI\ParticleSystem.ini`, `Data\INI\Default\FXList.ini`, `Data\INI\FXList.ini`. A file that does not
    /// parse is listed in [`ClientFx::errors`] (upstream refuses it).
    pub fn load(&mut self, open: impl Fn(&[u8]) -> Option<Vec<u8>>) {
        let mut note = |path: &[u8], r: IniResult<()>| {
            if let Err(e) = r {
                self.errors.push((path.to_vec(), format!("{e:?}")));
            }
        };
        let p = b"Data\\INI\\ParticleSystem.ini";
        if let Some(t) = open(p) {
            note(p, self.particles.load_text(p, &t));
        }
        for p in [b"Data\\INI\\Default\\FXList.ini".as_slice(), b"Data\\INI\\FXList.ini"] {
            if let Some(t) = open(p) {
                note(p, self.fx_lists.load_text(p, &t));
            }
        }
    }

    /// The particle settings from the World's game data, and (#160) the LOD manager's when there is one (see the
    /// module docs for the limit).
    pub fn settings(w: &World, lod: Option<&ParticleLod>) -> ParticleSettings {
        let mut s = ParticleSettings::default();
        if let Some(g) = w.statics.global_data.the_global_data() {
            s.use_fx = g.use_fx;
            s.particle_scale = g.particle_scale;
            s.max_particle_count = g.max_particle_count;
            s.max_field_particle_count = g.max_field_particle_count;
        }
        if let Some(l) = lod {
            s.max_particle_count = l.max_particle_count;
            s.min_dynamic_particle_priority = l.min_dynamic_particle_priority.unwrap_or(crate::client::particle_sys::priority::PARTICLE_PRIORITY_LOWEST);
            s.min_dynamic_particle_skip_priority = l.min_dynamic_particle_skip_priority.unwrap_or(crate::client::particle_sys::priority::PARTICLE_PRIORITY_LOWEST);
            s.particle_skip_mask = l.particle_skip_mask;
            s.particle_generations.set(l.particle_generations);
        }
        if s.max_particle_count == 0 {
            s.max_particle_count = DEFAULT_MAX_PARTICLE_COUNT;
        }
        s.max_particle_count = s.max_particle_count.min(MAX_CLIENT_PARTICLES);
        s
    }

    /// One of the logic's client events, if it is an effect's: FX lists played (a weapon's fire FX goes through
    /// [`ClientFx::weapon_fire_fx`] once the drawables have answered for it), the logic's attached particle systems
    /// made.
    pub fn apply_event(&mut self, host: &WorldFxHost<'_>, random: &mut RandomValues, e: &ClientEvent) {
        let frame = host.world.frame();
        let ClientEvent::Object(e) = e else { return };
        let mut play = FxPlay { random, particles: &mut self.particles, frame, requests: &mut self.requests };
        let store = &self.fx_lists;
        match e {
            ObjectClientEvent::FxListDoFxObj { fx, object_id } => store.do_fx_obj(fx, host, &mut play, Some(*object_id), None),
            ObjectClientEvent::FxListDoFxObjSecondary { fx, object_id, secondary } => {
                let secondary = (*secondary != INVALID_ID).then_some(*secondary);
                store.do_fx_obj(fx, host, &mut play, Some(*object_id), secondary)
            }
            ObjectClientEvent::FxListDoFxPos { fx, position } => store.do_fx_pos(fx, host, &mut play, Some(position), None, 0.0, None, 0.0),
            ObjectClientEvent::FxListDoFxPosRadius { fx, position, radius } => store.do_fx_pos(fx, host, &mut play, Some(position), None, 0.0, None, *radius),
            ObjectClientEvent::ParticleSystemCreateAttached { template, object_id } => {
                play.particles.create_attached_particle_system_id(template, *object_id, true, play.random, frame);
            }
            ObjectClientEvent::WeaponFx(WeaponFx::Disarm { fx: Some(fx), at }) => {
                // `FXList::doFXPos(fireFX, pos, transform, 0, pos, 0)` at the mine
                if let Some((pos, mtx)) = host.object(*at) {
                    store.do_fx_pos(fx, host, &mut play, Some(&pos), Some(&mtx), 0.0, Some(&pos), 0.0);
                }
            }
            _ => {}
        }
    }

    /// A weapon's fire FX (`fireWeaponTemplate`'s client half): handled by a draw module, at its FX bone (or the
    /// logic object when the render object is hidden); else at the target for a contact weapon, or the drawable.
    pub fn weapon_fire_fx(&mut self, host: &WorldFxHost<'_>, random: &mut RandomValues, f: &WeaponFireFx) {
        let frame = host.world.frame();
        let Some((obj_pos, obj_mtx)) = host.object(f.source) else { return };
        let mut play = FxPlay { random, particles: &mut self.particles, frame, requests: &mut self.requests };
        let store = &self.fx_lists;
        match f.handled {
            Some((Some(bone), false)) => {
                // (#172) `Matrix3D mtx = m_renderObject->Get_Bone_Transform(info.m_fxBone)` (W3DModelDraw.cpp:3727): already
                // in the world
                let mtx = bone;
                let t = mtx.get_translation();
                let pos = Coord3D { x: t.x, y: t.y, z: t.z };
                store.do_fx_pos(&f.fx, host, &mut play, Some(&pos), Some(&mtx), f.speed, Some(&f.target), f.radius);
            }
            Some(_) => store.do_fx_pos(&f.fx, host, &mut play, Some(&obj_pos), Some(&obj_mtx), f.speed, Some(&f.target), f.radius),
            None => {
                let at = if f.contact_weapon { f.target } else { obj_pos };
                store.do_fx_pos(&f.fx, host, &mut play, Some(&at), Some(&obj_mtx), f.speed, Some(&f.target), f.radius);
            }
        }
    }

    /// `TheParticleSystemManager->setLocalPlayerIndex(...)` and `update()` at the World's frame.
    pub fn update(&mut self, host: &WorldFxHost<'_>, random: &mut RandomValues) {
        self.particles.local_player_index = host.local_player_index.unwrap_or(0);
        let settings = Self::settings(host.world, self.lod.as_ref());
        let frame = host.world.frame();
        self.particles.update(host, random, &settings, frame, frame);
        if let Some(l) = self.lod.as_mut() {
            l.particle_generations = settings.particle_generations.get();
        }
    }
}
