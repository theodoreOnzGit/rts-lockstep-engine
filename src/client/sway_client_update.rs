// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable/Update/SwayClientUpdate.cpp (SwayClientUpdate: its
//     constructor, updateSway, clientUpdate, crc, xfer, loadPostProcess)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Module/SwayClientUpdate.h (stopSway)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc. (Matthew D. Campbell, May 2002)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The tree sway client update module (GitHub #165): `SwayClientUpdate`.
//!
//! # As upstream does it
//!
//! - **Its data** is the base `ModuleData`'s: no field of its own (`ClientUpdate = SwayClientUpdate`).
//! - **Made** (SwayClientUpdate.cpp:54-69) at rest, swaying, with `m_curVersion` -1 "so that we never match the first
//!   time"; the sway is not computed until the first update.
//! - **`updateSway`** (SwayClientUpdate.cpp:81-93): from the script engine's breeze (`getBreezeInfo`), with
//!   `delta = randomness * 0.5`: no randomness resets the phase; the angle limit is the intensity, the phase step
//!   `2 PI / period` and the lean the breeze's lean, each times its own `GameClientRandomValueReal(1 - delta, 1 +
//!   delta)` (in that order); the version is the breeze's.
//! - **`clientUpdate`** (SwayClientUpdate.cpp:101-142): nothing once stopped. A new breeze version always updates the
//!   sway, even for a drawable not seen ("so that things offscreen won't 'pop'"); otherwise only a visible drawable
//!   sways. The phase steps (less `2 PI` past `2 PI`); the target angle is `Cos(phase) * limit + lean`; the drawable's
//!   instance matrix is pre-rotated about X by `-delta * dir.x` and about Y by `delta * dir.y` (`delta` the change of
//!   angle); a burned object stops swaying.
//! - **`stopSway`** (SwayClientUpdate.h): the logic's `ToppleUpdate` stops every `SwayClientUpdate` of a toppling
//!   tree's drawable.
//! - **The save** (`xfer`, version 1, SwayClientUpdate.cpp:160-192): the base's (`DrawableModule`, `Module`), the phase,
//!   the angle, the phase step, the limit, the lean, the version (a `Short`) and the swaying flag (`m_unused` is not
//!   saved). **`loadPostProcess`** (SwayClientUpdate.cpp:197-205) recomputes the sway (`updateSway`, drawing the
//!   client's random numbers again). `crc` adds nothing.
//!
//! # Upstream quirks that are kept
//!
//! - A zero breeze period divides by zero: the phase step is infinite (`Real` arithmetic).
//! - The version is a `Short` compared as such: a breeze version of -1 matches a module never updated.
//!
//! # Undefined upstream ([`SwayUndefined`])
//!
//! - A new breeze version with no drawable reads the instance matrix through `NULL` (the module of a drawable always
//!   has one here; the host answers).
//!
//! # Where this differs from upstream
//!
//! - **The host** ([`SwayHost`]) answers what upstream reads of the script engine (the World's, read only), the
//!   client's random numbers, the drawable and its object.

use crate::common::base_type::PI;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult};
use crate::common::thing_template::ClientModuleData;
use crate::common::object_status_types::ObjectStatusTypes;
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::client::laser_update::drawable_module_base_xfer;
use crate::logic::modules::plant;
use crate::logic::world::script_engine::BreezeInfo;
use crate::ww::matrix3d::Matrix3D;

/// The module's name (`ClientUpdate = SwayClientUpdate`).
pub const SWAY_CLIENT_UPDATE: &[u8] = b"SwayClientUpdate";

/// `OBJECT_STATUS_BURNED`, the status that stops a sway.
pub const BURNED: ObjectStatusTypes = crate::common::object_status_types::OBJECT_STATUS_BURNED;

/// The base `ModuleData`'s field table (`ModuleData::buildFieldParse` adds none): the data of `SwayClientUpdate` and
/// `AnimatedParticleSysBoneClientUpdate`.
static NO_FIELDS: [FieldParse<()>; 1] = [FieldParse::end()];

fn parse_base_block(ini: &mut Ini<'_>, d: &mut ()) -> IniResult<()> {
    ini.init_from_ini(d, &NO_FIELDS)
}

