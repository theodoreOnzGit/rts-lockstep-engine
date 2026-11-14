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
//! - **Client update modules** (#162, #165): made from the template's `ClientUpdate` list in order (Drawable.cpp:473-491:
//!   a shrubbery's `SwayClientUpdate` is left out without `m_useTreeSway`), each `clientUpdate()` run in order before the
//!   drawable's other per-frame work (`updateDrawable`), `~LaserUpdate()` run when the drawable goes. The logic's calls
//!   arrive as events: `hideBeacon` (`ClientCall::HideBeacon`), `stopSway` (`DrawableStopSway`, `ToppleUpdate`) and
//!   `initLaser` (`LaserInit`, through `findClientUpdateModule`, which upstream never advances past the first module).
//! - **The instance matrix** (#179 item 3, #182): the draw's root post-multiplies the drawable's transform by it. The sway
//!   sets it on the client; the logic's updates set it as events, applied before the frame's client updates and draw, as
//!   upstream's logic sets it before `updateDrawable` and `draw`: `setInstanceMatrix` (`DrawableSetInstanceMatrix`),
//!   `StructureCollapseUpdate`'s new translation on the current matrix ([`Drawable::set_instance_translation`]) and
//!   `FloatUpdate`'s bob ([`Drawable::float_update_tilt`]).
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
//!   others (`W3DDefaultDraw`, trees, ~~lasers,~~ ...) are not translated yet. **Corrected 2026-11-12 (#168):**
//!   `W3DLaserDraw` is made too ([`Drawable::laser_draws`], kept apart from the others and written after them in a save).
//! - **The logic's object-less drawables** (#168): `TheThingFactory->newDrawable` without an object
//!   (`ParticleUplinkCannonUpdate`'s beams) arrives as `ClientNewDrawable` / `ClientDestroyDrawable`; the client keeps them by
//!   the logic's id ([`ClientDrawables::logic_drawables`]) and numbers them itself from [`LOGIC_DRAWABLE_ID_BASE`]
//!   (upstream's one `m_nextDrawableID`; the port's object-backed drawables take their objects' ids). They get
//!   `LaserDrawInit` and `LaserSetDecayFrames`, their client update modules each frame, and their laser draws. They are
//!   not written by a save (the logic keeps no record of which of its ids they are: a gap of #168).
//! - The static LOD is [`ClientDrawables::static_lod`] (`STATIC_GAME_LOD_HIGH`
//!   by default); ~~upstream measures the machine.~~ **Corrected 2026-11-12 (#160):** upstream's
//!   `TheGameLODManager->getStaticLODLevel()` (`crate::client::game_lod`): the owner copies the shell's LOD
//!   manager's level here once it has one (`client/src/shell.rs`); the manager measures nothing (the machine is
//!   unknown to it), so its level is the player's `Options.ini` choice or what an unknown machine is recommended.
//! - **Stealth and shroud** (GitHub #45): of `setStealthLook`, only the look and `m_hiddenByStealth` are kept; the
//!   stealth and second material pass opacities, the heat-vision pass, the shadows' `setShadowsEnabled`, and the
//!   deselection `updateHiddenStatus` asks of the in-game UI are not translated (nothing draws them yet), nor are
//!   `W3DModelDraw::setFullyObscuredByShroud`'s shadow, decal ~~and particle-system~~ switches (**Corrected 2026-11-12
//!   (#171):** its particle-system switch is translated, and the modules' bone particle systems are made, moved,
//!   stopped and destroyed with `TheParticleSystemManager`, `ClientFx::particles`). The shroud status is the
//!   World's (`World::get_shrouded_status`, a read of the partition's table); the scene's clear frame is set at the
//!   client's render step for a drawable with a render object, without the scene's view-frustum cull (every drawable
//!   counts as on screen). The render step itself ([`W3DModelDraw::render`], the animation's advance) is the scene's
//!   and runs for every drawable whose render object is not hidden, as before.

use std::collections::BTreeMap;
use std::sync::Arc;
use crate::logic::modules::plant;

use super::animated_particle_sys_bone_client_update::{AnimatedParticleSysBoneClientUpdate, AnimatedParticleSysBoneHost, ANIMATED_PARTICLE_SYS_BONE_CLIENT_UPDATE};
use super::beacon_client_update::{BeaconClientUpdate, BeaconClientUpdateModuleData, BeaconHost, ClientUpdateModule, TaggedClientUpdateModule, BEACON_CLIENT_UPDATE};
use super::laser_update::{LaserHost, LaserObject, LaserUpdate, LaserUpdateModuleData, LASER_UPDATE};
use super::sway_client_update::{SwayClientUpdate, SwayHost, SWAY_CLIENT_UPDATE};
use super::w3d_laser_draw::{W3DLaserDraw, W3DLaserDrawModuleData, W3D_LASER_DRAW};
use crate::common::game_type::DrawableId;
use crate::common::kind_of::KINDOF_SHRUBBERY;
use crate::logic::world::script_engine::BreezeInfo;
use crate::ww::matrix3d::Matrix3D;
use super::client_fx::{ClientFx, WeaponFireFx, WorldFxHost};
use super::particle_sys::{ParticleSystemId, ParticleSystemManager};
use super::radar::RadarEventType;
use crate::common::base_type::Coord3D;
use super::drawable_loco::physics_xform_facts;
use super::drawable_save::DrawableSaveFields;
use super::w3d_model_draw::{DrawEnv, DrawFacts, DrawParticles, MotionFacts, W3DModelDraw, W3DModelDrawModuleData, W3dAssetSource, WeaponFireFxAt, MAX_TURRETS, STATIC_GAME_LOD_HIGH};
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
    /// ~~New: upstream applies them to the draw's transform matrix in place.~~ **Corrected 2026-11-13 (#179 item 3):**
    /// they are applied to the draw's transform matrix as upstream applies them ([`Drawable::draw_transform_mtx`], the
    /// root every `doDrawModule` gets); kept here too for the renderer ([`Drawable::draw_local_matrix`]).
    pub physics_xform: Option<super::drawable_loco::PhysicsXformInfo>,
    /// The members upstream's drawable saves that the port's does not use, and the ambient sound's saved part (GitHub
    /// #59, [`super::drawable_save`]).
    pub saved: DrawableSaveFields,
    /// (#162) Its client update modules (`MODULETYPE_CLIENT_UPDATE`) that the client makes: the `BeaconClientUpdate`s
    /// ([`ClientUpdateModule`]), in the template's order. ~~The others (`SwayClientUpdate`, ...) are not made yet.~~
    /// **Corrected 2026-11-12 (#165):** all four classes are made (`SwayClientUpdate`,
    /// `AnimatedParticleSysBoneClientUpdate`, `LaserUpdate` too), each with its module tag.
    pub client_update_modules: Vec<TaggedClientUpdateModule>,
    /// (#168) Its `W3DLaserDraw` draw modules (`MODULETYPE_DRAW` too: upstream keeps them in one list with the others; here
    /// apart, after them), each the `LaserDrawInterface` that `LaserUpdate::getCurrentLaserRadius` asks for.
    pub laser_draws: Vec<W3DLaserDraw>,
}

impl Drawable {
    /// A drawable of object 1 with no draw modules, for tests elsewhere in the crate.
    #[cfg(test)]
    pub(crate) fn for_tests() -> Drawable {
        Drawable::bare(1, b"T".to_vec())
    }

    /// (#168) A drawable of object `object_id` (`INVALID_ID`: none) and template `template` with no modules, clean: what
    /// the constructor makes of a template with none (and the client crate's tests start from).
    pub fn bare(object_id: ObjectId, template: Vec<u8>) -> Drawable {
        Drawable {
            object_id,
            template,
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
            saved: DrawableSaveFields::new(object_id, 1.0),
            client_update_modules: Vec::new(),
            laser_draws: Vec::new(),
        }
    }

    /// (#168) `doDrawModule` of each `W3DLaserDraw`, given the drawable's `findClientUpdateModule("LaserUpdate")`: the first
    /// client update module when it is the laser, none without client update modules; any other first module makes
    /// upstream's lookup spin for ever (Drawable.cpp:4767-4773, the loop has no `++clientModules`): `Err`, nothing drawn.
    /// `ground` is `TheTerrainLogic->getGroundHeight(x, y)`.
    pub fn draw_laser_modules(&mut self, ground: &mut dyn FnMut(f32, f32) -> f32) -> Result<(), String> {
        if self.laser_draws.is_empty() {
            return Ok(());
        }
        let mut update = match self.client_update_modules.first_mut().map(|m| &mut m.module) {
            Some(ClientUpdateModule::Laser(l)) => Some(l),
            Some(_) => return Err("findClientUpdateModule(LaserUpdate) spins for ever: the first client update module is not the laser".into()),
            None => None,
        };
        for m in &mut self.laser_draws {
            m.do_draw_module(update.as_deref_mut(), ground);
        }
        Ok(())
    }

    /// (#168) The first draw module's `getLaserDrawInterface()->getLaserTemplateWidth()`: only `W3DLaserDraw` has the
    /// interface, so the first of those; `None` without one.
    pub fn laser_template_width(&self) -> Option<f32> {
        self.laser_draws.first().map(|m| m.get_laser_template_width())
    }

    /// (#174) `getInstanceScale()` (Drawable.h:404), which `getScale()` (Drawable.cpp:1093-1097) answers too:
    /// `m_instanceScale`, the template's asset scale at construction (Drawable.cpp:431). ~~Kept only for saves.~~ The
    /// drawable's state (`saved.instance_scale`, which `Drawable::xfer` saves); its modules keep a copy
    /// ([`W3DModelDraw::scale`]) that [`Drawable::set_instance_scale`] and a load keep equal to it.
    pub fn get_instance_scale(&self) -> f32 {
        self.saved.instance_scale
    }

    /// (#174) `setInstanceScale(value)` (Drawable.h:405; `EMPUpdate`'s shrink, EMPUpdate.cpp:145): `m_instanceScale`, and
    /// so `getScale()`: each draw module's copy follows.
    pub fn set_instance_scale(&mut self, value: f32) {
        self.saved.instance_scale = value;
        if !plant(9760) {
            for dm in &mut self.draw_modules {
                dm.scale = value;
            }
        }
    }

    /// (#179) `setInstanceMatrix(instance)` (Drawable.cpp:4281-4293): with a matrix, `m_instance` is it and not the
    /// identity; without (`NULL`), `m_instance` is made the identity and flagged so.
    pub fn set_instance_matrix(&mut self, instance: Option<&Matrix3D>) {
        match instance {
            Some(m) => {
                self.saved.instance = *m;
                self.saved.instance_is_identity = plant(9855);
            }
            None => {
                self.saved.instance.make_identity();
                self.saved.instance_is_identity = true;
            }
        }
    }

    /// (#182) The logic's [`ObjectClientEvent::DrawableSetInstanceTranslation`]. Upstream has no
    /// `Drawable::setInstanceTranslation`: the event stands for the four lines `StructureCollapseUpdate::update` writes
    /// three times (StructureCollapseUpdate.cpp:191-197, 218-224, 260-265, the shudder while waiting, the shudder and
    /// the height while collapsing, the zero when done):
    ///
    /// ```text
    /// const Matrix3D *instMatrix = building->getDrawable()->getInstanceMatrix();
    /// Matrix3D newInstMatrix;
    /// newInstMatrix = *instMatrix;
    /// newInstMatrix.Set_Translation(shudder);
    /// building->getDrawable()->setInstanceMatrix(&newInstMatrix);
    /// ```
    ///
    /// The drawable's own rotation is kept, only its translation is `shudder`; and the matrix is set through
    /// [`Drawable::set_instance_matrix`] with a matrix, so it is never flagged the identity again (not even by the
    /// final zero). Checked against those lines compiled verbatim (`tests/differential_model_draw_particles.rs`, op 14).
    pub fn set_instance_translation(&mut self, shudder: &Coord3D) {
        let mut new_inst_matrix = if plant(9871) { Matrix3D::IDENTITY } else { self.saved.instance };
        let (x, y) = if plant(9873) { (shudder.y, shudder.x) } else { (shudder.x, shudder.y) };
        let z = if plant(9874) { 0.0 } else { shudder.z };
        if plant(9872) {
            new_inst_matrix.translate(x, y, z);
        } else {
            new_inst_matrix.set_translation(&crate::ww::vector3::Vector3::new(x, y, z));
        }
        if plant(9883) && x == 0.0 && y == 0.0 && z == 0.0 {
            self.set_instance_matrix(None);
            return;
        }
        self.set_instance_matrix(Some(&new_inst_matrix));
    }

