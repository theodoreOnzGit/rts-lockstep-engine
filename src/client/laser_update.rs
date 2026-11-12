// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/LaserUpdate.cpp (LaserUpdateModuleData: its constructor
//     and buildFieldParse; LaserUpdate: its constructor, destructor, updateStartPos, updateEndPos, clientUpdate,
//     setDecayFrames, initLaser, getCurrentLaserRadius, crc, xfer, loadPostProcess)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/LaserUpdate.h (the inline accessors)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc. (Kris Morness / Graham Smallwood)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The laser's client update module (GitHub #165): `LaserUpdate`. Its source lies under `GameLogic/Object/Update/`
//! and it is registered among the update modules, but it is a `ClientUpdateModule` (`ModuleFactory.cpp:426`): a
//! drawable's, run each client frame, never part of the logic's state.
//!
//! # As upstream does it
//!
//! - **Its data** (`ClientUpdate = LaserUpdate`, LaserUpdate.cpp:54-72): `MuzzleParticleSystem` and
//!   `TargetParticleSystem` (`parseAsciiString`), `PunchThroughScalar` (`parseReal`, default 0).
//! - **Made** (LaserUpdate.cpp:76-96) clean: no positions, no systems, width scale 1, neither widening nor decaying, no
//!   parent or target drawable, no bone.
//! - **`initLaser(parent, target, startPos, endPos, parentBoneName, sizeDeltaFrames)`** (LaserUpdate.cpp:247-393):
//!   `sizeDeltaFrames > 0` widens over that many frames from now (width 0), `< 0` decays (width 1); the bone is kept;
//!   the start is the parent drawable's (bone) position, else `startPos`, else the drawable is destroyed; the end is the
//!   target's position (its drawable tracked) unless `endPos` is given, else the drawable is destroyed. Then, the first
//!   time (`!m_particleSystemID`), if there is no parent or the parent is at most partially clear to the local player,
//!   the muzzle and target systems are made from their templates; the systems are moved to the start and the end; the
//!   drawable goes to the middle of `startPos` and `endPos` (no parent) or to the parent's position; dirty.
//! - **`clientUpdate`** (LaserUpdate.cpp:200-232): the start follows the parent drawable (its bone, or its position;
//!   a bone it lacks puts the start at the drawable's position, upstream's `DEBUG_CRASH`, and returns without the dirty
//!   check), the end follows the target drawable; a target gone or dead stops being tracked and, with a punch-through
//!   scalar, the end is pushed through along the beam (`end = start + (end - start) * scalar`). A changed end or start
//!   is dirty. Decaying, the width is `1 - (now - start) / (finish - start)` until it reaches 0; widening, `(now -
//!   start) / (finish - start)` until it reaches 1 (then not widening).
//! - **`setDecayFrames(frames)`** (LaserUpdate.cpp:234-243): frames > 0 start a decay from now at width 1.
//! - **`getCurrentLaserRadius`** (LaserUpdate.cpp:396-413): the first draw module's laser template width (a
//!   `LaserDrawInterface`, `W3DLaserDraw`'s `m_outerBeamWidth * 0.5`) times the width scale; none, 0.
//! - **The destructor** (LaserUpdate.cpp:100-108) destroys both systems by id ([`LaserUpdate::destroy`]).
//! - **The save** (`xfer`, version 1, LaserUpdate.cpp:431-483): the base's (`DrawableModule`, `Module`), the start and
//!   end, dirty, both system ids (raw bytes), widening, decaying, the widen frames, the width scale, the decay frames,
//!   the parent and target drawable ids and the bone name. `crc` and `loadPostProcess` add nothing of their own.
//!
//! # Upstream quirks that are kept
//!
//! - The systems are made only when `m_particleSystemID` is 0 ("You cannot check an ID for NULL ... Can't change it on
//!   the last day without a bug though"): the target system is remade by a second `initLaser` when the muzzle one was
//!   never made.
//! - The frame sums and differences are `UnsignedInt` arithmetic, wrapping; `sizeDeltaFrames` is converted to it.
//! - A bone the parent's drawable lacks puts the start at the drawable's position without marking it dirty.
//! - The end's punch-through happens once: the target is forgotten in the same update.
//! - `initLaser` with no target and an `endPos` keeps a target drawable tracked from an earlier `initLaser`.
//!
//! # Undefined upstream ([`LaserUndefined`], the call does nothing more here)
//!
//! - `initLaser` with a parent and no local player reads through `NULL` (`localPlayer->getPlayerIndex()`).
//! - `initLaser` without a parent and with a target but no `endPos` adds `NULL` (`posToUse.add(endPos)`).
//! - `getCurrentLaserRadius` of a module without a drawable reads through `NULL`.
//!
//! # Where this differs from upstream
//!
//! - **The host** ([`LaserHost`]) answers what upstream reads of the client (`TheGameClient`, the drawables, the
//!   particle system manager, the player list) and the logic's frame.
//! - **The callers** are the logic's (`Weapon::fireWeaponTemplate`, `SpecialAbilityUpdate::initLaser`,
//!   `AssistedTargetingUpdate`, `ParticleUplinkCannonUpdate`), which upstream calls in place; here the logic emits client
//!   events and the client replays them after the frame (`client::drawable`), reading the parent's position then.

