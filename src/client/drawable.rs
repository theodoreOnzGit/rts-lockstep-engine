// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp
//   (Drawable::Drawable's draw modules and their LOD filter,
//   clearAndSetModelConditionState, clearAndSetModelConditionFlags,
//   replaceModelConditionFlags, getDrawModules with DIRTY_CONDITION_FLAGS,
//   reactToBodyDamageStateChange's flags, getShouldAnimate,
//   setAnimationLoopDuration; GitHub #27: showSubObject, updateSubObjects,
//   handleWeaponFireFX without the locomotor recoil; GitHub #41: Drawable::draw's
//   call of applyPhysicsXform, translated in drawable_loco.rs; GitHub #45:
//   Drawable::draw's early return, setStealthLook's m_hiddenByStealth,
//   updateHiddenStatus, setFullyObscuredByShroud, isDrawableEffectivelyHidden)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp
//   (GitHub #45: GameClient::update's shroud status of each drawable)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp
//   (GitHub #45: RTS3DScene::renderOneObject's setShroudClearFrame)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Drawable.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The client's drawables (GitHub #25): one per object the logic tells the
//! client about, holding its model condition flags and its `W3DModelDraw`
//! modules ([`super::w3d_model_draw`]).
//!
//! # As upstream reads it
//!
//! - **A drawable's draw modules** are made from its template's `Draw`
//!   module list, in order, leaving out a module whose `MinLODRequired` is
//!   above the static LOD when `m_useDrawModuleLOD` is set.
//! - **The flags**: `clearAndSetModelConditionFlags` clears, then sets; when
//!   the flags changed, the drawable is marked dirty (`DIRTY_CONDITION_FLAGS`
//!   is defined), and the next `getDrawModules()` gives each module
//!   `replaceModelConditionState(flags)` once. A body damage state replaces
//!   `DAMAGED`, `REALLY_DAMAGED` and `RUBBLE` with its own bit.
//! - **The client-side physics** (GitHub #41): before its draw modules, a drawable that is not hidden applies its
//!   object's physics transform (`applyPhysicsXform`, [`super::drawable_loco`]), whose wheel info the truck draws
//!   read.
//! - **`Drawable::draw` returns at once** (GitHub #45) when the drawable is hidden (`m_hidden`), hidden by stealth
//!   (`m_hiddenByStealth`, set by `setStealthLook(STEALTHLOOK_INVISIBLE)`) or fully obscured by the shroud
//!   (`getFullyObscuredByShroud()`): no `applyPhysicsXform`, no `getDrawModules()` (so dirty flags wait), no
//!   `doDrawModule`. The shroud flag is set each client frame before the draw (`GameClient::update`): fogged or
//!   shrouded for the local player, except for two seconds (five when the object is effectively dead) after the frame
//!   the scene last drew it clear (`setShroudClearFrame`, `RTS3DScene::renderOneObject`).
//! - **`getShouldAnimate(considerPower)`**: no when the object is unpowered by
//!   script (power considered), or disabled as hacked, paralyzed, EMP'd,
//!   subdued or unmanned (unless it is produced at a helipad), or underpowered
//!   (power considered).
//!
//! - **Saved games** (GitHub #59): `Drawable::xfer` and the modules' xfers are [`super::drawable_save`]'s; the members a
//!   save holds that this drawable does not use are kept in [`Drawable::saved`].
//!
//! # Where this differs from upstream
//!
//! - **Upstream's logic calls the drawable** while it updates. Here the logic
//!   emits client events ([`ClientEvent`]) and the client replays them after
//!   the frame ([`ClientDrawables::apply_events`]), in order: the flags end the
//!   frame as upstream's would. What the logic itself reads back of a
//!   drawable's flags is the logic's own state (`ClientReads`), which this
//!   module does not write: the client never changes the simulation.
//! - A drawable's first appearance is its `ObjectCreated` event, or (for
//!   objects made before the client attached) the first time the client sees
//!   its object.
//! - The module data are parsed from the `Draw` blocks the logic keeps as
//!   tokens ([`W3DModelDrawModuleData::parse`]), once per template; a block
//!   that does not parse is listed in [`ClientDrawables::errors`] and its
//!   module left out (upstream refuses the INI file).
//! - Only `W3DModelDraw` and the draw modules built on its data are made; the
//!   others (`W3DDefaultDraw`, trees, lasers, ...) are not translated yet.
//! - The static LOD is [`ClientDrawables::static_lod`] (`STATIC_GAME_LOD_HIGH`
//!   by default); upstream measures the machine.
//! - **Stealth and shroud** (GitHub #45): of `setStealthLook`, only the look and `m_hiddenByStealth` are kept; the
//!   stealth and second material pass opacities, the heat-vision pass, the shadows' `setShadowsEnabled`, and the
//!   deselection `updateHiddenStatus` asks of the in-game UI are not translated (nothing draws them yet), nor are
//!   `W3DModelDraw::setFullyObscuredByShroud`'s shadow, decal and particle-system switches. The shroud status is the
//!   World's (`World::get_shrouded_status`, a read of the partition's table); the scene's clear frame is set at the
//!   client's render step for a drawable with a render object, without the scene's view-frustum cull (every drawable
//!   counts as on screen). The render step itself ([`W3DModelDraw::render`], the animation's advance) is the scene's
//!   and runs for every drawable whose render object is not hidden, as before.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::client_fx::{ClientFx, WeaponFireFx, WorldFxHost};
use super::drawable_loco::physics_xform_facts;
use super::drawable_save::DrawableSaveFields;
use super::w3d_model_draw::{DrawEnv, DrawFacts, MotionFacts, W3DModelDraw, W3DModelDrawModuleData, W3dAssetSource, WeaponFireFxAt, MAX_TURRETS, STATIC_GAME_LOD_HIGH};
use crate::common::disabled_types::{DISABLED_EMP, DISABLED_HACKED, DISABLED_PARALYZED, DISABLED_SUBDUED, DISABLED_UNDERPOWERED, DISABLED_UNMANNED};
use crate::common::game_common::{ObjectShroudStatus, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::ObjectId;
use crate::common::kind_of::KINDOF_PRODUCED_AT_HELIPAD;
use crate::common::model_state::{ModelConditionFlags, MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE};
use crate::common::random_value::RandomValues;
use crate::common::thing_template::{ModuleInfoData, ThingTemplate};
use crate::logic::client_event::ClientEvent;
use crate::logic::modules::stealth_update::stealth_look;
use crate::logic::weapon_host::WeaponFx;
use crate::logic::object_script_status_bits::OBJECT_STATUS_SCRIPT_UNPOWERED;
use crate::logic::world::game_logic::World;
use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent, StealthClientOp};