    /// (#182) The logic's [`ObjectClientEvent::DrawableFloatTilt`]: `FloatUpdate::update`'s bob of the drawable
    /// (FloatUpdate.cpp:123-139), on the frame the logic read (`TheGameLogic->getFrame()`):
    ///
    /// ```text
    /// Real angle = INT_TO_REAL(TheGameLogic->getFrame());
    /// Real yaw = sin(angle * 0.0291f) * 0.05f;
    /// Real pitch = sin(angle * 0.0515f) * 0.05f;
    /// Matrix3D mx = *draw->getInstanceMatrix();
    /// Real zRot = mx.Get_Z_Rotation();
    /// mx.Make_Identity(); mx.Rotate_Z(zRot); mx.Rotate_Y(yaw); mx.Rotate_X(pitch);
    /// draw->setInstanceMatrix(&mx);
    /// ```
    ///
    /// `sin` of a `Real` is C's double `sin` (the MSVC headers the game was built with give the global functions C
    /// semantics, differential/shim/math.h): `yaw = (Real)(sin((double)(angle * 0.0291f)) * (double)0.05f)`, petir's
    /// double sine. Checked against those lines compiled verbatim
    /// (`tests/differential_model_draw_particles.rs`, op 15).
    pub fn float_update_tilt(&mut self, frame: u32) {
        let angle = (if plant(9879) { frame.wrapping_add(1) } else { frame }) as f32;
        let (fy, fp) = if plant(9875) { (0.0515f32, 0.0291f32) } else { (0.0291f32, 0.0515f32) };
        // `sin(float)` under the MSVC headers the game was built with is C's double `sin` (differential/shim/math.h):
        // the float product widened, the double sine, times `0.05f` widened, rounded to `Real` on the assignment
        let yaw = (petir::Real::sin(f64::from(angle * fy)) * f64::from(0.05f32)) as f32;
        let pitch = (petir::Real::sin(f64::from(angle * fp)) * f64::from(0.05f32)) as f32;
        let mut mx = self.saved.instance;
        let z_rot = if plant(9876) { 0.0 } else { mx.get_z_rotation() };
        if !plant(9878) {
            mx.make_identity();
        } else {
            // (plant) the old translation kept
            let t = mx.get_translation();
            mx.make_identity();
            mx.set_translation(&t);
        }
        mx.rotate_z(z_rot);
        if plant(9877) {
            mx.rotate_x(pitch);
            mx.rotate_y(yaw);
        } else {
            mx.rotate_y(yaw);
            mx.rotate_x(pitch);
        }
        self.set_instance_matrix(Some(&mx));
    }

    /// (#179) The transform `Drawable::draw` hands every `doDrawModule` (Drawable.cpp:2662-2673) for this drawable's
    /// transform `transform` (`*getTransformMatrix()`: its object's, see [`ClientDrawables::draw`]): [`draw_transform_mtx`]
    /// with this drawable's instance matrix (`m_instance` unless `isInstanceIdentity()`) and this draw's
    /// `applyPhysicsXform` totals ([`Drawable::physics_xform`], which [`Drawable::apply_physics_xform`] set just before).
    pub fn draw_transform_mtx(&self, transform: &Matrix3D) -> Matrix3D {
        let instance = (!self.saved.instance_is_identity || plant(9854)).then_some(&self.saved.instance);
        draw_transform_mtx(transform, instance, self.physics_xform.as_ref())
    }

    /// (#179) The part of [`Drawable::draw_transform_mtx`] after the drawable's own transform, for the client's renderer
    /// (new: the Bevy app draws the object at its interpolated position and orientation and puts this after it):
    /// [`draw_transform_mtx`] of the identity, `None` when it is the identity (no instance matrix and no physics
    /// totals). Upstream has no such part: its render object takes the whole matrix ([`Drawable::draw_transform_mtx`],
    /// which the live bones follow); this is the same product to the renderer's precision, not bit for bit.
    pub fn draw_local_matrix(&self) -> Option<Matrix3D> {
        if self.saved.instance_is_identity && self.physics_xform.is_none() {
            return None;
        }
        if plant(9856) {
            return self.physics_xform.map(|x| x.local_matrix());
        }
        Some(self.draw_transform_mtx(&Matrix3D::IDENTITY))
    }

    /// (#174) Each module's `W3DModelDrawModuleData::getAttachToDrawableBoneOffset(this)` (W3DModelDraw.cpp:1148-1174),
    /// filled once (in the module data the template's drawables share) from this drawable's pristine bones:
    /// `getPristineBonePositions(m_attachToDrawableBone, 0, NULL, &boneMtx, 1)` (Drawable.cpp:771-797, the logic's
    /// translation [`crate::logic::world::drawable_bones::DrawableArt::get_pristine_bone_positions`], here over the
    /// client's modules, flags and instance scale). Upstream fills it in the first `adjustTransformMtx` (or the logic's
    /// `getProjectileLaunchOffset`); the port's client fills it when the drawable is made and before its modules draw
    /// or react (the cache is already full then: nothing is asked).
    pub fn validate_attach_to_drawable_bone_offsets<A: W3dAssetSource>(&self, object_transform: Option<Matrix3D>, orientation: f32, env: &mut DrawEnv<'_, A>) {
        if self.draw_modules.iter().all(|m| m.data.attach_to_drawable_bone.is_empty()) {
            return;
        }
        let modules: Vec<Arc<W3DModelDrawModuleData>> = self.draw_modules.iter().map(|m| m.data.clone()).collect();
        let standard = self.draw_modules.first().map(|m| m.standard_public_bones.clone()).unwrap_or_default();
        let art = crate::logic::world::drawable_bones::DrawableArt {
            modules: &modules,
            condition: self.condition_state,
            scale: self.get_instance_scale(),
            standard_public_bones: &standard,
            object_transform,
            orientation,
        };
        for m in &modules {
            m.get_attach_to_drawable_bone_offset(|| {
                let mut pivot = [Matrix3D::IDENTITY];
                (art.get_pristine_bone_positions(&m.attach_to_drawable_bone, 0, None, Some(&mut pivot), 1, env) == 1).then_some(pivot[0])
            });
        }
    }

    /// (#174) `reactToTransformChange` of each draw module (Drawable.cpp:4222-4227's walk, W3DModelDraw.cpp:3647-3659):
    /// each render object put at the drawable's transform `transform` ([`W3DModelDraw::react_to_transform_change`]).
    pub fn react_to_transform_change(&mut self, transform: &Matrix3D, construction: Option<(f32, f32)>) {
        for dm in &mut self.draw_modules {
            dm.react_to_transform_change(transform, construction);
        }
    }

