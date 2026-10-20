// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SmartBombTargetHomingUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SmartBombTargetHomingUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Mark Lorenzen, July 2003
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SmartBombTargetHomingUpdate`: nudges a falling object's horizontal
//! position towards the target it was given, every frame, while the object
//! is significantly above the terrain. The height is left alone.
//!
//! The course correction is the share of the current position kept each
//! frame: `CourseCorrectionScalar` (clamped to [0, 1]) of the current `x` and
//! `y`, and the rest of the target's.

use std::sync::Arc;

use crate::common::base_type::{max, min, Coord3D};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::UpdateHost;

/// The calls the module makes on its object, besides [`UpdateHost`]'s.
pub trait SmartBombHost: UpdateHost {
    /// `getObject()->isSignificantlyAboveTerrain()`.
    fn is_significantly_above_terrain(&self) -> bool;
    /// `getObject()->getPosition()`.
    fn position(&self) -> Coord3D;
    /// `getObject()->setPosition(&pos)`.
    fn set_position(&mut self, pos: Coord3D);
}

/// `class SmartBombTargetHomingUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct SmartBombTargetHomingUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_courseCorrectionScalar` (`CourseCorrectionScalar`; 0.99 by default).
    pub course_correction_scalar: f32,
}

impl Default for SmartBombTargetHomingUpdateModuleData {
    /// `SmartBombTargetHomingUpdateModuleData()`: 0.99.
    fn default() -> Self {
        SmartBombTargetHomingUpdateModuleData { update: UpdateModuleData::default(), course_correction_scalar: 0.99 }
    }
}

impl SmartBombTargetHomingUpdateModuleData {
    /// `SmartBombTargetHomingUpdateModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `buildFieldParse(p)`: the base's, then `CourseCorrectionScalar`.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SmartBombTargetHomingUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&SMART_BOMB_TARGET_HOMING_UPDATE_MODULE_DATA_FIELD_PARSE)
    }

    /// `SmartBombTargetHomingUpdate::friend_newModuleData(ini)`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SmartBombTargetHomingUpdateModuleData> {
        let mut data = SmartBombTargetHomingUpdateModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SmartBombTargetHomingUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `dataFieldParse` of the class: `CourseCorrectionScalar`, `INI::parseReal`.
pub static SMART_BOMB_TARGET_HOMING_UPDATE_MODULE_DATA_FIELD_PARSE: [FieldParse<SmartBombTargetHomingUpdateModuleData>; 2] = [
    FieldParse::new(
        "CourseCorrectionScalar",
        |ini, d, _: &UserData| {
            d.course_correction_scalar = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class SmartBombTargetHomingUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct SmartBombTargetHomingUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getSmartBombTargetHomingUpdateModuleData()`.
    data: Arc<SmartBombTargetHomingUpdateModuleData>,
    /// `m_targetReceived`.
    target_received: bool,
    /// `m_target`.
    target: Coord3D,
}

impl SmartBombTargetHomingUpdate {
    /// `SmartBombTargetHomingUpdate(thing, moduleData)`: no target yet, due
    /// next frame. `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<SmartBombTargetHomingUpdateModuleData>>, host: &mut impl UpdateHost) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), UPDATE_SLEEP_NONE));
        Ok(SmartBombTargetHomingUpdate { update, data, target_received: false, target: Coord3D::default() })
    }

    /// `SetTargetPosition(target)`: a target of length zero (or NaN) is
    /// refused; anything else is the target from now on.
    pub fn set_target_position(&mut self, target: Coord3D) {
        let length = target.length();
        if length.is_nan() || length <= 0.0 {
            return;
        }
        self.target = target;
        self.target_received = true;
    }

    /// `update()`: while there is a target and the object is above the
    /// terrain, move its `x` and `y` towards the target by the course
    /// correction, and keep its height. Always due the next frame.
    pub fn update(&mut self, host: &mut impl SmartBombHost) -> UpdateSleepTime {
        if !self.target_received {
            return UPDATE_SLEEP_NONE;
        }
        if !host.is_significantly_above_terrain() {
            return UPDATE_SLEEP_NONE;
        }
        let current = host.position();
        let status_coeff = max(0.0, min(1.0, self.data.course_correction_scalar));
        let target_coeff = 1.0 - status_coeff;
        let pos = Coord3D {
            x: self.target.x * target_coeff + current.x * status_coeff,
            y: self.target.y * target_coeff + current.y * status_coeff,
            z: current.z,
        };
        host.set_position(pos);
        UPDATE_SLEEP_NONE
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getSmartBombTargetHomingUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<SmartBombTargetHomingUpdateModuleData> {
        &self.data
    }

    /// Whether a target has been set (`m_targetReceived`).
    pub fn target_received(&self) -> bool {
        self.target_received
    }

    /// The target (`m_target`).
    pub fn target(&self) -> Coord3D {
        self.target
    }
}

impl Snapshot for SmartBombTargetHomingUpdate {
    /// `SmartBombTargetHomingUpdate::crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// Version 1: the base.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.update.xfer(xfer)
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}