use crate::common::base_type::Coord3D;
use crate::common::game_common::ObjectShroudStatus;
use crate::common::game_type::{DrawableId, INVALID_DRAWABLE_ID};
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::common::thing_template::ClientModuleData;
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::client::particle_sys::{ParticleSystemId, INVALID_PARTICLE_SYSTEM_ID};
use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;

/// The module's name (`ClientUpdate = LaserUpdate`).
pub const LASER_UPDATE: &[u8] = b"LaserUpdate";

/// `LaserUpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct LaserUpdateModuleData {
    /// `m_particleSystemName`: the muzzle flare while the laser is active.
    pub particle_system_name: Vec<u8>,
    /// `m_targetParticleSystemName`: the target effect while the laser is active.
    pub target_particle_system_name: Vec<u8>,
    /// `m_punchThroughScalar`: if non-zero, the length modifier when the laser had a target object and now has none.
    pub punch_through_scalar: f32,
}

impl Default for LaserUpdateModuleData {
    /// `LaserUpdateModuleData()` (LaserUpdate.cpp:54): no names, no punch-through.
    fn default() -> Self {
        LaserUpdateModuleData { particle_system_name: Vec::new(), target_particle_system_name: Vec::new(), punch_through_scalar: 0.0 }
    }
}

/// `buildFieldParse`'s table (LaserUpdate.cpp:64-70; `ModuleData::buildFieldParse` adds none).
pub static LASER_UPDATE_FIELDS: [FieldParse<LaserUpdateModuleData>; 4] = [
    FieldParse::new("MuzzleParticleSystem", |i, d, _| i.parse_ascii_string().map(|v| d.particle_system_name = v), UserData::None),
    FieldParse::new("TargetParticleSystem", |i, d, _| i.parse_ascii_string().map(|v| d.target_particle_system_name = v), UserData::None),
    FieldParse::new("PunchThroughScalar", |i, d, _| i.parse_real().map(|v| d.punch_through_scalar = v), UserData::None),
    FieldParse::end(),
];

fn parse_client_update_block(ini: &mut Ini<'_>, d: &mut LaserUpdateModuleData) -> IniResult<()> {
    ini.init_from_ini(d, &LASER_UPDATE_FIELDS)
}

impl LaserUpdateModuleData {
    /// The module data from the `ClientUpdate` block the logic keeps as tokens, read through [`LASER_UPDATE_FIELDS`].
    pub fn parse(block: &ClientModuleData) -> IniResult<Self> {
        let mut text = b"ClientUpdate".to_vec();
        text.extend_from_slice(&crate::client::w3d_model_draw::client_block_text(block)[b"Draw".len()..]);
        let mut d = Self::default();
        let blocks = [BlockParse { token: "ClientUpdate", parse: parse_client_update_block }];
        Ini::load(b"ClientUpdate", &text, IniLoadType::Overwrite, None, &blocks, &mut d)?;
        Ok(d)
    }
}