    /// (#174) `getCurrentClientBonePositions(boneNamePrefix, startIndex, positions, transforms, maxBones)`
    /// (Drawable.cpp:799-822): each draw module's [`W3DModelDraw::get_current_bone_positions`] in turn while `maxBones`
    /// is above 0, the arrays moved on and `maxBones` lowered by each module's count. `None` for a `NULL` array. Returns
    /// the count. (#179) Each module's bones at `sync_time` (`WW3D::Get_Sync_Time()`), read without writing the render
    /// objects ([`W3DModelDraw::peek_current_bone_positions`]): its caller, the FX lists' `doFxAtBones`, holds the
    /// drawables only to read (upstream's query moves each animation on and validates its hierarchy: #179's gaps).
    pub fn get_current_client_bone_positions(&self, bone_name_prefix: &[u8], start_index: i32, positions: Option<&mut [Coord3D]>, transforms: Option<&mut [Matrix3D]>, max_bones: i32, sync_time: u32) -> i32 {
        let mut positions = positions;
        let mut transforms = transforms;
        let mut count = 0;
        let mut max_bones = max_bones;
        let (mut p_off, mut t_off) = (0usize, 0usize);
        for dm in &self.draw_modules {
            if max_bones <= 0 {
                break;
            }
            let p = positions.as_deref_mut().map(|p| p.get_mut(p_off..).unwrap_or_default());
            let t = transforms.as_deref_mut().map(|t| t.get_mut(t_off..).unwrap_or_default());
            let subcount = dm.peek_current_bone_positions(bone_name_prefix, start_index, p, t, max_bones, sync_time);
            if subcount > 0 {
                count += subcount;
                if positions.is_some() {
                    p_off += subcount as usize;
                }
                if transforms.is_some() {
                    t_off += subcount as usize;
                }
                max_bones -= subcount;
            }
        }
        count
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
    /// (#171) `particles`: `TheParticleSystemManager`, for the modules' bone particle systems (`None`: none found).
    pub fn set_drawable_hidden(&mut self, hidden: bool, particles: Option<&mut ParticleSystemManager>) {
        if hidden != self.hidden {
            self.hidden = hidden;
            self.update_hidden_status(particles);
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
    /// (#171) Each module's `setHidden` also stops or starts its bone particle systems (`particles`).
    pub fn update_hidden_status(&mut self, particles: Option<&mut ParticleSystemManager>) {
        let hidden = self.is_drawable_effectively_hidden();
        let mut particles = particles;
        for dm in &mut self.draw_modules {
            dm.set_hidden(hidden, particles.as_deref_mut());
        }
    }

    /// `setStealthLook(look)` (GitHub #45), the parts the client draws by: when the look changed, `m_hiddenByStealth`
    /// (true only for `STEALTHLOOK_INVISIBLE`: every other case of upstream's switch clears it, the disguised
    /// friendly's early `break` included), the look kept, then `updateHiddenStatus()`. The opacities are not
    /// translated (see the module docs).
    pub fn set_stealth_look(&mut self, look: i32, particles: Option<&mut ParticleSystemManager>) {
        if look != self.stealth_look {
            self.hidden_by_stealth = look == stealth_look::INVISIBLE;
            self.stealth_look = look;
            self.update_hidden_status(particles);
        }
    }

    /// `setFullyObscuredByShroud(fullyObscured)` (GitHub #45): the flag. Upstream also tells each draw module, whose
    /// `W3DModelDraw` switches its shadow, terrain decal and particle systems; ~~none is made here yet~~ **Corrected
    /// 2026-11-12 (#171):** each module is told first (Drawable.cpp:916-926), and switches its bone particle systems
    /// ([`W3DModelDraw::set_fully_obscured_by_shroud`]; no shadow or decal is made yet). Upstream walks
    /// `getDrawModules()`, which first gives dirty flags; here the modules are walked as they are (as
    /// [`Drawable::update_hidden_status`]).
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool, particles: Option<&mut ParticleSystemManager>) {
        if self.fully_obscured_by_shroud != fully_obscured {
            let mut particles = particles;
            for dm in &mut self.draw_modules {
                dm.set_fully_obscured_by_shroud(fully_obscured, particles.as_deref_mut());
            }
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
    pub fn handle_weapon_fire_fx<A: W3dAssetSource>(&mut self, wslot: usize, specific_barrel_to_use: i32, has_fxl: bool, has_object: bool, env: &mut DrawEnv<'_, A>) -> bool {
        // where it plays is not asked: any render object transform
        self.handle_weapon_fire_fx_at(wslot, specific_barrel_to_use, has_fxl, has_object, env).is_some()
    }

    /// [`Drawable::handle_weapon_fire_fx`] with where the handling module plays the FX list (GitHub #43,
    /// [`W3DModelDraw::handle_weapon_fire_fx_at`]); (#172) ~~`transform` is the render objects' (the drawable's transform,
    /// as the draw sets it), which the FX bone is chained from~~ **Corrected 2026-11-13 (#174):** the FX bone is chained
    /// from each render object's own transform ([`crate::client::w3d_model_draw::RenderObj::transform`]; the caller
    /// puts it at the drawable's, [`Drawable::react_to_transform_change`]).
    /// (#181 item 7b) `has_object`: whether the drawable has its object (`getObject() != NULL`).
    pub fn handle_weapon_fire_fx_at<A: W3dAssetSource>(&mut self, wslot: usize, specific_barrel_to_use: i32, has_fxl: bool, has_object: bool, env: &mut DrawEnv<'_, A>) -> Option<WeaponFireFxAt> {
        for dm in self.get_draw_modules(env) {
            if let Some(at) = dm.handle_weapon_fire_fx_at(wslot, specific_barrel_to_use, has_fxl, has_object, env.sync_time) {
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
    /// `TheGameLODManager->getStaticLODLevel()` (#160: the shell's manager's, copied by the owner; high without one).
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
    /// (#162) The radar events the client update modules asked for (`TheRadar->createEvent(pos, type, seconds)`), for
    /// the owner of the radar to make ([`ClientDrawables::take_radar_requests`]).
    pub radar_requests: Vec<(Coord3D, RadarEventType, f32)>,
    /// (#165) The objects whose drawable the client destroyed (`TheGameClient->destroyDrawable` of a `LaserUpdate` given
    /// nothing to aim at): not made again while the object lives.
    pub destroyed_by_client: std::collections::BTreeSet<ObjectId>,
    /// (#168) Each template's parsed `W3DLaserDraw` module data, by template name.
    laser_module_data: BTreeMap<Vec<u8>, Arc<Vec<Arc<W3DLaserDrawModuleData>>>>,
    /// (#168) The drawables the logic made with no object (`TheThingFactory->newDrawable`, `ClientNewDrawable`:
    /// `ParticleUplinkCannonUpdate`'s beams), by the logic's id for them. Each has a drawable id of the client's own
    /// ([`ClientDrawables::alloc_drawable_id`]).
    pub logic_drawables: BTreeMap<DrawableId, Drawable>,
    /// (#168) `GameClient::m_nextDrawableID` for the drawables the client numbers itself (the logic-made ones): from
    /// [`LOGIC_DRAWABLE_ID_BASE`], apart from the object-backed drawables, whose ids are their objects'.
    pub next_drawable_id: DrawableId,
    /// (#179) `WW3D::Get_Sync_Time()`: the sync time of the last [`ClientDrawables::draw`] or
    /// [`ClientDrawables::apply_events`] (upstream's is a global the display sets each frame), which the bone queries of
    /// the client update modules and the effects read (an animated bone is posed at it).
    pub sync_time: u32,
}

/// (#168) The first id [`ClientDrawables::alloc_drawable_id`] gives. Upstream numbers every drawable from one counter
/// (`GameClient::allocDrawableID`, GameClient.cpp:486-492, from 1); the port's object-backed drawables take their objects'
/// ids (the logic's counter, from 1), so the client's own count from 2^30: an object would need that many ids before the
/// two met, and the counter stops short of wrapping into them ([`ClientDrawables::alloc_drawable_id`]).
pub const LOGIC_DRAWABLE_ID_BASE: DrawableId = 1 << 30;

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
            radar_requests: Vec::new(),
            destroyed_by_client: std::collections::BTreeSet::new(),
            laser_module_data: BTreeMap::new(),
            logic_drawables: BTreeMap::new(),
            next_drawable_id: LOGIC_DRAWABLE_ID_BASE,
            sync_time: 0,
        }
    }
}

/// (#168) Which of the client's lists a drawable is in: an object's (by its id) or the logic's object-less ones (by the
/// logic's id).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Object(ObjectId),
    Logic(DrawableId),
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

    /// (#168) The parsed `W3DLaserDraw` module data of a template (cached), in the template's order.
    pub fn laser_module_data_for(&mut self, t: &ThingTemplate) -> Arc<Vec<Arc<W3DLaserDrawModuleData>>> {
        let name = t.get_name().to_vec();
        if let Some(d) = self.laser_module_data.get(&name) {
            return d.clone();
        }
        let mut out = Vec::new();
        for n in t.get_draw_module_info().nuggets() {
            let ModuleInfoData::Client(block) = &n.second else { continue };
            if n.first.as_slice() != W3D_LASER_DRAW {
                continue;
            }
            match W3DLaserDrawModuleData::parse(block) {
                Ok(mut d) => {
                    d.module_tag = n.module_tag.clone();
                    out.push(Arc::new(d));
                }
                Err(e) => self.errors.push((name.clone(), n.first.clone(), format!("{e:?}"))),
            }
        }
        let out = Arc::new(out);
        self.laser_module_data.insert(name, out.clone());
        out
    }

    /// (#168) `GameClient::allocDrawableID()` (GameClient.cpp:486-492) for a drawable the client numbers itself: the next
    /// id from [`LOGIC_DRAWABLE_ID_BASE`]. Upstream's counter wraps through every value; this one starts over at the base
    /// rather than wrap into the objects' ids (2^31 - 2^30 drawables later).
    pub fn alloc_drawable_id(&mut self) -> DrawableId {
        let ret = self.next_drawable_id;
        self.next_drawable_id = if ret == DrawableId::MAX { LOGIC_DRAWABLE_ID_BASE } else { ret + 1 };
        ret
    }

    /// `Drawable(thingTemplate)` for object `id`, if the World has it: its
    /// draw modules, each constructed (inside the logic update upstream, so
    /// at a valid time for the logic's bones).
    pub fn create_drawable<A: W3dAssetSource>(&mut self, w: &World, id: ObjectId, assets: &mut A, sync_time: u32) -> bool {
        if self.destroyed_by_client.contains(&id) {
            return false;
        }
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
        let laser_datas = self.laser_module_data_for(t);
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
        let mut env = DrawEnv { assets, random: &mut self.random, sync_time, logic_time, particles: Some(DrawParticles { manager: &mut self.fx.particles, frame: w.frame() }) };
        for d in datas.iter() {
            if use_lod && d.min_lod_required > self.static_lod {
                continue;
            }
            match W3DModelDraw::new_scaled(d.clone(), hex_color, night, snowy, scale, standard.clone(), &mut env) {
                Ok(mut m) => {
                    // (#171) the drawable the module's bone particle systems attach to (`getDrawable()->getID()`)
                    m.drawable_id = id;
                    draw_modules.push(m)
                }
                Err(e) => self.errors.push((t.get_name().to_vec(), d.module_name.clone(), format!("{e:?}"))),
            }
        }
        // (#168) the `W3DLaserDraw` modules, each made as its constructor does (its texture's size the assets')
        let mut laser_draws = Vec::new();
        for d in laser_datas.iter() {
            let size = if d.texture_name.is_empty() { None } else { env.assets.texture_size(&d.texture_name) };
            match W3DLaserDraw::new(d.clone(), size, sync_time) {
                Ok(m) => laser_draws.push(m),
                Err(e) => self.errors.push((t.get_name().to_vec(), W3D_LASER_DRAW.to_vec(), format!("undefined upstream: {e:?}"))),
            }
        }
        // (#162) the client update modules the client makes, each made as its constructor does; (#165) all four classes,
        // in the template's order (Drawable.cpp:473-491), a shrubbery's `SwayClientUpdate` left out without
        // `m_useTreeSway` ("this is evil, we shouldn't look at the module name directly!")
        let use_tree_sway = w.statics.global_data.the_global_data().is_none_or(|g| g.use_tree_sway);
        let mut client_update_modules = Vec::new();
        for n in t.get_client_update_module_info().nuggets() {
            let ModuleInfoData::Client(block) = &n.second else { continue };
            if t.is_kind_of(KINDOF_SHRUBBERY) && !use_tree_sway && n.first.eq_ignore_ascii_case(SWAY_CLIENT_UPDATE) {
                continue;
            }
            let made = match n.first.as_slice() {
                BEACON_CLIENT_UPDATE => BeaconClientUpdateModuleData::parse(block).map(|d| ClientUpdateModule::Beacon(BeaconClientUpdate::new(d, w.frame()))),
                SWAY_CLIENT_UPDATE => super::sway_client_update::parse_base_module_data(block).map(|()| ClientUpdateModule::Sway(SwayClientUpdate::new())),
                ANIMATED_PARTICLE_SYS_BONE_CLIENT_UPDATE => super::sway_client_update::parse_base_module_data(block).map(|()| ClientUpdateModule::AnimatedParticleSysBone(AnimatedParticleSysBoneClientUpdate::new())),
                LASER_UPDATE => LaserUpdateModuleData::parse(block).map(|d| ClientUpdateModule::Laser(LaserUpdate::new(d))),
                _ => continue,
            };
            match made {
                Ok(module) => client_update_modules.push(TaggedClientUpdateModule { module_tag: n.module_tag.clone(), module }),
                Err(e) => self.errors.push((t.get_name().to_vec(), n.first.clone(), format!("{e:?}"))),
            }
        }
        let d = Drawable {
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
            client_update_modules,
            laser_draws,
        };
        // (#174) the modules' `AttachToBoneInAnotherModule` offsets: upstream's first `adjustTransformMtx` is the
        // object's first `setTransformMatrix` (`reactToTransformChange`), right after the drawable is made, inside the
        // logic when the logic makes the object
        let object = w.find_object(id);
        d.validate_attach_to_drawable_bone_offsets(object.map(|o| *o.thing.transform_matrix()), object.map_or(0.0, |o| o.thing.orientation()), &mut env);
        d
    }

    /// (#168) `TheThingFactory->newDrawable(tmpl)` of the logic's (`ClientNewDrawable`): a drawable of no object, of the
    /// template named `template` (its final override), numbered by the client ([`ClientDrawables::alloc_drawable_id`]) and
    /// kept by the logic's id. A template the client does not know is recorded in [`ClientDrawables::errors`] (upstream's
    /// `newDrawable` would read through `NULL`).
    pub fn new_logic_drawable<A: W3dAssetSource>(&mut self, w: &World, logic_id: DrawableId, template: &[u8], assets: &mut A, sync_time: u32) {
        let Some(t) = w.statics.thing_factory.find_template(template) else {
            self.errors.push((template.to_vec(), Vec::new(), "ClientNewDrawable of a template the client does not know".into()));
            return;
        };
        let t = t.get_final_override().data.clone();
        let mut d = self.new_drawable(w, crate::common::game_type::INVALID_ID, &t, assets, sync_time, true);
        d.saved.drawable_id = self.alloc_drawable_id();
        if let Some(mut old) = self.logic_drawables.insert(logic_id, d) {
            // (the logic reuses no id while its drawable lives: not reached)
            self.destroy_client_update_modules(w, &mut old);
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
    /// destroyed, their flags, damage states, visibility, stealth looks (GitHub #45), loop durations, and (#182) instance
    /// matrices (`DrawableSetInstanceMatrix`, `DrawableSetInstanceTranslation`, `DrawableFloatTilt`); and, since
    /// GitHub #43, each event's effects in its place ([`ClientFx::apply_event`], a weapon's fire FX through
    /// [`ClientFx::weapon_fire_fx`] once the drawable has answered `handleWeaponFireFX`). Other events are left to
    /// their consumers.
    pub fn apply_events<A: W3dAssetSource>(&mut self, w: &World, events: &[ClientEvent], assets: &mut A, sync_time: u32) {
        self.sync_time = sync_time;
        for e in events {
            match e {
                ClientEvent::ObjectCreated { object_id, .. } => {
                    if !self.drawables.contains_key(object_id) {
                        self.create_drawable(w, *object_id, assets, sync_time);
                    }
                }
                ClientEvent::ObjectDestroyed { object_id } => {
                    // (#165) the drawable's modules destroyed with it: `~LaserUpdate()` destroys its systems
                    if let Some(mut d) = self.drawables.remove(object_id) {
                        self.destroy_client_update_modules(w, &mut d);
                    }
                    self.destroyed_by_client.remove(object_id);
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
                // (#174) `dr->setInstanceScale(scale)` (EMPUpdate.cpp:145)
                ClientEvent::Object(ObjectClientEvent::DrawableSetInstanceScale { object_id, scale }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        d.set_instance_scale(*scale);
                    }
                }
                // (#182) the drawable's instance matrix, which upstream's logic sets while it updates (so before this
                // frame's `updateDrawable` and draw, as here: the events are applied before `ClientDrawables::draw`
                // runs the client update modules and the draw): `setInstanceMatrix(m)` (NeutronMissileUpdate.cpp:386,
                // 411), `StructureCollapseUpdate`'s translation (StructureCollapseUpdate.cpp:191-197, 218-224,
                // 260-265) and `FloatUpdate`'s bob (FloatUpdate.cpp:123-139)
                ClientEvent::Object(ObjectClientEvent::DrawableSetInstanceMatrix { object_id, matrix }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        if !plant(9882) {
                            d.set_instance_matrix(matrix.as_ref());
                        }
                    }
                }
                ClientEvent::Object(ObjectClientEvent::DrawableSetInstanceTranslation { object_id, translation }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        if !plant(9880) {
                            d.set_instance_translation(translation);
                        }
                    }
                }
                ClientEvent::Object(ObjectClientEvent::DrawableFloatTilt { object_id, frame }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        if !plant(9881) {
                            d.float_update_tilt(*frame);
                        }
                    }
                }
                ClientEvent::Object(ObjectClientEvent::DrawableSetHidden { object_id, hidden }) => {
                    self.drawable(w, *object_id, assets, sync_time);
                    if let Some(d) = self.drawables.get_mut(object_id) {
                        d.set_drawable_hidden(*hidden, Some(&mut self.fx.particles));
                    }
                }
                ClientEvent::Object(ObjectClientEvent::Stealth { object_id, op: StealthClientOp::StealthLook(look) }) => {
                    self.drawable(w, *object_id, assets, sync_time);
                    if let Some(d) = self.drawables.get_mut(object_id) {
                        d.set_stealth_look(*look, Some(&mut self.fx.particles));
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
                        let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: true, particles: Some(DrawParticles { manager: &mut self.fx.particles, frame: w.frame() }) };
                        d.update_sub_objects(&mut env);
                    }
                }
                ClientEvent::Object(ObjectClientEvent::WeaponFx(WeaponFx::Fire { source, weapon_slot, barrel, fx, speed, target, radius, contact_weapon, .. })) => {
                    self.drawable(w, *source, assets, sync_time);
                    let mut handled = None;
                    if let (Some(d), Ok(wslot)) = (self.drawables.get_mut(source), usize::try_from(*weapon_slot)) {
                        // (#172) ~~the render object's transform as the draw sets it (`ClientDrawables::draw_facts`'s)~~
                        // **Corrected 2026-11-13 (#179 item 3):** the drawable's transform (`getTransformMatrix()`, as
                        // `reactToTransformChange` reads it; the draw's root adds the instance matrix and the physics):
                        // the object's, or without one the drawable's saved transform
                        let transform = match w.find_object(*source) {
                            Some(o) => *o.thing.transform_matrix(),
                            None => d.saved.transform,
                        };
                        let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: true, particles: Some(DrawParticles { manager: &mut self.fx.particles, frame: w.frame() }) };
                        // (#174) the render objects at the drawable's transform as `reactToTransformChange` sets them
                        // (W3DModelDraw.cpp:3647-3659: `adjustTransformMtx`, no instance scale), see its docs
                        let orientation = w.find_object(*source).map_or(0.0, |o| o.thing.orientation());
                        d.validate_attach_to_drawable_bone_offsets(w.find_object(*source).map(|o| *o.thing.transform_matrix()), orientation, &mut env);
                        d.react_to_transform_change(&transform, Self::construction(w, *source));
                        let has_object = w.find_object(*source).is_some();
                        handled = d.handle_weapon_fire_fx_at(wslot, *barrel, fx.is_some(), has_object, &mut env);
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
                        let host = WorldFxHost { world: w, drawables: &self.drawables, local_player_index: self.local_player_index.or(w.players.local_index()), sync_time: self.sync_time };
                        self.fx.weapon_fire_fx(&host, &mut self.random, &f);
                    }
                }
                // (#162) `BeaconClientUpdate::hideBeacon()` on each of the beacon's drawable's beacon modules
                ClientEvent::Dispatch(crate::logic::world::game_logic_dispatch::ClientCall::HideBeacon { obj }) => {
                    let id = *obj as ObjectId;
                    self.drawable(w, id, assets, sync_time);
                    self.client_update_modules(w, id, ClientUpdateCall::HideBeacon);
                }
                // (#165) `ToppleUpdate`: `stopSway()` on each `SwayClientUpdate` of the toppling object's drawable
                // (ToppleUpdate.cpp:170-181)
                ClientEvent::Object(ObjectClientEvent::DrawableStopSway { object_id }) => {
                    if let Some(d) = self.drawable(w, *object_id, assets, sync_time) {
                        for m in d.client_update_modules.iter_mut() {
                            if let ClientUpdateModule::Sway(s) = &mut m.module {
                                s.stop_sway();
                            }
                        }
                    }
                }
                // (#165) `LaserUpdate::initLaser(parent, target, &start, &end, bone)` on the laser object's drawable's
                // `findClientUpdateModule("LaserUpdate")` (`Weapon::fireWeaponTemplate`, `SpecialAbilityUpdate::initLaser`,
                // `AssistedTargetingUpdate`)
                ClientEvent::Object(ObjectClientEvent::LaserInit { laser, parent, target, start, end, bone }) => {
                    for id in [*laser, *parent, *target] {
                        if id != crate::common::game_type::INVALID_ID {
                            self.drawable(w, id, assets, sync_time);
                        }
                    }
                    let parent = self.laser_object(w, *parent);
                    let target = self.laser_object(w, *target);
                    self.client_update_modules(w, *laser, ClientUpdateCall::InitLaser { parent, target, start: *start, end: *end, bone: bone.clone(), size_delta_frames: 0 });
                }
                // ~~(#165) `ParticleUplinkCannonUpdate`'s beams (`LaserDrawInit`, `LaserSetDecayFrames`) are drawables the
                // logic makes with `TheThingFactory->newDrawable` and tells the client nothing of: the client has no such
                // drawable, so these are not applied (#165's gaps)~~ **Corrected 2026-11-12 (#168):** the logic tells the
                // client (`ClientNewDrawable`, `ClientDestroyDrawable`); the client makes them ([`ClientDrawables::logic_drawables`])
                // and gives their `findClientUpdateModule("LaserUpdate")` the two calls
                ClientEvent::Object(ObjectClientEvent::ClientNewDrawable { drawable, template }) => {
                    self.new_logic_drawable(w, *drawable, template, assets, sync_time);
                }
                ClientEvent::Object(ObjectClientEvent::ClientDestroyDrawable { drawable }) => {
                    if let Some(mut d) = self.logic_drawables.remove(drawable) {
                        self.destroy_client_update_modules(w, &mut d);
                    }
                }
                // `update->initLaser(NULL, NULL, &start, &end, "", growthFrames)` (ParticleUplinkCannonUpdate.cpp)
                ClientEvent::Object(ObjectClientEvent::LaserDrawInit { drawable, start, end, growth_frames }) => {
                    let call = ClientUpdateCall::InitLaser { parent: None, target: None, start: *start, end: *end, bone: Vec::new(), size_delta_frames: *growth_frames };
                    self.client_update_modules_in(w, Slot::Logic(*drawable), call);
                }
                // `update->setDecayFrames(frames)`
                ClientEvent::Object(ObjectClientEvent::LaserSetDecayFrames { drawable, frames }) => {
                    self.client_update_modules_in(w, Slot::Logic(*drawable), ClientUpdateCall::SetDecayFrames(*frames));
                }
                ClientEvent::Object(ObjectClientEvent::DrawableSetAnimationLoopDuration { object_id, frames }) => {
                    self.drawable(w, *object_id, assets, sync_time);
                    if let Some(d) = self.drawables.get_mut(object_id) {
                        let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: true, particles: Some(DrawParticles { manager: &mut self.fx.particles, frame: w.frame() }) };
                        d.set_animation_loop_duration(*frames, &mut env);
                    }
                }
                _ => {}
            }
            // the effects of the event, in its place among the drawables' (GitHub #43, #44)
            let host = WorldFxHost { world: w, drawables: &self.drawables, local_player_index: self.local_player_index.or(w.players.local_index()), sync_time: self.sync_time };
            self.fx.apply_event(&host, &mut self.random, e);
        }
    }

    /// (#165) An object as `initLaser` reads it: its id, its drawable's id (`getDrawable()->getID()`, the drawable's
    /// saved id: the object's own unless a load gave another) and its position; `None` for `INVALID_ID` or an object the
    /// World no longer has.
    fn laser_object(&self, w: &World, id: ObjectId) -> Option<LaserObject> {
        let o = w.find_object(id)?;
        Some(LaserObject { id, drawable: self.drawables.get(&id).map(|d| d.saved.drawable_id), position: o.get_position() })
    }

    /// (#165) The drawable's modules' destructors (`~LaserUpdate()`'s systems), the drawable going away.
    fn destroy_client_update_modules(&mut self, w: &World, d: &mut Drawable) {
        let mut modules = std::mem::take(&mut d.client_update_modules);
        let mut destroyed = false;
        let local_player_index = self.local_player_index.or(w.players.local_index());
        {
            let mut host = DrawableClientHost { world: w, drawable: d, others: &self.drawables, particles: &mut self.fx.particles, random: &mut self.random, local_player_index, destroyed: &mut destroyed, sync_time: self.sync_time };
            for m in &mut modules {
                if let ClientUpdateModule::Laser(l) = &mut m.module {
                    l.destroy(&mut host);
                }
            }
        }
        d.client_update_modules = modules;
    }

    /// (#162) The drawable's beacon modules: `clientUpdate()` (`update`) or `hideBeacon()` each, through a
    /// [`DrawableBeaconHost`]. **(#165)** Every client update module's call: `clientUpdate()` of each in order
    /// (`Drawable::updateDrawable`, Drawable.cpp:1176-1179), `hideBeacon()` of each beacon module, or `initLaser` of the
    /// module `findClientUpdateModule("LaserUpdate")` finds. That lookup never advances past the first module
    /// (Drawable.cpp:4767-4773: the loop has no `++clientModules`): it finds a `LaserUpdate` that is the first client
    /// update module, finds nothing without client update modules, and spins for ever otherwise (recorded in
    /// [`ClientDrawables::errors`], nothing done). A laser that destroys its drawable (`initLaser` with nothing to aim at)
    /// takes the drawable out of the client (not remade for its object: [`ClientDrawables::destroyed_by_client`]).
    fn client_update_modules(&mut self, w: &World, id: ObjectId, call: ClientUpdateCall) {
        self.client_update_modules_in(w, Slot::Object(id), call);
    }

    /// (#168) [`ClientDrawables::client_update_modules`] of a drawable in either list.
    fn client_update_modules_in(&mut self, w: &World, slot: Slot, call: ClientUpdateCall) {
        // the drawable taken out of the list while its modules run, so that they can find the others
        let taken = match slot {
            Slot::Object(id) => self.drawables.remove(&id),
            Slot::Logic(id) => self.logic_drawables.remove(&id),
        };
        let Some(mut d) = taken else { return };
        let mut modules = std::mem::take(&mut d.client_update_modules);
        let mut destroyed = false;
        let local_player_index = self.local_player_index.or(w.players.local_index());
        let laser = match &call {
            ClientUpdateCall::InitLaser { .. } | ClientUpdateCall::SetDecayFrames(_) => match modules.first().map(|m| &m.module) {
                Some(ClientUpdateModule::Laser(_)) => Some(0),
                Some(_) => {
                    self.errors.push((d.template.clone(), LASER_UPDATE.to_vec(), "findClientUpdateModule(LaserUpdate) spins for ever: the first client update module is not the laser".into()));
                    None
                }
                None => None,
            },
            _ => None,
        };
        for (i, m) in modules.iter_mut().enumerate() {
            if destroyed {
                break;
            }
            match (&call, &mut m.module) {
                (ClientUpdateCall::Update, ClientUpdateModule::Beacon(b)) => {
                    let mut host = DrawableBeaconHost { world: w, drawable: &mut d, particles: &mut self.fx.particles, random: &mut self.random, requests: &mut self.radar_requests };
                    b.client_update(&mut host);
                }
                (ClientUpdateCall::HideBeacon, ClientUpdateModule::Beacon(b)) => {
                    let mut host = DrawableBeaconHost { world: w, drawable: &mut d, particles: &mut self.fx.particles, random: &mut self.random, requests: &mut self.radar_requests };
                    b.hide_beacon(&mut host);
                }
                (ClientUpdateCall::Update, ClientUpdateModule::Sway(s)) => {
                    let mut host = DrawableClientHost { world: w, drawable: &mut d, others: &self.drawables, particles: &mut self.fx.particles, random: &mut self.random, local_player_index, destroyed: &mut destroyed, sync_time: self.sync_time };
                    if s.client_update(&mut host).is_err() {
                        // (not reached: the module's drawable is the host's)
                        self.errors.push((d.template.clone(), SWAY_CLIENT_UPDATE.to_vec(), "SwayClientUpdate without a drawable".into()));
                    }
                }
                (ClientUpdateCall::Update, ClientUpdateModule::AnimatedParticleSysBone(a)) => {
                    let mut host = DrawableClientHost { world: w, drawable: &mut d, others: &self.drawables, particles: &mut self.fx.particles, random: &mut self.random, local_player_index, destroyed: &mut destroyed, sync_time: self.sync_time };
                    a.client_update(&mut host);
                }
                (ClientUpdateCall::Update, ClientUpdateModule::Laser(l)) => {
                    let host = DrawableClientHost { world: w, drawable: &mut d, others: &self.drawables, particles: &mut self.fx.particles, random: &mut self.random, local_player_index, destroyed: &mut destroyed, sync_time: self.sync_time };
                    l.client_update(&host);
                }
                (ClientUpdateCall::SetDecayFrames(frames), ClientUpdateModule::Laser(l)) if laser == Some(i) => {
                    let host = DrawableClientHost { world: w, drawable: &mut d, others: &self.drawables, particles: &mut self.fx.particles, random: &mut self.random, local_player_index, destroyed: &mut destroyed, sync_time: self.sync_time };
                    l.set_decay_frames(*frames, &host);
                }
                (ClientUpdateCall::InitLaser { parent, target, start, end, bone, size_delta_frames }, ClientUpdateModule::Laser(l)) if laser == Some(i) => {
                    let mut host = DrawableClientHost { world: w, drawable: &mut d, others: &self.drawables, particles: &mut self.fx.particles, random: &mut self.random, local_player_index, destroyed: &mut destroyed, sync_time: self.sync_time };
                    if let Err(e) = l.init_laser(&mut host, parent.as_ref(), target.as_ref(), Some(start), Some(end), bone, *size_delta_frames) {
                        self.errors.push((d.template.clone(), LASER_UPDATE.to_vec(), format!("initLaser undefined upstream: {e:?}")));
                    }
                }
                _ => {}
            }
        }
        if destroyed {
            // `TheGameClient->destroyDrawable(getDrawable())`: the drawable and its modules go
            d.client_update_modules = modules;
            self.destroy_client_update_modules(w, &mut d);
            if let Slot::Object(id) = slot {
                self.destroyed_by_client.insert(id);
            }
            return;
        }
        d.client_update_modules = modules;
        match slot {
            Slot::Object(id) => {
                self.drawables.insert(id, d);
            }
            Slot::Logic(id) => {
                self.logic_drawables.insert(id, d);
            }
        }
    }

    /// (#165) `loadPostProcess()` of the drawable's client update modules after a load (`SwayClientUpdate`'s recomputes
    /// its sway; the others add nothing), in the drawables' order; (#168) and of its `W3DLaserDraw` modules.
    pub fn client_update_load_post_process(&mut self, w: &World) {
        let ids: Vec<ObjectId> = self.drawables.keys().copied().collect();
        let local_player_index = self.local_player_index.or(w.players.local_index());
        for id in ids {
            let Some(mut d) = self.drawables.remove(&id) else { continue };
            let mut modules = std::mem::take(&mut d.client_update_modules);
            let mut destroyed = false;
            for m in &mut modules {
                if let ClientUpdateModule::Sway(s) = &mut m.module {
                    let mut host = DrawableClientHost { world: w, drawable: &mut d, others: &self.drawables, particles: &mut self.fx.particles, random: &mut self.random, local_player_index, destroyed: &mut destroyed, sync_time: self.sync_time };
                    s.load_post_process(&mut host);
                }
            }
            d.client_update_modules = modules;
            // (#168) `W3DLaserDraw::loadPostProcess`: drawn again the first time after the load
            for m in &mut d.laser_draws {
                m.load_post_process();
            }
            self.drawables.insert(id, d);
        }
    }

    /// (#162) The radar events the client update modules asked for since the last call.
    pub fn take_radar_requests(&mut self) -> Vec<(Coord3D, RadarEventType, f32)> {
        std::mem::take(&mut self.radar_requests)
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
            // (#171) the render object's transform (`Set_Transform(mtx)`) as the port places it: the object's transform
            // matrix (the scale is the skeleton's); the identity without an object. (#179 item 3) `getTransformMatrix()`
            // only: `ClientDrawables::draw` replaces it with the draw's root (`Drawable::draw_transform_mtx`: the
            // instance matrix and `applyPhysicsXform` on it)
            transform: w.find_object(id).map_or(Matrix3D::IDENTITY, |o| *o.thing.transform_matrix()),
            drawable_transform: w.find_object(id).map_or(Matrix3D::IDENTITY, |o| *o.thing.transform_matrix()),
            construction: Self::construction(w, id),
            // (#184 item 1) the freeze the tank and truck draws test, the expression applyPhysicsXform's gates hold
            frozen: super::drawable_loco::PhysicsXformGates::of(w, id).frozen() && !plant(9936),
            has_object: w.find_object(id).is_some(),
        }
    }

    /// (#174) What `adjustTransformMtx` reads of the object (W3DModelDraw.cpp:2024-2035): `getConstructionPercent()` and
    /// `getGeometryInfo().getMaxHeightAbovePosition()`; `None` without an object.
    pub fn construction(w: &World, id: ObjectId) -> Option<(f32, f32)> {
        w.find_object(id).map(|o| (o.construction_percent, o.geometry_info.get_max_height_above_position()))
    }

    /// `GameClient::update`'s shroud status of `id`'s drawable (GitHub #45, the release build's `if (true)` branch):
    /// the object's `getShroudedStatus(localPlayerIndex)` ([`World::get_shrouded_status`], `&self`: a table read),
    /// taken as clear while fewer than `2*LOGICFRAMES_PER_SECOND` frames (3 more when the object is effectively dead)
    /// have passed since the scene last drew it clear, then `setFullyObscuredByShroud(ss >= OBJECTSHROUD_FOGGED)`. A
    /// drawable without an object, or a World without a local player, is left as it is.
    /// (#171) `particles`: the client's particle systems, which the draw modules' bone systems are stopped or started in.
    pub fn update_shroud(d: &mut Drawable, w: &World, local_player_index: Option<i32>, particles: Option<&mut ParticleSystemManager>) {
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
        d.set_fully_obscured_by_shroud(ss >= ObjectShroudStatus::Fogged, particles);
    }

    /// One client draw at sync time `sync_time` (milliseconds): make the
    /// drawables of objects not seen yet, then for each drawable its shroud
    /// status ([`ClientDrawables::update_shroud`], `GameClient::update`'s), then
    /// `Drawable::draw`: nothing more when it is hidden, hidden by stealth or
    /// fully obscured by the shroud (GitHub #45), else its
    /// `applyPhysicsXform` (GitHub #41, [`Drawable::apply_physics_xform`]), give its
    /// modules the flags if dirty (`getDrawModules`, from the client: not a
    /// valid time for the logic's bones) and `doDrawModule` (#179 item 3: with the
    /// draw's root, [`Drawable::draw_transform_mtx`]: the object's transform, the
    /// instance matrix and the physics totals); then the scene's
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
        self.sync_time = sync_time;
        for id in w.objects.ids_in_order() {
            if !self.drawables.contains_key(&id) && !self.destroyed_by_client.contains(&id) && w.find_object(id).is_some_and(|o| !o.is_destroyed()) {
                self.create_drawable(w, id, assets, sync_time);
            }
        }
        let ids: Vec<ObjectId> = self.drawables.keys().copied().collect();
        let show_client_physics = w.statics.global_data.the_global_data().is_none_or(|g| g.show_client_physics);
        let local_player_index = self.local_player_index.or(w.players.local_index());
        for id in ids {
            // (#162) `draw->updateDrawable()` (GameClient::update, after the shroud): the client update modules first
            // (#165) every client update module's `clientUpdate()` (a laser may destroy its drawable)
            self.client_update_modules(w, id, ClientUpdateCall::Update);
            let Some(d) = self.drawables.get_mut(&id) else { continue };
            Self::update_shroud(d, w, local_player_index, Some(&mut self.fx.particles));
            // `if (m_hidden || m_hiddenByStealth || getFullyObscuredByShroud()) return;`
            if d.is_drawable_effectively_hidden() || d.get_fully_obscured_by_shroud() {
                d.physics_xform = None;
            } else {
                // `applyPhysicsXform(&transformMtx)`, before the draw modules (GitHub #41)
                let xform = physics_xform_facts(w, id);
                d.apply_physics_xform(&xform, &mut self.random);
                let mut facts = Self::draw_facts(w, id, show_client_physics, d.get_wheel_info());
                // (#179 item 3) `transformMtx`: the drawable's transform, its instance matrix and this draw's physics
                // totals, in upstream's order (Drawable.cpp:2662-2673)
                if !plant(9857) {
                    facts.transform = d.draw_transform_mtx(&facts.drawable_transform);
                }
                let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: false, particles: Some(DrawParticles { manager: &mut self.fx.particles, frame: w.frame() }) };
                // (#174) the modules' `AttachToBoneInAnotherModule` offsets, which `adjustTransformMtx` reads (filled once)
                let orientation = w.find_object(id).map_or(0.0, |o| o.thing.orientation());
                d.validate_attach_to_drawable_bone_offsets(w.find_object(id).map(|o| *o.thing.transform_matrix()), orientation, &mut env);
                for dm in d.get_draw_modules(&mut env) {
                    dm.do_draw_module(&facts, &mut env);
                }
                // (#168) the `W3DLaserDraw` modules' `doDrawModule`
                if let Err(e) = d.draw_laser_modules(&mut |x, y| w.terrain.ground_height(x, y, false).0) {
                    self.errors.push((d.template.clone(), W3D_LASER_DRAW.to_vec(), e));
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
        // (#168) the logic's object-less drawables (`ParticleUplinkCannonUpdate`'s beams): their client update modules,
        // then `Drawable::draw` (no object: no shroud, no physics); their only draw modules the client makes are lasers
        let logic_ids: Vec<DrawableId> = self.logic_drawables.keys().copied().collect();
        for id in logic_ids {
            self.client_update_modules_in(w, Slot::Logic(id), ClientUpdateCall::Update);
            let Some(d) = self.logic_drawables.get_mut(&id) else { continue };
            if d.is_drawable_effectively_hidden() {
                continue;
            }
            let facts = Self::draw_facts(w, crate::common::game_type::INVALID_ID, show_client_physics, None);
            let mut env = DrawEnv { assets: &mut *assets, random: &mut self.random, sync_time, logic_time: false, particles: Some(DrawParticles { manager: &mut self.fx.particles, frame: w.frame() }) };
            for dm in d.get_draw_modules(&mut env) {
                dm.do_draw_module(&facts, &mut env);
            }
            if let Err(e) = d.draw_laser_modules(&mut |x, y| w.terrain.ground_height(x, y, false).0) {
                self.errors.push((d.template.clone(), W3D_LASER_DRAW.to_vec(), e));
            }
            for dm in &mut d.draw_modules {
                dm.render(sync_time);
            }
        }
        // `W3DDisplay::draw`'s `TheParticleSystemManager->update()`, once per logic frame (GitHub #43, #44)
        let host = WorldFxHost { world: w, drawables: &self.drawables, local_player_index, sync_time: self.sync_time };
        self.fx.update(&host, &mut self.random);
    }
}

