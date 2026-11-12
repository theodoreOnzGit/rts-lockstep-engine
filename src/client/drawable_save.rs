// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp (Drawable::xfer, Drawable::xferDrawableModules,
//     Drawable::loadPostProcess's transform, TintEnvelope::xfer, drawableIconIndexToName, drawableIconNameToIndex,
//     TheDrawableIconNames; the constructor's values of the saved members)
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/Anim2D.cpp (Anim2D::xfer)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/DynamicAudioEventInfo.cpp (DynamicAudioEventInfo::xferNoName)
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/Module.cpp (Module::xfer, DrawableModule::xfer)
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/DrawModule.cpp (DrawModule::xfer)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp (W3DModelDraw::xfer)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankDraw.cpp, W3DTruckDraw.cpp,
//     W3DTankTruckDraw.cpp, W3DPoliceCarDraw.cpp, W3DOverlordTankDraw.cpp, W3DOverlordTruckDraw.cpp,
//     W3DOverlordAircraftDraw.cpp, W3DSupplyDraw.cpp, W3DScienceModelDraw.cpp, W3DDependencyModelDraw.cpp (each
//     ::xfer)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! A drawable in a save file (GitHub #59): `Drawable::xfer` (version 7) with its modules (`xferDrawableModules`) and the
//! xfers of the draw modules the port has, each layered as upstream's class is.
//! **(#165)** The client update modules the port makes (all four classes: `BeaconClientUpdate`, `SwayClientUpdate`,
//! `AnimatedParticleSysBoneClientUpdate`, `LaserUpdate`) are written and read by their own xfers
//! ([`crate::client::beacon_client_update::ClientUpdateModule::xfer`]), compared byte for byte with upstream's
//! (`tests/differential_client_update.rs`).
//!
//! # As upstream writes it
//!
//! The drawable's id; its model condition flags (version 2 on: before the modules, so that a load forces them onto the
//! modules, `replaceModelConditionFlags(flags, TRUE)`, and a module's animation frame is then restored into the right
//! animation); its transform (a `Matrix3D` from version 5, a position and an orientation in versions 3 and 4); the two tint
//! envelopes; the terrain decal; six opacities; its object's id (a load checks it is the drawable's object); the status,
//! tint and fade members; `m_locoInfo` (without its yaw and pitch modulators); the modules; the stealth look; the flash; the
//! hidden flags; the second material pass opacity; the instance matrix and scale; the shroud status object; the expiration
//! date; the icons; and the ambient sound's two switches and customised sound. A load then sets the stealth look back to
//! `STEALTHLOOK_NONE` ("in the next stealth update, it will be set to its correct value"), updates the hidden status of the
//! modules when hidden, and marks the model dirty.
//!
//! The modules are written by type (`MODULETYPE_DRAW`, `MODULETYPE_CLIENT_UPDATE`), each by its module tag in a block; a
//! load finds each by its tag and skips a module it has not got by its block's size.
//!
//! # The fields kept but not used
//!
//! The port's drawable is its object's and draws by it (`client::drawable`); these members upstream's drawable has are read
//! into [`DrawableSaveFields`] and written back as read (or as upstream's constructor sets them), and nothing reads them yet:
//! the drawable id (`m_id`: the port addresses drawables by object), the transform (a drawable with an object takes its
//! object's at the load's post-process, as upstream; one without keeps it), the tint envelopes and the tint statuses, the
//! terrain decal, the opacities, `m_status`, the fade members, the flash count and colour, the second material pass opacity,
//! the instance matrix, its identity flag and scale (the modules were made with the template's scale), the shroud status
//! object, the expiration date, the icons (`TheAnim2DCollection` is not translated: an icon's template name is kept
//! unchecked), the modules the port does not make (their blocks kept as read, and written back; ~~a client update module
//! is one~~ **Corrected 2026-11-12 (#165):** the client update modules are made and saved by their own xfers, so only a
//! client update module whose tag the drawable has not got is kept), and
//! `W3DDependencyModelDraw::m_dependencyCleared`. The ambient sound's part is the client audio's
//! ([`crate::client::drawable_ambient::AmbientSounds::to_save`], `restore_saved`).
//!
//! # Where this differs from upstream
//!
//! - **The modules' order in a save:** the port's draw modules (#165: or client update modules) in their order, then the
//!   kept modules of that type;
//!   upstream writes its module list (which also holds the modules the port does not make).
//! - **An icon whose name is not one of upstream's** is refused (`SC_INVALID_DATA`); upstream indexes with `ICON_INVALID`
//!   (undefined).
//! - **The drawable id** a drawable is made with is its object's id (the port's convention, as the audio's); upstream's
//!   comes from the client's counter.

use std::collections::BTreeMap;