/// `TIME_OF_DAY_NIGHT`.
pub const TIME_OF_DAY_NIGHT: i32 = 4;
/// `WEATHER_SNOWY`.
pub const WEATHER_SNOWY: i32 = 1;

/// `class Drawable`, the parts for its draw modules.
#[derive(Clone, Debug)]
pub struct Drawable {
    /// The object it is bound to.
    pub object_id: ObjectId,
    /// Its template's name.
    pub template: Vec<u8>,
    /// `m_conditionState`.
    pub(crate) condition_state: ModelConditionFlags,
    /// `m_isModelDirty`.
    pub(crate) is_model_dirty: bool,
    /// Its `W3DModelDraw` modules (`MODULETYPE_DRAW`).
    pub draw_modules: Vec<W3DModelDraw>,
    /// `m_hidden`.
    pub hidden: bool,
    /// `m_hiddenByStealth` (GitHub #45): `setStealthLook(STEALTHLOOK_INVISIBLE)`.
    pub hidden_by_stealth: bool,
    /// `m_stealthLook` (a `StealthLookType` number, `STEALTHLOOK_NONE` at first).
    pub stealth_look: i32,
    /// `m_drawableFullyObscuredByShroud` (GitHub #45).
    pub fully_obscured_by_shroud: bool,
    /// `m_shroudClearFrame`: the logic frame the scene last drew it clear of the shroud (0: never).
    pub shroud_clear_frame: u32,
    /// `m_locoInfo` (GitHub #41): made by the first physics model that runs ([`Drawable::calc_physics_xform`]).
    pub loco_info: Option<super::drawable_loco::DrawableLocoInfo>,
    /// The last draw's `applyPhysicsXform` totals, for the client to put on the drawn model (`None`: none applied).
    /// New: upstream applies them to the draw's transform matrix in place.
    pub physics_xform: Option<super::drawable_loco::PhysicsXformInfo>,
    /// The members upstream's drawable saves that the port's does not use, and the ambient sound's saved part (GitHub
    /// #59, [`super::drawable_save`]).
    pub saved: DrawableSaveFields,
}

impl Drawable {
    /// A drawable of object 1 with no draw modules, for tests elsewhere in the crate.
    #[cfg(test)]
    pub(crate) fn for_tests() -> Drawable {
        Drawable {
            object_id: 1,
            template: b"T".to_vec(),
            condition_state: ModelConditionFlags::new(),
            is_model_dirty: false,
            draw_modules: Vec::new(),
            hidden: false,
            hidden_by_stealth: false,
            stealth_look: stealth_look::NONE,
            fully_obscured_by_shroud: false,
            shroud_clear_frame: 0,
            loco_info: None,
            physics_xform: None,
            saved: DrawableSaveFields::new(1, 1.0),
        }
    }

    /// `getModelConditionFlags()`.
    pub fn get_model_condition_flags(&self) -> &ModelConditionFlags {
        &self.condition_state
    }

    /// `clearAndSetModelConditionFlags(clr, set)`: dirty if they changed.
    pub fn clear_and_set_model_condition_flags(&mut self, clr: &ModelConditionFlags, setf: &ModelConditionFlags) {
        let old_flags = self.condition_state;
        self.condition_state.clear_and_set(clr, setf);
        if self.condition_state == old_flags {
            return;
        }
        self.is_model_dirty = true;
    }

    /// `replaceModelConditionFlags(flags, forceReplace)`.
    pub fn replace_model_condition_flags<A: W3dAssetSource>(&mut self, flags: &ModelConditionFlags, force_replace: bool, env: &mut DrawEnv<'_, A>) {
        if !force_replace && self.condition_state == *flags {
            return;
        }
        self.condition_state = *flags;
        if force_replace {
            let c = self.condition_state;
            for dm in &mut self.draw_modules {
                dm.replace_model_condition_state(&c, env);
            }
            self.is_model_dirty = false;
        } else {
            self.is_model_dirty = true;
        }
    }

    /// `reactToBodyDamageStateChange(newState)`'s flags (`BodyDamageType`:
    /// pristine, damaged, really damaged, rubble).
    pub fn react_to_body_damage_state_change(&mut self, new_state: i32) {
        const THE_DAMAGE_MAP: [i32; 4] = [-1, MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE];
        let mut new_damage = ModelConditionFlags::new();
        if let Some(&b) = THE_DAMAGE_MAP.get(new_state as usize) {
            if b >= 0 {
                new_damage.set(b, 1);
            }
        }
        let clr = ModelConditionFlags::from_indices(&[MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE]);
        self.clear_and_set_model_condition_flags(&clr, &new_damage);
    }

    /// `getDrawModules()`: the modules, after giving them the flags if dirty.
    pub fn get_draw_modules<A: W3dAssetSource>(&mut self, env: &mut DrawEnv<'_, A>) -> &mut [W3DModelDraw] {
        if self.is_model_dirty {
            let c = self.condition_state;
            for dm in &mut self.draw_modules {
                dm.replace_model_condition_state(&c, env);
            }
            self.is_model_dirty = false;
        }
        &mut self.draw_modules
    }

    /// `setDrawableHidden(hidden)`: when it changed, `updateHiddenStatus()`.
    ///
    /// ~~each module's render object~~ **Corrected 2026-11-06 (GitHub #45):** only when `m_hidden` changed, and the
    /// modules are given `m_hidden || m_hiddenByStealth` ([`Drawable::update_hidden_status`]).
    pub fn set_drawable_hidden(&mut self, hidden: bool) {
        if hidden != self.hidden {
            self.hidden = hidden;
            self.update_hidden_status();
        }
    }

    /// `isDrawableEffectivelyHidden()`: `m_hidden || m_hiddenByStealth`.
    pub fn is_drawable_effectively_hidden(&self) -> bool {
        self.hidden || self.hidden_by_stealth
    }