/// (#165) What [`ClientDrawables`] asks of a drawable's client update modules.
#[derive(Clone, Debug)]
enum ClientUpdateCall {
    /// `clientUpdate()` of each.
    Update,
    /// `hideBeacon()` of each beacon module.
    HideBeacon,
    /// `initLaser(parent, target, &start, &end, bone)` of the module `findClientUpdateModule("LaserUpdate")` finds.
    InitLaser { parent: Option<LaserObject>, target: Option<LaserObject>, start: Coord3D, end: Coord3D, bone: Vec<u8>, size_delta_frames: i32 },
    /// (#168) `setDecayFrames(frames)` of the module `findClientUpdateModule("LaserUpdate")` finds.
    SetDecayFrames(u32),
}

/// (#179) `Drawable::draw`'s transform for the draw modules (Drawable.cpp:2662-2673):
///
/// ```text
/// Matrix3D transformMtx = *getTransformMatrix();
/// if (!isInstanceIdentity())
///     transformMtx.postMul(*getInstanceMatrix());   // NO_ALLOW_TEMPORARIES (matrix3d.h:79)
/// applyPhysicsXform(&transformMtx);
/// ```
///
/// `transform` is the drawable's transform, `instance` its instance matrix when it is not flagged the identity, and
/// `physics` the totals `applyPhysicsXform` applies (`None` when it returns before them: no object, `DISABLED_HELD`,
/// no `ShowClientPhysics`, or `calcPhysicsXform` false; [`Drawable::apply_physics_xform`]), put on in its order
/// ([`PhysicsXformInfo::apply`]: `Translate(0, 0, totalZ)`, `Rotate_Y(totalPitch)`, `Rotate_X(-totalRoll)`,
/// `Rotate_Z(totalYaw)`). Checked against the upstream text compiled verbatim (`tests/differential_model_draw_particles.rs`).
///
/// [`PhysicsXformInfo::apply`]: super::drawable_loco::PhysicsXformInfo::apply
pub fn draw_transform_mtx(transform: &Matrix3D, instance: Option<&Matrix3D>, physics: Option<&super::drawable_loco::PhysicsXformInfo>) -> Matrix3D {
    let mut transform_mtx = *transform;
    if plant(9870) {
        // the root before #179: the drawable's transform alone
        return transform_mtx;
    }
    if plant(9848) {
        if let Some(p) = physics {
            p.apply(&mut transform_mtx);
        }
    }
    if let Some(i) = instance {
        if !plant(9846) {
            if plant(9847) {
                transform_mtx.pre_mul(i);
            } else {
                transform_mtx.post_mul(i);
            }
        }
    }
    if let Some(p) = physics {
        if !plant(9849) && !plant(9848) {
            p.apply(&mut transform_mtx);
        }
    }
    transform_mtx
}

