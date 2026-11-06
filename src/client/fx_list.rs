// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/FXList.cpp
//     (adjustVector, FXNugget::doFXObj, SoundFXNugget, TracerFXNugget, RayEffectFXNugget, LightPulseFXNugget,
//     ViewShakeFXNugget, TerrainScorchFXNugget, ParticleSystemFXNugget, FXListAtBonePosFXNugget: their parse tables,
//     constructors, doFXPos and doFXObj; TheFXListFieldParse; FXList::clear, doFXPos, doFXObj;
//     FXListStore::findFXList, parseFXListDefinition)
//   GeneralsMD/Code/GameEngine/Include/GameClient/FXList.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The FX lists (GitHub #43, #44): `FXList.ini`'s named lists of effects ("nuggets"), which the logic names when a
//! weapon fires, an object dies or an upgrade lands, and which the client plays.
//!
//! # As upstream reads it
//!
//! - **`FXList name` blocks** (`FXListStore::parseFXListDefinition`): the list of that name (made if new) is cleared,
//!   then each nugget block is read in order: `Sound`, `RayEffect`, `Tracer`, `LightPulse`, `ViewShake`,
//!   `TerrainScorch`, `ParticleSystem`, `FXListAtBonePos`, each `... End` with its own fields.
//! - **`findFXList(name)`**: `None` (any case) is no list; otherwise the list by name, or none.
//! - **`doFXPos(primary, mtx, speed, secondary, radius)`**: nothing unless the primary position's cell is clear of
//!   the shroud for the local player; then each nugget's `doFXPos`. **`doFXObj(primary, secondary)`**: nothing when the
//!   primary object is fogged or shrouded for the local player; then each nugget's `doFXObj` (by default its
//!   `doFXPos` at the object's position and transform, speed 0).
//! - **The particle system nugget** makes `Count` systems of its template, each placed at the primary position plus
//!   its `Offset` (turned by the transform), plus a random point on a circle of `Radius` (and `Height` above), or at
//!   the ground (`CreateAtGroundHeight`); oriented to the object (`OrientToObject`), rotated (`RotateX/Y/Z`), attached
//!   to the object (`AttachToObject`), delayed (`InitialDelay` in ms), and given the caller's radius
//!   (`UseCallersRadius`).
//!
//! # Where this differs from upstream
//!
//! - The nuggets that need what the client does not draw yet (sounds, light pulses, view shakes, terrain scorches,
//!   tracers and ray effects) are recorded as [`FxRequest`]s for the caller, after the client random draws their
//!   `doFXPos` makes (the tracer's probability, the scorch's random type), so the client stream stays upstream's.
//! - `FXListAtBonePos` names its list (`INI::parseFXList` keeps the name, as the logic does,
//!   `crate::common::damage_fx::FxListName`), and is played through [`FxHost::bone_positions`].
//! - The world is read through [`FxHost`]; the particle systems are the [`ParticleSystemManager`]'s.
//! - `BuryFX` is not in this release's `FXList.cpp` (no such nugget exists upstream).
//!
//! # Verified against the compiled upstream (#46)
//!
//! `tests/differential_particle_sys.rs` runs `FXList.cpp`, compiled, with the real particle systems: random `FXList`
//! INI files (every nugget field compared) and random `doFXPos`/`doFXObj` plays (every request, the particle systems
//! made and the client random state compared); 0 mismatches. It found one defect, fixed here: the particle system
//! nugget's `radius * cos(angle)` (C's `double` `cos`, the sum in `double`).

use std::collections::BTreeMap;

use crate::client::particle_sys::{volume_type, ParticleSystemManager};
use crate::common::base_type::{real_to_int_ceil, Coord3D, RGBColor};
use crate::common::game_common::convert_duration_from_msecs_to_frames;
use crate::common::game_type::ObjectId;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, LookupListRec, UserData};
use crate::common::random_value::{DistributionType, GameClientRandomVariable, RandomValues};
use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `PI`.
const PI: f32 = std::f32::consts::PI;

/// `View::CameraShakeType` names (`parseShakeType`).
pub const SHAKE_TYPE_NAMES: &[LookupListRec] = &[
    LookupListRec { name: "SUBTLE", value: 0 },
    LookupListRec { name: "NORMAL", value: 1 },
    LookupListRec { name: "STRONG", value: 2 },
    LookupListRec { name: "SEVERE", value: 3 },
    LookupListRec { name: "CINE_EXTREME", value: 4 },
    LookupListRec { name: "CINE_INSANE", value: 5 },
];
/// `View::SHAKE_NORMAL`.
pub const SHAKE_NORMAL: i32 = 1;

/// `Scorches`: `SCORCH_1`..`SCORCH_4`, `SHADOW_SCORCH`.
pub const SCORCH_1: i32 = 0;
pub const SCORCH_4: i32 = 3;
/// `parseScorchType`'s names (`RANDOM` is -1).
pub const SCORCH_TYPE_NAMES: &[LookupListRec] = &[
    LookupListRec { name: "SCORCH_1", value: 0 },
    LookupListRec { name: "SCORCH_2", value: 1 },
    LookupListRec { name: "SCORCH_3", value: 2 },
    LookupListRec { name: "SCORCH_4", value: 3 },
    LookupListRec { name: "SHADOW_SCORCH", value: 4 },
    LookupListRec { name: "RANDOM", value: -1 },
];

