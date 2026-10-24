// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CrushDie.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CrushDie.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CrushDie`: a vehicle crushed to death. Only crush damage counts; the crush point nearest the crusher (the middle while neither end is crushed, the
//! front and the back half the major radius along the vehicle's facing, an end that completes the other's crush counting as a total crush) decides
//! the crush (`crushLocationCheck`; total when the crusher is gone). The crush's sound plays with its percentage (a logic random draw, 0 to 99, only
//! when the sound has a name), the body's front and back crushed flags are set, and the drawable's two crushed conditions follow them.
//!
//! Upstream facts kept: the vehicle's body module is read unchecked, in `crushLocationCheck` and after it ([`Undefined::NullBodyModule`]); so is its
//! drawable ([`Undefined::NullDrawable`]); the distances are squared in single precision; `NO_CRUSH` (no crush point left) does nothing.

use std::sync::Arc;

use crate::common::audio_event_name::AudioEventName;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{MODELCONDITION_BACKCRUSHED, MODELCONDITION_FRONTCRUSHED};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_CRUSH};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::modules::die_body::{die_module_call, DieBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use crate::logic::world::update_body::UpdateBody;

/// `enum CrushEnum`.
pub const TOTAL_CRUSH: usize = 0;
/// `BACK_END_CRUSH`.
pub const BACK_END_CRUSH: usize = 1;
/// `FRONT_END_CRUSH`.
pub const FRONT_END_CRUSH: usize = 2;
/// `NO_CRUSH`.
pub const NO_CRUSH: usize = 3;
/// `CRUSH_COUNT`.
pub const CRUSH_COUNT: usize = 4;

/// `class CrushDieModuleData : public DieModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CrushDieModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
    /// `m_crushSounds`, by `CrushEnum`.
    pub crush_sounds: [AudioEventName; CRUSH_COUNT],
    /// `m_crushSoundPercent`, by `CrushEnum` (100 each).
    pub crush_sound_percent: [i32; CRUSH_COUNT],
}

impl CrushDieModuleData {
    /// `CrushDieModuleData()`.
    pub fn new() -> Self {
        CrushDieModuleData { die: DieModuleData::new(), crush_sounds: Default::default(), crush_sound_percent: [100; CRUSH_COUNT] }
    }

