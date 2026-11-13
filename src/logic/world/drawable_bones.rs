// SPDX-License-Identifier: GPL-3.0-only
//
// GitHub #175, translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp (Drawable::getProjectileLaunchOffset, 679-687;
//   getPristineBonePositions, 771-797; getBarrelCount, 4266-4275; getScale, 1093-1097, and the constructor's
//   m_instanceScale, 431)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp (the module's
//   getPristineBonePositionsForConditionState, getProjectileLaunchOffset and getBarrelCount, in
//   client::w3d_model_draw; the bone caches they validate, validateCachedBones and the barrel and turret caches,
//   are the client's translation, reused)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp (getSingleLogicalBonePosition,
//   getSingleLogicalBonePositionOnTurret, getMultiLogicalBonePosition, 6014-6105: logic::world::object_weapons'
//   translation, reused over the World)
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp (convertBonePosToWorldPos, 341-362:
//   logic::thing::convert_bone_pos_to_world_pos)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #175: the logic's bone and launch-offset questions answered from the art, as upstream answers them.
//!
//! Upstream's logic asks its objects' drawables where their bones are (`Object::getSingleLogicalBonePosition`,
//! `getMultiLogicalBonePosition`, `getSingleLogicalBonePositionOnTurret`; `Drawable::getPristineBonePositions`,
//! `getProjectileLaunchOffset`, `getBarrelCount`): a weapon's launch point, a transport's exits, a garrison's fire
//! points, a dock's waiting places, the effects' bones. Each drawable's `W3DModelDraw` modules answer from their
//! module data's bone caches (`ModelConditionInfo::validateStuff`): the state the **logic** believes current
//! (`findBestInfo` of the drawable's model condition flags, never the client's transition state), its model made at
//! the drawable's scale (`Drawable::getScale`, `m_instanceScale`, the template's `Scale`), posed at its first
//! animation's first frame (the last with `PRISTINE_BONE_POS_IN_FINAL_FRAME`) under `Matrix3D(true).Scale(scale)`.
//! So a pristine bone's translation is `s² * t` and its rotation part `s * R` (the hierarchy is scaled by
//! `HTreeClass::Scale`, then the root by the scaled identity): the quirk is upstream's and kept.
//!
//! Before #175 the port's logic read the tables `cuts.contain.{pristine_bones, pristine_bone_transforms,
//! single_bones, fire_points, turret_bones}`, `cuts.bone_transforms`, `cuts.drawable_barrel_count`,
//! `cuts.drawable_launch_offset` and `cuts.body.bones`, which only the differential tests fill: in a game every
//! bone question answered "the art has no such bone". With `cuts.live_bones` (on in a game, `set_all_live`) the
//! answers are upstream's, from:
//!
//! - **The art, the World's own.** [`World::load_art_bones`] reads every template's `W3DModelDraw` module data (the
//!   client's parse, `W3DModelDrawModuleData::parse`) and validates each of its states' bone caches from the art at
//!   the template's asset scale, through a [`W3dAssetSource`] the caller gives: natively the install's own files
//!   ([`crate::client::w3d_model_draw::W3dFiles`] over the game's archives, the same files the client draws); in
//!   the browser the art the page hands over by range, fetched first (the client's `import.rs`). It runs **once,
//!   before the game's first frame**, and is complete or refused: a source that answered "not yet" for any file
//!   (`deferred_answers`, GitHub #42's browser reads) leaves the World unchanged and returns the count, so the caller
//!   fetches and asks again. No logic frame ever waits for, or sees half of, the art (lockstep: every machine's logic
//!   reads the same bones). Upstream's logic likewise reads each machine's own art: games agree only between
//!   machines with the same files.
//! - **The caches, validated at load rather than on demand.** Upstream validates a state's caches the first time
//!   it is asked at a logic time, at the asking drawable's scale, and keeps them in the module data the template's
//!   drawables share. The scale is the template's for every drawable of it (`m_instanceScale = getAssetScale()`;
//!   the fuzzy scale is commented out), so validating every state at load gives the caches upstream's first asker
//!   makes. **Not modelled:** `EMPUpdate` (EMPUpdate.cpp:145) sets its own drawable's instance scale later; upstream
//!   would differ only for a state of the EMP's template first asked after that, and the EMP's drawable validates
//!   its mode's states in its constructor (`validateStuffForTimeAndWeather`), before the change. And
//!   `stateToUse == m_curState ? m_renderObject : NULL`: upstream poses the *live* render object when the state asked
//!   is the client's current one; the logic has no render object, so the state's caches are made from a fresh one
//!   (the `NULL` branch). The two differ only for a state with no animation of its own whose live render object
//!   plays one (the pose of `Peek_Animation_And_Info`), and only when that call is the state's first validation.
//! - **A missing model** answers as upstream's "no render object": `validateCachedBones` finds no model, sets the
//!   bones valid and empty; the queries find no bone, no barrel, no launch bone (zero turret positions). A World
//!   whose art was never loaded has no module data at all and gives the same answers.
//! - **The flags**: the drawable's model condition flags as the logic set them ([`World::drawable_condition`],
//!   kept by [`World::emit`] from the flag events it sends, and by the contain modules' flag calls).
//!
//! **Saves.** Upstream saves none of the caches (they are rebuilt on demand: `ModelConditionInfo`'s mutable members
//! are not in any xfer); a loaded World keeps the bones its art gave. The drawables' flags are the client's block's
//! (`CHUNK_GameClient`, `Drawable::xfer`'s `m_conditionState`); a load with the client's save
//! (`client::save_game`) restores them. A World loaded alone (no client block) starts with no flags.
//!
//! The tables stay as the differential tests' stand-ins with `live_bones` off.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::client::w3d_model_draw::{AnimHandle, DrawEnv, W3DModelDrawModuleData, W3dAssetSource};
use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;
use crate::common::model_state::ModelConditionFlags;
use crate::common::random_value::RandomValues;
use crate::common::thing_template::ModuleInfoData;
use crate::logic::modules::plant;
use crate::logic::world::game_logic::World;
use crate::ww::matrix3d::Matrix3D;

/// One template's draw modules as the logic reads their art.
#[derive(Clone, Debug)]
pub struct TemplateArt {
    /// The template's `W3DModelDraw` module data (its `Draw` modules whose data is a `W3DModelDrawModuleData`, in the
    /// template's order), each state's bone caches validated.
    pub modules: Arc<Vec<Arc<W3DModelDrawModuleData>>>,
    /// The scale the caches were made at: the template's `getAssetScale()` (`Drawable::getScale()` of its drawables).
    pub scale: f32,
}

/// The drawables' art the logic reads (see the module docs): every template's, by name.
#[derive(Clone, Debug, Default)]
pub struct ArtBones {
    pub templates: BTreeMap<Vec<u8>, TemplateArt>,
    /// `TheGlobalData->m_standardPublicBones`, as the caches were made with.
    pub standard_public_bones: Arc<Vec<Vec<u8>>>,
    /// What the load found.
    pub report: ArtBonesReport,
}

/// What [`World::load_art_bones`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArtBonesReport {
    /// Templates with at least one `W3DModelDraw` module.
    pub templates: usize,
    /// States validated.
    pub states: usize,
    /// States whose model has at least one pristine bone.
    pub states_with_bones: usize,
    /// States whose model was not found (`*** ASSET ERROR: Model %s not found!`): no render object, no bones.
    pub models_missing: usize,
    /// Draw modules whose data did not parse: (template, module, error). Upstream's INI load would have thrown.
    pub errors: Vec<(Vec<u8>, Vec<u8>, String)>,
}