/// `class ParticleSystemFXNugget`'s fields.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleSystemFxNugget {
    pub name: Vec<u8>,
    pub count: i32,
    pub offset: Coord3D,
    pub radius: GameClientRandomVariable,
    pub height: GameClientRandomVariable,
    /// `m_delay` (ms; -1: the system's own).
    pub delay: GameClientRandomVariable,
    pub rotate_x: f32,
    pub rotate_y: f32,
    pub rotate_z: f32,
    pub orient_to_object: bool,
    pub attach_to_object: bool,
    pub create_at_ground_height: bool,
    pub use_callers_radius: bool,
    /// `m_ricochet` (the constructor leaves it to the zeroing pool: false).
    pub ricochet: bool,
}

impl Default for ParticleSystemFxNugget {
    /// `ParticleSystemFXNugget()`.
    fn default() -> Self {
        let c = |v| GameClientRandomVariable::new(v, v, DistributionType::Constant);
        ParticleSystemFxNugget {
            name: Vec::new(),
            count: 1,
            offset: Coord3D::default(),
            radius: c(0.0),
            height: c(0.0),
            delay: c(if plant(4415) { 0.0 } else { -1.0 }),
            rotate_x: 0.0,
            rotate_y: 0.0,
            rotate_z: 0.0,
            orient_to_object: false,
            attach_to_object: false,
            create_at_ground_height: false,
            use_callers_radius: false,
            ricochet: false,
        }
    }
}

/// `class TracerFXNugget`'s fields.
#[derive(Clone, Debug, PartialEq)]
pub struct TracerFxNugget {
    pub tracer_name: Vec<u8>,
    pub bone_name: Vec<u8>,
    pub speed: f32,
    pub decay_at: f32,
    pub length: f32,
    pub width: f32,
    pub color: RGBColor,
    pub probability: f32,
}

impl Default for TracerFxNugget {
    /// `TracerFXNugget()`.
    fn default() -> Self {
        TracerFxNugget {
            tracer_name: b"GenericTracer".to_vec(),
            bone_name: Vec::new(),
            speed: 0.0,
            decay_at: 1.0,
            length: 10.0,
            width: 1.0,
            color: RGBColor { red: 1.0, green: 1.0, blue: 1.0 },
            probability: 1.0,
        }
    }
}

/// `class LightPulseFXNugget`'s fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LightPulseFxNugget {
    pub color: RGBColor,
    pub radius: f32,
    pub bounding_circle_pct: f32,
    pub increase_frames: u32,
    pub decrease_frames: u32,
}

/// One nugget of an FX list (`class FXNugget`'s subclasses).
#[derive(Clone, Debug, PartialEq)]
pub enum FxNugget {
    /// `SoundFXNugget`: `m_soundName`.
    Sound { sound_name: Vec<u8> },
    /// `RayEffectFXNugget`.
    RayEffect { template_name: Vec<u8>, primary_offset: Coord3D, secondary_offset: Coord3D },
    Tracer(TracerFxNugget),
    LightPulse(LightPulseFxNugget),
    /// `ViewShakeFXNugget`: `m_shake` (`View::CameraShakeType`).
    ViewShake { shake: i32 },
    /// `TerrainScorchFXNugget`: `m_scorch` (-1: random), `m_radius`.
    TerrainScorch { scorch: i32, radius: f32 },
    ParticleSystem(ParticleSystemFxNugget),
    /// `FXListAtBonePosFXNugget`: `m_fx` (by name), `m_boneName`, `m_orientToBone`.
    FxListAtBonePos { fx: Option<Vec<u8>>, bone_name: Vec<u8>, orient_to_bone: bool },
}

/// `class FXList`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FxList {
    /// `m_nuggets`, in file order.
    pub nuggets: Vec<FxNugget>,
}

/// What a played nugget asks of the parts of the client not drawn yet (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub enum FxRequest {
    /// `TheAudio->addAudioEvent(&sound)` at the position (none for `doFXPos` without a primary).
    Sound { name: Vec<u8>, position: Option<Coord3D> },
    /// `TheDisplay->createLightPulse(pos, &color, 1, radius, increase, decrease)`.
    LightPulse { position: Coord3D, color: RGBColor, radius: f32, increase_frames: u32, decrease_frames: u32 },
    /// `TheTacticalView->shake(pos, type)`.
    ViewShake { position: Coord3D, shake: i32 },
    /// `TheGameClient->addScorch(pos, radius, type)`.
    TerrainScorch { position: Coord3D, radius: f32, scorch: i32 },
    /// A tracer drawable from `primary` to `secondary` (`TracerFXNugget::doFXPos`): its template, speed, length,
    /// width, colour and frames to live.
    Tracer { template: Vec<u8>, primary: Coord3D, secondary: Coord3D, speed: f32, length: f32, width: f32, color: RGBColor, frames: i32 },
    /// `TheGameClient->createRayEffectByTemplate(&source, &target, tmpl)`.
    RayEffect { template: Vec<u8>, source: Coord3D, target: Coord3D },
}