    /// `buildFieldParse(p)`: the behaviour base, the four die mux fields (read from the layer-5 table into `die`), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CrushDieModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CrushDieModuleData> {
        let mut data = CrushDieModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CrushDieModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `CrushDieModuleData::die`.
fn lift_die_field(k: usize, i: &mut Ini<'_>, d: &mut CrushDieModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `DieModuleData::buildFieldParse`'s fields, then `CrushDieModuleData`'s.
static FIELD_PARSE: [FieldParse<CrushDieModuleData>; 11] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die_field(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die_field(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die_field(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die_field(3, i, d, u), UserData::None),
    FieldParse::new("TotalCrushSound", |ini, d, _| d.crush_sounds[TOTAL_CRUSH].parse(ini), UserData::None),
    FieldParse::new("BackEndCrushSound", |ini, d, _| d.crush_sounds[BACK_END_CRUSH].parse(ini), UserData::None),
    FieldParse::new("FrontEndCrushSound", |ini, d, _| d.crush_sounds[FRONT_END_CRUSH].parse(ini), UserData::None),
    FieldParse::new("TotalCrushSoundPercent", |ini, d, _| { d.crush_sound_percent[TOTAL_CRUSH] = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("BackEndCrushSoundPercent", |ini, d, _| { d.crush_sound_percent[BACK_END_CRUSH] = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("FrontEndCrushSoundPercent", |ini, d, _| { d.crush_sound_percent[FRONT_END_CRUSH] = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class CrushDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct CrushDie {
    die: DieModule,
    /// `getCrushDieModuleData()`.
    data: Arc<CrushDieModuleData>,
}

impl PartialEq for CrushDie {
    /// The same object, the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl CrushDie {
    /// `CrushDie(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<CrushDieModuleData>>) -> IniResult<CrushDie> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(CrushDie { die, data })
    }

    /// The data, as `getCrushDieModuleData()`.
    pub fn module_data(&self) -> &Arc<CrushDieModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        die_module_call(self, world, me, call)
    }

    /// `static crushLocationCheck(crusherObject, victimObject)`: the crush point nearest the crusher. An error is upstream's undefined behaviour (the
    /// victim has no body module).
    pub fn crush_location_check(world: &mut World, crusher: ObjectId, victim: ObjectId) -> Result<usize, IniError> {
        if world.objects.find(crusher).is_none() || world.objects.find(victim).is_none() {
            return Ok(NO_CRUSH);
        }
        let body = world.get_body_module(victim).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
        let front_crushed = world.module_call(victim, body, ModuleCall::GetFrontCrushed).as_bool();
        let back_crushed = world.module_call(victim, body, ModuleCall::GetBackCrushed).as_bool();
        let other_dir = world.objects.find_mut(victim).expect("checked above").thing.unit_direction_vector_2d();
        let pos = world.objects.find(crusher).expect("checked above").get_position();
        let (other_pos, major) = {
            let o = world.objects.find(victim).expect("checked above");
            (o.get_position(), o.geometry_info.get_major_radius())
        };
        // `getMajorRadius() * 0.5`: a double product stored in a Real (exact)
        let offset_distance = (f64::from(major) * if crate::logic::modules::plant(1761) { 1.0 } else { 0.5 }) as f32;
        let (off_x, off_y) = (other_dir.x * offset_distance, other_dir.y * offset_distance);
        let dist = |x: f32, y: f32| {
            let (dx, dy) = (x - pos.x, y - pos.y);
            dx * dx + dy * dy
        };
        let mut retval = NO_CRUSH;
        let mut best_dist = 99999.0f32;
        // "PhysicsCollide has already done the logic of which point to smoosh ... we just need to know which crush point is closest."
        if !front_crushed && !back_crushed {
            // Check the middle crush point
            retval = TOTAL_CRUSH;
            best_dist = dist(other_pos.x, other_pos.y);
        }
        if !front_crushed {
            // Check the front point.
            let d = dist(other_pos.x + off_x, other_pos.y + off_y);
            if d < best_dist {
                retval = if back_crushed { TOTAL_CRUSH } else { FRONT_END_CRUSH };
                best_dist = d;
            }
        }
        if !back_crushed && !crate::logic::modules::plant(1762) {
            // Check back point
            let d = dist(other_pos.x - off_x, other_pos.y - off_y);
            if d < best_dist {
                retval = if front_crushed { TOTAL_CRUSH } else { BACK_END_CRUSH };
            }
        }
        Ok(retval)
    }

    /// The body of `onDie` after the applicability test. An error is upstream's undefined behaviour.
    fn crush(&self, world: &mut World, me: ObjectId, info: &DamageInfo) -> Result<(), IniError> {
        if info.input.damage_type != DAMAGE_CRUSH && !crate::logic::modules::plant(1763) {
            return Ok(());
        }
        let dealer = if world.objects.find(info.input.source_id).is_some() { info.input.source_id } else { INVALID_ID };
        let crush_type = if dealer != INVALID_ID { Self::crush_location_check(world, dealer, me)? } else { TOTAL_CRUSH };
        if crush_type == NO_CRUSH {
            return Ok(());
        }
        let sound = &self.data.crush_sounds[crush_type];
        if !sound.event_name.is_empty() {
            // "be sure that 0==never, 100==always"
            let draw = world.random.game_logic_random_value(0, 99);
            if draw < self.data.crush_sound_percent[crush_type] && !crate::logic::modules::plant(1764) {
                world.emit(ObjectClientEvent::AudioAddEvent { name: sound.event_name.clone(), object_id: me, position: None, player_index: None });
            }
        }
        let front = crush_type == TOTAL_CRUSH || crush_type == FRONT_END_CRUSH;
        let back = (crush_type == TOTAL_CRUSH || crush_type == BACK_END_CRUSH) != crate::logic::modules::plant(1765);
        let body = world.get_body_module(me).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
        world.module_call(me, body, ModuleCall::SetFrontCrushed(front));
        world.module_call(me, body, ModuleCall::SetBackCrushed(back));
        if !world.has_drawable(me) {
            return Err(IniError::undefined(Undefined::NullDrawable));
        }
        let set: Vec<i32> = [(MODELCONDITION_FRONTCRUSHED, front), (MODELCONDITION_BACKCRUSHED, back)].iter().filter(|(_, on)| *on).map(|(c, _)| *c).collect();
        let mut set = set;
        set.sort_unstable();
        let mut clear = vec![MODELCONDITION_BACKCRUSHED, MODELCONDITION_FRONTCRUSHED];
        clear.sort_unstable();
        world.emit(ObjectClientEvent::DrawableModelCondition { object_id: me, op: ModelConditionOp::ClearAndSet { clear, set } });
        Ok(())
    }
}

impl DieBehavior for CrushDie {
    fn die(&self) -> &DieModule {
        &self.die
    }
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.is_die_applicable(world, me, info) && !crate::logic::modules::plant(1766) {
            return;
        }
        if let Err(e) = self.crush(world, me.object, info) {
            world.cuts.module_errors.push((me, e));
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CrushDie(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CrushDie(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for CrushDie {
    /// `crc`: `DieModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.die.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `DieModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.die.xfer(xfer)?;
        Ok(())
    }
    /// `loadPostProcess`: `DieModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.die.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `CrushDie`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CrushDie(Arc::new(CrushDieModuleData::friend_new_module_data(ini)?)))
}