/// What upstream's `LaserUpdate` leaves undefined (module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaserUndefined {
    /// `ThePlayerList->getLocalPlayer()` is `NULL` and is read through.
    NullLocalPlayer,
    /// `posToUse.add(endPos)` with `endPos` `NULL`.
    NullEndPos,
    /// `getDrawable()` is `NULL` and is read through.
    NullDrawable,
}

/// An object `initLaser` is given (`const Object *`): what it reads of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaserObject {
    /// The object's id (for its shrouded status).
    pub id: crate::common::game_type::ObjectId,
    /// `getDrawable() ? getDrawable()->getID() : none`.
    pub drawable: Option<DrawableId>,
    /// `*getPosition()`.
    pub position: Coord3D,
}

/// What `LaserUpdate` reads and calls besides itself.
pub trait LaserHost {
    /// `TheGameLogic->getFrame()`.
    fn logic_frame(&self) -> u32;
    /// `getDrawable() != NULL`.
    fn has_drawable(&self) -> bool;
    /// `*TheGameClient->findDrawableByID(id)->getPosition()`, `None` when there is no such drawable.
    fn drawable_position(&self, id: DrawableId) -> Option<Coord3D>;
    /// `findDrawableByID(id)->getCurrentWorldspaceClientBonePositions(bone, m)` (the drawable exists): the bone's
    /// transform, `None` when no draw module finds it.
    fn drawable_bone_transform(&self, id: DrawableId, bone: &[u8]) -> Option<Matrix3D>;
    /// `findDrawableByID(id)->getObject() ? getObject()->isEffectivelyDead() : FALSE` (the drawable exists).
    fn drawable_object_effectively_dead(&self, id: DrawableId) -> bool;
    /// `TheGameClient->destroyDrawable(getDrawable())`.
    fn destroy_own_drawable(&mut self);
    /// `ThePlayerList->getLocalPlayer()->getPlayerIndex()`, `None` for a `NULL` local player.
    fn local_player_index(&self) -> Option<i32>;
    /// `obj->getShroudedStatus(playerIndex)`.
    fn shrouded_status(&self, obj: &LaserObject, player_index: i32) -> ObjectShroudStatus;
    /// `TheParticleSystemManager->findTemplate(name) != NULL`.
    fn find_template(&self, name: &[u8]) -> bool;
    /// `TheParticleSystemManager->createParticleSystem(template)` (slaves made): the system's id, `None` for none.
    fn create_particle_system(&mut self, name: &[u8]) -> Option<ParticleSystemId>;
    /// `TheParticleSystemManager->findParticleSystem(id)`, then `system->setPosition(pos)` if found.
    fn set_particle_system_position(&mut self, id: ParticleSystemId, pos: &Coord3D);
    /// `TheParticleSystemManager->destroyParticleSystemByID(id)`.
    fn destroy_particle_system_by_id(&mut self, id: ParticleSystemId);
    /// `getDrawable()->setPosition(pos)` (the drawable exists).
    fn set_drawable_position(&mut self, pos: &Coord3D);
    /// The first draw module's `getLaserDrawInterface()->getLaserTemplateWidth()`, `None` when no draw module has the
    /// interface (the drawable exists).
    fn laser_template_width(&self) -> Option<f32>;
}

/// `Coord3D::operator==` (each member compared as a `Real`).
fn coord_eq(a: &Coord3D, b: &Coord3D) -> bool {
    a.x == b.x && a.y == b.y && a.z == b.z
}