/// (#165) A drawable's position as the port keeps it: its object's (`Thing::getPosition`), or, without one, its saved
/// transform's translation.
fn drawable_position(w: &World, d: &Drawable) -> Coord3D {
    match w.find_object(d.object_id) {
        Some(o) => o.get_position(),
        None => {
            let t = d.saved.transform.get_translation();
            Coord3D { x: t.x, y: t.y, z: t.z }
        }
    }
}

/// (#165) The host of `SwayClientUpdate`, `AnimatedParticleSysBoneClientUpdate` and `LaserUpdate` on a drawable of the
/// client's (taken out of the list while its modules run), the other drawables (`TheGameClient->findDrawableByID`), the
/// World (read only: its objects, the script engine's breeze, the shroud, the frame), the particle systems and the client's
/// random numbers.
pub struct DrawableClientHost<'a> {
    pub world: &'a World,
    pub drawable: &'a mut Drawable,
    pub others: &'a BTreeMap<ObjectId, Drawable>,
    pub particles: &'a mut ParticleSystemManager,
    pub random: &'a mut RandomValues,
    /// `ThePlayerList->getLocalPlayer()`'s index.
    pub local_player_index: Option<i32>,
    /// Set by `TheGameClient->destroyDrawable(getDrawable())`.
    pub destroyed: &'a mut bool,
    /// (#179) `WW3D::Get_Sync_Time()` ([`ClientDrawables::sync_time`]).
    pub sync_time: u32,
}