use super::drawable::Drawable;
use super::drawable_ambient::{AmbientSave, CustomAmbientSave, DynamicAudioSave};
use super::drawable_loco::DrawableLocoInfo;
use super::w3d_model_draw::{anim_mode_index, DrawEnv, RecoilState, W3DModelDraw, W3dAssetSource, WeaponRecoilInfo, HideShowSubObjInfo};
use crate::common::base_type::Coord3D;
use crate::common::game_state::SaveError;
use crate::common::game_type::{DrawableId, ObjectId, INVALID_ID};
use crate::common::model_state::ModelConditionFlags;
use crate::common::xfer::{Color, Xfer, XferMode, XferVersion};
use crate::logic::modules::stealth_update::stealth_look;
use crate::logic::weapon_set::SLOTS;
use crate::logic::world::game_logic::World;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `TERRAIN_DECAL_NONE` (without `ALLOW_DEMORALIZE`).
pub const TERRAIN_DECAL_NONE: i32 = 8;
/// `FADING_NONE`.
pub const FADING_NONE: i32 = 0;
/// `MODULETYPE_DRAW - FIRST_DRAWABLE_MODULE_TYPE`, `MODULETYPE_CLIENT_UPDATE - FIRST_DRAWABLE_MODULE_TYPE`.
pub const DRAW_MODULES: u16 = 0;
pub const CLIENT_UPDATE_MODULES: u16 = 1;
/// `NUM_DRAWABLE_MODULE_TYPES`.
pub const NUM_DRAWABLE_MODULE_TYPES: u16 = 2;

/// `TheDrawableIconNames` (without `ALLOW_DEMORALIZE`), in `DrawableIconType`'s order; `MAX_ICONS` is its length.
pub const DRAWABLE_ICON_NAMES: [&str; 14] = [
    "DefaultHeal",
    "StructureHeal",
    "VehicleHeal",
    "Demoralized_OBSOLETE",
    "BombTimed",
    "BombRemote",
    "Disabled",
    "BattlePlanIcon_Bombard",
    "BattlePlanIcon_HoldTheLine",
    "BattlePlanIcon_SeekAndDestroy",
    "Emoticon",
    "Enthusiastic",
    "Subliminal",
    "CarBomb",
];

/// `drawableIconNameToIndex(name)`: by name ignoring case (`stricmp`); `None` is `ICON_INVALID`.
pub fn drawable_icon_name_to_index(name: &[u8]) -> Option<usize> {
    DRAWABLE_ICON_NAMES.iter().position(|n| n.as_bytes().eq_ignore_ascii_case(name))
}

/// `class TintEnvelope`'s saved members (`TintEnvelope()`: all zero, `ENVELOPE_STATE_REST`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TintEnvelope {
    /// `m_attackRate`, `m_decayRate`, `m_peakColor`, `m_currentColor` (`Vector3`s).
    pub attack_rate: [f32; 3],
    pub decay_rate: [f32; 3],
    pub peak_color: [f32; 3],
    pub current_color: [f32; 3],
    /// `m_sustainCounter`.
    pub sustain_counter: u32,
    /// `m_affect`.
    pub affect: bool,
    /// `m_envState` (`ENVELOPE_STATE_REST` 0, `ATTACK`, `DECAY`, `SUSTAIN`).
    pub env_state: i8,
}

/// `Anim2D`'s saved members.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anim2DSave {
    pub current_frame: u16,
    pub last_update_frame: u32,
    pub status: u8,
    pub min_frame: u16,
    pub max_frame: u16,
    pub frames_between_updates: u32,
    pub alpha: f32,
}

/// One of the drawable's icons (`DrawableIconInfo`'s `m_icon[i]` and `m_keepTillFrame[i]`).
#[derive(Clone, Debug, PartialEq)]
pub struct DrawableIconSave {
    pub keep_till_frame: u32,
    /// The icon's `Anim2DTemplate` name (kept unchecked: no `TheAnim2DCollection` here).
    pub template_name: Vec<u8>,
    pub anim: Anim2DSave,
}

/// A module the port does not make, kept as a save gave it (its type, tag and block).
#[derive(Clone, Debug, PartialEq)]
pub struct KeptModule {
    pub module_type: u16,
    pub tag: Vec<u8>,
    pub data: Vec<u8>,
}