/// `class LaserUpdate`.
#[derive(Clone, Debug, PartialEq)]
pub struct LaserUpdate {
    pub data: LaserUpdateModuleData,
    /// `m_startPos`, `m_endPos`.
    pub start_pos: Coord3D,
    pub end_pos: Coord3D,
    /// `m_parentID`, `m_targetID`.
    pub parent_id: DrawableId,
    pub target_id: DrawableId,
    /// `m_dirty`.
    pub dirty: bool,
    /// `m_particleSystemID`, `m_targetParticleSystemID`.
    pub particle_system_id: ParticleSystemId,
    pub target_particle_system_id: ParticleSystemId,
    /// `m_widening`, `m_decaying`.
    pub widening: bool,
    pub decaying: bool,
    /// `m_widenStartFrame`, `m_widenFinishFrame`.
    pub widen_start_frame: u32,
    pub widen_finish_frame: u32,
    /// `m_currentWidthScalar`.
    pub current_width_scalar: f32,
    /// `m_decayStartFrame`, `m_decayFinishFrame`.
    pub decay_start_frame: u32,
    pub decay_finish_frame: u32,
    /// `m_parentBoneName`.
    pub parent_bone_name: Vec<u8>,
}

impl LaserUpdate {
    /// `LaserUpdate(thing, moduleData)` (LaserUpdate.cpp:76-96).
    pub fn new(data: LaserUpdateModuleData) -> Self {
        LaserUpdate {
            data,
            start_pos: Coord3D::default(),
            end_pos: Coord3D::default(),
            parent_id: INVALID_DRAWABLE_ID,
            target_id: INVALID_DRAWABLE_ID,
            dirty: false,
            particle_system_id: INVALID_PARTICLE_SYSTEM_ID,
            target_particle_system_id: INVALID_PARTICLE_SYSTEM_ID,
            widening: false,
            decaying: false,
            widen_start_frame: 0,
            widen_finish_frame: 0,
            current_width_scalar: 1.0,
            decay_start_frame: 0,
            decay_finish_frame: 0,
            parent_bone_name: Vec::new(),
        }
    }

    /// `~LaserUpdate()` (LaserUpdate.cpp:100-108): both systems destroyed by id.
    pub fn destroy(&mut self, host: &mut impl LaserHost) {
        if self.particle_system_id != 0 {
            host.destroy_particle_system_by_id(self.particle_system_id);
        }
        if self.target_particle_system_id != 0 {
            host.destroy_particle_system_by_id(if plant(9621) { self.particle_system_id } else { self.target_particle_system_id });
        }
    }

    /// `updateStartPos()` (LaserUpdate.cpp:111-155).
    pub fn update_start_pos(&mut self, host: &impl LaserHost) {
        let old_start_pos = self.start_pos;
        if self.parent_id == INVALID_DRAWABLE_ID {
            return; // Can't update if not told to update
        }
        let Some(parent_pos) = host.drawable_position(self.parent_id) else {
            return; // Can't update if no one to ask
        };
        if !self.parent_bone_name.is_empty() {
            let Some(start_pos_matrix) = host.drawable_bone_transform(self.parent_id, &self.parent_bone_name) else {
                // failed to find the required bone: the drawable's position (and upstream's DEBUG_CRASH)
                self.start_pos = parent_pos;
                return;
            };
            self.start_pos.x = start_pos_matrix.get_x_translation();
            self.start_pos.y = start_pos_matrix.get_y_translation();
            self.start_pos.z = if plant(9622) { start_pos_matrix.get_y_translation() } else { start_pos_matrix.get_z_translation() };
        } else {
            // Just use parent position
            self.start_pos = parent_pos;
        }
        if !coord_eq(&self.start_pos, &old_start_pos) {
            self.dirty = true;
        }
    }

    /// `updateEndPos()` (LaserUpdate.cpp:158-195).
    pub fn update_end_pos(&mut self, host: &impl LaserHost) {
        let old_end_pos = self.end_pos;
        if self.target_id == INVALID_DRAWABLE_ID {
            return; // Can't update if not told to update
        }
        let target = host.drawable_position(self.target_id);
        let target_dead = target.is_some() && host.drawable_object_effectively_dead(self.target_id);
        match target {
            Some(pos) if !target_dead => self.end_pos = pos,
            _ => {
                // we used to track something, but now it is gone: pierce through the old spot and stop tracking
                if self.data.punch_through_scalar > 0.0 {
                    // `laserVector = (end - start) * scalar + start`, by Vector3's operators
                    let s = self.data.punch_through_scalar;
                    let mut v = [self.end_pos.x - self.start_pos.x, self.end_pos.y - self.start_pos.y, self.end_pos.z - self.start_pos.z];
                    for c in v.iter_mut() {
                        *c *= s;
                    }
                    let from = if plant(9623) { self.end_pos } else { self.start_pos };
                    self.end_pos.x = v[0] + from.x;
                    self.end_pos.y = v[1] + from.y;
                    self.end_pos.z = v[2] + from.z;
                }
                self.target_id = INVALID_DRAWABLE_ID;
            }
        }
        if !coord_eq(&self.end_pos, &old_end_pos) {
            self.dirty = true;
        }
    }