/// [`World::load_art_bones`] refused: its source answered this many questions "not yet" (files still being fetched).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtBonesDeferred(pub usize);

/// No art: every model and animation not found (what a query validates with when the load did not reach a state:
/// upstream's "no render object").
pub struct NoArt;

impl W3dAssetSource for NoArt {
    fn create_render_obj(&mut self, _name: &[u8]) -> Option<bool> {
        None
    }
    fn get_hanim(&mut self, _name: &[u8]) -> Option<AnimHandle> {
        None
    }
}

/// Validates every state of `modules` (`validateStuff(NULL, scale, m_extraPublicBones)` at a logic time), as a
/// state's first `getPristineBonePositionsForConditionState` would. Returns whether a file was being fetched.
pub fn validate_template_art<A: W3dAssetSource>(modules: &[Arc<W3DModelDrawModuleData>], scale: f32, standard_public_bones: &[Vec<u8>], assets: &mut A, report: &mut ArtBonesReport) -> bool {
    let mut random = RandomValues::default();
    let mut env = DrawEnv { assets, random: &mut random, sync_time: 0, logic_time: true, particles: None };
    let scale = if plant(9740) { 1.0 } else { scale };
    let mut fetching = false;
    for d in modules {
        for s in &d.condition_states {
            let missing = !s.model_name.is_empty() && env.assets.create_render_obj(&s.model_name).is_none();
            fetching |= s.validate_stuff(None, scale, &d.extra_public_bones, standard_public_bones, &mut env);
            report.states += 1;
            report.models_missing += usize::from(missing);
            report.states_with_bones += usize::from(!s.bone_cache().pristine_bones.is_empty());
        }
    }
    fetching
}