/// (#165) A client update module's data that is the base `ModuleData`'s, from the `ClientUpdate` block the logic keeps
/// as tokens: any field is the INI error upstream's `initFromINI` gives for an unknown field.
pub fn parse_base_module_data(block: &ClientModuleData) -> IniResult<()> {
    let mut text = b"ClientUpdate".to_vec();
    text.extend_from_slice(&crate::client::w3d_model_draw::client_block_text(block)[b"Draw".len()..]);
    let blocks = [BlockParse { token: "ClientUpdate", parse: parse_base_block }];
    Ini::load(b"ClientUpdate", &text, IniLoadType::Overwrite, None, &blocks, &mut ())
}

/// What upstream's `SwayClientUpdate` leaves undefined (module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwayUndefined {
    /// `draw->getInstanceMatrix()` with `draw` `NULL`.
    NullDrawable,
}

/// What `SwayClientUpdate` reads and calls besides itself.
pub trait SwayHost {
    /// `TheScriptEngine->getBreezeInfo()`.
    fn breeze_info(&self) -> BreezeInfo;
    /// `GameClientRandomValueReal(lo, hi)`.
    fn game_client_random_value_real(&mut self, lo: f32, hi: f32) -> f32;
    /// `getDrawable() != NULL`.
    fn has_drawable(&self) -> bool;
    /// `draw->isVisible()` (the drawable exists).
    fn is_visible(&self) -> bool;
    /// `*draw->getInstanceMatrix()`.
    fn instance_matrix(&self) -> Matrix3D;
    /// `draw->setInstanceMatrix(&m)`.
    fn set_instance_matrix(&mut self, m: &Matrix3D);
    /// `obj && obj->getStatusBits().test(OBJECT_STATUS_BURNED)` of the drawable's object.
    fn object_burned(&self) -> bool;
}

/// `class SwayClientUpdate`.
#[derive(Clone, Debug, PartialEq)]
pub struct SwayClientUpdate {
    /// `m_curValue`, `m_curAngle`, `m_curDelta`, `m_curAngleLimit`, `m_leanAngle`.
    pub cur_value: f32,
    pub cur_angle: f32,
    pub cur_delta: f32,
    pub cur_angle_limit: f32,
    pub lean_angle: f32,
    /// `m_curVersion`.
    pub cur_version: i16,
    /// `m_swaying`, `m_unused`.
    pub swaying: bool,
    pub unused: bool,
}

impl Default for SwayClientUpdate {
    fn default() -> Self {
        Self::new()
    }
}

impl SwayClientUpdate {
    /// `SwayClientUpdate(thing, moduleData)` (SwayClientUpdate.cpp:54-69).
    pub fn new() -> Self {
        SwayClientUpdate { cur_value: 0.0, cur_angle: 0.0, cur_delta: 0.0, cur_angle_limit: 0.0, lean_angle: 0.0, cur_version: -1, swaying: true, unused: false }
    }

    /// `updateSway()` (SwayClientUpdate.cpp:81-93).
    pub fn update_sway(&mut self, host: &mut impl SwayHost) {
        let info = host.breeze_info();
        if info.randomness == 0.0 {
            self.cur_value = 0.0;
        }
        let delta = info.randomness * if plant(9601) { 0.25 } else { 0.5 };
        self.cur_angle_limit = info.intensity * host.game_client_random_value_real(1.0 - delta, 1.0 + delta);
        let step = if plant(9602) { PI / info.breeze_period as f32 } else { 2.0 * PI / info.breeze_period as f32 };
        self.cur_delta = step * host.game_client_random_value_real(1.0 - delta, 1.0 + delta);
        self.lean_angle = if plant(9603) { info.intensity } else { info.lean } * host.game_client_random_value_real(1.0 - delta, 1.0 + delta);
        self.cur_version = info.breeze_version;
    }

    /// `clientUpdate()` (SwayClientUpdate.cpp:101-142).
    pub fn client_update(&mut self, host: &mut impl SwayHost) -> Result<(), SwayUndefined> {
        if !self.swaying {
            return Ok(());
        }
        // if breeze changes, always process the full update, even if not visible, so that things offscreen won't 'pop'
        // when first viewed
        let info = host.breeze_info();
        if info.breeze_version != self.cur_version {
            self.update_sway(host);
            if !host.has_drawable() {
                return Err(SwayUndefined::NullDrawable);
            }
        } else if !host.has_drawable() || (!host.is_visible() && !plant(9604)) {
            // Otherwise, only update visible drawables
            return Ok(());
        }
        self.cur_value += self.cur_delta;
        let limit = if plant(9605) { PI } else { 2.0 * PI };
        if self.cur_value > limit {
            self.cur_value -= 2.0 * PI;
        }
        let cosine = if plant(9606) { crate::common::trig::sin(self.cur_value) } else { crate::common::trig::cos(self.cur_value) };
        let target_angle = cosine * self.cur_angle_limit + self.lean_angle;
        let delta_angle = target_angle - self.cur_angle;
        let mut xfrm = host.instance_matrix();
        if plant(9607) {
            xfrm.in_place_pre_rotate_x(delta_angle * info.direction_vec.x);
        } else {
            xfrm.in_place_pre_rotate_x(-delta_angle * info.direction_vec.x);
        }
        xfrm.in_place_pre_rotate_y(delta_angle * if plant(9608) { info.direction_vec.x } else { info.direction_vec.y });
        host.set_instance_matrix(&xfrm);
        self.cur_angle = if plant(9609) { delta_angle } else { target_angle };
        // burned things don't sway.
        if host.object_burned() {
            self.stop_sway();
        }
        Ok(())
    }