    /// `clientUpdate()` (LaserUpdate.cpp:200-232).
    pub fn client_update(&mut self, host: &impl LaserHost) {
        self.update_start_pos(host);
        self.update_end_pos(host);
        if self.decaying {
            let now = host.logic_frame();
            self.current_width_scalar = 1.0 - now.wrapping_sub(self.decay_start_frame) as f32 / self.decay_finish_frame.wrapping_sub(self.decay_start_frame) as f32;
            self.dirty = true;
            if self.current_width_scalar <= 0.0 {
                self.current_width_scalar = 0.0;
                // (When decay is finished... upstream's `TheGameLogic->destroyObject(getObject())` is commented out)
            }
        } else if self.widening {
            // resize the laser's width by the growth ratio completed
            let now = host.logic_frame();
            let span = if plant(9624) { self.widen_finish_frame.wrapping_sub(self.widen_start_frame).wrapping_add(1) } else { self.widen_finish_frame.wrapping_sub(self.widen_start_frame) };
            self.current_width_scalar = now.wrapping_sub(self.widen_start_frame) as f32 / span as f32;
            self.dirty = true;
            let done = if plant(9625) { self.current_width_scalar > 1.0 } else { self.current_width_scalar >= 1.0 };
            if done {
                self.current_width_scalar = 1.0;
                self.widening = false;
            }
        }
    }

    /// `setDecayFrames(decayFrames)` (LaserUpdate.cpp:234-243).
    pub fn set_decay_frames(&mut self, decay_frames: u32, host: &impl LaserHost) {
        if decay_frames > 0 {
            self.decaying = true;
            self.decay_start_frame = host.logic_frame();
            self.decay_finish_frame = self.decay_start_frame.wrapping_add(decay_frames);
            self.current_width_scalar = 1.0;
        }
    }