/// The members of upstream's `Drawable` its xfer saves that the port's drawable does not use (module docs), at the
/// constructor's values until a load reads them.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawableSaveFields {
    /// `m_id`.
    pub drawable_id: DrawableId,
    /// `getTransformMatrix()`.
    pub transform: Matrix3D,
    /// `m_selectionFlashEnvelope`, `m_colorTintEnvelope` (`NULL` until used).
    pub selection_flash_envelope: Option<TintEnvelope>,
    pub color_tint_envelope: Option<TintEnvelope>,
    /// `m_terrainDecalType`.
    pub terrain_decal_type: i32,
    /// `m_explicitOpacity`, `m_stealthOpacity`, `m_effectiveStealthOpacity`, `m_decalOpacityFadeTarget`,
    /// `m_decalOpacityFadeRate`, `m_decalOpacity`.
    pub explicit_opacity: f32,
    pub stealth_opacity: f32,
    pub effective_stealth_opacity: f32,
    pub decal_opacity_fade_target: f32,
    pub decal_opacity_fade_rate: f32,
    pub decal_opacity: f32,
    /// `m_status` (`DrawableStatus` bits), `m_tintStatus`, `m_prevTintStatus`.
    pub status: u32,
    pub tint_status: u32,
    pub prev_tint_status: u32,
    /// `m_fadeMode` (`FadingMode`), `m_timeElapsedFade`, `m_timeToFade`.
    pub fade_mode: i32,
    pub time_elapsed_fade: u32,
    pub time_to_fade: u32,
    /// `m_flashCount`, `m_flashColor`.
    pub flash_count: i32,
    pub flash_color: Color,
    /// `m_secondMaterialPassOpacity`.
    pub second_material_pass_opacity: f32,
    /// `m_instanceIsIdentity`, `m_instance`, `m_instanceScale`.
    pub instance_is_identity: bool,
    pub instance: Matrix3D,
    pub instance_scale: f32,
    /// `m_drawableInfo.m_shroudStatusObjectID`.
    pub shroud_status_object_id: ObjectId,
    /// `m_expirationDate` (0: never).
    pub expiration_date: u32,
    /// The icons by `DrawableIconType` (`m_iconInfo`, lazily made).
    pub icons: BTreeMap<usize, DrawableIconSave>,
    /// The modules a load found no module of the port's for.
    pub kept_modules: Vec<KeptModule>,
    /// The ambient sound's switches and customised sound, as the save holds them (module docs).
    pub ambient: AmbientSave,
}

impl DrawableSaveFields {
    /// `Drawable(thingTemplate)`'s values: no envelopes, `TERRAIN_DECAL_NONE`, the opacities 1 (the decal's 0), no status,
    /// `FADING_NONE`, no flash, the identity instance of the template's scale, no shroud status object, never expiring, no
    /// icons; the id is the object's (module docs).
    pub fn new(drawable_id: DrawableId, instance_scale: f32) -> Self {
        DrawableSaveFields {
            drawable_id,
            transform: Matrix3D::IDENTITY,
            selection_flash_envelope: None,
            color_tint_envelope: None,
            terrain_decal_type: TERRAIN_DECAL_NONE,
            explicit_opacity: 1.0,
            stealth_opacity: 1.0,
            effective_stealth_opacity: 1.0,
            decal_opacity_fade_target: 0.0,
            decal_opacity_fade_rate: 0.0,
            decal_opacity: 0.0,
            status: 0,
            tint_status: 0,
            prev_tint_status: 0,
            fade_mode: FADING_NONE,
            time_elapsed_fade: 0,
            time_to_fade: 0,
            flash_count: 0,
            flash_color: 0,
            second_material_pass_opacity: 0.0,
            instance_is_identity: true,
            instance: Matrix3D::IDENTITY,
            instance_scale,
            shroud_status_object_id: INVALID_ID,
            expiration_date: 0,
            icons: BTreeMap::new(),
            kept_modules: Vec::new(),
            ambient: AmbientSave::default(),
        }
    }
}

/// An `int`-sized enum or a `Real` triple by `xferUser(&x, sizeof(x))`: its little-endian bytes.
fn xfer_user_i32(xfer: &mut dyn Xfer, v: &mut i32) -> Result<(), SaveError> {
    let mut b = v.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    *v = i32::from_le_bytes(b);
    Ok(())
}

/// `xferUser(&vector3, sizeof(Vector3))`: three reals' bytes.
fn xfer_user_vector3(xfer: &mut dyn Xfer, v: &mut [f32; 3]) -> Result<(), SaveError> {
    let mut b = [0u8; 12];
    for (i, x) in v.iter().enumerate() {
        b[i * 4..i * 4 + 4].copy_from_slice(&x.to_bits().to_le_bytes());
    }
    xfer.xfer_user(&mut b)?;
    for (i, x) in v.iter_mut().enumerate() {
        *x = f32::from_bits(u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().expect("four bytes")));
    }
    Ok(())
}

/// `xferUser(&m_instance, sizeof(Matrix3D))`: the twelve reals' bytes, row by row (no version, unlike `xferMatrix3D`).
fn xfer_user_matrix3d(xfer: &mut dyn Xfer, m: &mut Matrix3D) -> Result<(), SaveError> {
    let mut b = [0u8; 48];
    for (i, x) in m.to_array().iter().enumerate() {
        b[i * 4..i * 4 + 4].copy_from_slice(&x.to_bits().to_le_bytes());
    }
    xfer.xfer_user(&mut b)?;
    let mut a = [0f32; 12];
    for (i, x) in a.iter_mut().enumerate() {
        *x = f32::from_bits(u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().expect("four bytes")));
    }
    *m = Matrix3D::from_array(a);
    Ok(())
}