    /// `updateHiddenStatus()`: each module's render object hidden when the drawable is effectively hidden.
    /// (`TheInGameUI->deselectDrawable(this)` is not translated: the client's selection is the logic-side model's.)
    /// Upstream walks `getDrawModules()`, which first gives dirty flags; here the modules are walked as they are, as
    /// before (no draw environment at hand), so the flags wait for the next draw.
    pub fn update_hidden_status(&mut self) {
        let hidden = self.is_drawable_effectively_hidden();
        for dm in &mut self.draw_modules {
            dm.set_hidden(hidden);
        }
    }

    /// `setStealthLook(look)` (GitHub #45), the parts the client draws by: when the look changed, `m_hiddenByStealth`
    /// (true only for `STEALTHLOOK_INVISIBLE`: every other case of upstream's switch clears it, the disguised
    /// friendly's early `break` included), the look kept, then `updateHiddenStatus()`. The opacities are not
    /// translated (see the module docs).
    pub fn set_stealth_look(&mut self, look: i32) {
        if look != self.stealth_look {
            self.hidden_by_stealth = look == stealth_look::INVISIBLE;
            self.stealth_look = look;
            self.update_hidden_status();
        }
    }

    /// `setFullyObscuredByShroud(fullyObscured)` (GitHub #45): the flag. Upstream also tells each draw module, whose
    /// `W3DModelDraw` switches its shadow, terrain decal and particle systems; none is made here yet.
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        if self.fully_obscured_by_shroud != fully_obscured {
            self.fully_obscured_by_shroud = fully_obscured;
        }
    }

    /// `getFullyObscuredByShroud()`.
    pub fn get_fully_obscured_by_shroud(&self) -> bool {
        self.fully_obscured_by_shroud
    }

    /// `showSubObject(name, show)`: each module's list.
    pub fn show_sub_object(&mut self, name: &[u8], show: bool) {
        for dm in &mut self.draw_modules {
            dm.show_sub_object(name, show);
        }
    }

    /// `updateSubObjects()`: each module's list applied.
    pub fn update_sub_objects<A: W3dAssetSource>(&mut self, env: &mut DrawEnv<'_, A>) {
        for dm in self.get_draw_modules(env) {
            dm.update_sub_objects();
        }
    }

    /// `handleWeaponFireFX(wslot, specificBarrelToUse, fxl, ...)`: the first module that handles
    /// it ends the walk (the drawable's own recoil pitch and roll, `m_locoInfo`, is not
    /// translated: no locomotor info on the client yet).
    pub fn handle_weapon_fire_fx<A: W3dAssetSource>(&mut self, wslot: usize, specific_barrel_to_use: i32, has_fxl: bool, env: &mut DrawEnv<'_, A>) -> bool {
        self.handle_weapon_fire_fx_at(wslot, specific_barrel_to_use, has_fxl, env).is_some()
    }

    /// [`Drawable::handle_weapon_fire_fx`] with where the handling module plays the FX list (GitHub #43,
    /// [`W3DModelDraw::handle_weapon_fire_fx_at`]).
    pub fn handle_weapon_fire_fx_at<A: W3dAssetSource>(&mut self, wslot: usize, specific_barrel_to_use: i32, has_fxl: bool, env: &mut DrawEnv<'_, A>) -> Option<WeaponFireFxAt> {
        for dm in self.get_draw_modules(env) {
            if let Some(at) = dm.handle_weapon_fire_fx_at(wslot, specific_barrel_to_use, has_fxl) {
                return Some(at);
            }
        }
        None
    }

    /// `setAnimationLoopDuration(numFrames)`: each module's.
    pub fn set_animation_loop_duration<A: W3dAssetSource>(&mut self, num_frames: u32, env: &mut DrawEnv<'_, A>) {
        for dm in self.get_draw_modules(env) {
            dm.set_animation_loop_duration(num_frames);
        }
    }
}

/// A flag mask from bit indices (`MAKE_MODELCONDITION_MASK*`), out-of-range
/// indices left out.
fn mask(bits: &[i32]) -> ModelConditionFlags {
    let mut f = ModelConditionFlags::new();
    for &b in bits {
        let _ = f.try_set(b, 1);
    }
    f
}

/// Every drawable the client has, by object.
#[derive(Debug)]
pub struct ClientDrawables {
    pub drawables: BTreeMap<ObjectId, Drawable>,
    /// Each template's parsed draw module data, by template name.
    module_data: BTreeMap<Vec<u8>, Arc<Vec<Arc<W3DModelDrawModuleData>>>>,
    /// The client's random generator (`theGameClientSeed`), the client's own.
    pub random: RandomValues,
    /// `TheGameLODManager->getStaticLODLevel()`.
    pub static_lod: i32,
    /// What did not parse or construct: template, module, why.
    pub errors: Vec<(Vec<u8>, Vec<u8>, String)>,
    /// `ThePlayerList->getLocalPlayer()->getPlayerIndex()` for the shroud (GitHub #45): the client's local player
    /// when it sets one, else the World's player list's (`None` in both: no shroud status is taken).
    pub local_player_index: Option<i32>,
    /// The effects (GitHub #43, #44): FX lists and particle systems, fed by the same events in the same order.
    pub fx: ClientFx,
    /// GitHub #59: the drawables a save held with no object (upstream's `newDrawable` without an object: debris and the
    /// like), in the save's order. Kept but not drawn (the port's drawables are its objects'), and written back by a save.
    pub object_less: Vec<Drawable>,
}

impl Default for ClientDrawables {
    fn default() -> Self {
        ClientDrawables {
            drawables: BTreeMap::new(),
            module_data: BTreeMap::new(),
            random: RandomValues::new(0),
            static_lod: STATIC_GAME_LOD_HIGH,
            errors: Vec::new(),
            local_player_index: None,
            fx: ClientFx::new(),
            object_less: Vec::new(),
        }
    }
}

impl ClientDrawables {
    pub fn new() -> Self {
        Self::default()
    }

    /// The parsed draw module data of a template (cached).
    pub fn module_data_for(&mut self, t: &ThingTemplate) -> Arc<Vec<Arc<W3DModelDrawModuleData>>> {
        let name = t.get_name().to_vec();
        if let Some(d) = self.module_data.get(&name) {
            return d.clone();
        }
        let mi = t.get_draw_module_info();
        let mut out = Vec::new();
        for n in mi.nuggets() {
            let ModuleInfoData::Client(block) = &n.second else { continue };
            match W3DModelDrawModuleData::parse(&n.first, block) {
                Ok(Some(mut d)) => {
                    // (the module's tag, for the save's module list: GitHub #59)
                    d.module_tag = n.module_tag.clone();
                    out.push(Arc::new(d))
                }
                Ok(None) => {}
                Err(e) => self.errors.push((name.clone(), n.first.clone(), format!("{e:?}"))),
            }
        }
        let out = Arc::new(out);
        self.module_data.insert(name, out.clone());
        out
    }