/// What playing an FX list reads of the world.
pub trait FxHost {
    /// `ThePartitionManager->getShroudStatusForPlayer(localPlayerIndex, pos) == CELLSHROUD_CLEAR`.
    fn cell_clear_for_local_player(&self, pos: &Coord3D) -> bool;
    /// `obj->getShroudedStatus(localPlayerIndex) > OBJECTSHROUD_PARTIAL_CLEAR`.
    fn object_fogged_for_local_player(&self, obj: ObjectId) -> bool;
    /// `obj->getPosition()` and `obj->getTransformMatrix()` (`None`: no such object).
    fn object_position_and_transform(&self, obj: ObjectId) -> Option<(Coord3D, Matrix3D)>;
    /// `obj->getGeometryInfo().getBoundingCircleRadius()`.
    fn object_bounding_circle_radius(&self, obj: ObjectId) -> f32;
    /// `TheTerrainLogic->getLayerHeight(x, y, getLayerForDestination(pos))`.
    fn layer_height_for_destination(&self, pos: &Coord3D) -> f32;
    /// `draw->getCurrentClientBonePositions(boneName, start, ...)` then `obj->convertBonePosToWorldPos`: the world
    /// positions and transforms of the object's bones (none without a drawable).
    fn bone_positions(&self, obj: ObjectId, bone_name: &[u8], start: i32) -> Vec<(Coord3D, Matrix3D)>;
}

/// What playing needs of the client: its random, the particle systems, the client frame, and where requests go.
pub struct FxPlay<'a> {
    pub random: &'a mut RandomValues,
    pub particles: &'a mut ParticleSystemManager,
    pub frame: u32,
    pub requests: &'a mut Vec<FxRequest>,
}

/// `adjustVector(vec, mtx)`: the vector turned by the matrix.
fn adjust_vector(v: &Coord3D, mtx: Option<&Matrix3D>) -> Coord3D {
    match mtx {
        Some(m) => {
            let r = m.rotate_vector(&Vector3 { x: v.x, y: v.y, z: v.z });
            Coord3D { x: r.x, y: r.y, z: r.z }
        }
        None => *v,
    }
}

/// `calcDist(src, dst)`.
fn calc_dist(src: &Coord3D, dst: &Coord3D) -> f32 {
    let (dx, dy, dz) = (dst.x - src.x, dst.y - src.y, dst.z - src.z);
    crate::ww::wwmath::sqrt(dx * dx + dy * dy + dz * dz)
}

impl ParticleSystemFxNugget {
    /// `reallyDoFX(primary, mtx, thingToAttachTo, overrideRadius)`: the new systems' ids.
    pub fn really_do_fx<H: FxHost>(&self, host: &H, play: &mut FxPlay<'_>, primary: &Coord3D, mtx: Option<&Matrix3D>, thing_to_attach_to: Option<ObjectId>, override_radius: f32) -> Vec<u32> {
        let mut made = Vec::new();
        let offset = adjust_vector(&self.offset, mtx);
        if play.particles.find_template(&self.name).is_none() {
            return made;
        }
        for _ in 0..self.count {
            let Some(id) = play.particles.create_particle_system(&self.name, true, play.random, play.frame) else { continue };
            made.push(id);
            let radius = self.radius.get_value(play.random);
            let angle = play.random.game_client_random_value_real(0.0, 2.0 * PI);
            // ~~`radius * WWMath::Cos(angle)` in `float`~~ **Corrected 2026-11-06 (#46):** upstream's `cos(angle)` is C's
            // `double` `cos` (`differential/shim/math.h`), so the sum is taken in `double` and rounded once (the
            // differential test found the last bit)
            let mut new_pos = Coord3D {
                x: (f64::from(primary.x + offset.x) + f64::from(radius) * petir::Real::cos(f64::from(angle))) as f32,
                y: (f64::from(primary.y + offset.y) + f64::from(radius) * petir::Real::sin(f64::from(angle))) as f32,
                z: 0.0,
            };
            if plant(4414) {
                new_pos.x = primary.x + offset.x + radius * crate::ww::wwmath::cos(angle);
                new_pos.y = primary.y + offset.y + radius * crate::ww::wwmath::sin(angle);
            }
            if self.create_at_ground_height {
                new_pos.z = host.layer_height_for_destination(&new_pos);
            } else {
                new_pos.z = primary.z + offset.z + self.height.get_value(play.random);
            }
            let sys = play.particles.find_particle_system_mut(id).expect("made");
            if self.orient_to_object {
                if let Some(m) = mtx {
                    sys.set_local_transform(m);
                }
            }
            if self.rotate_x != 0.0 {
                sys.rotate_local_transform_x(self.rotate_x);
            }
            if self.rotate_y != 0.0 {
                sys.rotate_local_transform_y(self.rotate_y);
            }
            if self.rotate_z != 0.0 {
                sys.rotate_local_transform_z(self.rotate_z);
            }
            match thing_to_attach_to {
                Some(o) if self.attach_to_object => sys.attach_to_object(o),
                _ => sys.set_position(&new_pos),
            }
            let delay_in_msec = self.delay.get_value(play.random);
            let sys = play.particles.find_particle_system_mut(id).expect("made");
            if delay_in_msec >= 0.0 {
                let delay_in_frames = real_to_int_ceil(convert_duration_from_msecs_to_frames(delay_in_msec)) as u32;
                sys.set_initial_delay(delay_in_frames);
            }
            if self.use_callers_radius && override_radius != 0.0 {
                if sys.info.emission_volume_type == volume_type::SPHERE {
                    sys.set_emission_volume_sphere_radius(override_radius);
                } else if sys.info.emission_volume_type == volume_type::CYLINDER {
                    sys.set_emission_volume_cylinder_radius(override_radius);
                }
            }
        }
        made
    }
}