    /// `initLaser(parent, target, startPos, endPos, parentBoneName, sizeDeltaFrames)` (LaserUpdate.cpp:247-393).
    /// `Err` where upstream reads through `NULL` (what was done before it stays done).
    #[allow(clippy::too_many_arguments)]
    pub fn init_laser(
        &mut self,
        host: &mut impl LaserHost,
        parent: Option<&LaserObject>,
        target: Option<&LaserObject>,
        start_pos: Option<&Coord3D>,
        end_pos: Option<&Coord3D>,
        parent_bone_name: &[u8],
        size_delta_frames: i32,
    ) -> Result<(), LaserUndefined> {
        if size_delta_frames > 0 {
            self.widening = true;
            self.widen_start_frame = host.logic_frame();
            self.widen_finish_frame = self.widen_start_frame.wrapping_add(size_delta_frames as u32);
            self.current_width_scalar = 0.0;
        } else if size_delta_frames < 0 {
            self.decaying = true;
            self.decay_start_frame = host.logic_frame();
            self.decay_finish_frame = self.decay_start_frame.wrapping_sub(size_delta_frames as u32);
            self.current_width_scalar = 1.0;
        }
        // Write down the bone name override
        self.parent_bone_name = parent_bone_name.to_vec();
        // Record IDs if we have them, then figure out starting points
        if let Some(p) = parent {
            // If a source object, use it
            if let Some(id) = p.drawable {
                self.parent_id = id;
            }
            self.update_start_pos(host);
        } else if let Some(s) = start_pos {
            // or just use what they gave
            self.start_pos = *s;
        } else {
            // if they gave nothing, then we are screwed
            host.destroy_own_drawable();
            return Ok(());
        }
        match (target, end_pos) {
            (Some(t), None) => {
                // If a target object, use it (unless we override it!)
                if let Some(id) = t.drawable {
                    self.target_id = id;
                }
                self.end_pos = t.position;
            }
            (_, Some(e)) => {
                // just use what they gave, no override here
                self.end_pos = if plant(9626) { self.start_pos } else { *e };
            }
            (None, None) => {
                // if they gave nothing, then we are screwed
                host.destroy_own_drawable();
                return Ok(());
            }
        }
        // Create special particle systems (upstream checks the id against 0, its comment says it should not)
        if self.particle_system_id == 0 {
            let visible = match parent {
                None => true,
                Some(p) => {
                    let player = host.local_player_index().ok_or(LaserUndefined::NullLocalPlayer)?;
                    let ss = host.shrouded_status(p, player);
                    if plant(9627) { ss < ObjectShroudStatus::PartialClear } else { ss <= ObjectShroudStatus::PartialClear }
                }
            };
            // Make sure the laser flare is visible to the player. If no parent, assume laser owner will handle it.
            if visible {
                // If we don't have a particle system for the lense flare (muzzle flare), create it.
                if !self.data.particle_system_name.is_empty() && host.find_template(&self.data.particle_system_name) {
                    if let Some(id) = host.create_particle_system(&self.data.particle_system_name) {
                        self.particle_system_id = id;
                    }
                }
                // If we don't have a particle system for the target effect, create it.
                if !self.data.target_particle_system_name.is_empty() && host.find_template(&self.data.target_particle_system_name) {
                    if let Some(id) = host.create_particle_system(&self.data.target_particle_system_name) {
                        self.target_particle_system_id = id;
                    }
                }
            }
        }
        // Adjust the position of any existing particle system.
        if self.particle_system_id != 0 {
            host.set_particle_system_position(self.particle_system_id, &self.start_pos);
        }
        if self.target_particle_system_id != 0 {
            let at = if plant(9628) { self.start_pos } else { self.end_pos };
            host.set_particle_system_position(self.target_particle_system_id, &at);
        }
        // Set the laser position to the average of both points or else it probably won't get rendered (and as a
        // client update, we cannot set the logic position)
        let pos_to_use = match parent {
            None => {
                // (startPos is not NULL here: without a parent and a startPos the drawable was destroyed above)
                let s = start_pos.ok_or(LaserUndefined::NullEndPos)?;
                let e = end_pos.ok_or(LaserUndefined::NullEndPos)?;
                let mut p = *s;
                p.add(e);
                p.scale(0.5);
                p
            }
            Some(p) => p.position,
        };
        if host.has_drawable() {
            host.set_drawable_position(&pos_to_use);
        }
        self.dirty = true;
        Ok(())
    }

    /// `getCurrentLaserRadius()` (LaserUpdate.cpp:396-413).
    pub fn get_current_laser_radius(&self, host: &impl LaserHost) -> Result<f32, LaserUndefined> {
        if !host.has_drawable() {
            return Err(LaserUndefined::NullDrawable);
        }
        Ok(host.laser_template_width().map_or(0.0, |w| w * self.current_width_scalar))
    }

    /// `getStartPos()`, `getEndPos()`.
    pub fn get_start_pos(&self) -> &Coord3D {
        &self.start_pos
    }
    pub fn get_end_pos(&self) -> &Coord3D {
        &self.end_pos
    }

    /// `setDirty(dirty)`, `isDirty()`.
    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// `getWidthScale()`.
    pub fn get_width_scale(&self) -> f32 {
        self.current_width_scalar
    }