/// `TintEnvelope::xfer` (version 1).
pub fn tint_envelope_xfer(xfer: &mut dyn Xfer, t: &mut TintEnvelope) -> Result<(), SaveError> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)?;
    xfer_user_vector3(xfer, &mut t.attack_rate)?;
    xfer_user_vector3(xfer, &mut t.decay_rate)?;
    xfer_user_vector3(xfer, &mut t.peak_color)?;
    xfer_user_vector3(xfer, &mut t.current_color)?;
    xfer.xfer_unsigned_int(&mut t.sustain_counter)?;
    xfer.xfer_bool(&mut t.affect)?;
    xfer.xfer_byte(&mut t.env_state)?;
    Ok(())
}

/// `Anim2D::xfer` (version 1).
pub fn anim2d_xfer(xfer: &mut dyn Xfer, a: &mut Anim2DSave) -> Result<(), SaveError> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)?;
    xfer.xfer_unsigned_short(&mut a.current_frame)?;
    xfer.xfer_unsigned_int(&mut a.last_update_frame)?;
    xfer.xfer_unsigned_byte(&mut a.status)?;
    xfer.xfer_unsigned_short(&mut a.min_frame)?;
    xfer.xfer_unsigned_short(&mut a.max_frame)?;
    xfer.xfer_unsigned_int(&mut a.frames_between_updates)?;
    xfer.xfer_real(&mut a.alpha)?;
    Ok(())
}

/// `DynamicAudioEventInfo::xferNoName` (version 1): the overridden fields' bits as one `UnsignedByte` ("count on our local
/// version to keep things straight"), then each overridden field (the loop flag as a `Bool`, the priority as an
/// `UnsignedByte`).
pub fn dynamic_audio_xfer_no_name(xfer: &mut dyn Xfer, d: &mut DynamicAudioSave) -> Result<(), SaveError> {
    use crate::logic::world::object_map_props::override_bits as o;
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)?;
    xfer.xfer_unsigned_byte(&mut d.overridden)?;
    let bits = u32::from(d.overridden);
    if bits & o::LOOP_FLAG != 0 {
        xfer.xfer_bool(&mut d.loop_flag)?;
    }
    if bits & o::LOOP_COUNT != 0 {
        xfer.xfer_int(&mut d.loop_count)?;
    }
    if bits & o::VOLUME != 0 {
        xfer.xfer_real(&mut d.volume)?;
    }
    if bits & o::MIN_VOLUME != 0 {
        xfer.xfer_real(&mut d.min_volume)?;
    }
    if bits & o::MIN_RANGE != 0 {
        xfer.xfer_real(&mut d.min_distance)?;
    }
    if bits & o::MAX_RANGE != 0 {
        xfer.xfer_real(&mut d.max_distance)?;
    }
    if bits & o::PRIORITY != 0 {
        xfer.xfer_unsigned_byte(&mut d.priority)?;
    }
    Ok(())
}

/// `Module::xfer`, `DrawableModule::xfer` and `DrawModule::xfer` (each version 1, outermost last): the base classes'
/// three version bytes.
fn draw_module_base_xfer(xfer: &mut dyn Xfer) -> Result<(), SaveError> {
    for _ in 0..3 {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
    }
    Ok(())
}