/// A drawable as its `W3DModelDraw` modules answer the logic (`Drawable`'s part of the queries).
pub struct DrawableArt<'a> {
    /// `getDrawModules()`'s modules that are `ObjectDrawInterface`s (the `W3DModelDraw` family), in order.
    pub modules: &'a [Arc<W3DModelDrawModuleData>],
    /// `m_conditionState`.
    pub condition: ModelConditionFlags,
    /// `getScale()`.
    pub scale: f32,
    /// `TheGlobalData->m_standardPublicBones`.
    pub standard_public_bones: &'a [Vec<u8>],
    /// `getObject()->getTransformMatrix()` (`None`: no object).
    pub object_transform: Option<Matrix3D>,
    /// `getOrientation()` (the drawable's, set from its object's transform).
    pub orientation: f32,
}

impl DrawableArt<'_> {
    fn scale(&self) -> f32 {
        if plant(9740) { 1.0 } else { self.scale }
    }

    /// `Drawable::getPristineBonePositions(boneNamePrefix, startIndex, positions, transforms, maxBones)`
    /// (Drawable.cpp:771-797): each module's bones in turn (`getPristineBonePositionsForConditionState` with the
    /// drawable's flags), the arrays moved on by each module's count and `maxBones` lowered by it. `None` for a `NULL`
    /// array (a module then writes its own `tmpMtx`). A module's miss writes the object's transform where its next
    /// bone would have gone, as upstream's (the next module may overwrite it). Returns the count.
    pub fn get_pristine_bone_positions<A: W3dAssetSource>(
        &self,
        bone_name_prefix: &[u8],
        start_index: i32,
        positions: Option<&mut [Coord3D]>,
        transforms: Option<&mut [Matrix3D]>,
        max_bones: i32,
        env: &mut DrawEnv<'_, A>,
    ) -> i32 {
        let mut positions = positions;
        let mut transforms = transforms;
        let mut count = 0;
        let mut max_bones = max_bones;
        let (mut p_off, mut t_off) = (0usize, 0usize);
        for d in self.modules {
            if max_bones <= 0 {
                break;
            }
            // `const int MAX_BONE_GET = 64; Matrix3D tmpMtx[MAX_BONE_GET]` when `transforms == NULL`
            let mut tmp = [Matrix3D::IDENTITY; 64];
            let subcount = {
                let buf: &mut [Matrix3D] = match transforms.as_deref_mut() {
                    Some(t) => t.get_mut(t_off..).unwrap_or_default(),
                    None => &mut tmp,
                };
                d.get_pristine_bone_positions_for_condition_state(
                    &self.condition,
                    bone_name_prefix,
                    start_index,
                    buf,
                    max_bones,
                    self.scale(),
                    self.standard_public_bones,
                    self.object_transform.as_ref(),
                    env,
                )
            };
            // `if (positions && transforms)`: the module's positions are its transforms' translations
            if let Some(p) = positions.as_deref_mut() {
                let src: &[Matrix3D] = match transforms.as_deref() {
                    Some(t) => t.get(t_off..).unwrap_or_default(),
                    None => &tmp,
                };
                for i in 0..subcount.max(0) as usize {
                    if let (Some(slot), Some(m)) = (p.get_mut(p_off + i), src.get(i)) {
                        let v = m.get_translation();
                        *slot = Coord3D { x: v.x, y: v.y, z: v.z };
                    }
                }
            }
            if subcount > 0 {
                count += subcount;
                if positions.is_some() && !plant(9747) {
                    p_off += subcount as usize;
                }
                if transforms.is_some() && !plant(9727) {
                    t_off += subcount as usize;
                }
                if !plant(9726) {
                    max_bones -= subcount;
                }
            }
        }
        count
    }

    /// `Drawable::getProjectileLaunchOffset(wslot, specificBarrelToUse, launchPos, tur, turretRotPos, turretPitchPos)`
    /// (Drawable.cpp:679-687): the first module that answers true; every module is asked until then, each writing
    /// the turret positions (so a later module's overwrite an earlier's). `None` for a `NULL` pointer.
    #[allow(clippy::too_many_arguments)]
    pub fn get_projectile_launch_offset<A: W3dAssetSource>(
        &self,
        wslot: usize,
        specific_barrel_to_use: i32,
        launch_pos: Option<&mut Matrix3D>,
        tur: i32,
        turret_rot_pos: Option<&mut Coord3D>,
        turret_pitch_pos: Option<&mut Coord3D>,
        env: &mut DrawEnv<'_, A>,
    ) -> bool {
        let mut launch_pos = launch_pos;
        let mut turret_rot_pos = turret_rot_pos;
        let mut turret_pitch_pos = turret_pitch_pos;
        for (k, d) in self.modules.iter().enumerate() {
            // `getDrawable()->getPristineBonePositions(m_attachToDrawableBone.str(), 0, NULL, &pivot, 1) == 1`
            let mut attach = |env: &mut DrawEnv<'_, A>| -> Option<Matrix3D> {
                let mut pivot = [Matrix3D::IDENTITY];
                (self.get_pristine_bone_positions(&d.attach_to_drawable_bone, 0, None, Some(&mut pivot), 1, env) == 1).then_some(pivot[0])
            };
            let found = d.get_projectile_launch_offset(
                &self.condition,
                wslot,
                specific_barrel_to_use,
                launch_pos.as_deref_mut(),
                tur,
                turret_rot_pos.as_deref_mut(),
                turret_pitch_pos.as_deref_mut(),
                self.scale(),
                self.standard_public_bones,
                self.orientation,
                &mut attach,
                env,
            );
            if found || (plant(9733) && k == 0) {
                return found;
            }
        }
        false
    }

    /// `Drawable::getBarrelCount(wslot)` (Drawable.cpp:4266-4275): the first module's count that is not 0 (see
    /// `W3DModelDrawModuleData::get_barrel_count_for_condition_state` for the state the logic reads).
    pub fn get_barrel_count<A: W3dAssetSource>(&self, wslot: usize, env: &mut DrawEnv<'_, A>) -> i32 {
        let mut last = 0;
        for d in self.modules {
            let count = d.get_barrel_count_for_condition_state(&self.condition, wslot, self.scale(), self.standard_public_bones, env);
            if count != 0 && !plant(9734) {
                return count;
            }
            last = count;
        }
        if plant(9734) { last } else { 0 }
    }
}

