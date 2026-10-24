// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/UpdateModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/UpdateModule.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, September 2001 (sleepy updates: Steven Johnson)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `UpdateModule`: the base of every module that is called once a logic
//! frame, or, for a "sleepy" one, only on the frame it asked to be woken at.
//!
//! # The sleepy-update bookkeeping
//!
//! An update module keeps **one `UnsignedInt`**, `m_nextCallFrameAndPhase`:
//! the frame it next wants to be called on in the upper 30 bits, and the
//! [`SleepyUpdatePhase`] in the lower two. The game logic's scheduler orders
//! modules by that whole number ([`UpdateModule::friend_get_priority`]), so
//! within a frame all phase 0 are called before all phase 1, and so on. A
//! frame past [`UPDATE_SLEEP_FOREVER`] (`0x3fffffff`, "~414 days at 30 fps")
//! is cut to it, so that adding offsets cannot overflow once shifted up by
//! two bits. `update()` returns an [`UpdateSleepTime`]: the number of frames
//! to sleep ([`UPDATE_SLEEP_NONE`] = 1 = call again next frame).
//!
//! The virtual `getUpdatePhase()` ("you should pretty much never redefine
//! this") is a constant of each concrete class (`PHASE_PHYSICS` for
//! `PhysicsBehavior`, `PHASE_INITIAL` for `AIUpdateInterface`, `PHASE_NORMAL`
//! for the rest), so the Rust takes it as a constructor argument
//! ([`UpdateModule::new`]) and stores it.
//!
//! # Cut points (layer 8: `TheGameLogic`)
//!
//! Three members read `TheGameLogic->getFrame()`, and `setWakeFrame` calls
//! `TheGameLogic->friend_awakenUpdateModule(obj, this, frame)` (which
//! re-sorts the module in the scheduler). The Rust takes the current frame
//! as an argument (`now`), and [`UpdateModule::set_wake_frame`] returns the
//! frame the caller must hand to `friend_awakenUpdateModule`.
//!
//! # Where upstream is "debug only"
//!
//! `UpdateModule::xfer` has a `FIX_OLD_SAVES` block, compiled only under
//! `_DEBUG` or `_INTERNAL` (and, inside it, `ALLOW_NONSLEEPY_UPDATES`, which
//! `GameLogic.h` turns off): an old save of a module that has since become
//! sleepy gets `friend_setNextCallFrame(now)`. The shipped build has none of
//! it, and neither has this translation. The `DEBUG_ASSERTCRASH`s
//! (`frameToSleepTime`'s zero check, the destructor's "still in the logic
//! list") and `DEBUG_CRASH` ("frame is in the past") are likewise compiled
//! out; the past frame returns `UPDATE_SLEEP_NONE`.
//!
//! # Left out
//!
//! `UpdateModuleInterface::update()` is pure virtual (each concrete module's
//! own `update`); `friend_getObject` is [`BehaviorModule::object`].
//! `SlavedUpdateInterface`, `ProjectileUpdateInterface`, `DockUpdateInterface`,
//! `ExitInterface`, `ExitDoorType` and `UpdateModulePtr` (the typedef
//! `DIRECT_UPDATEMODULE_ACCESS` selects) belong to the concrete modules that
//! implement them (layer 9); `ExitDoorType`'s constants are copied here
//! because `ContainModule` and the factory-level code name them.

use crate::common::disabled_types::DisabledMaskType;
use crate::common::ini::{Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_UPDATE};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};
use std::sync::Arc;

/// `enum UpdateSleepTime`: a number of frames. (Upstream is an `enum` that
/// `UPDATE_SLEEP(n)` casts any unsigned number to.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UpdateSleepTime(pub u32);

/// `UPDATE_SLEEP_INVALID`.
pub const UPDATE_SLEEP_INVALID: UpdateSleepTime = UpdateSleepTime(0);
/// `UPDATE_SLEEP_NONE`: call again next frame.
pub const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime(1);
/// `UPDATE_SLEEP_FOREVER`: "we use 0x3fffffff so that we can add offsets and
/// not overflow... and also 'cuz we shift the value up by two bits for the
/// phase".
pub const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime(0x3fff_ffff);
/// `FOREVER` (`GameCommon.h`): the default of `frameToSleepTime`'s frames.
pub const FOREVER: u32 = 0x3fff_ffff;