/// `W3DModelDraw::xfer` (version 2): the base classes, each weapon slot's recoil infos, the `showSubObject` list, and
/// (version 2) the animation as a fraction of its frames (not for a transition state: "we can't really restore them
/// effectively"). A load sets the fraction into the current animation (its mode kept: "it's too risky"), at sync time
/// `now`, and applies the sub-object list.
pub fn w3d_model_draw_xfer(xfer: &mut dyn Xfer, m: &mut W3DModelDraw, now: u32) -> Result<(), SaveError> {
    let current: XferVersion = 2;
    let mut version = current;
    xfer.xfer_version(&mut version, current)?;
    draw_module_base_xfer(xfer)?;
    let saving = xfer.get_xfer_mode() == XferMode::Save;
    for i in 0..SLOTS {
        let mut count = m.weapon_recoil_info_vec[i].len() as u8;
        xfer.xfer_unsigned_byte(&mut count)?;
        if saving {
            for r in m.weapon_recoil_info_vec[i].iter() {
                let mut state = recoil_state_index(r.state);
                xfer_user_i32(xfer, &mut state)?;
                let (mut shift, mut rate) = (r.shift, r.recoil_rate);
                xfer.xfer_real(&mut shift)?;
                xfer.xfer_real(&mut rate)?;
            }
        } else {
            m.weapon_recoil_info_vec[i].clear();
            for _ in 0..count {
                let mut state = 0;
                xfer_user_i32(xfer, &mut state)?;
                let mut r = WeaponRecoilInfo { state: recoil_state_from_index(state), ..Default::default() };
                xfer.xfer_real(&mut r.shift)?;
                xfer.xfer_real(&mut r.recoil_rate)?;
                m.weapon_recoil_info_vec[i].push(r);
            }
        }
    }
    let mut count = m.sub_object_vec.len() as u8;
    xfer.xfer_unsigned_byte(&mut count)?;
    if saving {
        for s in m.sub_object_vec.iter_mut() {
            xfer.xfer_ascii_string(&mut s.sub_obj_name)?;
            xfer.xfer_bool(&mut s.hide)?;
        }
    } else {
        m.sub_object_vec.clear();
        for _ in 0..count {
            let mut s = HideShowSubObjInfo { sub_obj_name: Vec::new(), hide: false };
            xfer.xfer_ascii_string(&mut s.sub_obj_name)?;
            xfer.xfer_bool(&mut s.hide)?;
            m.sub_object_vec.push(s);
        }
    }
    if version >= 2 {
        if saving {
            let not_transition = m.cur_state_ref().is_some_and(|s| m.data.state(s).transition_sig.is_none());
            let anim = match &m.render_object {
                Some(r) if r.is_hlod && not_transition => Some(r.anim.as_ref().map(|a| (anim_mode_index(a.anim_mode), a.frame, a.motion.num_frames))),
                _ => None,
            };
            match anim {
                Some(anim) => {
                    let mut present = anim.is_some();
                    xfer.xfer_bool(&mut present)?;
                    if let Some((mut mode, frame, num_frames)) = anim {
                        xfer.xfer_int(&mut mode)?;
                        // "xfer frame as a fraction (this will allow the animations to change in future patches but will
                        // still be mostly correct)"
                        let mut percent = frame / (num_frames - 1) as f32;
                        xfer.xfer_real(&mut percent)?;
                    }
                }
                None => {
                    let mut present = false;
                    xfer.xfer_bool(&mut present)?;
                }
            }
        } else {
            let mut present = false;
            xfer.xfer_bool(&mut present)?;
            if present {
                let mut mode = 0i32;
                xfer.xfer_int(&mut mode)?; // note, this will be ignored
                let mut percent = 0f32;
                xfer.xfer_real(&mut percent)?;
                if let Some(r) = m.render_object.as_mut() {
                    if r.is_hlod {
                        if let Some(a) = r.anim.clone() {
                            let frame = percent * (a.motion.num_frames - 1) as f32;
                            r.set_animation(Some(a.motion), frame, a.anim_mode, now);
                        }
                    }
                }
            }
        }
    }
    if !saving && !m.sub_object_vec.is_empty() {
        m.update_sub_objects();
    }
    Ok(())
}

/// The draw module's own xfer, by its class: each class's version byte, then its base class's (`W3DOverlordTankDraw` on
/// `W3DTankDraw`; `W3DOverlordTruckDraw` and `W3DPoliceCarDraw` on `W3DTruckDraw`; the others on `W3DModelDraw`), and
/// `W3DDependencyModelDraw`'s `m_dependencyCleared` after. None of the tank and truck draws saves its own members ("John A
/// and Mark W say there is no data to save here"): their treads, wheels and the police car's frame start as made.
pub fn draw_module_xfer(xfer: &mut dyn Xfer, m: &mut W3DModelDraw, now: u32) -> Result<(), SaveError> {
    let layers = match m.data.module_name.as_slice() {
        b"W3DOverlordTankDraw" | b"W3DOverlordTruckDraw" | b"W3DPoliceCarDraw" => 2,
        b"W3DTankDraw" | b"W3DTruckDraw" | b"W3DTankTruckDraw" | b"W3DSupplyDraw" | b"W3DScienceModelDraw" | b"W3DOverlordAircraftDraw" | b"W3DDependencyModelDraw" => 1,
        _ => 0,
    };
    for _ in 0..layers {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
    }
    w3d_model_draw_xfer(xfer, m, now)?;
    if m.data.module_name == b"W3DDependencyModelDraw" {
        xfer.xfer_bool(&mut m.dependency_cleared)?;
    }
    Ok(())
}

/// `WeaponRecoilInfo::RecoilState` as its enum's number.
fn recoil_state_index(s: RecoilState) -> i32 {
    match s {
        RecoilState::Idle => 0,
        RecoilState::RecoilStart => 1,
        RecoilState::Recoil => 2,
        RecoilState::Settle => 3,
    }
}