/// The logic's queries' asset source and environment: no art is read at a query (the load read it; a state the load
/// did not reach is "no render object"), at a logic time.
fn with_env<R>(f: impl FnOnce(&mut DrawEnv<'_, NoArt>) -> R) -> R {
    let mut assets = NoArt;
    let mut random = RandomValues::default();
    let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: 0, logic_time: true, particles: None };
    f(&mut env)
}

impl World {
    /// GitHub #175: `GameClient::init`'s art for the logic's bones (see the module docs): every template's
    /// `W3DModelDraw` module data, each state's bone caches validated from `assets` at the template's asset scale,
    /// with the global data's standard public bones. Run once the INI is loaded and before the first frame; complete
    /// or refused: when `assets` answered any question "not yet" ([`W3dAssetSource::deferred_answers`]) the World is
    /// left unchanged and the count is returned (fetch, then ask again).
    pub fn load_art_bones<A: W3dAssetSource>(&mut self, assets: &mut A) -> Result<ArtBonesReport, ArtBonesDeferred> {
        let before = assets.deferred_answers();
        let standard = Arc::new(self.statics.global_data.the_global_data().map_or_else(Vec::new, |g| g.standard_public_bones.clone()));
        let mut report = ArtBonesReport::default();
        let mut templates = BTreeMap::new();
        let mut fetching = false;
        for t in self.statics.thing_factory.templates() {
            let t = &t.get_final_override().data;
            let mut modules = Vec::new();
            for n in t.get_draw_module_info().nuggets() {
                let ModuleInfoData::Client(block) = &n.second else { continue };
                match W3DModelDrawModuleData::parse(&n.first, block) {
                    Ok(Some(mut d)) => {
                        d.module_tag = n.module_tag.clone();
                        modules.push(Arc::new(d));
                    }
                    Ok(None) => {}
                    Err(e) => report.errors.push((t.get_name().to_vec(), n.first.clone(), format!("{e:?}"))),
                }
            }
            if modules.is_empty() {
                continue;
            }
            let scale = t.get_asset_scale();
            fetching |= validate_template_art(&modules, scale, &standard, assets, &mut report);
            report.templates += 1;
            templates.insert(t.get_name().to_vec(), TemplateArt { modules: Arc::new(modules), scale });
        }
        let deferred = assets.deferred_answers().wrapping_sub(before);
        if fetching || deferred != 0 {
            return Err(ArtBonesDeferred(deferred.max(1)));
        }
        self.art_bones = Arc::new(ArtBones { templates, standard_public_bones: standard, report: report.clone() });
        Ok(report)
    }