    /// `xfer(xfer)` (version 1, LaserUpdate.cpp:431-483), with `ClientUpdateModule::xfer` (which is
    /// `DrawableModule::xfer`, then `Module::xfer`: a version byte each).
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        drawable_module_base_xfer(xfer)?;
        xfer.xfer_coord3d(&mut self.start_pos)?;
        xfer.xfer_coord3d(&mut self.end_pos)?;
        xfer.xfer_bool(&mut self.dirty)?;
        xfer_particle_system_id(xfer, &mut self.particle_system_id)?;
        xfer_particle_system_id(xfer, &mut self.target_particle_system_id)?;
        if plant(9629) {
            xfer.xfer_bool(&mut self.decaying)?;
            xfer.xfer_bool(&mut self.widening)?;
        } else {
            xfer.xfer_bool(&mut self.widening)?;
            xfer.xfer_bool(&mut self.decaying)?;
        }
        xfer.xfer_unsigned_int(&mut self.widen_start_frame)?;
        xfer.xfer_unsigned_int(&mut self.widen_finish_frame)?;
        xfer.xfer_real(&mut self.current_width_scalar)?;
        xfer.xfer_unsigned_int(&mut self.decay_start_frame)?;
        xfer.xfer_unsigned_int(&mut self.decay_finish_frame)?;
        xfer.xfer_drawable_id(&mut self.parent_id)?;
        xfer.xfer_drawable_id(&mut self.target_id)?;
        xfer.xfer_ascii_string(&mut self.parent_bone_name)?;
        Ok(())
    }
}

/// `ClientUpdateModule::xfer`: the class declares none, so it is `DrawableModule::xfer` (version 1) and its
/// `Module::xfer` (version 1), Module.cpp:212-223 and 92-100.
pub fn drawable_module_base_xfer(xfer: &mut dyn Xfer) -> XferResult<()> {
    for _ in 0..2 {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
    }
    Ok(())
}