/// The `RecoilState` of a number (one outside the enum is taken as `SETTLE`'s last state, `IDLE`: upstream keeps the
/// number, which no `switch` case matches, so the barrel does nothing, as idle).
fn recoil_state_from_index(i: i32) -> RecoilState {
    match i {
        1 => RecoilState::RecoilStart,
        2 => RecoilState::Recoil,
        3 => RecoilState::Settle,
        _ => RecoilState::Idle,
    }
}

impl Drawable {
    /// `Drawable::xferDrawableModules` (version 1): on a save, the dirty flags given to the modules first
    /// (`DIRTY_CONDITION_FLAGS`'s `getDrawModules()`); then `NUM_DRAWABLE_MODULE_TYPES` and, per type, the module count and
    /// each module's tag and block. A load finds each module by its tag; one the port has not got is kept (module docs).
    pub fn xfer_drawable_modules<A: W3dAssetSource>(&mut self, xfer: &mut dyn Xfer, env: &mut DrawEnv<'_, A>) -> Result<(), SaveError> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        let saving = xfer.get_xfer_mode() == XferMode::Save;
        if saving {
            self.get_draw_modules(env);
        } else {
            self.saved.kept_modules.clear();
        }
        let now = env.sync_time;
        let mut module_types = NUM_DRAWABLE_MODULE_TYPES;
        xfer.xfer_unsigned_short(&mut module_types)?;
        for cur in 0..module_types {
            // (#165) the client update modules the port makes are its own too
            // (#168) the `W3DLaserDraw` modules are draw modules too (written after the others)
            let own = if cur == DRAW_MODULES { self.draw_modules.len() + self.laser_draws.len() } else { self.client_update_modules.len() };
            let kept = self.saved.kept_modules.iter().filter(|k| k.module_type == cur).count();
            let mut count = (own + kept) as u16;
            xfer.xfer_unsigned_short(&mut count)?;
            if saving {
                if cur == DRAW_MODULES {
                    for m in self.draw_modules.iter_mut() {
                        let mut tag = m.data.module_tag.clone();
                        xfer.xfer_ascii_string(&mut tag)?;
                        xfer.begin_block()?;
                        draw_module_xfer(xfer, m, now)?;
                        xfer.end_block()?;
                    }
                    for m in self.laser_draws.iter_mut() {
                        let mut tag = m.data.module_tag.clone();
                        xfer.xfer_ascii_string(&mut tag)?;
                        xfer.begin_block()?;
                        m.xfer(xfer)?;
                        xfer.end_block()?;
                    }
                } else if cur == CLIENT_UPDATE_MODULES {
                    // (#165) each client update module's own xfer (`BeaconClientUpdate`, `SwayClientUpdate`,
                    // `AnimatedParticleSysBoneClientUpdate`, `LaserUpdate`)
                    for m in self.client_update_modules.iter_mut() {
                        let mut tag = m.module_tag.clone();
                        xfer.xfer_ascii_string(&mut tag)?;
                        xfer.begin_block()?;
                        m.module.xfer(xfer)?;
                        xfer.end_block()?;
                    }
                }
                for k in self.saved.kept_modules.iter_mut().filter(|k| k.module_type == cur) {
                    xfer.xfer_ascii_string(&mut k.tag)?;
                    xfer.begin_block()?;
                    xfer.xfer_user(&mut k.data)?;
                    xfer.end_block()?;
                }
            } else {
                for _ in 0..count {
                    let mut tag = Vec::new();
                    xfer.xfer_ascii_string(&mut tag)?;
                    let found = if cur == DRAW_MODULES { self.draw_modules.iter().position(|m| m.data.module_tag == tag) } else { None };
                    // (#168) a `W3DLaserDraw` found by its tag
                    let found_laser = if cur == DRAW_MODULES && found.is_none() { self.laser_draws.iter().position(|m| m.data.module_tag == tag) } else { None };
                    // (#165) a client update module found by its tag (`findModule(moduleIdentifier)`, the first)
                    let found_client = if cur == CLIENT_UPDATE_MODULES { self.client_update_modules.iter().position(|m| m.module_tag == tag) } else { None };
                    let data_size = xfer.begin_block()?;
                    match (found, found_client) {
                        (Some(i), _) => draw_module_xfer(xfer, &mut self.draw_modules[i], now)?,
                        (None, Some(i)) => self.client_update_modules[i].module.xfer(xfer)?,
                        (None, None) if found_laser.is_some() => self.laser_draws[found_laser.unwrap_or_default()].xfer(xfer)?,
                        (None, None) => {
                            // ("Module '%s' was indicated in file, but not found on Drawable": its data skipped upstream,
                            // kept here)
                            let mut data = vec![0u8; data_size.max(0) as usize];
                            xfer.xfer_user(&mut data)?;
                            self.saved.kept_modules.push(KeptModule { module_type: cur, tag, data });
                        }
                    }
                    xfer.end_block()?;
                }
            }
        }
        Ok(())
    }