impl DrawableClientHost<'_> {
    /// `TheGameClient->findDrawableByID(id)`: this drawable or another, by its (saved) drawable id.
    fn find_drawable(&self, id: DrawableId) -> Option<&Drawable> {
        if self.drawable.saved.drawable_id == id {
            return Some(self.drawable);
        }
        self.others.values().find(|d| d.saved.drawable_id == id)
    }
}

impl SwayHost for DrawableClientHost<'_> {
    fn breeze_info(&self) -> BreezeInfo {
        self.world.script_engine.breeze_info
    }
    fn game_client_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.random.game_client_random_value_real(lo, hi)
    }
    fn has_drawable(&self) -> bool {
        true
    }
    fn is_visible(&self) -> bool {
        // `Drawable::isVisible` (Drawable.cpp:613-623): any draw module visible
        self.drawable.draw_modules.iter().any(|m| m.is_visible())
    }
    fn instance_matrix(&self) -> Matrix3D {
        self.drawable.saved.instance
    }
    fn set_instance_matrix(&mut self, m: &Matrix3D) {
        // `setInstanceMatrix(&m)` (Drawable.cpp:4281-4293)
        self.drawable.set_instance_matrix(Some(m));
    }
    fn object_burned(&self) -> bool {
        self.world.find_object(self.drawable.object_id).is_some_and(|o| o.test_status(super::sway_client_update::BURNED))
    }
}

impl AnimatedParticleSysBoneHost for DrawableClientHost<'_> {
    fn has_drawable(&self) -> bool {
        true
    }
    fn draw_module_count(&self) -> usize {
        self.drawable.draw_modules.len()
    }
    fn has_object_draw_interface(&self, _: usize) -> bool {
        // every draw module the client makes is a `W3DModelDraw`, an `ObjectDrawInterface`
        true
    }
    fn update_bones_for_client_particle_systems(&mut self, i: usize) -> bool {
        // (#171) ~~the render object's transform as the draw sets it (`ClientDrawables::draw_facts`'s)~~ **Corrected
        // 2026-11-13 (#179 item 3):** the drawable's transform (`getTransformMatrix()`, as `reactToTransformChange` reads
        // it; the draw's root adds the instance matrix and the physics): the object's, or without one the drawable's
        // saved transform
        let transform = match self.world.find_object(self.drawable.object_id) {
            Some(o) => *o.thing.transform_matrix(),
            None => self.drawable.saved.transform,
        };
        // (#174) the render object's own transform: put at the drawable's as `reactToTransformChange` puts it (see
        // `W3DModelDraw::react_to_transform_change`'s docs)
        let construction = ClientDrawables::construction(self.world, self.drawable.object_id);
        let dm = &mut self.drawable.draw_modules[i];
        dm.react_to_transform_change(&transform, construction);
        dm.update_bones_for_client_particle_systems(Some(&mut *self.particles), self.sync_time)
    }
}

impl LaserHost for DrawableClientHost<'_> {
    fn logic_frame(&self) -> u32 {
        self.world.frame()
    }
    fn has_drawable(&self) -> bool {
        true
    }
    fn drawable_position(&self, id: DrawableId) -> Option<Coord3D> {
        self.find_drawable(id).map(|d| drawable_position(self.world, d))
    }
    fn drawable_bone_transform(&self, id: DrawableId, bone: &[u8]) -> Option<Matrix3D> {
        // `Drawable::getCurrentWorldspaceClientBonePositions` (Drawable.cpp:829-838): the first draw module that finds it
        let d = self.find_drawable(id)?;
        let world = match self.world.find_object(d.object_id) {
            Some(o) => *o.thing.transform_matrix(),
            None => d.saved.transform,
        };
        // (#174) each render object's transform as `reactToTransformChange` would put it (nothing written: another drawable)
        let construction = ClientDrawables::construction(self.world, d.object_id);
        // (#179) at the sync time, read without writing (#179's gaps)
        d.draw_modules.iter().find_map(|m| m.peek_current_worldspace_client_bone_positions(bone, &m.reacted_transform(&world, construction), self.sync_time))
    }
    fn drawable_object_effectively_dead(&self, id: DrawableId) -> bool {
        self.find_drawable(id).and_then(|d| self.world.find_object(d.object_id)).is_some_and(|o| o.is_effectively_dead())
    }
    fn destroy_own_drawable(&mut self) {
        *self.destroyed = true;
    }
    fn local_player_index(&self) -> Option<i32> {
        self.local_player_index
    }
    fn shrouded_status(&self, obj: &LaserObject, player_index: i32) -> ObjectShroudStatus {
        self.world.get_shrouded_status(obj.id, player_index)
    }
    fn find_template(&self, name: &[u8]) -> bool {
        self.particles.find_template(name).is_some()
    }
    fn create_particle_system(&mut self, name: &[u8]) -> Option<ParticleSystemId> {
        self.particles.create_particle_system(name, true, self.random, self.world.frame())
    }
    fn set_particle_system_position(&mut self, id: ParticleSystemId, pos: &Coord3D) {
        if let Some(s) = self.particles.find_particle_system_mut(id) {
            s.set_position(pos);
        }
    }
    fn destroy_particle_system_by_id(&mut self, id: ParticleSystemId) {
        self.particles.destroy_particle_system_by_id(id);
    }
    fn set_drawable_position(&mut self, pos: &Coord3D) {
        // `Drawable::setPosition` (`Thing::setPosition`: the transform's translation); a drawable with an object is
        // drawn at its object's position here, so this is what a save of it holds
        self.drawable.saved.transform.set_translation(&crate::ww::vector3::Vector3 { x: pos.x, y: pos.y, z: pos.z });
    }
    fn laser_template_width(&self) -> Option<f32> {
        // `getLaserDrawInterface()`: only `W3DLaserDraw` has one; ~~and the client does not make it (#165's gaps)~~
        // **Corrected 2026-11-12 (#168):** the client makes it (`Drawable::laser_draws`)
        self.drawable.laser_template_width()
    }
}

/// (#162) [`BeaconHost`] on a drawable of the client's, its object in the World, the client's particle systems and the
/// radar's requests.
pub struct DrawableBeaconHost<'a> {
    pub world: &'a World,
    pub drawable: &'a mut Drawable,
    pub particles: &'a mut ParticleSystemManager,
    pub random: &'a mut RandomValues,
    pub requests: &'a mut Vec<(Coord3D, RadarEventType, f32)>,
}