impl FxNugget {
    /// The nugget's `doFXPos(primary, primaryMtx, primarySpeed, secondary, overrideRadius)`.
    #[allow(clippy::too_many_arguments)]
    pub fn do_fx_pos<H: FxHost>(&self, store: &FxListStore, host: &H, play: &mut FxPlay<'_>, primary: Option<&Coord3D>, mtx: Option<&Matrix3D>, speed: f32, secondary: Option<&Coord3D>, override_radius: f32) {
        match self {
            FxNugget::Sound { sound_name } => play.requests.push(FxRequest::Sound { name: sound_name.clone(), position: primary.copied() }),
            FxNugget::Tracer(t) => {
                if t.probability <= play.random.game_client_random_value_real(0.0, 1.0) {
                    return;
                }
                if let (Some(p), Some(s)) = (primary, secondary) {
                    let sp = if t.speed == 0.0 { speed } else { t.speed };
                    let dist = calc_dist(p, s) - t.length;
                    let frames = if dist >= 0.0 && sp >= 0.0 { dist / sp } else { 1.0 };
                    let frames = real_to_int_ceil(frames * t.decay_at);
                    play.requests.push(FxRequest::Tracer { template: t.tracer_name.clone(), primary: *p, secondary: *s, speed: sp, length: t.length, width: t.width, color: t.color, frames });
                }
            }
            FxNugget::RayEffect { template_name, primary_offset, secondary_offset } => {
                if let (Some(p), Some(s)) = (primary, secondary) {
                    let source = Coord3D { x: p.x + primary_offset.x, y: p.y + primary_offset.y, z: p.z + primary_offset.z };
                    let target = Coord3D { x: s.x + secondary_offset.x, y: s.y + secondary_offset.y, z: s.z + secondary_offset.z };
                    play.requests.push(FxRequest::RayEffect { template: template_name.clone(), source, target });
                }
            }
            FxNugget::LightPulse(l) => {
                if let Some(p) = primary {
                    play.requests.push(FxRequest::LightPulse { position: *p, color: l.color, radius: l.radius, increase_frames: l.increase_frames, decrease_frames: l.decrease_frames });
                }
            }
            FxNugget::ViewShake { shake } => {
                if let Some(p) = primary {
                    play.requests.push(FxRequest::ViewShake { position: *p, shake: *shake });
                }
            }
            FxNugget::TerrainScorch { scorch, radius } => {
                if let Some(p) = primary {
                    let mut s = *scorch;
                    if s < 0 {
                        s = play.random.game_client_random_value(SCORCH_1, if plant(4412) { SCORCH_4 - 1 } else { SCORCH_4 });
                    }
                    play.requests.push(FxRequest::TerrainScorch { position: *p, radius: *radius, scorch: s });
                }
            }
            FxNugget::ParticleSystem(n) => {
                if let Some(p) = primary {
                    n.really_do_fx(host, play, p, mtx, None, override_radius);
                }
            }
            // "You must use the object form for this effect"
            FxNugget::FxListAtBonePos { .. } => {}
        }
        let _ = store;
    }

    /// The nugget's `doFXObj(primary, secondary)`.
    pub fn do_fx_obj<H: FxHost>(&self, store: &FxListStore, host: &H, play: &mut FxPlay<'_>, primary: Option<ObjectId>, secondary: Option<ObjectId>) {
        let at = |o: Option<ObjectId>| o.and_then(|o| host.object_position_and_transform(o));
        match self {
            FxNugget::Sound { sound_name } => {
                // (`setPlayerIndex(primary's controlling player)`: the audio is not translated)
                play.requests.push(FxRequest::Sound { name: sound_name.clone(), position: at(primary).map(|a| a.0) });
            }
            FxNugget::LightPulse(l) => {
                if let (Some(o), Some((pos, _))) = (primary, at(primary)) {
                    let mut radius = l.radius;
                    if l.bounding_circle_pct > 0.0 {
                        radius = host.object_bounding_circle_radius(o) * l.bounding_circle_pct;
                    }
                    play.requests.push(FxRequest::LightPulse { position: pos, color: l.color, radius, increase_frames: l.increase_frames, decrease_frames: l.decrease_frames });
                }
            }
            FxNugget::ParticleSystem(n) => {
                let Some((ppos, pmtx)) = at(primary) else { return };
                if let (true, Some((spos, _))) = (n.ricochet, at(secondary)) {
                    let (delta_x, delta_y) = if plant(4413) { (ppos.y - spos.y, ppos.x - spos.x) } else { (ppos.x - spos.x, ppos.y - spos.y) };
                    let aiming_angle = crate::ww::wwmath::atan2(delta_y, delta_x);
                    let mut aiming = Matrix3D::IDENTITY;
                    aiming.rotate_z(aiming_angle);
                    n.really_do_fx(host, play, &ppos, Some(&aiming), primary, 0.0);
                } else {
                    n.really_do_fx(host, play, &ppos, Some(&pmtx), primary, 0.0);
                }
            }
            FxNugget::FxListAtBonePos { fx, bone_name, .. } => {
                let Some(o) = primary else { return };
                for start in [0, 1] {
                    for (p, m) in host.bone_positions(o, bone_name, start) {
                        if let Some(list) = fx.as_deref().and_then(|n| store.find_fx_list(n)) {
                            list.do_fx_pos(store, host, play, Some(&p), Some(&m), 0.0, None, 0.0);
                        }
                    }
                }
            }
            // `FXNugget::doFXObj`: `doFXPos` at the primary's position and transform, speed 0 ("NOT the object's
            // speed"), the secondary's position
            _ => {
                let p = at(primary);
                let s = at(secondary);
                self.do_fx_pos(store, host, play, p.as_ref().map(|a| &a.0), p.as_ref().map(|a| &a.1), 0.0, s.as_ref().map(|a| &a.0), 0.0);
            }
        }
    }
}