    /// `Drawable::xfer` (version 7; versions 1 to 6 read as upstream reads them), the drawable's object being `object`
    /// (`INVALID_ID`: none) in World `w`. See the module docs.
    pub fn xfer<A: W3dAssetSource>(&mut self, xfer: &mut dyn Xfer, w: &World, env: &mut DrawEnv<'_, A>) -> Result<(), SaveError> {
        let current: XferVersion = 7;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let saving = xfer.get_xfer_mode() == XferMode::Save;
        // (a load drops the ambient sound here, "restored in loadPostProcess()": the client audio's, see the module docs)
        xfer.xfer_drawable_id(&mut self.saved.drawable_id)?;
        if version >= 2 {
            self.xfer_condition_state(xfer, env)?;
        }
        if version >= 3 {
            if saving {
                if let Some(o) = w.find_object(self.object_id) {
                    self.saved.transform = *o.thing.transform_matrix();
                }
            }
            if version >= 5 {
                xfer.xfer_matrix3d(&mut self.saved.transform)?;
            } else {
                // (`setPosition`, `setOrientation`: the transform about z at the position)
                let t = self.saved.transform.get_translation();
                let mut pos = Coord3D { x: t.x, y: t.y, z: t.z };
                xfer.xfer_coord3d(&mut pos)?;
                let mut orientation = self.saved.transform.get_z_rotation();
                xfer.xfer_real(&mut orientation)?;
                let mut m = Matrix3D::IDENTITY;
                m.rotate_z(orientation);
                m.set_translation(&Vector3 { x: pos.x, y: pos.y, z: pos.z });
                self.saved.transform = m;
            }
        }
        for env_slot in [&mut self.saved.selection_flash_envelope, &mut self.saved.color_tint_envelope] {
            let mut present = env_slot.is_some();
            xfer.xfer_bool(&mut present)?;
            if present {
                tint_envelope_xfer(xfer, env_slot.get_or_insert_with(TintEnvelope::default))?;
            }
        }
        let s = &mut self.saved;
        xfer_user_i32(xfer, &mut s.terrain_decal_type)?;
        xfer.xfer_real(&mut s.explicit_opacity)?;
        xfer.xfer_real(&mut s.stealth_opacity)?;
        xfer.xfer_real(&mut s.effective_stealth_opacity)?;
        xfer.xfer_real(&mut s.decal_opacity_fade_target)?;
        xfer.xfer_real(&mut s.decal_opacity_fade_rate)?;
        xfer.xfer_real(&mut s.decal_opacity)?;
        let mut object_id = if w.find_object(self.object_id).is_some() { self.object_id } else { INVALID_ID };
        xfer.xfer_object_id(&mut object_id)?;
        if !saving {
            let has_object = self.object_id != INVALID_ID && w.find_object(self.object_id).is_some();
            // ("is attached to wrong object", "is not attached to an object but should be attached to object")
            if (has_object && object_id != self.object_id) || (!has_object && object_id != INVALID_ID) {
                return Err(SaveError::InvalidData);
            }
        }
        let s = &mut self.saved;
        xfer.xfer_unsigned_int(&mut s.status)?;
        xfer.xfer_unsigned_int(&mut s.tint_status)?;
        xfer.xfer_unsigned_int(&mut s.prev_tint_status)?;
        xfer_user_i32(xfer, &mut s.fade_mode)?;
        xfer.xfer_unsigned_int(&mut s.time_elapsed_fade)?;
        xfer.xfer_unsigned_int(&mut s.time_to_fade)?;
        let mut has_loco_info = self.loco_info.is_some();
        xfer.xfer_bool(&mut has_loco_info)?;
        if has_loco_info {
            let l = self.loco_info.get_or_insert_with(DrawableLocoInfo::default);
            for v in [
                &mut l.pitch,
                &mut l.pitch_rate,
                &mut l.roll,
                &mut l.roll_rate,
                &mut l.yaw,
                &mut l.acceleration_pitch,
                &mut l.acceleration_pitch_rate,
                &mut l.acceleration_roll,
                &mut l.acceleration_roll_rate,
                &mut l.overlap_z_vel,
                &mut l.overlap_z,
                &mut l.wobble,
                &mut l.wheel_info.front_left_height_offset,
                &mut l.wheel_info.front_right_height_offset,
                &mut l.wheel_info.rear_left_height_offset,
                &mut l.wheel_info.rear_right_height_offset,
                &mut l.wheel_info.wheel_angle,
            ] {
                xfer.xfer_real(v)?;
            }
            xfer.xfer_int(&mut l.wheel_info.frames_airborne_counter)?;
            xfer.xfer_int(&mut l.wheel_info.frames_airborne)?;
        }
        self.xfer_drawable_modules(xfer, env)?;
        xfer_user_i32(xfer, &mut self.stealth_look)?;
        let s = &mut self.saved;
        xfer.xfer_int(&mut s.flash_count)?;
        xfer.xfer_color(&mut s.flash_color)?;
        xfer.xfer_bool(&mut self.hidden)?;
        xfer.xfer_bool(&mut self.hidden_by_stealth)?;
        let s = &mut self.saved;
        xfer.xfer_real(&mut s.second_material_pass_opacity)?;
        xfer.xfer_bool(&mut s.instance_is_identity)?;
        xfer_user_matrix3d(xfer, &mut s.instance)?;
        xfer.xfer_real(&mut s.instance_scale)?;
        xfer.xfer_object_id(&mut s.shroud_status_object_id)?;
        if version < 2 {
            // "condition state used to be here so we must keep it here for compatibility" (read only)
            self.xfer_condition_state(xfer, env)?;
        }
        let s = &mut self.saved;
        xfer.xfer_unsigned_int(&mut s.expiration_date)?;
        let mut icon_count = s.icons.len() as u8;
        xfer.xfer_unsigned_byte(&mut icon_count)?;
        if saving {
            for (i, icon) in s.icons.iter_mut() {
                let mut name = DRAWABLE_ICON_NAMES[*i].as_bytes().to_vec();
                xfer.xfer_ascii_string(&mut name)?;
                xfer.xfer_unsigned_int(&mut icon.keep_till_frame)?;
                xfer.xfer_ascii_string(&mut icon.template_name)?;
                anim2d_xfer(xfer, &mut icon.anim)?;
            }
        } else {
            // "destroy any icons that might be present right now in favor of what we'll load from the file"
            s.icons.clear();
            for _ in 0..icon_count {
                let mut name = Vec::new();
                xfer.xfer_ascii_string(&mut name)?;
                let index = drawable_icon_name_to_index(&name);
                let mut icon = DrawableIconSave { keep_till_frame: 0, template_name: Vec::new(), anim: Anim2DSave::default() };
                xfer.xfer_unsigned_int(&mut icon.keep_till_frame)?;
                xfer.xfer_ascii_string(&mut icon.template_name)?;
                anim2d_xfer(xfer, &mut icon.anim)?;
                let Some(index) = index else { return Err(SaveError::InvalidData) };
                s.icons.insert(index, icon);
            }
        }
        if !saving {
            // "On load, we want to set it to none, because stealthlook updates when it changes. So in the next stealth
            // update, it will be set to it's correct value"
            self.stealth_look = stealth_look::NONE;
            // "Also, need to update the hidden status for all sub-modules."
            if self.hidden || self.hidden_by_stealth {
                self.update_hidden_status();
            }
            // "when loading we will force the modules to be dirty just to be sure that they get re-evaluated"
            self.is_model_dirty = true;
        }
        let a = &mut self.saved.ambient;
        if version >= 4 {
            xfer.xfer_bool(&mut a.enabled)?;
        }
        if version >= 6 {
            xfer.xfer_bool(&mut a.enabled_from_script)?;
        }
        if version >= 7 {
            let mut customized = a.custom.is_some();
            xfer.xfer_bool(&mut customized)?;
            if customized {
                let mut to_silence = matches!(a.custom, Some(CustomAmbientSave::NoSound));
                xfer.xfer_bool(&mut to_silence)?;
                if saving {
                    if let Some(CustomAmbientSave::Info(d)) = &mut a.custom {
                        xfer.xfer_ascii_string(&mut d.original_name)?;
                        dynamic_audio_xfer_no_name(xfer, d)?;
                    }
                } else if to_silence {
                    a.custom = Some(CustomAmbientSave::NoSound);
                } else {
                    let mut d = DynamicAudioSave::default();
                    xfer.xfer_ascii_string(&mut d.original_name)?;
                    dynamic_audio_xfer_no_name(xfer, &mut d)?;
                    a.custom = Some(CustomAmbientSave::Info(d));
                }
            }
        }
        Ok(())
    }

    /// `m_conditionState.xfer(xfer)`; on a load, `replaceModelConditionFlags(m_conditionState, TRUE)`.
    fn xfer_condition_state<A: W3dAssetSource>(&mut self, xfer: &mut dyn Xfer, env: &mut DrawEnv<'_, A>) -> Result<(), SaveError> {
        let mut flags: ModelConditionFlags = self.condition_state;
        flags.xfer(xfer)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.replace_model_condition_flags(&flags, true, env);
        }
        Ok(())
    }

    /// `Drawable::loadPostProcess`'s transform: "if we have an object, we don't need to save/load the pos, just restore
    /// it". (Its ambient sound part is the client audio's, `AmbientSounds::restore_saved`.)
    pub fn load_post_process(&mut self, w: &World) {
        if let Some(o) = w.find_object(self.object_id) {
            self.saved.transform = *o.thing.transform_matrix();
        }
    }
}