/// `UPDATE_SLEEP(numFrames)`.
pub const fn update_sleep(num_frames: u32) -> UpdateSleepTime {
    UpdateSleepTime(num_frames)
}

/// `enum SleepyUpdatePhase`: "all phase 0 are called before all phase 1
/// within a given frame, etc." (two bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum SleepyUpdatePhase {
    /// `PHASE_INITIAL`.
    Initial = 0,
    /// `PHASE_PHYSICS`.
    Physics = 1,
    /// `PHASE_NORMAL`.
    Normal = 2,
    /// `PHASE_FINAL`.
    Final = 3,
}

impl SleepyUpdatePhase {
    /// `(SleepyUpdatePhase)(v & 3)`.
    pub fn from_low_bits(v: u32) -> SleepyUpdatePhase {
        match v & 3 {
            0 => SleepyUpdatePhase::Initial,
            1 => SleepyUpdatePhase::Physics,
            2 => SleepyUpdatePhase::Normal,
            _ => SleepyUpdatePhase::Final,
        }
    }
}

/// `enum ExitDoorType`.
pub type ExitDoorType = i32;
/// `DOOR_1`.
pub const DOOR_1: ExitDoorType = 0;
/// `DOOR_2`.
pub const DOOR_2: ExitDoorType = 1;
/// `DOOR_3`.
pub const DOOR_3: ExitDoorType = 2;
/// `DOOR_4`.
pub const DOOR_4: ExitDoorType = 3;
/// `DOOR_COUNT_MAX`.
pub const DOOR_COUNT_MAX: ExitDoorType = 4;
/// `DOOR_NONE_AVAILABLE`: "need a door, but none currently available".
pub const DOOR_NONE_AVAILABLE: ExitDoorType = -1;
/// `DOOR_NONE_NEEDED`: "don't need a door reservation".
pub const DOOR_NONE_NEEDED: ExitDoorType = -2;

/// `class UpdateModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateModuleData {
    /// The `BehaviorModuleData` base.
    pub behavior: BehaviorModuleData,
}