/// `xferUser(&id, sizeof(ParticleSystemID))`: the id's four bytes, little-endian. `fread` copies the bytes there are
/// before a short read fails: they are left in the id.
pub fn xfer_particle_system_id(xfer: &mut dyn Xfer, id: &mut ParticleSystemId) -> XferResult<()> {
    let mut b = id.to_le_bytes();
    let r = xfer.xfer_user(&mut b);
    *id = u32::from_le_bytes(b);
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::thing_template::ClientLine;

    #[derive(Default)]
    struct Host {
        frame: u32,
        log: Vec<String>,
        next: u32,
        target_alive: bool,
    }

    impl LaserHost for Host {
        fn logic_frame(&self) -> u32 {
            self.frame
        }
        fn has_drawable(&self) -> bool {
            true
        }
        fn drawable_position(&self, id: DrawableId) -> Option<Coord3D> {
            (id == 7 || (id == 8 && self.target_alive)).then_some(Coord3D { x: id as f32, y: 0.0, z: 0.0 })
        }
        fn drawable_bone_transform(&self, _: DrawableId, _: &[u8]) -> Option<Matrix3D> {
            None
        }
        fn drawable_object_effectively_dead(&self, _: DrawableId) -> bool {
            false
        }
        fn destroy_own_drawable(&mut self) {
            self.log.push("destroy".into());
        }
        fn local_player_index(&self) -> Option<i32> {
            Some(0)
        }
        fn shrouded_status(&self, _: &LaserObject, _: i32) -> ObjectShroudStatus {
            ObjectShroudStatus::Clear
        }
        fn find_template(&self, _: &[u8]) -> bool {
            true
        }
        fn create_particle_system(&mut self, name: &[u8]) -> Option<ParticleSystemId> {
            self.next += 1;
            self.log.push(format!("make {} {}", String::from_utf8_lossy(name), self.next));
            Some(self.next)
        }
        fn set_particle_system_position(&mut self, id: ParticleSystemId, pos: &Coord3D) {
            self.log.push(format!("at {id} {}", pos.x));
        }
        fn destroy_particle_system_by_id(&mut self, id: ParticleSystemId) {
            self.log.push(format!("kill {id}"));
        }
        fn set_drawable_position(&mut self, pos: &Coord3D) {
            self.log.push(format!("draw at {} {}", pos.x, pos.y));
        }
        fn laser_template_width(&self) -> Option<f32> {
            Some(4.0)
        }
    }

    /// Predicted: a widening laser between a parent (drawable 7) and a target given by its end: the two systems made
    /// once and placed; the width grows 0.25 a frame and stops at 1; the punch-through doubles the beam when the target
    /// goes; the destructor kills both systems.
    #[test]
    fn a_laser_widens_and_punches_through() {
        let data = LaserUpdateModuleData { particle_system_name: b"Muzzle".to_vec(), target_particle_system_name: b"Hit".to_vec(), punch_through_scalar: 2.0 };
        let mut l = LaserUpdate::new(data);
        let mut h = Host { frame: 100, target_alive: true, ..Default::default() };
        let parent = LaserObject { id: 1, drawable: Some(7), position: Coord3D { x: 7.0, y: 1.0, z: 0.0 } };
        let target = LaserObject { id: 2, drawable: Some(8), position: Coord3D { x: 8.0, y: 0.0, z: 0.0 } };
        l.init_laser(&mut h, Some(&parent), Some(&target), None, None, b"", 4).unwrap();
        assert_eq!(h.log, ["make Muzzle 1", "make Hit 2", "at 1 7", "at 2 8", "draw at 7 1"]);
        assert_eq!((l.parent_id, l.target_id, l.current_width_scalar), (7, 8, 0.0));
        h.frame = 101;
        l.client_update(&h);
        assert_eq!(l.current_width_scalar, 0.25);
        h.frame = 104;
        l.client_update(&h);
        assert_eq!((l.current_width_scalar, l.widening), (1.0, false));
        h.target_alive = false;
        l.client_update(&h);
        assert_eq!((l.end_pos.x, l.target_id), (9.0, INVALID_DRAWABLE_ID));
        assert_eq!(l.get_current_laser_radius(&h), Ok(4.0));
        h.log.clear();
        l.destroy(&mut h);
        assert_eq!(h.log, ["kill 1", "kill 2"]);
    }

    /// The save's layout: 3 version bytes, 2 coords, a bool, 2 ids, 2 bools, 2 frames, a real, 2 frames, 2 drawable
    /// ids, the bone (a length byte and its bytes): 63 bytes with a 3-byte bone; a load gives the module back.
    #[test]
    fn the_save_round_trips() {
        use crate::common::xfer::Xfer;
        let mut l = LaserUpdate::new(LaserUpdateModuleData::default());
        l.start_pos = Coord3D { x: 1.0, y: 2.0, z: 3.0 };
        l.particle_system_id = 9;
        l.decaying = true;
        l.decay_finish_frame = 77;
        l.parent_id = 5;
        l.parent_bone_name = b"MUZ".to_vec();
        let mut s = crate::common::xfer_save::XferSave::new();
        s.open(b"t").unwrap();
        l.xfer(&mut s).unwrap();
        let bytes = s.bytes().to_vec();
        assert_eq!(bytes.len(), 3 + 24 + 1 + 8 + 2 + 8 + 4 + 8 + 8 + 4);
        let mut back = LaserUpdate::new(LaserUpdateModuleData::default());
        let mut ld = crate::common::xfer_load::XferLoad::new(bytes);
        ld.open(b"t").unwrap();
        back.xfer(&mut ld).unwrap();
        assert_eq!(back, l);
    }

    /// The data: the defaults and the three fields.
    #[test]
    fn module_data_parses() {
        let line = |f: &str, a: &str| ClientLine { depth: 0, field: f.as_bytes().to_vec(), args: vec![a.as_bytes().to_vec()] };
        let block = ClientModuleData { module_tag_name_key: Default::default(), lines: vec![line("MuzzleParticleSystem", "Flare"), line("TargetParticleSystem", "Sparks"), line("PunchThroughScalar", "1.5")] };
        let d = LaserUpdateModuleData::parse(&block).unwrap();
        assert_eq!((d.particle_system_name.as_slice(), d.target_particle_system_name.as_slice(), d.punch_through_scalar), (&b"Flare"[..], &b"Sparks"[..], 1.5));
        let empty = ClientModuleData { module_tag_name_key: Default::default(), lines: vec![] };
        assert_eq!(LaserUpdateModuleData::parse(&empty).unwrap(), LaserUpdateModuleData::default());
    }
}