    /// The object's drawable as the logic reads its art (`None`: no drawable). A template the load did not find has no
    /// `W3DModelDraw` module.
    pub fn drawable_art(&self, id: ObjectId) -> Option<DrawableArt<'_>> {
        if !self.has_drawable(id) {
            return None;
        }
        let o = self.objects.find(id)?;
        let art = self.art_bones.templates.get(o.template().get_name());
        let transform = *o.thing.transform_matrix();
        Some(DrawableArt {
            modules: art.map_or(&[][..], |a| a.modules.as_slice()),
            condition: self.drawable_condition.get(&id).copied().unwrap_or_default(),
            // `getScale()`: `m_instanceScale`, the template's asset scale
            scale: art.map_or_else(|| o.template().get_asset_scale(), |a| a.scale),
            standard_public_bones: self.art_bones.standard_public_bones.as_slice(),
            object_transform: Some(transform),
            orientation: o.thing.orientation(),
        })
    }

    /// `getDrawable()->getPristineBonePositions(prefix, startIndex, positions, transforms, maxBones)` from the art: the
    /// bones found, each its position and transform (none without a drawable).
    pub fn live_pristine_bones(&self, id: ObjectId, prefix: &[u8], start_index: i32, max_bones: i32) -> Vec<(Coord3D, Matrix3D)> {
        let Some(art) = self.drawable_art(id) else { return Vec::new() };
        let n = max_bones.clamp(0, 1024) as usize;
        let mut positions = vec![Coord3D::default(); n];
        let mut transforms = vec![Matrix3D::IDENTITY; n];
        let count = with_env(|env| art.get_pristine_bone_positions(prefix, start_index, Some(&mut positions), Some(&mut transforms), max_bones, env));
        positions.into_iter().zip(transforms).take(count.max(0) as usize).collect()
    }

    /// `getDrawable()->getPristineBonePositions(prefix, startIndex, NULL, transforms, maxBones)` into the caller's
    /// array, as upstream writes it (a miss's object transform included): the count. Without a drawable, 0.
    pub fn live_pristine_bones_into(&self, id: ObjectId, prefix: &[u8], start_index: i32, transforms: &mut [Matrix3D], max_bones: i32) -> i32 {
        let Some(art) = self.drawable_art(id) else { return 0 };
        with_env(|env| art.get_pristine_bone_positions(prefix, start_index, None, Some(transforms), max_bones, env))
    }

    /// `getDrawable()->getProjectileLaunchOffset(wslot, barrel, &attach, tur, &rotPos, &pitchPos)` from the art, the
    /// three outputs given as `Weapon::calcProjectileLaunchPosition` gives them (the identity, zero, zero): whether
    /// it found the launch bone, and the three. Without a drawable: false, the three as given.
    pub fn live_projectile_launch_offset(&self, id: ObjectId, wslot: i32, barrel: i32, tur: i32) -> (bool, crate::logic::weapon_host::LaunchOffset) {
        let mut lo = crate::logic::weapon_host::LaunchOffset { attach_transform: Matrix3D::IDENTITY, turret_rot_pos: Coord3D::default(), turret_pitch_pos: Coord3D::default() };
        let Some(art) = self.drawable_art(id) else { return (false, lo) };
        let Ok(wslot) = usize::try_from(wslot) else { return (false, lo) };
        let found = with_env(|env| {
            art.get_projectile_launch_offset(wslot, barrel, Some(&mut lo.attach_transform), tur, Some(&mut lo.turret_rot_pos), Some(&mut lo.turret_pitch_pos), env)
        });
        (found, lo)
    }

    /// `getDrawable()->getProjectileLaunchOffset(PRIMARY_WEAPON, 1, NULL, tur, &turretPosition, NULL)`'s
    /// `turretPosition` from the art: `None` when no module wrote it (none has a state for the flags; upstream's
    /// variable is then uninitialised).
    pub fn live_turret_rot_pos(&self, id: ObjectId, tur: i32) -> Option<Coord3D> {
        let art = self.drawable_art(id)?;
        if !art.modules.iter().any(|d| d.find_best_info(&art.condition).is_some()) {
            return None;
        }
        let mut p = Coord3D::default();
        with_env(|env| art.get_projectile_launch_offset(crate::logic::weapon_fire::PRIMARY_WEAPON as usize, 1, None, tur, Some(&mut p), None, env));
        Some(p)
    }

    /// `getDrawable()->getBarrelCount(wslot)` from the art; `None` without a drawable.
    pub fn live_barrel_count(&self, id: ObjectId, wslot: i32) -> Option<i32> {
        let art = self.drawable_art(id)?;
        let Ok(wslot) = usize::try_from(wslot) else { return Some(0) };
        Some(with_env(|env| art.get_barrel_count(wslot, env)))
    }

    /// The flags of the drawable of `id` as the logic set them (`getDrawable()->getModelConditionFlags()`).
    pub fn drawable_condition_flags(&self, id: ObjectId) -> ModelConditionFlags {
        self.drawable_condition.get(&id).copied().unwrap_or_default()
    }

    /// Keeps [`World::drawable_condition`] as `Drawable::m_conditionState` changes for an event the World sends: the
    /// flag operations (`clearAndSetModelConditionFlags`, `setModelConditionState`, `clearModelConditionState`), the
    /// map object's time and weather (`objectTime`, `objectWeather`), the body's damage state
    /// (`Drawable::reactToBodyDamageStateChange`, Drawable.cpp:3914-3945: DAMAGED, REALLY_DAMAGED or RUBBLE) and the
    /// topple's burned stump. Only objects with a drawable.
    pub(crate) fn note_drawable_condition_event(&mut self, e: &crate::logic::world::object_events::ObjectClientEvent) {
        use crate::common::model_state::{MODELCONDITION_BURNED, MODELCONDITION_DAMAGED, MODELCONDITION_NIGHT, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE, MODELCONDITION_SNOW};
        use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
        // (the client's own reading of the indices (`client::drawable`'s `mask`): one out of range is ignored)
        let mask = |v: &[i32]| {
            let mut f = ModelConditionFlags::new();
            for &i in v {
                let _ = f.try_set(i, 1);
            }
            f
        };
        let (id, clear, set) = match e {
            ObjectClientEvent::DrawableModelCondition { object_id, op } => match op {
                ModelConditionOp::Set(s) => (*object_id, ModelConditionFlags::new(), mask(s)),
                ModelConditionOp::Clear(c) => (*object_id, mask(c), ModelConditionFlags::new()),
                ModelConditionOp::ClearAndSet { clear, set } => (*object_id, mask(clear), mask(set)),
            },
            ObjectClientEvent::DrawableTimeOrWeather { object_id, night, snow } => {
                let (mut c, mut s) = (ModelConditionFlags::new(), ModelConditionFlags::new());
                for (bit, v) in [(MODELCONDITION_NIGHT, night), (MODELCONDITION_SNOW, snow)] {
                    match v {
                        Some(true) => s.set(bit, 1),
                        Some(false) => c.set(bit, 1),
                        None => {}
                    }
                }
                (*object_id, c, s)
            }
            ObjectClientEvent::DrawableReactToBodyDamageStateChange { object_id, state } if !plant(9745) => {
                const THE_DAMAGE_MAP: [i32; 4] = [-1, MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE];
                let mut s = ModelConditionFlags::new();
                if let Some(&b) = usize::try_from(*state).ok().and_then(|i| THE_DAMAGE_MAP.get(i)) {
                    if b >= 0 {
                        s.set(b, 1);
                    }
                }
                (*object_id, mask(&[MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE]), s)
            }
            ObjectClientEvent::DrawableBurnStumpIfBurned { object_id, stump } => {
                if !self.drawable_condition_flags(*object_id).test(MODELCONDITION_BURNED) {
                    return;
                }
                (*stump, ModelConditionFlags::new(), mask(&[MODELCONDITION_BURNED]))
            }
            _ => return,
        };
        self.drawable_clear_and_set_condition(id, &clear, &set);
    }

    /// `Drawable::clearAndSetModelConditionFlags(clr, set)` on the logic's copy of the flags (nothing without a
    /// drawable).
    pub(crate) fn drawable_clear_and_set_condition(&mut self, id: ObjectId, clear: &ModelConditionFlags, set: &ModelConditionFlags) {
        if !self.has_drawable(id) {
            return;
        }
        self.drawable_condition.entry(id).or_default().clear_and_set(clear, set);
    }
}