    /// `Drawable(thingTemplate)` for object `id`, if the World has it: its
    /// draw modules, each constructed (inside the logic update upstream, so
    /// at a valid time for the logic's bones).
    pub fn create_drawable<A: W3dAssetSource>(&mut self, w: &World, id: ObjectId, assets: &mut A, sync_time: u32) -> bool {
        let Some(o) = w.find_object(id) else { return false };
        let t = o.template().clone();
        let drawable = self.new_drawable(w, id, &t, assets, sync_time, true);
        self.drawables.insert(id, drawable);
        true
    }

    /// `TheThingFactory->newDrawable(thingTemplate)` bound to object `id` (`INVALID_ID`: none, then no indicator colour):
    /// its draw modules, each constructed (`logic_time`: inside the logic update, a valid time for the logic's bones).
    /// GitHub #59: a save's drawable whose template is not its object's (upstream's disguised bomb trucks), or that has no
    /// object, is made this way.
    pub fn new_drawable<A: W3dAssetSource>(&mut self, w: &World, id: ObjectId, t: &ThingTemplate, assets: &mut A, sync_time: u32, logic_time: bool) -> Drawable {
        let datas = self.module_data_for(t);
        let (use_lod, night, snowy) = w
            .statics
            .global_data
            .the_global_data()
            .map_or((false, false, false), |g| (g.use_draw_module_lod, g.time_of_day == TIME_OF_DAY_NIGHT, g.weather == WEATHER_SNOWY));
        let hex_color = if w.find_object(id).is_some() { w.get_indicator_color(id) } else { 0 };
        // `getScale()`: `m_instanceScale`, the template's asset scale
        let scale = t.get_asset_scale();
        let standard = Arc::new(w.statics.global_data.the_global_data().map_or_else(Vec::new, |g| g.standard_public_bones.clone()));
        let mut draw_modules = Vec::new();
        let mut env = DrawEnv { assets, random: &mut self.random, sync_time, logic_time };
        for d in datas.iter() {
            if use_lod && d.min_lod_required > self.static_lod {
                continue;
            }
            match W3DModelDraw::new_scaled(d.clone(), hex_color, night, snowy, scale, standard.clone(), &mut env) {
                Ok(m) => draw_modules.push(m),
                Err(e) => self.errors.push((t.get_name().to_vec(), d.module_name.clone(), format!("{e:?}"))),
            }
        }
        Drawable {
            object_id: id,
            template: t.get_name().to_vec(),
            condition_state: ModelConditionFlags::new(),
            is_model_dirty: false,
            draw_modules,
            hidden: false,
            hidden_by_stealth: false,
            stealth_look: stealth_look::NONE,
            fully_obscured_by_shroud: false,
            shroud_clear_frame: 0,
            loco_info: None,
            physics_xform: None,
            saved: DrawableSaveFields::new(id, scale),
        }
    }

    /// The drawable of `id`, made if the World has the object and the client
    /// has none yet.
    fn drawable<A: W3dAssetSource>(&mut self, w: &World, id: ObjectId, assets: &mut A, sync_time: u32) -> Option<&mut Drawable> {
        if !self.drawables.contains_key(&id) && !self.create_drawable(w, id, assets, sync_time) {
            return None;
        }
        self.drawables.get_mut(&id)
    }

