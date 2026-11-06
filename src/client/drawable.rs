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
//   handleWeaponFireFX without the locomotor recoil)
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
//! - **`getShouldAnimate(considerPower)`**: no when the object is unpowered by
//!   script (power considered), or disabled as hacked, paralyzed, EMP'd,
//!   subdued or unmanned (unless it is produced at a helipad), or underpowered
//!   (power considered).
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

use std::collections::BTreeMap;
use std::sync::Arc;

use super::w3d_model_draw::{DrawEnv, DrawFacts, MotionFacts, W3DModelDraw, W3DModelDrawModuleData, W3dAssetSource, MAX_TURRETS, STATIC_GAME_LOD_HIGH};
use crate::common::disabled_types::{DISABLED_EMP, DISABLED_HACKED, DISABLED_PARALYZED, DISABLED_SUBDUED, DISABLED_UNDERPOWERED, DISABLED_UNMANNED};
use crate::common::game_type::ObjectId;
use crate::common::kind_of::KINDOF_PRODUCED_AT_HELIPAD;
use crate::common::model_state::{ModelConditionFlags, MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE};
use crate::common::random_value::RandomValues;
use crate::common::thing_template::{ModuleInfoData, ThingTemplate};
use crate::logic::client_event::ClientEvent;
use crate::logic::weapon_host::WeaponFx;
use crate::logic::object_script_status_bits::OBJECT_STATUS_SCRIPT_UNPOWERED;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};

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
    condition_state: ModelConditionFlags,
    /// `m_isModelDirty`.
    is_model_dirty: bool,
    /// Its `W3DModelDraw` modules (`MODULETYPE_DRAW`).
    pub draw_modules: Vec<W3DModelDraw>,
    /// `m_hidden`.
    pub hidden: bool,
}

impl Drawable {
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

    /// `setDrawableHidden(hidden)`: each module's render object.
    pub fn set_drawable_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        for dm in &mut self.draw_modules {
            dm.set_hidden(hidden);
        }
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
        for dm in self.get_draw_modules(env) {
            if dm.handle_weapon_fire_fx(wslot, specific_barrel_to_use, has_fxl) {
                return true;
            }
        }
        false
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
}

impl Default for ClientDrawables {
    fn default() -> Self {
        ClientDrawables { drawables: BTreeMap::new(), module_data: BTreeMap::new(), random: RandomValues::new(0), static_lod: STATIC_GAME_LOD_HIGH, errors: Vec::new() }
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
                Ok(Some(d)) => out.push(Arc::new(d)),
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
        let datas = self.module_data_for(&t);
        let (use_lod, night, snowy) = w
            .statics
            .global_data
            .the_global_data()
            .map_or((false, false, false), |g| (g.use_draw_module_lod, g.time_of_day == TIME_OF_DAY_NIGHT, g.weather == WEATHER_SNOWY));
        let hex_color = w.get_indicator_color(id);
        // `getScale()`: `m_instanceScale`, the template's asset scale
        let scale = t.get_asset_scale();
        let standard = Arc::new(w.statics.global_data.the_global_data().map_or_else(Vec::new, |g| g.standard_public_bones.clone()));
        let mut draw_modules = Vec::new();
        let mut env = DrawEnv { assets, random: &mut self.random, sync_time, logic_time: true };
        for d in datas.iter() {
            if use_lod && d.min_lod_required > self.static_lod {
                continue;
            }
            match W3DModelDraw::new_scaled(d.clone(), hex_color, night, snowy, scale, standard.clone(), &mut env) {
                Ok(m) => draw_modules.push(m),
                Err(e) => self.errors.push((t.get_name().to_vec(), d.module_name.clone(), format!("{e:?}"))),
            }
        }
        let drawable = Drawable {
            object_id: id,
            template: t.get_name().to_vec(),
            condition_state: ModelConditionFlags::new(),
            is_model_dirty: false,
            draw_modules,
            hidden: false,
        };
        self.drawables.insert(id, drawable);
        true
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
    /// destroyed, their flags, damage states, visibility and loop durations.
    /// Other events are left to their consumers.
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
                ClientEvent::Object(ObjectClientEvent::WeaponFx(WeaponFx::Fire { source, weapon_slot, barrel, fx, .. })) => {
                    self.drawable(w, *source, assets, sync_time);
                    if let (Some(d), Ok(wslot)) = (self.drawables.get_mut(source), usize::try_from(*weapon_slot)) {
                        let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: true };
                        d.handle_weapon_fire_fx(wslot, *barrel, fx.is_some(), &mut env);
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
    /// (`Thing::peek_unit_direction_vector_2d`). `None` without an object or physics.
    pub fn motion(w: &World, id: ObjectId) -> Option<MotionFacts> {
        let o = w.find_object(id)?;
        let p = w.physics_peek(id)?;
        let dir = o.thing.peek_unit_direction_vector_2d();
        Some(MotionFacts {
            velocity_magnitude: p.peek_velocity_magnitude(),
            turning: p.peek_turning(),
            is_motive: p.peek_is_motive(w.frame()),
            cur_locomotor_speed: w.peek_cur_locomotor_speed(id),
            unit_direction_2d: (dir.x, dir.y),
        })
    }

    /// One client draw at sync time `sync_time` (milliseconds): make the
    /// drawables of objects not seen yet, then for each drawable give its
    /// modules the flags if dirty (`getDrawModules`, from the client: not a
    /// valid time for the logic's bones), `doDrawModule`, and render (the
    /// animation advances).
    ///
    /// `&mut World` because the speed is the physics module's answer
    /// (`getVelocityMagnitude`, a module call); nothing is written.
    pub fn draw<A: W3dAssetSource>(&mut self, w: &mut World, assets: &mut A, sync_time: u32) {
        for id in w.objects.ids_in_order() {
            if !self.drawables.contains_key(&id) && w.find_object(id).is_some_and(|o| !o.is_destroyed()) {
                self.create_drawable(w, id, assets, sync_time);
            }
        }
        let ids: Vec<ObjectId> = self.drawables.keys().copied().collect();
        for id in ids {
            let facts = DrawFacts {
                should_animate_with_power: Self::get_should_animate(w, id, true),
                should_animate_without_power: Self::get_should_animate(w, id, false),
                speed: w.find_object(id).and_then(|o| o.physics).map(|ph| w.module_call(id, ph, ModuleCall::PhysicsGetVelocityMagnitude).as_real(0.0)),
                turrets: Self::turret_rot_and_pitch(w, id),
                motion: Self::motion(w, id),
            };
            let d = self.drawables.get_mut(&id).expect("listed");
            let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: false };
            for dm in d.get_draw_modules(&mut env) {
                dm.do_draw_module(&facts, &mut env);
                dm.render(sync_time);
            }
        }
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
        let mut d = Drawable { object_id: 1, template: b"T".to_vec(), condition_state: ModelConditionFlags::new(), is_model_dirty: false, draw_modules: vec![m], hidden: false };
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
}