impl FxList {
    /// `FXList::doFXPos(primary, primaryMtx, primarySpeed, secondary, overrideRadius)`: nothing unless the primary's
    /// cell is clear for the local player (upstream reads the cell of a `NULL` primary; here no primary plays nothing).
    #[allow(clippy::too_many_arguments)]
    pub fn do_fx_pos<H: FxHost>(&self, store: &FxListStore, host: &H, play: &mut FxPlay<'_>, primary: Option<&Coord3D>, mtx: Option<&Matrix3D>, speed: f32, secondary: Option<&Coord3D>, override_radius: f32) {
        let Some(p) = primary else { return };
        if !host.cell_clear_for_local_player(p) {
            return;
        }
        for n in &self.nuggets {
            n.do_fx_pos(store, host, play, primary, mtx, speed, secondary, override_radius);
        }
    }

    /// `FXList::doFXObj(primary, secondary)`: nothing when the primary is fogged or shrouded for the local player.
    pub fn do_fx_obj<H: FxHost>(&self, store: &FxListStore, host: &H, play: &mut FxPlay<'_>, primary: Option<ObjectId>, secondary: Option<ObjectId>) {
        if primary.is_some_and(|o| host.object_fogged_for_local_player(o)) {
            return;
        }
        for n in &self.nuggets {
            n.do_fx_obj(store, host, play, primary, secondary);
        }
    }
}

/// `class FXListStore`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FxListStore {
    /// `m_fxmap`, by name (`NAMEKEY(name)`: exact).
    pub fxmap: BTreeMap<Vec<u8>, FxList>,
}

/// A nugget block read into its fields (`initFromINI(nugget, myFieldParse)`), then added to the list.
fn parse_nugget<N>(ini: &mut Ini<'_>, list: &mut FxList, mut n: N, table: &[FieldParse<N>], wrap: fn(N) -> FxNugget) -> IniResult<()> {
    ini.init_from_ini(&mut n, table)?;
    list.nuggets.push(wrap(n));
    Ok(())
}

/// `INI::parseFXList`'s token: `None` (any case) is no list, else the name.
fn parse_fx_list_name(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let t = ini.get_next_token(None)?;
    Ok((!t.eq_ignore_ascii_case(b"None")).then_some(t))
}

/// Sound's fields.
#[derive(Default)]
struct SoundFields(Vec<u8>);
/// RayEffect's fields.
#[derive(Default)]
struct RayFields(Vec<u8>, Coord3D, Coord3D);
/// ViewShake's fields.
struct ShakeFields(i32);
/// TerrainScorch's fields.
struct ScorchFields(i32, f32);
/// FXListAtBonePos's fields.
struct BoneFields(Option<Vec<u8>>, Vec<u8>, bool);