impl UpdateModuleData {
    /// `UpdateModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `UpdateModuleData::buildFieldParse(p)`: `BehaviorModuleData::buildFieldParse`.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, UpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)
    }

    /// `UpdateModule::friend_newModuleData(ini)` (`MAKE_STANDARD_MODULE_DATA_MACRO_ABC`):
    /// new data, read with `initFromINIMultiProc(data, buildFieldParse)` if
    /// there is an `ini`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<UpdateModuleData> {
        let mut data = UpdateModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, UpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class UpdateModule : public BehaviorModule, public UpdateModuleInterface`.
#[derive(Clone, Debug)]
pub struct UpdateModule {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// The typed module data (`getUpdateModuleData()`).
    data: Arc<UpdateModuleData>,
    /// `m_nextCallFrameAndPhase`: "an absolute frame, not a frame count.
    /// actually, it's not a real frame at all, it has phase info in the lower
    /// bits".
    next_call_frame_and_phase: u32,
    /// `m_indexInLogic`.
    index_in_logic: i32,
    /// What `getUpdatePhase()` returns for the concrete class.
    update_phase: SleepyUpdatePhase,
    // --- p1w11-sonnet ---
    /// What `getDisabledTypesToProcess()` returns for the concrete class (the base's is `DISABLEDMASK_NONE`). The scheduler reads the
    /// module in its arena, which is this base, so the concrete class that overrides the virtual sets it here when it is made.
    disabled_types_to_process: DisabledMaskType,
    // --- end p1w11-sonnet ---
}

impl UpdateModule {
    /// `UpdateModule(thing, moduleData)`: the next call frame and phase 0,
    /// the index in the logic list -1. `update_phase` is the concrete class's
    /// `getUpdatePhase()` (the base's is `PHASE_NORMAL`). `INI_INVALID_DATA`
    /// for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>, update_phase: SleepyUpdatePhase) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let behavior = BehaviorModule::new(object, Some(ModuleDataKind::Update(data.clone())))?;
        Ok(UpdateModule { behavior, data, next_call_frame_and_phase: 0, index_in_logic: -1, update_phase, disabled_types_to_process: DisabledMaskType::NONE })
    }

    /// `UpdateModule::getInterfaceMask()`: `MODULEINTERFACE_UPDATE`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_UPDATE
    }

    /// The `BehaviorModule` base.
    pub fn behavior(&self) -> &BehaviorModule {
        &self.behavior
    }

    /// `getUpdateModuleData()`.
    pub fn update_module_data(&self) -> &Arc<UpdateModuleData> {
        &self.data
    }

    /// `getUpdatePhase()`.
    pub fn get_update_phase(&self) -> SleepyUpdatePhase {
        self.update_phase
    }

    /// `getDisabledTypesToProcess()`: `DISABLEDMASK_NONE` (the concrete
    /// modules that run while disabled redefine it).
    pub fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        self.disabled_types_to_process
    }

    // --- p1w11-sonnet ---
    /// The concrete class's override of `getDisabledTypesToProcess()` (`StatusDamageHelper`: `DISABLEDMASK_ALL`).
    pub fn set_disabled_types_to_process(&mut self, mask: DisabledMaskType) {
        self.disabled_types_to_process = mask;
    }
    // --- end p1w11-sonnet ---

    /// `friend_getPriority()`: the whole `m_nextCallFrameAndPhase`, which is
    /// what the scheduler sorts by.
    pub fn friend_get_priority(&self) -> u32 {
        self.next_call_frame_and_phase
    }

    /// `friend_getNextCallFrame()`.
    pub fn friend_get_next_call_frame(&self) -> u32 {
        self.next_call_frame_and_phase >> 2
    }

    /// `friend_getNextCallPhase()`.
    pub fn friend_get_next_call_phase(&self) -> SleepyUpdatePhase {
        SleepyUpdatePhase::from_low_bits(self.next_call_frame_and_phase)
    }

    /// `friend_setNextCallFrame(frame)`: "anything greater than 'forever' is
    /// still 'forever'", shifted up by two with the phase in the low bits.
    pub fn friend_set_next_call_frame(&mut self, frame: u32) {
        let frame = frame.min(UPDATE_SLEEP_FOREVER.0);
        self.next_call_frame_and_phase = (frame << 2) | self.update_phase as u32;
    }

    /// `friend_getIndexInLogic()`.
    pub fn friend_get_index_in_logic(&self) -> i32 {
        self.index_in_logic
    }

    /// `friend_setIndexInLogic(i)`.
    pub fn friend_set_index_in_logic(&mut self, i: i32) {
        self.index_in_logic = i;
    }

    /// `frameToSleepTime(frame1, frame2, frame3, frame4)` (the last three
    /// default to [`FOREVER`]): the sleep time until the earliest of the
    /// frames, given the current frame `now`. A frame in the past gives
    /// `UPDATE_SLEEP_NONE` (upstream asserts in debug builds).
    pub fn frame_to_sleep_time(now: u32, frame1: u32, frame2: u32, frame3: u32, frame4: u32) -> UpdateSleepTime {
        let frame = frame1.min(frame2).min(frame3).min(frame4);
        if frame > now {
            update_sleep(frame - now)
        } else {
            UPDATE_SLEEP_NONE
        }
    }

    /// `getWakeFrame()`: frames from `now` until the next call, or
    /// `UPDATE_SLEEP_NONE` if it is not in the future.
    pub fn get_wake_frame(&self, now: u32) -> UpdateSleepTime {
        let next_call_frame = self.next_call_frame_and_phase >> 2;
        if next_call_frame > now {
            update_sleep(next_call_frame - now)
        } else {
            UPDATE_SLEEP_NONE
        }
    }

    /// `setWakeFrame(obj, wakeDelay)`: `now + wakeDelay` (unsigned, wrapping),
    /// the frame upstream passes to `TheGameLogic->friend_awakenUpdateModule`;
    /// the caller (layer 8) does that call.
    pub fn set_wake_frame(now: u32, wake_delay: UpdateSleepTime) -> u32 {
        now.wrapping_add(wake_delay.0)
    }
}

impl Snapshot for UpdateModule {
    /// `UpdateModule::crc`: `BehaviorModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)
    }

    /// `UpdateModule::xfer` (version 1): the version, `BehaviorModule::xfer`,
    /// `m_nextCallFrameAndPhase`; on a load the phase bits are made to match
    /// the class's `getUpdatePhase()` ("ensure the phase matches": the phase
    /// numbering changed on January 2, 2003) and `m_indexInLogic` is -1 ("not
    /// saved -- it's restored in gamelogic::postprocess").
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)?;
        xfer.xfer_unsigned_int(&mut self.next_call_frame_and_phase)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.next_call_frame_and_phase &= !3;
            self.next_call_frame_and_phase |= self.update_phase as u32;
            self.index_in_logic = -1;
        }
        Ok(())
    }

    /// `UpdateModule::loadPostProcess`: `BehaviorModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniErrorKind, IniLoadType};
    use crate::logic::module::test_support::{crc_of, load_bytes, save_bytes};

    fn module(phase: SleepyUpdatePhase) -> UpdateModule {
        UpdateModule::new(7, Some(Arc::new(UpdateModuleData::new())), phase).unwrap()
    }

    #[test]
    fn a_new_module_has_no_next_call_and_is_not_in_the_logic_list() {
        let m = module(SleepyUpdatePhase::Normal);
        assert_eq!(m.friend_get_priority(), 0);
        assert_eq!(m.friend_get_next_call_frame(), 0);
        assert_eq!(m.friend_get_next_call_phase(), SleepyUpdatePhase::Initial);
        assert_eq!(m.friend_get_index_in_logic(), -1);
        assert_eq!(m.get_update_phase(), SleepyUpdatePhase::Normal);
        assert_eq!(m.behavior().object(), 7);
        assert_eq!(m.get_disabled_types_to_process(), DisabledMaskType::NONE);
        assert_eq!(UpdateModule::get_interface_mask(), 1);
        assert!(UpdateModule::new(7, None, SleepyUpdatePhase::Normal).is_err());
    }

    #[test]
    fn the_next_call_frame_and_phase_share_one_word() {
        for phase in [SleepyUpdatePhase::Initial, SleepyUpdatePhase::Physics, SleepyUpdatePhase::Normal, SleepyUpdatePhase::Final] {
            let mut m = module(phase);
            m.friend_set_next_call_frame(100);
            assert_eq!(m.friend_get_priority(), (100 << 2) | phase as u32);
            assert_eq!(m.friend_get_next_call_frame(), 100);
            assert_eq!(m.friend_get_next_call_phase(), phase);
        }
        // within a frame the lower phase sorts first, across frames the earlier frame
        let (mut a, mut b) = (module(SleepyUpdatePhase::Final), module(SleepyUpdatePhase::Initial));
        a.friend_set_next_call_frame(10);
        b.friend_set_next_call_frame(11);
        assert!(a.friend_get_priority() < b.friend_get_priority());
        b.friend_set_next_call_frame(10);
        assert!(b.friend_get_priority() < a.friend_get_priority());
    }

    #[test]
    fn a_frame_past_forever_is_forever() {
        let mut m = module(SleepyUpdatePhase::Physics);
        for f in [0x3fff_ffff, 0x4000_0000, 0x7fff_ffff, 0xffff_ffff] {
            m.friend_set_next_call_frame(f);
            assert_eq!(m.friend_get_next_call_frame(), 0x3fff_ffff, "{f:#x}");
            assert_eq!(m.friend_get_priority(), 0xffff_fffc | 1);
        }
    }

    #[test]
    fn frame_to_sleep_time_is_the_earliest_frame_from_now() {
        let f = UpdateModule::frame_to_sleep_time;
        assert_eq!(f(10, 12, FOREVER, FOREVER, FOREVER), update_sleep(2));
        assert_eq!(f(10, FOREVER, 15, FOREVER, FOREVER), update_sleep(5));
        assert_eq!(f(10, 40, 30, 20, 25), update_sleep(10));
        assert_eq!(f(10, FOREVER, FOREVER, FOREVER, FOREVER), update_sleep(FOREVER - 10));
        // now: sleep none; in the past: sleep none (a DEBUG_CRASH upstream)
        assert_eq!(f(10, 10, FOREVER, FOREVER, FOREVER), UPDATE_SLEEP_NONE);
        assert_eq!(f(10, 3, FOREVER, FOREVER, FOREVER), UPDATE_SLEEP_NONE);
        assert_eq!(f(10, FOREVER, 5, FOREVER, FOREVER), UPDATE_SLEEP_NONE);
        assert_eq!(f(0, 0, 0, 0, 0), UPDATE_SLEEP_NONE);
    }

    #[test]
    fn get_wake_frame_counts_frames_to_the_next_call() {
        let mut m = module(SleepyUpdatePhase::Normal);
        assert_eq!(m.get_wake_frame(5), UPDATE_SLEEP_NONE);
        m.friend_set_next_call_frame(50);
        assert_eq!(m.get_wake_frame(40), update_sleep(10));
        assert_eq!(m.get_wake_frame(49), update_sleep(1));
        assert_eq!(m.get_wake_frame(50), UPDATE_SLEEP_NONE);
        assert_eq!(m.get_wake_frame(51), UPDATE_SLEEP_NONE);
        m.friend_set_next_call_frame(u32::MAX);
        assert_eq!(m.get_wake_frame(0), update_sleep(0x3fff_ffff));
    }

    #[test]
    fn set_wake_frame_is_now_plus_delay_unsigned() {
        assert_eq!(UpdateModule::set_wake_frame(100, update_sleep(7)), 107);
        assert_eq!(UpdateModule::set_wake_frame(100, UPDATE_SLEEP_NONE), 101);
        assert_eq!(UpdateModule::set_wake_frame(0xffff_fff0, update_sleep(0x20)), 0x10);
        assert_eq!(UPDATE_SLEEP_INVALID.0, 0);
        assert_eq!(UPDATE_SLEEP_FOREVER.0, FOREVER);
    }

    #[test]
    fn xfer_saves_the_word_and_a_load_forces_the_phase_and_the_index() {
        let mut m = module(SleepyUpdatePhase::Physics);
        m.friend_set_next_call_frame(0x1234);
        m.friend_set_index_in_logic(9);
        let bytes = save_bytes(&mut m).unwrap();
        // the versions of UpdateModule, BehaviorModule, ObjectModule and Module, then the word
        assert_eq!(bytes, [1, 1, 1, 1, 0xd1, 0x48, 0, 0]);
        assert_eq!(m.friend_get_index_in_logic(), 9, "a save leaves the index");

        // a save from before the phase numbering changed (phase 2 = old FINAL)
        let mut old = module(SleepyUpdatePhase::Normal);
        load_bytes(&mut old, vec![1, 1, 1, 1, 0xd2, 0x48, 0, 0]).unwrap();
        assert_eq!(old.friend_get_next_call_frame(), 0x1234);
        assert_eq!(old.friend_get_next_call_phase(), SleepyUpdatePhase::Normal);
        let mut other = module(SleepyUpdatePhase::Final);
        other.friend_set_index_in_logic(3);
        load_bytes(&mut other, vec![1, 1, 1, 1, 0xd1, 0x48, 0, 0]).unwrap();
        assert_eq!(other.friend_get_next_call_phase(), SleepyUpdatePhase::Final);
        assert_eq!(other.friend_get_priority(), (0x1234 << 2) | 3);
        assert_eq!(other.friend_get_index_in_logic(), -1);
        // a version above the current one is refused
        assert!(load_bytes(&mut other, vec![2, 1, 1, 1, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn crc_writes_nothing() {
        let mut m = module(SleepyUpdatePhase::Normal);
        m.friend_set_next_call_frame(77);
        assert_eq!(crc_of(&mut m).unwrap(), 0);
    }

    #[test]
    fn update_module_data_reads_only_end() {
        fn block(ini: &mut Ini<'_>, out: &mut Vec<IniResult<UpdateModuleData>>) -> IniResult<()> {
            out.push(UpdateModuleData::friend_new_module_data(Some(ini)));
            Ok(())
        }
        static BLOCKS: [BlockParse<Vec<IniResult<UpdateModuleData>>>; 1] = [BlockParse { token: "D", parse: block }];
        let mut out = Vec::new();
        Ini::load(b"u.ini", b"D\nEnd\nD\n Foo = 1\nEnd\n", IniLoadType::Overwrite, None, &BLOCKS, &mut out).unwrap_err();
        assert!(out[0].is_ok());
        assert_eq!(out[1].clone().unwrap_err().kind, IniErrorKind::UnknownToken);
    }
}