impl BeaconHost for DrawableBeaconHost<'_> {
    fn has_drawable(&self) -> bool {
        true
    }
    fn object_indicator_color(&self) -> Option<i32> {
        let id = self.drawable.object_id;
        self.world.find_object(id).map(|_| self.world.get_indicator_color(id))
    }
    fn find_template(&self, name: &[u8]) -> bool {
        self.particles.find_template(name).is_some()
    }
    fn create_attached_particle_system(&mut self, name: &[u8]) -> Option<ParticleSystemId> {
        // `createParticleSystem(template)` (slaves made, the default) at the logic's frame, then `attachToDrawable(draw)`
        let id = self.particles.create_particle_system(name, true, self.random, self.world.frame())?;
        if let Some(s) = self.particles.find_particle_system_mut(id) {
            s.attach_to_drawable(self.drawable.object_id);
        }
        Some(id)
    }
    fn tint_all_colors(&mut self, id: ParticleSystemId, color: i32) {
        if let Some(s) = self.particles.find_particle_system_mut(id) {
            s.info.tint_all_colors(color);
        }
    }
    fn stop_particle_system(&mut self, id: ParticleSystemId) {
        if let Some(s) = self.particles.find_particle_system_mut(id) {
            s.stop();
        }
    }
    fn hide_drawable(&mut self) {
        // `setDrawableHidden(true)`; `setShadowsEnabled(false)` has nothing to turn off (no shadows are drawn yet)
        self.drawable.set_drawable_hidden(true, Some(&mut *self.particles));
    }
    fn is_drawable_effectively_hidden(&self) -> bool {
        self.drawable.is_drawable_effectively_hidden()
    }
    fn drawable_position(&self) -> Coord3D {
        self.world.find_object(self.drawable.object_id).map_or_else(Coord3D::default, |o| o.get_position())
    }
    fn logic_frame(&self) -> u32 {
        self.world.frame()
    }
    fn radar_create_event(&mut self, pos: &Coord3D, event_type: RadarEventType, seconds_to_live: f32) {
        self.requests.push((*pos, event_type, seconds_to_live));
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
    /// (#165) Predicted: a drawable's client update modules are written by their own xfers in its module list (the
    /// client update type's count 3, each by its tag) and a load into fresh modules of the same tags gives them back;
    /// a tag the drawable has not got is kept as a raw block.
    #[test]
    fn client_update_modules_round_trip_in_the_module_list() {
        use crate::common::xfer::Xfer;
        let tagged = |tag: &str, module| TaggedClientUpdateModule { module_tag: tag.as_bytes().to_vec(), module };
        let fresh = || {
            vec![
                tagged("ModuleTag_Sway", ClientUpdateModule::Sway(SwayClientUpdate::new())),
                tagged("ModuleTag_Bones", ClientUpdateModule::AnimatedParticleSysBone(AnimatedParticleSysBoneClientUpdate::new())),
                tagged("ModuleTag_Laser", ClientUpdateModule::Laser(LaserUpdate::new(LaserUpdateModuleData::default()))),
            ]
        };
        let mut d = Drawable { client_update_modules: fresh(), ..Drawable::for_tests() };
        if let ClientUpdateModule::Sway(s) = &mut d.client_update_modules[0].module {
            s.cur_angle = 0.25;
            s.cur_version = 3;
        }
        if let ClientUpdateModule::Laser(l) = &mut d.client_update_modules[2].module {
            l.end_pos = Coord3D { x: 1.0, y: 2.0, z: 3.0 };
            l.parent_bone_name = b"MUZZLE".to_vec();
        }
        let mut random = RandomValues::new(0);
        let mut env = DrawEnv { assets: &mut NoAssets, random: &mut random, sync_time: 0, logic_time: false, particles: None };
        let mut save = crate::common::xfer_save::XferSave::new();
        save.open(b"t").unwrap();
        d.xfer_drawable_modules(&mut save, &mut env).unwrap();
        let bytes = save.bytes().to_vec();
        // version, type count 2, draw modules 0, client update modules 3
        assert_eq!(&bytes[..7], &[1, 2, 0, 0, 0, 3, 0]);
        let mut back = Drawable { client_update_modules: fresh(), ..Drawable::for_tests() };
        back.client_update_modules.remove(1);
        let mut load = crate::common::xfer_load::XferLoad::new(bytes);
        load.open(b"t").unwrap();
        back.xfer_drawable_modules(&mut load, &mut env).unwrap();
        assert_eq!(back.client_update_modules[0], d.client_update_modules[0]);
        assert_eq!(back.client_update_modules[1], d.client_update_modules[2]);
        assert_eq!(back.saved.kept_modules.iter().map(|k| (k.module_type, k.tag.clone(), k.data.len())).collect::<Vec<_>>(), [(1, b"ModuleTag_Bones".to_vec(), 3)]);
    }

    /// (#181 item 7b) `handleWeaponFireFX` on a drawable without draw modules: no module handles it, with or without
    /// its object (the module's own answer is `tests/differential_model_draw_anim.rs`'s). Predicted: false both ways.
    #[test]
    fn a_fire_without_draw_modules_is_not_handled() {
        let mut d = Drawable::for_tests();
        let mut random = RandomValues::new(0);
        let mut env = DrawEnv { assets: &mut NoAssets, random: &mut random, sync_time: 0, logic_time: true, particles: None };
        assert!(!d.handle_weapon_fire_fx(0, 0, true, true, &mut env));
        assert!(!d.handle_weapon_fire_fx(0, 0, true, false, &mut env));
    }

    #[test]
    fn flags_are_dirty_until_the_draw_modules_are_asked() {
        let line = |depth, f: &str, a: &str| ClientLine { depth, field: f.as_bytes().to_vec(), args: a.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() };
        let block = ClientModuleData {
            module_tag_name_key: Default::default(),
            lines: vec![line(0, "DefaultConditionState", ""), line(1, "Model", "A"), line(0, "ConditionState", "DAMAGED"), line(1, "Model", "B")],
        };
        let data = Arc::new(W3DModelDrawModuleData::parse(b"W3DModelDraw", &block).unwrap().unwrap());
        let mut random = RandomValues::new(0);
        let mut env = DrawEnv { assets: &mut NoAssets, random: &mut random, sync_time: 0, logic_time: true, particles: None };
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
        d.set_drawable_hidden(true, None);
        assert!(d.draw_modules[0].render_object.as_ref().unwrap().hidden);
    }

    /// (#174) `setInstanceScale` reaches `getScale()` (the modules' copy), and `adjustTransformMtx` moves a module's render
    /// transform by its `AttachToBoneInAnotherModule` offset turned by the matrix, then lowers an unfinished building.
    /// Predicted before the run (issue174_predictions.md): the scale 0.5 is the drawable's and the module's; with the
    /// offset (1, 2, 3) from the pristine bone and the matrix a quarter turn about Z at (10, 0, 0), the adjusted
    /// translation is (8, 1, 3); under `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT` at 25 % of a height 8, 6 lower: (8, 1, -3);
    /// no render object gives no current bones.
    #[test]
    fn the_instance_scale_and_the_attach_offset_reach_the_modules() {
        let line = |depth, f: &str, a: &str| ClientLine { depth, field: f.as_bytes().to_vec(), args: a.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() };
        let block = ClientModuleData {
            module_tag_name_key: Default::default(),
            lines: vec![
                line(0, "AttachToBoneInAnotherModule", "TurretBone"),
                line(0, "DefaultConditionState", ""),
                line(1, "Model", "A"),
                line(1, "Flags", "ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT"),
            ],
        };
        let data = Arc::new(W3DModelDrawModuleData::parse(b"W3DModelDraw", &block).unwrap().unwrap());
        let mut random = RandomValues::new(0);
        let mut env = DrawEnv { assets: &mut NoAssets, random: &mut random, sync_time: 0, logic_time: true, particles: None };
        let m = W3DModelDraw::new(data.clone(), 0, false, false, &mut env).unwrap();
        let mut d = Drawable { draw_modules: vec![m], ..Drawable::for_tests() };
        d.set_instance_scale(0.5);
        assert_eq!((d.get_instance_scale(), d.draw_modules[0].scale), (0.5, 0.5));
        let mut bone = Matrix3D::IDENTITY;
        bone.set_translation(&crate::ww::vector3::Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(data.get_attach_to_drawable_bone_offset(|| Some(bone)), Some(crate::ww::vector3::Vector3::new(1.0, 2.0, 3.0)));
        // filled once: a later answer is not asked
        assert_eq!(data.get_attach_to_drawable_bone_offset(|| unreachable!()), Some(crate::ww::vector3::Vector3::new(1.0, 2.0, 3.0)));
        let mut mtx = Matrix3D::IDENTITY;
        for (c, v) in [0.0, -1.0, 0.0, 10.0].into_iter().enumerate() {
            mtx.row[0][c] = v;
        }
        for (c, v) in [1.0, 0.0, 0.0, 0.0].into_iter().enumerate() {
            mtx.row[1][c] = v;
        }
        let mut a = mtx;
        d.draw_modules[0].adjust_transform_mtx(&mut a, None);
        assert_eq!(a.get_translation(), crate::ww::vector3::Vector3::new(8.0, 1.0, 3.0));
        let mut a = mtx;
        d.draw_modules[0].adjust_transform_mtx(&mut a, Some((25.0, 8.0)));
        assert_eq!(a.get_translation(), crate::ww::vector3::Vector3::new(8.0, 1.0, -3.0));
        assert_eq!(d.get_current_client_bone_positions(b"Bone", 1, None, None, 4, 0), 0);
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

    /// (#168) Predicted: a `ClientNewDrawable` makes an object-less drawable kept by the logic's id (1000) and numbered
    /// from 2^30 (a second one 2^30 + 1); with a `LaserUpdate` and a `W3DLaserDraw` on it, `LaserDrawInit` (growth 4 frames)
    /// starts it at width 0 and the draw puts the start and end on the line at width 0; four frames on the width is the
    /// full 2 and the radius `outer * 0.5 * 1` = 3; `LaserSetDecayFrames` starts a decay; `ClientDestroyDrawable` removes
    /// it; the World's objects keep their drawables, and nothing of the World is written.
    #[test]
    fn logic_made_laser_drawables_get_their_calls() {
        use crate::client::laser_update::LaserUpdateModuleData;
        use crate::client::w3d_laser_draw::{W3DLaserDraw, W3DLaserDrawModuleData};
        let mut w = crate::logic::world::gate_scenario::gate_world();
        let mut c = ClientDrawables::new();
        let ev = w.take_client_events();
        c.apply_events(&w, &ev, &mut NoAssets, 0);
        let objects = c.drawables.len();
        let template = c.drawables.values().next().unwrap().template.clone();
        let new = |d: i32| ClientEvent::Object(ObjectClientEvent::ClientNewDrawable { drawable: d, template: template.clone() });
        c.apply_events(&w, &[new(1000), new(1001)], &mut NoAssets, 0);
        assert_eq!(c.logic_drawables.len(), 2);
        assert_eq!(c.logic_drawables[&1000].saved.drawable_id, LOGIC_DRAWABLE_ID_BASE);
        assert_eq!(c.logic_drawables[&1001].saved.drawable_id, LOGIC_DRAWABLE_ID_BASE + 1);
        assert_eq!(c.drawables.len(), objects);
        {
            let d = c.logic_drawables.get_mut(&1000).unwrap();
            d.client_update_modules = vec![TaggedClientUpdateModule { module_tag: b"ModuleTag_L".to_vec(), module: ClientUpdateModule::Laser(LaserUpdate::new(LaserUpdateModuleData::default())) }];
            let data = W3DLaserDrawModuleData { inner_beam_width: 2.0, outer_beam_width: 6.0, ..Default::default() };
            d.laser_draws = vec![W3DLaserDraw::new(Arc::new(data), None, 0).unwrap()];
        }
        let frame = w.frame();
        let start = Coord3D { x: 1.0, y: 2.0, z: 3.0 };
        let end = Coord3D { x: 4.0, y: 5.0, z: 6.0 };
        c.apply_events(&w, &[ClientEvent::Object(ObjectClientEvent::LaserDrawInit { drawable: 1000, start, end, growth_frames: 4 })], &mut NoAssets, 0);
        c.draw(&w, &mut NoAssets, 33);
        let line = &c.logic_drawables[&1000].laser_draws[0].lines[0];
        assert_eq!((line.points[0].x, line.points[1].z, line.width_set), (1.0, 6.0, 0.0));
        for _ in 0..4 {
            w.logic_update().expect("a frame");
        }
        assert_eq!(w.frame(), frame + 4);
        c.draw(&w, &mut NoAssets, 66);
        assert_eq!(c.logic_drawables[&1000].laser_draws[0].lines[0].width_set, 2.0);
        assert_eq!(c.logic_drawables[&1000].laser_template_width(), Some(3.0));
        c.apply_events(&w, &[ClientEvent::Object(ObjectClientEvent::LaserSetDecayFrames { drawable: 1000, frames: 10 })], &mut NoAssets, 0);
        let ClientUpdateModule::Laser(l) = &c.logic_drawables[&1000].client_update_modules[0].module else { unreachable!() };
        assert!(l.decaying);
        c.apply_events(&w, &[ClientEvent::Object(ObjectClientEvent::ClientDestroyDrawable { drawable: 1000 })], &mut NoAssets, 0);
        assert_eq!(c.logic_drawables.keys().copied().collect::<Vec<_>>(), [1001]);
        assert!(c.errors.is_empty(), "{:?}", c.errors);
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
        let mut env = DrawEnv { assets: &mut NoAssets, random: &mut c.random, sync_time: 0, logic_time: true, particles: None };
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

    /// (#182 item 3) `applyPhysicsXform`'s gates read from a real World ([`PhysicsXformGates::of`]): the gate's object
    /// with every gate open; a `FREEZE_TIME` (`ScriptEngine::doFreezeTime`) freezes it, `doUnfreezeTime` lets it go;
    /// then each of the World's other stand-ins (the debug window, the view frozen with its camera still moving, which
    /// a finished camera does not freeze), `DISABLED_HELD`, and an id with no object.
    ///
    /// *Predictions, written before the first run* (`issue182i3_predictions.md`): all hold. Plants 9925 (the script
    /// engine's freeze not read), 9926 (debug), 9927 (view), 9928 (held) each fail it.
    ///
    /// *Results (2026-11-13), first run:* all held. Plants 9925, 9926, 9927 and 9928 each fail it, as predicted; so do
    /// 9916-9920, 9922, 9924 and 9929 (the gates themselves). 9921 (the call order) and 9923 (`ShowClientPhysics`, never
    /// off in the gate's World) do not: the truth-table test and the differentials catch those.
    ///
    /// (#184 item 1) Also [`PhysicsXformGates::frozen`] and [`DrawFacts::frozen`] (`draw_facts`): frozen by the script's
    /// freeze, not by `DISABLED_HELD`. *Prediction, written before the run:* plant 9936 (the draw facts' freeze not
    /// read) fails it. *Result (2026-11-13):* held, and 9936 fails it.
    #[test]
    fn the_physics_gates_read_the_worlds_time_freezes() {
        use crate::client::drawable_loco::{physics_xform_facts, PhysicsXformGates};
        let (mut w, mut c, id) = gate_with_a_transition_module();
        assert_eq!(PhysicsXformGates::of(&w, id), PhysicsXformGates::OPEN);
        assert!(physics_xform_facts(&w, id).gates.passes());

        assert!(!ClientDrawables::draw_facts(&w, id, true, None).frozen);
        w.script_engine.do_freeze_time();
        let g = PhysicsXformGates::of(&w, id);
        assert!(g.time_frozen_script && !g.passes(), "a script froze time: {g:?}");
        // (#184 item 1) the draw modules' freeze is the same expression, read with the same gates
        assert!(g.frozen());
        assert!(ClientDrawables::draw_facts(&w, id, true, None).frozen, "the tank and truck draws see the freeze");
        assert_eq!(physics_xform_facts(&w, id).gates, g);
        c.draw(&w, &mut NoAssets, 33);
        assert!(c.drawables[&id].physics_xform.is_none());
        w.script_engine.do_unfreeze_time();
        assert!(PhysicsXformGates::of(&w, id).passes());

        w.cuts.game_loop.script_time_frozen_debug = true;
        assert!(!PhysicsXformGates::of(&w, id).passes(), "the debug window paused");
        w.cuts.game_loop.script_time_frozen_debug = false;

        w.cuts.game_loop.view_time_frozen = true;
        assert!(PhysicsXformGates::of(&w, id).passes(), "the camera's movement is finished");
        w.cuts.game_loop.view_camera_movement_finished = false;
        assert!(!PhysicsXformGates::of(&w, id).passes(), "frozen for the camera's movement");
        w.cuts.game_loop.view_time_frozen = false;
        assert!(PhysicsXformGates::of(&w, id).passes());
        w.cuts.game_loop.view_camera_movement_finished = true;

        w.set_disabled(id, crate::common::disabled_types::DISABLED_HELD);
        let g = PhysicsXformGates::of(&w, id);
        assert!(g.held && !g.passes(), "{g:?}");
        // (#184 item 1) held is not frozen: the tank and truck draws do not test it
        assert!(!g.frozen() && !ClientDrawables::draw_facts(&w, id, true, None).frozen);

        let g = PhysicsXformGates::of(&w, crate::common::game_type::INVALID_ID);
        assert!(!g.has_object && !g.passes());
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
        d.set_drawable_hidden(true, None);
        assert!(d.draw_modules[0].render_object.as_ref().unwrap().hidden);
        c.draw(&w, &mut NoAssets, 33);
        let d = &c.drawables[&id];
        assert!(d.is_model_dirty, "the modules were not asked");
        assert_eq!(d.draw_modules[0].cur_state_ref(), Some(&StateRef::Normal(0)));
        assert!(d.physics_xform.is_none());
        assert_eq!(d.shroud_clear_frame, 0);

        c.drawables.get_mut(&id).unwrap().set_drawable_hidden(false, None);
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
    /// (#179 item 3) `Drawable::draw`'s root reaches the render object (Drawable.cpp:2662-2673, W3DModelDraw.cpp:2094-2100).
    ///
    /// *Predictions (issue179i3_predictions.md, before the run):* with the instance matrix a turn of 0.3 about Z and a
    /// lift of 5 (`setInstanceMatrix`), the gate object's render object after a draw is the object's transform
    /// post-multiplied by it, then this draw's physics totals if any (the module has no attach bone, no height flag, scale
    /// 1); the module keeps the object's transform for `nukeCurrentRender`; the renderer's local matrix is the instance
    /// matrix then the totals; `setInstanceMatrix(NULL)` gives the identity, flagged, and the renderer's local matrix is
    /// the totals alone (or none). Plants 9853, 9855, 9856 and 9857 each fail it.
    ///
    /// *Results (2026-11-13), first run:* all held. Plants 9846, 9847, 9853, 9855, 9856, 9857 and 9870 each fail it;
    /// 9849 (no physics on the root) does not: the gate object has no physics totals here (the differential test and the
    /// client's truck test cover the physics).
    #[test]
    fn the_draw_root_carries_the_instance_matrix_and_the_physics() {
        let (w, mut c, id) = gate_with_a_transition_module();
        let mut inst = Matrix3D::IDENTITY;
        inst.rotate_z(0.3);
        inst.translate(0.0, 0.0, 5.0);
        let d = c.drawables.get_mut(&id).unwrap();
        d.set_instance_matrix(Some(&inst));
        assert!(!d.saved.instance_is_identity);
        c.draw(&w, &mut NoAssets, 33);
        let d = &c.drawables[&id];
        let object = *w.find_object(id).unwrap().thing.transform_matrix();
        let mut expected = object;
        expected.post_mul(&inst);
        if let Some(p) = &d.physics_xform {
            p.apply(&mut expected);
        }
        assert_ne!(expected, object);
        assert_eq!(d.draw_modules[0].render_object.as_ref().unwrap().transform, expected, "the render object's transform is the draw's root");
        assert_eq!(d.draw_transform_mtx(&object), expected);
        assert_eq!(d.draw_modules[0].drawable_transform, object, "nukeCurrentRender's transform is the drawable's");
        let mut local = inst;
        if let Some(p) = &d.physics_xform {
            p.apply(&mut local);
        }
        assert_eq!(d.draw_local_matrix(), Some(local));

        let d = c.drawables.get_mut(&id).unwrap();
        d.set_instance_matrix(None);
        assert!(d.saved.instance_is_identity);
        assert_eq!(d.saved.instance, Matrix3D::IDENTITY);
        assert_eq!(d.draw_local_matrix(), d.physics_xform.map(|p| p.local_matrix()));
        // the free function: no instance, no physics is the drawable's transform
        assert_eq!(draw_transform_mtx(&object, None, None), object);
    }

    /// (#182) The logic's instance matrices reach the drawable and the draw's root: a `StructureCollapseUpdate`
    /// structure killed in the gate's game, and a `FloatUpdate` object (`Enabled = No`: only the bob), run frame by
    /// frame; each frame's client events are applied (`ClientDrawables::apply_events`) and the drawables drawn.
    ///
    /// *Predictions (issue182_predictions.md, before the run):* the collapse's events are first the waiting shudder
    /// (z 0), then the collapsing shudder (z the falling height, below 0), then once the zero; after each, the drawable's
    /// instance matrix is the identity's rotation with the event's translation, never flagged the identity (not even
    /// after the zero), and its render object's transform is the object's transform post-multiplied by it. The float's
    /// instance matrix after each frame is `Rotate_Z(0)`, `Rotate_Y(sin(f * 0.0291) * 0.05)`, `Rotate_X(sin(f * 0.0515) *
    /// 0.05)` of the event's frame `f` (its Z rotation stays 0). Plants 9880, 9881, 9871-9874, 9883 and 9875-9879 each
    /// fail it.
    ///
    /// *Results (2026-11-13):* first run passed (9 waiting, 5 collapsing, 1 zero; 15 tilts). Plants 9872-9875, 9877,
    /// 9879, 9880, 9881 and 9883 failed it; **9871, 9876 and 9878 did not (a prediction miss):** both drawables started
    /// at the identity, so keeping the rotation, the Z rotation or the translation changed nothing. The test now starts
    /// them at a client-set turn and lift (the building turned 0.3 about Z and moved (5, 5, 5), the floater turned 0.5
    /// and lifted 4): the collapse keeps the turn and replaces the translation, the bob keeps the Z rotation of the
    /// matrix it read and drops the lift. Re-run: passes (the same 9, 5, 1 and 15), and plants 9871-9881 and 9883 each
    /// fail it (9882 fails `the_logic_s_set_instance_matrix_reaches_the_drawable`).
    #[test]
    fn the_logic_s_instance_matrices_reach_the_draw_root() {
        use crate::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
        use crate::logic::world::gate_scenario::{gate_world_with, make};
        use crate::logic::world::team_ops::factory_find_team;
        use crate::ww::vector3::Vector3;
        let ini = "Object Collapser\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 20\n GeometryMinorRadius = 20\n GeometryHeight = 10\n\
                   Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                   Behavior = StructureCollapseUpdate ModuleTag_Collapse\n  MinCollapseDelay = 300\n  MaxCollapseDelay = 300\n  MinBurstDelay = 1000\n  MaxBurstDelay = 2000\n  CollapseDamping = .5\n  MaxShudder = 1.5\n  BigBurstFrequency = 4\n End\nEnd\n\
                   Object Floater\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                   Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                   Behavior = FloatUpdate ModuleTag_Float\n  Enabled = No\n End\nEnd\n";
        let mut w = gate_world_with(ini);
        let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
        let building = make(&mut w, b"Collapser", team, Coord3D { x: 300.0, y: 300.0, z: 0.0 });
        let floater = make(&mut w, b"Floater", team, Coord3D { x: 60.0, y: 300.0, z: 0.0 });
        let mut c = ClientDrawables::new();
        let module = |c: &mut ClientDrawables| {
            let line = |depth, f: &str, a: &str| ClientLine { depth, field: f.as_bytes().to_vec(), args: a.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() };
            let block = ClientModuleData { module_tag_name_key: Default::default(), lines: vec![line(0, "DefaultConditionState", ""), line(1, "Model", "T")] };
            let data = Arc::new(W3DModelDrawModuleData::parse(b"W3DModelDraw", &block).unwrap().unwrap());
            let mut env = DrawEnv { assets: &mut NoAssets, random: &mut c.random, sync_time: 0, logic_time: true, particles: None };
            W3DModelDraw::new(data, 0, false, false, &mut env).unwrap()
        };
        // the drawables (made at the first draw), each with a model and an instance matrix the client set before (a
        // turn and a lift): the collapse keeps the building's turn, the bob keeps the floater's Z rotation alone
        c.draw(&w, &mut NoAssets, 0);
        let mut turned = Matrix3D::IDENTITY;
        turned.rotate_z(0.3);
        turned.translate(5.0, 5.0, 5.0);
        let mut lifted = Matrix3D::IDENTITY;
        lifted.rotate_z(0.5);
        lifted.translate(0.0, 0.0, 4.0);
        for (id, inst) in [(building, turned), (floater, lifted)] {
            let m = module(&mut c);
            let d = c.drawables.get_mut(&id).expect("a drawable");
            d.draw_modules = vec![m];
            d.set_instance_matrix(Some(&inst));
        }
        w.kill(building, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
        let (mut waiting, mut falling, mut zeros, mut floats) = (0, 0, 0, 0);
        for frame in 0..900u32 {
            w.logic_update().expect("a frame");
            let ev = w.take_client_events();
            let before = c.drawables[&floater].saved.instance;
            let (mut last, mut tilt) = (None, None);
            for e in &ev {
                match e {
                    ClientEvent::Object(ObjectClientEvent::DrawableSetInstanceTranslation { object_id, translation }) if *object_id == building => last = Some(*translation),
                    ClientEvent::Object(ObjectClientEvent::DrawableFloatTilt { object_id, frame }) if *object_id == floater => tilt = Some(*frame),
                    _ => {}
                }
            }
            c.apply_events(&w, &ev, &mut NoAssets, frame * 33);
            c.draw(&w, &mut NoAssets, frame * 33);
            let check = |c: &ClientDrawables, id: ObjectId, expected: &Matrix3D| {
                let d = &c.drawables[&id];
                assert!(!d.saved.instance_is_identity, "set with a matrix: never flagged the identity");
                assert_eq!(d.saved.instance, *expected);
                let object = *w.find_object(id).unwrap().thing.transform_matrix();
                let mut root = object;
                root.post_mul(expected);
                if let Some(p) = &d.physics_xform {
                    p.apply(&mut root);
                }
                assert_eq!(d.draw_transform_mtx(&object), root);
                assert_eq!(d.draw_modules[0].render_object.as_ref().unwrap().transform, root, "the render object's transform is the draw's root");
            };
            if let Some(t) = last {
                if (t.x, t.y, t.z) == (0.0, 0.0, 0.0) {
                    zeros += 1;
                } else if t.z == 0.0 {
                    assert_eq!((falling, zeros), (0, 0), "the waiting shudder comes first");
                    waiting += 1;
                } else {
                    assert!(t.z < 0.0 && zeros == 0);
                    falling += 1;
                }
                let mut expected = turned;
                expected.set_translation(&Vector3::new(t.x, t.y, t.z));
                check(&c, building, &expected);
            }
            if let Some(f) = tilt {
                let mut expected = Matrix3D::IDENTITY;
                expected.rotate_z(before.get_z_rotation());
                expected.rotate_y((petir::Real::sin(f64::from(f as f32 * 0.0291)) * f64::from(0.05f32)) as f32);
                expected.rotate_x((petir::Real::sin(f64::from(f as f32 * 0.0515)) * f64::from(0.05f32)) as f32);
                check(&c, floater, &expected);
                floats += 1;
            }
            if zeros > 0 {
                break;
            }
        }
        println!("collapse events: waiting {waiting}, falling {falling}, zeros {zeros}; float tilts {floats}");
        assert!(waiting > 0 && falling > 0 && zeros == 1, "waiting {waiting}, falling {falling}, zeros {zeros}");
        assert!(floats > 0);
        // a later frame: the collapse sleeps for ever, its drawable keeps the last matrix
        let kept = c.drawables[&building].saved.instance;
        w.logic_update().expect("a frame");
        let ev = w.take_client_events();
        c.apply_events(&w, &ev, &mut NoAssets, 0);
        assert_eq!(c.drawables[&building].saved.instance, kept);
        assert!(!c.drawables[&building].saved.instance_is_identity);
    }

    /// (#182) `DrawableSetInstanceMatrix` (`NeutronMissileUpdate`'s `setInstanceMatrix(&m)` and `(NULL)`) reaches the
    /// drawable: with a matrix it is the instance matrix, not flagged; with none the identity, flagged. Predicted: both
    /// hold; plant 9882 fails it.
    #[test]
    fn the_logic_s_set_instance_matrix_reaches_the_drawable() {
        let (w, mut c, id) = gate_with_a_transition_module();
        let mut m = Matrix3D::IDENTITY;
        m.rotate_x(0.4);
        m.translate(1.0, 2.0, 3.0);
        let ev = [ClientEvent::Object(ObjectClientEvent::DrawableSetInstanceMatrix { object_id: id, matrix: Some(m) })];
        c.apply_events(&w, &ev, &mut NoAssets, 0);
        assert_eq!((c.drawables[&id].saved.instance, c.drawables[&id].saved.instance_is_identity), (m, false));
        let ev = [ClientEvent::Object(ObjectClientEvent::DrawableSetInstanceMatrix { object_id: id, matrix: None })];
        c.apply_events(&w, &ev, &mut NoAssets, 0);
        assert_eq!((c.drawables[&id].saved.instance, c.drawables[&id].saved.instance_is_identity), (Matrix3D::IDENTITY, true));
    }

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