/// `TheFXListFieldParse`: the nugget blocks.
fn fx_list_field_parse_table() -> Vec<FieldParse<FxList>> {
    vec![
        FieldParse::new(
            "Sound",
            |ini, l, _| parse_nugget(ini, l, SoundFields::default(), &[FieldParse::new("Name", |i, n: &mut SoundFields, _| i.parse_ascii_string().map(|v| n.0 = v), UserData::None)], |n| FxNugget::Sound { sound_name: n.0 }),
            UserData::None,
        ),
        FieldParse::new(
            "RayEffect",
            |ini, l, _| {
                parse_nugget(
                    ini,
                    l,
                    RayFields::default(),
                    &[
                        FieldParse::new("Name", |i, n: &mut RayFields, _| i.parse_ascii_string().map(|v| n.0 = v), UserData::None),
                        FieldParse::new("PrimaryOffset", |i, n: &mut RayFields, _| i.parse_coord3d().map(|v| n.1 = v), UserData::None),
                        FieldParse::new("SecondaryOffset", |i, n: &mut RayFields, _| i.parse_coord3d().map(|v| n.2 = v), UserData::None),
                    ],
                    |n| FxNugget::RayEffect { template_name: n.0, primary_offset: n.1, secondary_offset: n.2 },
                )
            },
            UserData::None,
        ),
        FieldParse::new(
            "Tracer",
            |ini, l, _| {
                type T = TracerFxNugget;
                parse_nugget(
                    ini,
                    l,
                    T::default(),
                    &[
                        FieldParse::new("TracerName", |i, n: &mut T, _| i.parse_ascii_string().map(|v| n.tracer_name = v), UserData::None),
                        FieldParse::new("BoneName", |i, n: &mut T, _| i.parse_ascii_string().map(|v| n.bone_name = v), UserData::None),
                        FieldParse::new("Speed", |i, n: &mut T, _| i.parse_velocity_real().map(|v| n.speed = v), UserData::None),
                        FieldParse::new("DecayAt", |i, n: &mut T, _| i.parse_real().map(|v| n.decay_at = v), UserData::None),
                        FieldParse::new("Length", |i, n: &mut T, _| i.parse_real().map(|v| n.length = v), UserData::None),
                        FieldParse::new("Width", |i, n: &mut T, _| i.parse_real().map(|v| n.width = v), UserData::None),
                        FieldParse::new("Color", |i, n: &mut T, _| i.parse_rgb_color().map(|v| n.color = v), UserData::None),
                        FieldParse::new("Probability", |i, n: &mut T, _| i.parse_real().map(|v| n.probability = v), UserData::None),
                    ],
                    FxNugget::Tracer,
                )
            },
            UserData::None,
        ),
        FieldParse::new(
            "LightPulse",
            |ini, l, _| {
                type T = LightPulseFxNugget;
                parse_nugget(
                    ini,
                    l,
                    T::default(),
                    &[
                        FieldParse::new("Color", |i, n: &mut T, _| i.parse_rgb_color().map(|v| n.color = v), UserData::None),
                        FieldParse::new("Radius", |i, n: &mut T, _| i.parse_real().map(|v| n.radius = v), UserData::None),
                        FieldParse::new("RadiusAsPercentOfObjectSize", |i, n: &mut T, _| i.parse_percent_to_real().map(|v| n.bounding_circle_pct = v), UserData::None),
                        FieldParse::new("IncreaseTime", |i, n: &mut T, _| i.parse_duration_unsigned_int().map(|v| n.increase_frames = v), UserData::None),
                        FieldParse::new("DecreaseTime", |i, n: &mut T, _| i.parse_duration_unsigned_int().map(|v| n.decrease_frames = v), UserData::None),
                    ],
                    FxNugget::LightPulse,
                )
            },
            UserData::None,
        ),
        FieldParse::new(
            "ViewShake",
            |ini, l, _| {
                let t = [FieldParse::new("Type", |i, n: &mut ShakeFields, ud| i.parse_lookup_list(ud.lookup()).map(|v| n.0 = v), UserData::Lookup(SHAKE_TYPE_NAMES))];
                parse_nugget(ini, l, ShakeFields(SHAKE_NORMAL), &t, |n| FxNugget::ViewShake { shake: n.0 })
            },
            UserData::None,
        ),
        FieldParse::new(
            "TerrainScorch",
            |ini, l, _| {
                let t = [
                    FieldParse::new("Type", |i, n: &mut ScorchFields, ud| i.parse_lookup_list(ud.lookup()).map(|v| n.0 = v), UserData::Lookup(SCORCH_TYPE_NAMES)),
                    FieldParse::new("Radius", |i, n: &mut ScorchFields, _| i.parse_real().map(|v| n.1 = v), UserData::None),
                ];
                parse_nugget(ini, l, ScorchFields(-1, 0.0), &t, |n| FxNugget::TerrainScorch { scorch: n.0, radius: n.1 })
            },
            UserData::None,
        ),
        FieldParse::new(
            "ParticleSystem",
            |ini, l, _| {
                type T = ParticleSystemFxNugget;
                parse_nugget(
                    ini,
                    l,
                    T::default(),
                    &[
                        FieldParse::new("Name", |i, n: &mut T, _| i.parse_ascii_string().map(|v| n.name = v), UserData::None),
                        FieldParse::new("Count", |i, n: &mut T, _| i.parse_int().map(|v| n.count = v), UserData::None),
                        FieldParse::new("Offset", |i, n: &mut T, _| i.parse_coord3d().map(|v| n.offset = v), UserData::None),
                        FieldParse::new("Radius", |i, n: &mut T, _| i.parse_game_client_random_variable(&mut n.radius), UserData::None),
                        FieldParse::new("Height", |i, n: &mut T, _| i.parse_game_client_random_variable(&mut n.height), UserData::None),
                        FieldParse::new("InitialDelay", |i, n: &mut T, _| i.parse_game_client_random_variable(&mut n.delay), UserData::None),
                        FieldParse::new("RotateX", |i, n: &mut T, _| i.parse_angle_real().map(|v| n.rotate_x = v), UserData::None),
                        FieldParse::new("RotateY", |i, n: &mut T, _| i.parse_angle_real().map(|v| n.rotate_y = v), UserData::None),
                        FieldParse::new("RotateZ", |i, n: &mut T, _| i.parse_angle_real().map(|v| n.rotate_z = v), UserData::None),
                        FieldParse::new("OrientToObject", |i, n: &mut T, _| i.parse_bool().map(|v| n.orient_to_object = v), UserData::None),
                        FieldParse::new("Ricochet", |i, n: &mut T, _| i.parse_bool().map(|v| n.ricochet = v), UserData::None),
                        FieldParse::new("AttachToObject", |i, n: &mut T, _| i.parse_bool().map(|v| n.attach_to_object = v), UserData::None),
                        FieldParse::new("CreateAtGroundHeight", |i, n: &mut T, _| i.parse_bool().map(|v| n.create_at_ground_height = v), UserData::None),
                        FieldParse::new("UseCallersRadius", |i, n: &mut T, _| i.parse_bool().map(|v| n.use_callers_radius = v), UserData::None),
                    ],
                    FxNugget::ParticleSystem,
                )
            },
            UserData::None,
        ),
        FieldParse::new(
            "FXListAtBonePos",
            |ini, l, _| {
                let t = [
                    FieldParse::new("FX", |i, n: &mut BoneFields, _| parse_fx_list_name(i).map(|v| n.0 = v), UserData::None),
                    FieldParse::new("BoneName", |i, n: &mut BoneFields, _| i.parse_ascii_string().map(|v| n.1 = v), UserData::None),
                    FieldParse::new("OrientToBone", |i, n: &mut BoneFields, _| i.parse_bool().map(|v| n.2 = v), UserData::None),
                ];
                parse_nugget(ini, l, BoneFields(None, Vec::new(), true), &t, |n| FxNugget::FxListAtBonePos { fx: n.0, bone_name: n.1, orient_to_bone: n.2 })
            },
            UserData::None,
        ),
    ]
}