/// The drawable, the AI and the control bar as `Object`'s bone functions (`logic::world::object_weapons`) ask them of
/// the World.
impl crate::logic::world::object_weapons::ObjectBoneHost for World {
    fn pristine_bone_positions(&self, o: ObjectId, bone_name_prefix: &[u8], start_index: i32, max_bones: i32) -> Vec<(Coord3D, Matrix3D)> {
        if self.cuts.live_bones {
            return self.live_pristine_bones(o, bone_name_prefix, start_index, max_bones);
        }
        let p = self.drawable_pristine_bones(o, bone_name_prefix, start_index, max_bones);
        let t = self.drawable_pristine_bone_transforms(o, bone_name_prefix, start_index, max_bones);
        p.into_iter().zip(t).collect()
    }
    fn convert_bone_pos_to_world_pos(&self, o: ObjectId, bone_pos: Option<&Coord3D>, bone_transform: Option<&Matrix3D>) -> (Option<Coord3D>, Option<Matrix3D>) {
        let Some(x) = self.objects.find(o) else { return (bone_pos.copied(), bone_transform.copied()) };
        let (wp, wt) = crate::logic::thing::convert_bone_pos_to_world_pos(x.thing.transform_matrix(), &bone_pos.copied().unwrap_or_default(), &bone_transform.copied().unwrap_or(Matrix3D::IDENTITY));
        (bone_pos.map(|p| if plant(9735) { *p } else { wp }), bone_transform.map(|_| wt))
    }
    fn has_ai(&self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.ai.is_some())
    }
    fn ai_turret_rot_and_pitch(&self, o: ObjectId, turret: i32) -> Option<(f32, f32)> {
        World::ai_turret_rot_and_pitch(self, o, turret)
    }
    fn command_set_upgrades(&self, name: &[u8]) -> Option<Vec<Option<u32>>> {
        let set = self.control_bar_find_command_set(name)?;
        Some(
            (0..crate::logic::command_button::MAX_COMMANDS_PER_SET)
                .map(|i| self.command_set_get_command_button(set, i).and_then(|b| self.cuts.command_buttons.get(b).and_then(|x| x.upgrade)))
                .collect(),
        )
    }
    fn command_set_string_override(&self, o: ObjectId) -> Vec<u8> {
        self.objects.find(o).map_or_else(Vec::new, |x| x.command_set_string_override.clone())
    }
    fn template_command_set_string(&self, o: ObjectId) -> Vec<u8> {
        self.objects.find(o).map_or_else(Vec::new, |x| x.template().friend_get_command_set_string().to_vec())
    }
    fn turret_rot_pos_for_bone(&mut self, o: ObjectId, which_turret: i32) -> Option<Coord3D> {
        if self.cuts.live_bones {
            return self.live_turret_rot_pos(o, which_turret);
        }
        crate::logic::weapon_host::WeaponHost::projectile_launch_offset(self, o, crate::logic::weapon_fire::PRIMARY_WEAPON, 1, which_turret).map(|l| l.turret_rot_pos)
    }
}