    /// The logic's client events of a frame, in order: drawables made and
    /// destroyed, their flags, damage states, visibility, stealth looks (GitHub #45) and loop durations; and, since
    /// GitHub #43, each event's effects in its place ([`ClientFx::apply_event`], a weapon's fire FX through
    /// [`ClientFx::weapon_fire_fx`] once the drawable has answered `handleWeaponFireFX`). Other events are left to
    /// their consumers.
    pub fn apply_events<A: W3dAssetSource>(&mut self, w: &World, events: &[ClientEvent], assets: &mut A, sync_time: u32) {
        for e in events {
            match e {
                ClientEvent::ObjectCreated { object_id, .. } => {
                    if !self.drawables.contains_key(object_id) {
                        self.create_drawable(w, *object_id, assets, sync_time);
                    }
                }
                ClientEvent::ObjectDestroyed { object_id } => {
                    self.drawables.remove(object_id);
                }
                ClientEvent::Object(ObjectClientEvent::DrawableModelCondition { object_id, op }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        match op {
                            ModelConditionOp::Set(s) => d.clear_and_set_model_condition_flags(&ModelConditionFlags::new(), &mask(s)),
                            ModelConditionOp::Clear(c) => d.clear_and_set_model_condition_flags(&mask(c), &ModelConditionFlags::new()),
                            ModelConditionOp::ClearAndSet { clear, set } => d.clear_and_set_model_condition_flags(&mask(clear), &mask(set)),
                        }
                    }
                }
                ClientEvent::Object(ObjectClientEvent::DrawableReactToBodyDamageStateChange { object_id, state }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        d.react_to_body_damage_state_change(*state);
                    }
                }
                ClientEvent::Object(ObjectClientEvent::DrawableSetHidden { object_id, hidden }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        d.set_drawable_hidden(*hidden);
                    }
                }
                ClientEvent::Object(ObjectClientEvent::Stealth { object_id, op: StealthClientOp::StealthLook(look) }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        d.set_stealth_look(*look);
                    }
                }
                ClientEvent::Object(ObjectClientEvent::DrawableShowSubObject { object_id, name, show }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        d.show_sub_object(name, *show);
                    }
                }
                ClientEvent::Object(ObjectClientEvent::DrawableUpdateSubObjects { object_id }) => {
                    self.drawable(w, *object_id, assets, sync_time);
                    if let Some(d) = self.drawables.get_mut(object_id) {
                        let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: true };
                        d.update_sub_objects(&mut env);
                    }
                }
                ClientEvent::Object(ObjectClientEvent::WeaponFx(WeaponFx::Fire { source, weapon_slot, barrel, fx, speed, target, radius, contact_weapon, .. })) => {
                    self.drawable(w, *source, assets, sync_time);
                    let mut handled = None;
                    if let (Some(d), Ok(wslot)) = (self.drawables.get_mut(source), usize::try_from(*weapon_slot)) {
                        let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: true };
                        handled = d.handle_weapon_fire_fx_at(wslot, *barrel, fx.is_some(), &mut env);
                    }
                    // the FX list itself (GitHub #43): at the handling module's FX bone, else where
                    // `fireWeaponTemplate` plays it
                    if let Some(fx) = fx {
                        let f = WeaponFireFx {
                            source: *source,
                            fx: fx.clone(),
                            speed: *speed,
                            target: *target,
                            radius: *radius,
                            contact_weapon: *contact_weapon,
                            handled: handled.map(|h| (h.bone_transform, h.render_hidden)),
                        };
                        let host = WorldFxHost { world: w, drawables: &self.drawables, local_player_index: self.local_player_index.or(w.players.local_index()) };
                        self.fx.weapon_fire_fx(&host, &mut self.random, &f);
                    }
                }
                ClientEvent::Object(ObjectClientEvent::DrawableSetAnimationLoopDuration { object_id, frames }) => {
                    self.drawable(w, *object_id, assets, sync_time);
                    if let Some(d) = self.drawables.get_mut(object_id) {
                        let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: true };
                        d.set_animation_loop_duration(*frames, &mut env);
                    }
                }
                _ => {}
            }
            // the effects of the event, in its place among the drawables' (GitHub #43, #44)
            let host = WorldFxHost { world: w, drawables: &self.drawables, local_player_index: self.local_player_index.or(w.players.local_index()) };
            self.fx.apply_event(&host, &mut self.random, e);
        }
    }

    /// `getShouldAnimate(considerPower)` of the object's drawable.
    pub fn get_should_animate(w: &World, id: ObjectId, consider_power: bool) -> bool {
        let Some(obj) = w.find_object(id) else { return true };
        if consider_power && obj.test_script_status_bit(OBJECT_STATUS_SCRIPT_UNPOWERED) {
            return false;
        }
        if obj.is_disabled() {
            if !obj.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD)
                && (obj.is_disabled_by_type(DISABLED_HACKED)
                    || obj.is_disabled_by_type(DISABLED_PARALYZED)
                    || obj.is_disabled_by_type(DISABLED_EMP)
                    || obj.is_disabled_by_type(DISABLED_SUBDUED)
                    || obj.is_disabled_by_type(DISABLED_UNMANNED))
            {
                return false;
            }
            if consider_power && obj.is_disabled_by_type(DISABLED_UNDERPOWERED) {
                return false;
            }
        }
        true
    }

    /// `obj->getAIUpdateInterface()->getTurretRotAndPitch(tslot, &angle, &pitch)` for each
    /// turret (`handleClientTurretPositioning`'s read): `None` without an object or an AI, or
    /// when the AI has no such turret. A read of the World's turret table
    /// ([`World::ai_turret_rot_and_pitch`], `&self`: map lookups, nothing written or cached).
    pub fn turret_rot_and_pitch(w: &World, id: ObjectId) -> [Option<(f32, f32)>; MAX_TURRETS] {
        let mut out = [None; MAX_TURRETS];
        if w.find_object(id).is_some_and(|o| o.ai.is_some()) {
            for (tslot, t) in out.iter_mut().enumerate() {
                *t = w.ai_turret_rot_and_pitch(id, tslot as i32);
            }
        }
        out
    }

    /// What the subclass draw modules read of the object's physics and AI (GitHub #41), all read in
    /// place with `&World` (nothing is written, no cache filled): `getVelocityMagnitude`,
    /// `getTurning`, `isMotive` (`PhysicsBehavior::peek_*`), `getCurLocomotorSpeed`
    /// (`World::peek_cur_locomotor_speed`) and `getUnitDirectionVector2D`
    /// (`Thing::peek_unit_direction_vector_2d`); the velocity (`PhysicsBehavior::peek_velocity`), the current
    /// locomotor's `isMovingBackwards()` ([`World::ai_peek`], a flag read) and the relative angle to the AI path's cached
    /// point (`ai->getPath()->peekCachedPointOnPath`, a field read, and
    /// [`World::partition_relative_angle_2d`], `&self` arithmetic) for the trucks. `None` without an object or physics.
    pub fn motion(w: &World, id: ObjectId) -> Option<MotionFacts> {
        let o = w.find_object(id)?;
        let p = w.physics_peek(id)?;
        let dir = o.thing.peek_unit_direction_vector_2d();
        let ai = w.ai_peek(id);
        Some(MotionFacts {
            velocity_magnitude: p.peek_velocity_magnitude(),
            turning: p.peek_turning(),
            is_motive: p.peek_is_motive(w.frame()),
            cur_locomotor_speed: w.peek_cur_locomotor_speed(id),
            unit_direction_2d: (dir.x, dir.y),
            velocity: p.peek_velocity(),
            moving_backwards: ai.and_then(|a| a.get_cur_locomotor()).is_some_and(|l| l.is_moving_backwards()),
            angle_to_path_point: ai.and_then(|a| a.path.as_ref()).map(|path| w.partition_relative_angle_2d(id, &path.peek_cached_point_on_path())),
        })
    }

    /// What the draw modules of `id`'s drawable read of the World for one draw (`doDrawModule`'s reads), all with
    /// `&World`: the animation's power checks ([`ClientDrawables::get_should_animate`]), the speed
    /// (`getPhysics()->getVelocityMagnitude()`, read with [`PhysicsBehavior::peek_velocity_magnitude`]: the cached
    /// value, or the expression the cache would be filled with, so the same bits, and `m_velMag` is not written), the
    /// turrets ([`ClientDrawables::turret_rot_and_pitch`]) and the motion ([`ClientDrawables::motion`]).
    ///
    /// [`PhysicsBehavior::peek_velocity_magnitude`]: crate::logic::modules::physics_behavior::PhysicsBehavior::peek_velocity_magnitude
    pub fn draw_facts(w: &World, id: ObjectId, show_client_physics: bool, wheel_info: Option<super::drawable_loco::TWheelInfo>) -> DrawFacts {
        DrawFacts {
            should_animate_with_power: Self::get_should_animate(w, id, true),
            should_animate_without_power: Self::get_should_animate(w, id, false),
            speed: w.physics_peek(id).map(|ph| ph.peek_velocity_magnitude()),
            turrets: Self::turret_rot_and_pitch(w, id),
            motion: Self::motion(w, id),
            show_client_physics,
            wheel_info,
        }
    }

    /// `GameClient::update`'s shroud status of `id`'s drawable (GitHub #45, the release build's `if (true)` branch):
    /// the object's `getShroudedStatus(localPlayerIndex)` ([`World::get_shrouded_status`], `&self`: a table read),
    /// taken as clear while fewer than `2*LOGICFRAMES_PER_SECOND` frames (3 more when the object is effectively dead)
    /// have passed since the scene last drew it clear, then `setFullyObscuredByShroud(ss >= OBJECTSHROUD_FOGGED)`. A
    /// drawable without an object, or a World without a local player, is left as it is.
    pub fn update_shroud(d: &mut Drawable, w: &World, local_player_index: Option<i32>) {
        let (Some(object), Some(local)) = (w.find_object(d.object_id), local_player_index) else { return };
        let mut ss = w.get_shrouded_status(d.object_id, local);
        if ss >= ObjectShroudStatus::Fogged && d.shroud_clear_frame != 0 {
            let mut limit = 2 * LOGICFRAMES_PER_SECOND as u32;
            if object.is_effectively_dead() {
                // extend the time, so we can see the dead plane blow up & crash.
                limit += 3 * LOGICFRAMES_PER_SECOND as u32;
            }
            if w.frame() < limit + d.shroud_clear_frame {
                // It's been less than 2 seconds since we could see them clear, so keep showing them.
                ss = ObjectShroudStatus::Clear;
            }
        }
        d.set_fully_obscured_by_shroud(ss >= ObjectShroudStatus::Fogged);
    }

    /// One client draw at sync time `sync_time` (milliseconds): make the
    /// drawables of objects not seen yet, then for each drawable its shroud
    /// status ([`ClientDrawables::update_shroud`], `GameClient::update`'s), then
    /// `Drawable::draw`: nothing more when it is hidden, hidden by stealth or
    /// fully obscured by the shroud (GitHub #45), else its
    /// `applyPhysicsXform` (GitHub #41, [`Drawable::apply_physics_xform`]), give its
    /// modules the flags if dirty (`getDrawModules`, from the client: not a
    /// valid time for the logic's bones) and `doDrawModule`; then the scene's
    /// render step (the animation advances, unless the render object is hidden;
    /// a drawable not effectively hidden, with a render object, drawn clear of
    /// the shroud marks the frame, `setShroudClearFrame`). Last, the particle
    /// systems' update (`W3DDisplay::draw`'s `TheParticleSystemManager->update()`,
    /// once per logic frame, GitHub #43, [`ClientFx::update`]).
    ///
    /// ~~`&mut World` because the speed is the physics module's answer
    /// (`getVelocityMagnitude`, a module call); nothing is written.~~
    /// **Corrected 2026-11-06 (GitHub #45):** the module call wrote the physics
    /// module's `m_velMag` cache. The speed is now read in place
    /// ([`ClientDrawables::draw_facts`]) and the World is taken as `&World`:
    /// nothing is written.
    pub fn draw<A: W3dAssetSource>(&mut self, w: &World, assets: &mut A, sync_time: u32) {
        for id in w.objects.ids_in_order() {
            if !self.drawables.contains_key(&id) && w.find_object(id).is_some_and(|o| !o.is_destroyed()) {
                self.create_drawable(w, id, assets, sync_time);
            }
        }
        let ids: Vec<ObjectId> = self.drawables.keys().copied().collect();
        let show_client_physics = w.statics.global_data.the_global_data().is_none_or(|g| g.show_client_physics);
        let local_player_index = self.local_player_index.or(w.players.local_index());
        for id in ids {
            let d = self.drawables.get_mut(&id).expect("listed");
            Self::update_shroud(d, w, local_player_index);
            // `if (m_hidden || m_hiddenByStealth || getFullyObscuredByShroud()) return;`
            if d.is_drawable_effectively_hidden() || d.get_fully_obscured_by_shroud() {
                d.physics_xform = None;
            } else {
                // `applyPhysicsXform(&transformMtx)`, before the draw modules (GitHub #41)
                let xform = physics_xform_facts(w, id);
                d.apply_physics_xform(&xform, &mut self.random);
                let facts = Self::draw_facts(w, id, show_client_physics, d.get_wheel_info());
                let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: false };
                for dm in d.get_draw_modules(&mut env) {
                    dm.do_draw_module(&facts, &mut env);
                }
            }
            // the scene's render step
            let mut rendered = false;
            for dm in &mut d.draw_modules {
                dm.render(sync_time);
                rendered |= dm.render_object.is_some();
            }
            // `RTS3DScene::renderOneObject`: `if (ss == OBJECTSHROUD_CLEAR) draw->setShroudClearFrame(frame)`
            if rendered && !d.is_drawable_effectively_hidden() {
                if let Some(local) = local_player_index {
                    if w.find_object(id).is_some() && w.get_shrouded_status(id, local) == ObjectShroudStatus::Clear {
                        d.shroud_clear_frame = w.frame();
                    }
                }
            }
        }
        // `W3DDisplay::draw`'s `TheParticleSystemManager->update()`, once per logic frame (GitHub #43, #44)
        let host = WorldFxHost { world: w, drawables: &self.drawables, local_player_index };
        self.fx.update(&host, &mut self.random);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::w3d_model_draw::{AnimHandle, StateRef};
    use crate::common::thing_template::{ClientLine, ClientModuleData};

    struct NoAssets;
    impl W3dAssetSource for NoAssets {
        fn create_render_obj(&mut self, _: &[u8]) -> Option<bool> {
            Some(false)
        }
        fn get_hanim(&mut self, _: &[u8]) -> Option<AnimHandle> {
            None
        }
    }

    /// Predictions: setting flags the drawable already has does not dirty it; a change does, and the modules see the
    /// new flags only at the next `getDrawModules` (once); the body damage states replace one another's bits (really
    /// damaged clears damaged; pristine clears all three); the module follows: DAMAGED's state, then the default.
    #[test]
    fn flags_are_dirty_until_the_draw_modules_are_asked() {
        let line = |depth, f: &str, a: &str| ClientLine { depth, field: f.as_bytes().to_vec(), args: a.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() };
        let block = ClientModuleData {
            module_tag_name_key: Default::default(),
            lines: vec![line(0, "DefaultConditionState", ""), line(1, "Model", "A"), line(0, "ConditionState", "DAMAGED"), line(1, "Model", "B")],
        };
        let data = Arc::new(W3DModelDrawModuleData::parse(b"W3DModelDraw", &block).unwrap().unwrap());
        let mut random = RandomValues::new(0);
        let mut env = DrawEnv { assets: &mut NoAssets, random: &mut random, sync_time: 0, logic_time: true };
        let m = W3DModelDraw::new(data, 0, false, false, &mut env).unwrap();
        let mut d = Drawable { object_id: 1, template: b"T".to_vec(), condition_state: ModelConditionFlags::new(), is_model_dirty: false, draw_modules: vec![m], ..Drawable::for_tests() };
        d.clear_and_set_model_condition_flags(&ModelConditionFlags::new(), &ModelConditionFlags::new());
        assert!(!d.is_model_dirty);
        d.react_to_body_damage_state_change(1);
        assert!(d.is_model_dirty);
        assert_eq!(d.draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(0)), "not before the modules are asked");
        d.get_draw_modules(&mut env);
        assert!(!d.is_model_dirty);
        assert_eq!(d.draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(1)));
        assert_eq!(d.draw_modules[0].render_object.as_ref().map(|r| r.model_name.clone()), Some(b"b".to_vec()));
        d.react_to_body_damage_state_change(2);
        assert_eq!(*d.get_model_condition_flags(), ModelConditionFlags::from_indices(&[MODELCONDITION_REALLY_DAMAGED]));
        d.react_to_body_damage_state_change(0);
        assert!(!d.get_model_condition_flags().any());
        d.get_draw_modules(&mut env);
        assert_eq!(d.draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(0)));
        d.set_drawable_hidden(true);
        assert!(d.draw_modules[0].render_object.as_ref().unwrap().hidden);
    }

    /// Predictions: in the gate game, every object gets a drawable from its creation event (13 objects); none has a
    /// draw module (the gate's templates have no `Draw` block); an `ObjectDestroyed` removes it; nothing animates
    /// paused (no object is disabled).
    #[test]
    fn the_gate_game_s_objects_get_drawables_from_the_events() {
        let mut w = crate::logic::world::gate_scenario::gate_world();
        let mut c = ClientDrawables::new();
        let ev = w.take_client_events();
        c.apply_events(&w, &ev, &mut NoAssets, 0);
        assert_eq!(c.drawables.len(), 13);
        assert!(c.drawables.values().all(|d| d.draw_modules.is_empty()));
        let first = *c.drawables.keys().next().unwrap();
        assert!(ClientDrawables::get_should_animate(&w, first, true));
        c.apply_events(&w, &[ClientEvent::ObjectDestroyed { object_id: first }], &mut NoAssets, 0);
        assert_eq!(c.drawables.len(), 12);
        c.draw(&mut w, &mut NoAssets, 33);
        assert_eq!(c.drawables.len(), 13, "an object the client has not seen gets its drawable at the draw");
    }

    /// The drawable of the gate game's first object, with one module (no art: `NoAssets` gives a render object that is
    /// not an HLOD, so its animation counts as complete) whose MOVING state is reached through a `TransitionState`.
    fn gate_with_a_transition_module() -> (World, ClientDrawables, ObjectId) {
        let mut w = crate::logic::world::gate_scenario::gate_world();
        for _ in 0..3 {
            w.logic_update().expect("a frame");
        }
        let mut c = ClientDrawables::new();
        let ev = w.take_client_events();
        c.apply_events(&w, &ev, &mut NoAssets, 0);
        let id = *c.drawables.keys().next().unwrap();
        let line = |depth, f: &str, a: &str| ClientLine { depth, field: f.as_bytes().to_vec(), args: a.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() };
        let block = ClientModuleData {
            module_tag_name_key: Default::default(),
            lines: vec![
                line(0, "DefaultConditionState", ""),
                line(1, "Model", "T"),
                line(1, "TransitionKey", "Up"),
                line(0, "ConditionState", "MOVING"),
                line(1, "TransitionKey", "Down"),
                line(0, "TransitionState", "Up Down"),
                line(1, "AnimationMode", "ONCE"),
            ],
        };
        let data = Arc::new(W3DModelDrawModuleData::parse(b"W3DModelDraw", &block).unwrap().unwrap());
        let mut env = DrawEnv { assets: &mut NoAssets, random: &mut c.random, sync_time: 0, logic_time: true };
        let m = W3DModelDraw::new(data, 0, false, false, &mut env).unwrap();
        c.drawables.get_mut(&id).unwrap().draw_modules = vec![m];
        // (the gate's World has no local player; the client names its own)
        c.local_player_index = Some(1);
        (w, c, id)
    }

    fn moving(d: &mut Drawable, on: bool) {
        let m = ModelConditionFlags::from_indices(&[crate::common::model_state::MODELCONDITION_MOVING]);
        if on {
            d.clear_and_set_model_condition_flags(&ModelConditionFlags::new(), &m);
        } else {
            d.clear_and_set_model_condition_flags(&m, &ModelConditionFlags::new());
        }
    }

    /// GitHub #45, `Drawable::draw`'s early return for `m_hidden` and `m_hiddenByStealth`.
    ///
    /// *Predictions, written before the first run:* ~~the gate world has a local player~~ (missed: it has none; the
    /// client's own [`ClientDrawables::local_player_index`] was added and is set to 1 here). Hidden (`setDrawableHidden`),
    /// a drawable whose flags went to MOVING is not drawn: its modules are not asked (`getDrawModules` is after the
    /// return), so it stays dirty in the default state, no physics transform is kept and the scene does not mark it
    /// clear of the shroud. Shown, one draw gives the flags (the transition `up -> down`) and `doDrawModule` ends the
    /// transition at once (a non-HLOD render object's animation is complete): MOVING, and the clear frame is the logic
    /// frame (3). `setStealthLook(STEALTHLOOK_INVISIBLE)` hides the render object and stops the draw the same way (the
    /// flags back to none wait); `STEALTHLOOK_VISIBLE_DETECTED` shows it and the next draw reaches the default (no
    /// `down -> up` transition).
    ///
    /// *Results (2026-11-06):* first run, the local-player assertion failed (see above); after the client's own local
    /// player, all held.
    #[test]
    fn a_hidden_or_stealthed_drawable_is_not_drawn() {
        let (w, mut c, id) = gate_with_a_transition_module();
        let d = c.drawables.get_mut(&id).unwrap();
        moving(d, true);
        d.set_drawable_hidden(true);
        assert!(d.draw_modules[0].render_object.as_ref().unwrap().hidden);
        c.draw(&w, &mut NoAssets, 33);
        let d = &c.drawables[&id];
        assert!(d.is_model_dirty, "the modules were not asked");
        assert_eq!(d.draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(0)));
        assert!(d.physics_xform.is_none());
        assert_eq!(d.shroud_clear_frame, 0);

        c.drawables.get_mut(&id).unwrap().set_drawable_hidden(false);
        c.draw(&w, &mut NoAssets, 66);
        let d = &c.drawables[&id];
        assert!(!d.is_model_dirty);
        assert_eq!(d.draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(1)));
        assert_eq!(d.draw_modules[0].next_state_ref(), None);
        assert_eq!(d.shroud_clear_frame, w.frame());
        assert_eq!(w.frame(), 3);

        let ev = |look| ClientEvent::Object(ObjectClientEvent::Stealth { object_id: id, op: StealthClientOp::StealthLook(look) });
        c.apply_events(&w, &[ev(stealth_look::INVISIBLE)], &mut NoAssets, 66);
        let d = c.drawables.get_mut(&id).unwrap();
        assert!(d.hidden_by_stealth && !d.hidden && d.is_drawable_effectively_hidden());
        assert!(d.draw_modules[0].render_object.as_ref().unwrap().hidden);
        moving(d, false);
        c.draw(&w, &mut NoAssets, 99);
        let d = &c.drawables[&id];
        assert!(d.is_model_dirty);
        assert_eq!(d.draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(1)));

        c.apply_events(&w, &[ev(stealth_look::VISIBLE_DETECTED)], &mut NoAssets, 99);
        let d = &c.drawables[&id];
        assert!(!d.hidden_by_stealth && !d.draw_modules[0].render_object.as_ref().unwrap().hidden);
        c.draw(&w, &mut NoAssets, 132);
        assert_eq!(c.drawables[&id].draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(0)));
    }

    /// GitHub #45, `Drawable::draw`'s early return for `getFullyObscuredByShroud()`, set by `GameClient::update`.
    ///
    /// *Predictions, written before the first run:* the first object is in the partition, so the World's shroud table
    /// answers for it. Fogged for the local player with no clear frame: obscured, not drawn (flags to MOVING wait).
    /// Fogged but drawn clear one frame ago (clear frame 2, frame 3 < 2 + 60): taken as clear, drawn (MOVING reached);
    /// the scene does not mark it (the status is fogged), so the clear frame stays 2. Clear again: not obscured, and
    /// marked at frame 3.
    ///
    /// *Results (2026-11-06):* the first run stopped at the World's local player (none; see the test above); with the
    /// client's own, all held.
    #[test]
    fn a_drawable_fogged_for_the_local_player_is_not_drawn() {
        let (mut w, mut c, id) = gate_with_a_transition_module();
        let local = c.local_player_index.unwrap();
        assert!(w.find_object(id).unwrap().in_partition);
        w.cuts.shrouded_status.insert((id, local), ObjectShroudStatus::Fogged);
        moving(c.drawables.get_mut(&id).unwrap(), true);
        c.draw(&w, &mut NoAssets, 33);
        let d = &c.drawables[&id];
        assert!(d.get_fully_obscured_by_shroud());
        assert!(d.is_model_dirty);
        assert_eq!(d.draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(0)));

        c.drawables.get_mut(&id).unwrap().shroud_clear_frame = 2;
        c.draw(&w, &mut NoAssets, 66);
        let d = &c.drawables[&id];
        assert!(!d.get_fully_obscured_by_shroud(), "seen clear less than two seconds ago");
        assert_eq!(d.draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(1)));
        assert_eq!(d.shroud_clear_frame, 2);

        w.cuts.shrouded_status.insert((id, local), ObjectShroudStatus::Clear);
        c.draw(&w, &mut NoAssets, 99);
        let d = &c.drawables[&id];
        assert!(!d.get_fully_obscured_by_shroud());
        assert_eq!(d.shroud_clear_frame, 3);
    }

    /// GitHub #45: the draw reads the speed without filling the physics module's `m_velMag` cache.
    ///
    /// *Predictions, written before the first run:* after 30 frames of the gate game, a client draw leaves every
    /// physics module's state words (which hold `m_velMag`) as they were; each object's `DrawFacts::speed` has the
    /// same bits as the module call `getVelocityMagnitude()` made afterwards; and that module call does change the
    /// words of at least one module (its cache was not valid), which is the write the draw used to make.
    ///
    /// *Results (2026-11-06), first run:* all held.
    #[test]
    fn the_draw_reads_the_speed_without_writing_the_physics_cache() {
        use crate::logic::world::module_call::ModuleCall;
        let mut w = crate::logic::world::gate_scenario::gate_world();
        for _ in 0..30 {
            w.logic_update().expect("a frame");
        }
        let mut c = ClientDrawables::new();
        let ev = w.take_client_events();
        c.apply_events(&w, &ev, &mut NoAssets, 0);
        let ids: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| w.physics_peek(id).is_some()).collect();
        assert!(!ids.is_empty());
        let words = |w: &World| ids.iter().map(|&id| w.physics_peek(id).unwrap().state_words()).collect::<Vec<_>>();
        let before = words(&w);
        c.draw(&w, &mut NoAssets, 33);
        let speeds: Vec<Option<f32>> = ids.iter().map(|&id| ClientDrawables::draw_facts(&w, id, true, None).speed).collect();
        assert_eq!(words(&w), before);
        let mut changed = 0;
        for (k, &id) in ids.iter().enumerate() {
            let ph = w.find_object(id).unwrap().physics.unwrap();
            let v = w.module_call(id, ph, ModuleCall::PhysicsGetVelocityMagnitude).as_real(0.0);
            assert_eq!(speeds[k].map(f32::to_bits), Some(v.to_bits()));
            changed += usize::from(w.physics_peek(id).unwrap().state_words() != before[k]);
        }
        assert!(changed > 0, "the module call fills the cache");
    }
}