/// `FXListStore::parseFXListDefinition`: the list of that name, cleared, read.
fn parse_fx_list_definition(ini: &mut Ini<'_>, store: &mut FxListStore) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let fxl = store.fxmap.entry(name).or_default();
    fxl.nuggets.clear();
    let table = fx_list_field_parse_table();
    ini.init_from_ini(fxl, &table)
}

/// The block table of `FXList.ini`.
pub const FX_LIST_BLOCKS: &[BlockParse<FxListStore>] = &[BlockParse { token: "FXList", parse: parse_fx_list_definition }];

impl FxListStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// `INI::load` of one FX list file (`Data\INI\Default\FXList.ini`, then `Data\INI\FXList.ini`).
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, FX_LIST_BLOCKS, self)
    }

    /// `findFXList(name)`.
    pub fn find_fx_list(&self, name: &[u8]) -> Option<&FxList> {
        if name.eq_ignore_ascii_case(b"None") {
            return None;
        }
        self.fxmap.get(name)
    }

    /// `FXList::doFXPos(fxl, ...)` (the static form: nothing for no list) of the list named `name`.
    #[allow(clippy::too_many_arguments)]
    pub fn do_fx_pos<H: FxHost>(&self, name: &[u8], host: &H, play: &mut FxPlay<'_>, primary: Option<&Coord3D>, mtx: Option<&Matrix3D>, speed: f32, secondary: Option<&Coord3D>, override_radius: f32) {
        if let Some(l) = self.find_fx_list(name) {
            l.do_fx_pos(self, host, play, primary, mtx, speed, secondary, override_radius);
        }
    }

    /// `FXList::doFXObj(fxl, primary, secondary)` of the list named `name`.
    pub fn do_fx_obj<H: FxHost>(&self, name: &[u8], host: &H, play: &mut FxPlay<'_>, primary: Option<ObjectId>, secondary: Option<ObjectId>) {
        if let Some(l) = self.find_fx_list(name) {
            l.do_fx_obj(self, host, play, primary, secondary);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FX: &[u8] = b"FXList FX_Test\n\
  Sound\n    Name = ExplosionSmall\n  End\n\
  ParticleSystem\n    Name = Smoke\n    Count = 2\n    Offset = X:1 Y:0 Z:0\n    Radius = 5 5\n    InitialDelay = 100 100\n    OrientToObject = Yes\n  End\n\
  TerrainScorch\n    Type = RANDOM\n    Radius = 7\n  End\n\
  ViewShake\n    Type = STRONG\n  End\n\
  Tracer\n    Probability = 0.5\n  End\n\
  LightPulse\n    Color = R:255 G:0 B:0\n    Radius = 20\n    IncreaseTime = 100\n    DecreaseTime = 200\n  End\n\
  FXListAtBonePos\n    FX = None\n    BoneName = Muzzle\n  End\n\
End\n\
FXList FX_Empty\nEnd\n";

    struct Open;
    impl FxHost for Open {
        fn cell_clear_for_local_player(&self, _: &Coord3D) -> bool {
            true
        }
        fn object_fogged_for_local_player(&self, _: ObjectId) -> bool {
            false
        }
        fn object_position_and_transform(&self, _: ObjectId) -> Option<(Coord3D, Matrix3D)> {
            Some((Coord3D { x: 100.0, y: 50.0, z: 0.0 }, Matrix3D::IDENTITY))
        }
        fn object_bounding_circle_radius(&self, _: ObjectId) -> f32 {
            10.0
        }
        fn layer_height_for_destination(&self, _: &Coord3D) -> f32 {
            3.0
        }
        fn bone_positions(&self, _: ObjectId, _: &[u8], _: i32) -> Vec<(Coord3D, Matrix3D)> {
            Vec::new()
        }
    }

    /// *Predictions, written before the first run:* `FX_Test` has its 7 nuggets in file order; the particle nugget
    /// keeps the constructor's defaults where not given (height constant 0, rotations 0, not attached) and its delay
    /// is 100..100 uniform; the tracer keeps `GenericTracer`, length 10, white, with probability 0.5; the light pulse's
    /// times are 3 and 6 frames (100 and 200 ms at 30 a second, ceiling); the shake is `STRONG` (2), the scorch random
    /// (-1); `FX = None` is no list. `FX_Empty` is an empty list, and `findFXList("NONE")` is none.
    ///
    /// *Results (2026-11-06), first run:* all held.
    #[test]
    fn fx_lists_parse_their_nuggets() {
        let mut s = FxListStore::new();
        s.load_text(b"Data\\INI\\FXList.ini", FX).expect("parses");
        let l = s.find_fx_list(b"FX_Test").unwrap();
        assert_eq!(l.nuggets.len(), 7);
        assert_eq!(l.nuggets[0], FxNugget::Sound { sound_name: b"ExplosionSmall".to_vec() });
        let FxNugget::ParticleSystem(p) = &l.nuggets[1] else { panic!() };
        assert_eq!((p.name.as_slice(), p.count, p.offset.x, p.radius.low, p.orient_to_object), (b"Smoke".as_slice(), 2, 1.0, 5.0, true));
        assert_eq!((p.height, p.delay), (GameClientRandomVariable::new(0.0, 0.0, DistributionType::Constant), GameClientRandomVariable::new(100.0, 100.0, DistributionType::Uniform)));
        assert!(!p.attach_to_object && !p.ricochet && p.rotate_x == 0.0);
        assert_eq!(l.nuggets[2], FxNugget::TerrainScorch { scorch: -1, radius: 7.0 });
        assert_eq!(l.nuggets[3], FxNugget::ViewShake { shake: 2 });
        let FxNugget::Tracer(t) = &l.nuggets[4] else { panic!() };
        assert_eq!((t.tracer_name.as_slice(), t.length, t.probability, t.color.red), (b"GenericTracer".as_slice(), 10.0, 0.5, 1.0));
        let FxNugget::LightPulse(lp) = &l.nuggets[5] else { panic!() };
        assert_eq!((lp.increase_frames, lp.decrease_frames, lp.radius), (3, 6, 20.0));
        assert_eq!(l.nuggets[6], FxNugget::FxListAtBonePos { fx: None, bone_name: b"Muzzle".to_vec(), orient_to_bone: true });
        assert!(s.find_fx_list(b"FX_Empty").unwrap().nuggets.is_empty());
        assert!(s.find_fx_list(b"NONE").is_none());
    }

    /// *Predictions, written before the first run:* playing `FX_Test` on an object at (100, 50, 0) with the identity
    /// transform makes 2 `Smoke` systems (ids 1 and 2), each placed 5 from (101, 50) on a random angle at z 0, with a
    /// ~~3-frame~~ initial delay (100 ms); then the sound at the object, a random scorch type in 0..=3 of radius 7, a
    /// strong shake, the light pulse at the object; the tracer has no secondary, so it only draws its probability.
    /// A second play from the same seed gives the same systems. With the primary fogged, nothing.
    ///
    /// *Results (2026-11-06):* all held but the delay: 4 frames, not 3. `REAL_TO_INT_CEIL` is upstream's
    /// `fast_float_ceil`, which adds `ALMOST1` and truncates: 3.0 + 0.99999994 rounds to 4.0 in `float`, so a whole
    /// number of frames comes out one more (the INI's `parseDurationUnsignedInt` uses `ceilf` and gives 3).
    #[test]
    fn playing_an_fx_list_makes_its_systems_and_requests() {
        let mut parts = ParticleSystemManager::new();
        parts.load_text(b"p.ini", b"ParticleSystem Smoke\n  Lifetime = 10 10\n  BurstCount = 1 1\nEnd\n").unwrap();
        let mut s = FxListStore::new();
        s.load_text(b"f.ini", FX).unwrap();
        let run = |parts: &mut ParticleSystemManager| {
            let mut r = RandomValues::new(11);
            let mut req = Vec::new();
            let mut play = FxPlay { random: &mut r, particles: parts, frame: 0, requests: &mut req };
            s.do_fx_obj(b"FX_Test", &Open, &mut play, Some(5), None);
            req
        };
        let req = run(&mut parts);
        assert_eq!(parts.systems.len(), 2);
        for sys in &parts.systems {
            let p = sys.get_position();
            let d = crate::ww::wwmath::sqrt((p.x - 101.0) * (p.x - 101.0) + (p.y - 50.0) * (p.y - 50.0));
            assert!((d - 5.0).abs() < 1e-4, "{d}");
            assert_eq!((p.z, sys.delay_left), (0.0, 4));
        }
        assert!(matches!(&req[0], FxRequest::Sound { position: Some(p), .. } if p.x == 100.0));
        assert!(matches!(req[1], FxRequest::TerrainScorch { scorch: 0..=3, radius: 7.0, .. }));
        assert!(matches!(req[2], FxRequest::ViewShake { shake: 2, .. }));
        assert!(matches!(req[3], FxRequest::LightPulse { radius: 20.0, .. }));
        assert_eq!(req.len(), 4);
        let first: Vec<_> = parts.systems.iter().map(|s| s.get_position()).collect();
        let mut again = ParticleSystemManager::new();
        again.templates = parts.templates.clone();
        run(&mut again);
        assert_eq!(again.systems.iter().map(|s| s.get_position()).collect::<Vec<_>>(), first);

        struct Fogged;
        impl FxHost for Fogged {
            fn cell_clear_for_local_player(&self, _: &Coord3D) -> bool {
                false
            }
            fn object_fogged_for_local_player(&self, _: ObjectId) -> bool {
                true
            }
            fn object_position_and_transform(&self, o: ObjectId) -> Option<(Coord3D, Matrix3D)> {
                Open.object_position_and_transform(o)
            }
            fn object_bounding_circle_radius(&self, _: ObjectId) -> f32 {
                0.0
            }
            fn layer_height_for_destination(&self, _: &Coord3D) -> f32 {
                0.0
            }
            fn bone_positions(&self, _: ObjectId, _: &[u8], _: i32) -> Vec<(Coord3D, Matrix3D)> {
                Vec::new()
            }
        }
        let mut none = ParticleSystemManager::new();
        none.templates = parts.templates.clone();
        let mut r = RandomValues::new(11);
        let mut req = Vec::new();
        let mut play = FxPlay { random: &mut r, particles: &mut none, frame: 0, requests: &mut req };
        s.do_fx_obj(b"FX_Test", &Fogged, &mut play, Some(5), None);
        s.do_fx_pos(b"FX_Test", &Fogged, &mut play, Some(&Coord3D::default()), None, 0.0, None, 0.0);
        assert!(req.is_empty() && none.systems.is_empty());
    }
}