    /// `stopSway()` (SwayClientUpdate.h).
    pub fn stop_sway(&mut self) {
        self.swaying = false;
    }

    /// `xfer(xfer)` (version 1, SwayClientUpdate.cpp:160-192).
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        drawable_module_base_xfer(xfer)?;
        xfer.xfer_real(&mut self.cur_value)?;
        if plant(9610) {
            xfer.xfer_real(&mut self.cur_delta)?;
            xfer.xfer_real(&mut self.cur_angle)?;
        } else {
            xfer.xfer_real(&mut self.cur_angle)?;
            xfer.xfer_real(&mut self.cur_delta)?;
        }
        xfer.xfer_real(&mut self.cur_angle_limit)?;
        xfer.xfer_real(&mut self.lean_angle)?;
        xfer.xfer_short(&mut self.cur_version)?;
        xfer.xfer_bool(&mut self.swaying)?;
        Ok(())
    }

    /// `loadPostProcess()` (SwayClientUpdate.cpp:197-205).
    pub fn load_post_process(&mut self, host: &mut impl SwayHost) {
        self.update_sway(host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::base_type::Coord2D;
    use crate::common::random_value::RandomValues;

    struct Host {
        breeze: BreezeInfo,
        random: RandomValues,
        visible: bool,
        m: Matrix3D,
        sets: u32,
        burned: bool,
    }

    impl SwayHost for Host {
        fn breeze_info(&self) -> BreezeInfo {
            self.breeze
        }
        fn game_client_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
            self.random.game_client_random_value_real(lo, hi)
        }
        fn has_drawable(&self) -> bool {
            true
        }
        fn is_visible(&self) -> bool {
            self.visible
        }
        fn instance_matrix(&self) -> Matrix3D {
            self.m
        }
        fn set_instance_matrix(&mut self, m: &Matrix3D) {
            self.m = *m;
            self.sets += 1;
        }
        fn object_burned(&self) -> bool {
            self.burned
        }
    }

    /// Predicted: the first update takes the breeze even off screen; then only a visible drawable sways; with no
    /// randomness the step is exactly `2 PI / period`; a burned object stops; the save is 26 bytes and round-trips.
    #[test]
    fn a_tree_sways_with_the_breeze() {
        let breeze = BreezeInfo { direction_vec: Coord2D { x: 0.5, y: 0.75 }, intensity: 0.1, lean: 0.05, randomness: 0.0, breeze_period: 150, breeze_version: 0, ..Default::default() };
        let mut h = Host { breeze, random: RandomValues::new(1), visible: false, m: Matrix3D::IDENTITY, sets: 0, burned: false };
        let mut s = SwayClientUpdate::new();
        s.client_update(&mut h).unwrap();
        assert_eq!((s.cur_version, s.cur_delta, h.sets), (0, 2.0 * PI / 150.0, 1));
        s.client_update(&mut h).unwrap();
        assert_eq!(h.sets, 1);
        h.visible = true;
        s.client_update(&mut h).unwrap();
        assert_eq!(h.sets, 2);
        h.burned = true;
        s.client_update(&mut h).unwrap();
        assert!(!s.swaying);
        s.client_update(&mut h).unwrap();
        assert_eq!(h.sets, 3);
        let mut x = crate::common::xfer_save::XferSave::new();
        x.open(b"t").unwrap();
        s.xfer(&mut x).unwrap();
        assert_eq!(x.bytes().len(), 26);
        let mut back = SwayClientUpdate::new();
        let mut l = crate::common::xfer_load::XferLoad::new(x.bytes().to_vec());
        l.open(b"t").unwrap();
        back.xfer(&mut l).unwrap();
        assert_eq!(back, s);
    }
}
