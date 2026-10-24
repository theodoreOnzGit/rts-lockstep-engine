// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StealthDetectorUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StealthDetectorUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StealthDetectorUpdate`: every `DetectionRate`, the stealthed enemies and neutrals in range (or stealth units garrisoned in a building) are
//! marked detected until the next scan, with the UI feedback of a discovery and the detector's IR ping.
//!
//! # Design
//!
//! The scan is [`World::partition_iterate_objects_in_range_of`] (the partition manager's three filters, relationship, kinds and map status, are
//! its own: the scan is the list it gives); the module's own filter, `PartitionFilterStealthedOrStealthGarrisoned`, runs here, as upstream's
//! does. The feedback of a discovery or a neutralisation (the radar event, the sound, the message, the Eva line) is one client event
//! ([`ObjectClientEvent::StealthDetectorFeedback`]): `TheRadar->tryEvent` decides whether the rest plays, which only changes what is heard and
//! shown. The IR ping's particle systems sit at a bone of the drawable, which only the client knows ([`ObjectClientEvent::ParticleSystemAtBone`]).
//! A detected object's stealth update is told with [`ModuleCall::StealthMarkAsDetectedFor`], and asked its Eva events with
//! [`ModuleCall::StealthGetEnemyDetectionEvaEvent`] and [`ModuleCall::StealthGetOwnDetectionEvaEvent`].
//!
//! # Upstream facts kept
//!
//! - The two feedbacks compare the local player with a controlling player as pointers (`NULL` equals `NULL`). The sound of a feedback reads the
//!   detector's player (a discovery) or the detected object's (a neutralisation) unchecked: for a discovery, and a neutralised mine, only if
//!   `tryEvent` says to play it, which the client decides (the event carries `None`, and playing it is undefined); for any other neutralisation
//!   always (`Undefined::NullControllingPlayer`). The local player's shroud is read unchecked (`Undefined::NullLocalPlayer`).
//! - The detected object is marked for one frame more than the rate, a stealthed rider of a garrison for two.
//! - The grid particle's position is the detected object's, at the detector's height plus 17, with `x` and `y` brought down to a multiple of 12 by
//!   `(Int)` truncation.
//! - `UpdateRate` 0 makes the constructor draw `GameLogicRandomValue(1, 0)`.
//!
//! `p1w13-sonnet`, batch R27, 2026-10-23.

use std::sync::Arc;

use crate::common::audio_event_name::AudioEventName;
use crate::common::base_type::Coord3D;
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_common::{ObjectShroudStatus, Relationship};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::kind_of::{KindOfMaskType, KINDOF_BOOBY_TRAP, KINDOF_DEMOTRAP, KINDOF_MINE};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{
    OBJECT_STATUS_DETECTED, OBJECT_STATUS_SOLD, OBJECT_STATUS_STEALTHED,
    OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime,
    UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::stealth_update::EVA_INVALID;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::{ObjectClientEvent, StealthClientOp, StealthFeedback};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `FROM_CENTER_2D` (`PartitionManager.h`'s `DistanceCalculationType`).
const FROM_CENTER_2D: i32 = 0;
/// `RADAR_EVENT_STEALTH_DISCOVERED` (`Radar.h`).
pub const RADAR_EVENT_STEALTH_DISCOVERED: i32 = 8;
/// `RADAR_EVENT_STEALTH_NEUTRALIZED`.
pub const RADAR_EVENT_STEALTH_NEUTRALIZED: i32 = 9;
/// The IR ping's position when the drawable has no such bone (`Coord3D bonePosition = {-1.66f,5.5f,15}`).
pub const IR_DEFAULT_BONE_POSITION: Coord3D = Coord3D {
    x: -1.66,
    y: 5.5,
    z: 15.0,
};

/// `class StealthDetectorUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct StealthDetectorUpdateModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_updateRate` (`DetectionRate`).
    pub update_rate: u32,
    /// `m_detectionRange` (0: the vision range).
    pub detection_range: f32,
    /// `m_initiallyDisabled`.
    pub initially_disabled: bool,
    /// `m_pingSound`.
    pub ping_sound: AudioEventName,
    /// `m_loudPingSound`.
    pub loud_ping_sound: AudioEventName,
    /// `m_IRBeaconParticleSysTmpl`, by name (`None` for `NULL`).
    pub ir_beacon_particle_sys: Option<Vec<u8>>,
    /// `m_IRParticleSysTmpl`.
    pub ir_particle_sys: Option<Vec<u8>>,
    /// `m_IRBrightParticleSysTmpl`.
    pub ir_bright_particle_sys: Option<Vec<u8>>,
    /// `m_IRGridParticleSysTmpl`.
    pub ir_grid_particle_sys: Option<Vec<u8>>,
    /// `m_IRParticleSysBone`.
    pub ir_particle_sys_bone: Vec<u8>,
    /// `m_extraDetectKindof` ("units must match any kindof bits set here, in order to be detected").
    pub extra_detect_kindof: KindOfMaskType,
    /// `m_extraDetectKindofNot`.
    pub extra_detect_kindof_not: KindOfMaskType,
    /// `m_canDetectWhileGarrisoned`.
    pub can_detect_while_garrisoned: bool,
    /// `m_canDetectWhileTransported` (`CanDetectWhileContained`).
    pub can_detect_while_transported: bool,
}

impl Default for StealthDetectorUpdateModuleData {
    /// `StealthDetectorUpdateModuleData()`: a rate of one frame, the vision range, enabled, no particles, no extra kinds.
    fn default() -> Self {
        StealthDetectorUpdateModuleData {
            update: UpdateModuleData::default(),
            update_rate: 1,
            detection_range: 0.0,
            initially_disabled: false,
            ping_sound: AudioEventName::default(),
            loud_ping_sound: AudioEventName::default(),
            ir_beacon_particle_sys: None,
            ir_particle_sys: None,
            ir_bright_particle_sys: None,
            ir_grid_particle_sys: None,
            ir_particle_sys_bone: Vec::new(),
            extra_detect_kindof: KindOfMaskType::new(),
            extra_detect_kindof_not: KindOfMaskType::new(),
            can_detect_while_garrisoned: false,
            can_detect_while_transported: false,
        }
    }
}

/// `INI::parseParticleSystemTemplate`: `None` is `NULL`; any other name is looked up when it is used.
fn parse_particle_system(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

/// `StealthDetectorUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<StealthDetectorUpdateModuleData>; 15] = [
    FieldParse::new(
        "DetectionRate",
        |ini, d, _| {
            d.update_rate = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "DetectionRange",
        |ini, d, _| {
            d.detection_range = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "InitiallyDisabled",
        |ini, d, _| {
            d.initially_disabled = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "PingSound",
        |ini, d, _| d.ping_sound.parse(ini),
        UserData::None,
    ),
    FieldParse::new(
        "LoudPingSound",
        |ini, d, _| d.loud_ping_sound.parse(ini),
        UserData::None,
    ),
    FieldParse::new(
        "IRBeaconParticleSysName",
        |ini, d, _| {
            d.ir_beacon_particle_sys = parse_particle_system(ini)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "IRParticleSysName",
        |ini, d, _| {
            d.ir_particle_sys = parse_particle_system(ini)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "IRBrightParticleSysName",
        |ini, d, _| {
            d.ir_bright_particle_sys = parse_particle_system(ini)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "IRGridParticleSysName",
        |ini, d, _| {
            d.ir_grid_particle_sys = parse_particle_system(ini)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "IRParticleSysBone",
        |ini, d, _| {
            d.ir_particle_sys_bone = ini.parse_ascii_string()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ExtraRequiredKindOf",
        |ini, d, _| d.extra_detect_kindof.parse_from_ini(ini),
        UserData::None,
    ),
    FieldParse::new(
        "ExtraForbiddenKindOf",
        |ini, d, _| d.extra_detect_kindof_not.parse_from_ini(ini),
        UserData::None,
    ),
    FieldParse::new(
        "CanDetectWhileGarrisoned",
        |ini, d, _| {
            d.can_detect_while_garrisoned = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "CanDetectWhileContained",
        |ini, d, _| {
            d.can_detect_while_transported = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl StealthDetectorUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (no fields of its own besides the behaviour base's), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, StealthDetectorUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<StealthDetectorUpdateModuleData> {
        let mut data = StealthDetectorUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                StealthDetectorUpdateModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class StealthDetectorUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct StealthDetectorUpdate {
    /// The `UpdateModule` base (the scheduling half; it processes `DISABLED_HELD` too).
    update: UpdateModule,
    /// `getStealthDetectorUpdateModuleData()`.
    data: Arc<StealthDetectorUpdateModuleData>,
    /// `m_enabled`.
    enabled: bool,
}

impl PartialEq for StealthDetectorUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.enabled == other.enabled
    }
}

impl StealthDetectorUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base (processing `DISABLED_HELD` too), the data (`INI_INVALID_DATA` for
    /// null data) and `m_enabled`.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<StealthDetectorUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let mut update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        // getDisabledTypesToProcess(): MAKE_DISABLED_MASK( DISABLED_HELD )
        let mut mask = crate::common::disabled_types::DisabledMaskType::default();
        mask.set(DISABLED_HELD, 1);
        update.set_disabled_types_to_process(mask);
        let enabled = !data.initially_disabled;
        Ok(StealthDetectorUpdate {
            update,
            data,
            enabled,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getStealthDetectorUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<StealthDetectorUpdateModuleData> {
        &self.data
    }

    /// `isSDEnabled()`.
    pub fn is_sd_enabled(&self) -> bool {
        self.enabled
    }

    /// `setSDEnabled(enabled)`: the flag, and awake now or asleep forever.
    pub fn set_sd_enabled(&mut self, world: &mut World, enabled: bool) {
        self.enabled = enabled;
        update_behavior::set_wake_frame(
            world,
            if self.enabled {
                UPDATE_SLEEP_NONE
            } else {
                UPDATE_SLEEP_FOREVER
            },
        );
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: the random first wake.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }

    /// `ContainModuleInterface::isGarrisonable() && getStealthUnitsContained()` of `obj`'s contain module.
    fn garrisons_stealth_units(world: &mut World, obj: ObjectId) -> bool {
        let Some(contain) = world.get_contain(obj) else {
            return false;
        };
        world
            .module_call(obj, contain, ModuleCall::IsGarrisonable)
            .as_bool()
            && matches!(world.module_call(obj, contain, ModuleCall::GetStealthUnitsContained), ModuleReply::Int(n) if n != 0)
    }

    /// `PartitionFilterStealthedOrStealthGarrisoned::allow`.
    fn stealthed_or_stealth_garrisoned(world: &mut World, other: ObjectId) -> bool {
        let Some(o) = world.objects.find(other) else {
            return false;
        };
        if o.test_status(OBJECT_STATUS_STEALTHED) {
            return true;
        }
        !crate::logic::modules::plant(1973) && Self::garrisons_stealth_units(world, other)
    }

    /// The Eva event a stealth update answers (`EVA_Invalid` for none).
    fn eva_of(
        world: &mut World,
        obj: ObjectId,
        stealth: crate::logic::world::object::ModuleIndex,
        call: ModuleCall,
    ) -> i32 {
        match world.module_call(obj, stealth, call) {
            ModuleReply::Int(e) => e,
            _ => EVA_INVALID,
        }
    }

    /// The feedback to the local player for a detection of `them`: `UpdateSleepTime`-free, `Err` where upstream is undefined.
    fn feedback(
        &self,
        world: &mut World,
        id: ObjectId,
        them: ObjectId,
        stealth: crate::logic::world::object::ModuleIndex,
    ) -> Result<(), Undefined> {
        let local = world.players.local_index();
        let relationship = world.get_relationship(id, them);
        let position = world
            .objects
            .find(them)
            .map(|o| o.get_position())
            .unwrap_or_default();
        // for the player revealing the stealth unit do some UI feedback
        if local == world.get_controlling_player(id)
            && (relationship != Relationship::Allies || crate::logic::modules::plant(1972))
        {
            let player_index = world.get_controlling_player(id);
            let eva = Self::eva_of(
                world,
                them,
                stealth,
                ModuleCall::StealthGetEnemyDetectionEvaEvent,
            );
            world.emit(ObjectClientEvent::StealthDetectorFeedback {
                feedback: StealthFeedback::Discovered,
                try_radar_event: true,
                position,
                player_index,
                eva,
            });
        }
        // for the unit being revealed, do some UI feedback
        if local == world.get_controlling_player(them) && relationship != Relationship::Allies {
            let mine = world.objects.find(them).is_some_and(|o| {
                o.is_kind_of(KINDOF_MINE)
                    || o.is_kind_of(KINDOF_BOOBY_TRAP)
                    || o.is_kind_of(KINDOF_DEMOTRAP)
            });
            let player_index = world.get_controlling_player(them);
            if player_index.is_none() && !mine {
                // the radar event is created and the feedback always plays: `them->getControllingPlayer()->getPlayerIndex()`
                return Err(Undefined::NullControllingPlayer);
            }
            let eva = Self::eva_of(
                world,
                them,
                stealth,
                ModuleCall::StealthGetOwnDetectionEvaEvent,
            );
            world.emit(ObjectClientEvent::StealthDetectorFeedback {
                feedback: StealthFeedback::Neutralized,
                try_radar_event: mine,
                position,
                player_index,
                eva,
            });
        }
        Ok(())
    }

    fn update_inner(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
    ) -> Result<UpdateSleepTime, Undefined> {
        let id = me.object;
        let d = self.data.clone();
        let Some(o) = world.objects.find(id) else {
            return Ok(update_sleep(d.update_rate));
        };
        if o.is_effectively_dead() {
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        // We have to wait until we are fully constructed, but we will detect the moment we finish
        if o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) {
            return Ok(UPDATE_SLEEP_NONE);
        }
        // We turn off forever the moment we are sold.
        if o.test_status(OBJECT_STATUS_SOLD) {
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        // Are we contained by anything?
        let contained_by = o.contained_by;
        if world.objects.find(contained_by).is_some() {
            if let Some(contain) = world.get_contain(contained_by) {
                if world
                    .module_call(contained_by, contain, ModuleCall::IsGarrisonable)
                    .as_bool()
                {
                    if !d.can_detect_while_garrisoned && !crate::logic::modules::plant(1977) {
                        // But we can't detect stuff while inside.
                        return Ok(update_sleep(d.update_rate));
                    }
                } else if !d.can_detect_while_transported {
                    // We are in a normal container and can't detect!
                    return Ok(update_sleep(d.update_rate));
                }
            }
        }
        let vision_range = if d.detection_range > 0.0 {
            d.detection_range
        } else {
            world.get_vision_range(id)
        };
        let mut found_someone = false;
        let candidates =
            world.partition_iterate_objects_in_range_of(id, vision_range, FROM_CENTER_2D);
        for them in candidates {
            // only consider items that are currently stealthed (the module's own filter; the manager's are the scan's)
            if !Self::stealthed_or_stealth_garrisoned(world, them) {
                continue;
            }
            let Some(t) = world.objects.find(them) else {
                continue;
            };
            if t.is_effectively_dead() {
                continue;
            }
            let detected = t.test_status(OBJECT_STATUS_DETECTED);
            let not_mine = !t.is_kind_of(KINDOF_MINE);
            if let Some(stealth) = t.stealth {
                // we have found someone
                found_someone = true;
                // if this object was not previously detected it is now being revealed and we want to do some UI feedback
                if !detected {
                    self.feedback(world, id, them, stealth)?;
                }
                // updateRate PLUS 1 is necessary to ensure it stays detected 'till we are called again...
                world.module_call(
                    them,
                    stealth,
                    ModuleCall::StealthMarkAsDetectedFor(
                        d.update_rate
                            .wrapping_add(u32::from(!crate::logic::modules::plant(1971))),
                    ),
                );
                if world.has_drawable(them) && not_mine {
                    world.emit(ObjectClientEvent::Stealth {
                        object_id: them,
                        op: StealthClientOp::SecondMaterialPassOpacity(1.0),
                    });
                }
                if let Some(grid) = d.ir_grid_particle_sys.clone() {
                    let mut grid_position = world
                        .objects
                        .find(them)
                        .map(|o| o.get_position())
                        .unwrap_or_default();
                    grid_position.z =
                        world.objects.find(id).map_or(0.0, |o| o.get_position().z) + 17.0;
                    grid_position.x -= ((grid_position.x as i32) % 12) as f32;
                    if !crate::logic::modules::plant(1974) {
                        grid_position.y -= ((grid_position.y as i32) % 12) as f32;
                    }
                    world.create_particle_system(grid, grid_position);
                }
            } else if Self::garrisons_stealth_units(world, them) {
                // perhaps they are garrisoning something stealthy, eh?
                let contain = world.get_contain(them).expect("checked above");
                let riders = match world.module_call(them, contain, ModuleCall::GetContainedItems) {
                    ModuleReply::Ids(v) => v,
                    _ => Vec::new(),
                };
                for rider in riders {
                    let Some(stealth) = world.objects.find(rider).and_then(|o| o.stealth) else {
                        continue;
                    };
                    // we have found someone
                    found_someone = true;
                    if world.get_controlling_player(id) != world.get_controlling_player(rider)
                        && world.get_relationship(id, rider) != Relationship::Allies
                    {
                        world.module_call(
                            rider,
                            stealth,
                            ModuleCall::StealthMarkAsDetectedFor(d.update_rate.wrapping_add(2)),
                        );
                    }
                }
            }
        }
        let local = world
            .players
            .local_index()
            .ok_or(Undefined::NullLocalPlayer)?;
        // Make sure the detector is visible to the local player before we add effects or sounds.
        let shrouded = world.get_shrouded_status(id, local);
        if shrouded <= ObjectShroudStatus::PartialClear {
            let stealthed = world
                .objects
                .find(id)
                .is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED));
            if !stealthed || world.get_controlling_player(id) == Some(local) {
                let to_drawable = world.has_drawable(id);
                let ping = if found_someone != crate::logic::modules::plant(1975) {
                    d.ir_bright_particle_sys.clone()
                } else {
                    d.ir_particle_sys.clone()
                };
                for template in [ping, d.ir_beacon_particle_sys.clone()]
                    .into_iter()
                    .flatten()
                {
                    world.create_particle_system_at_bone(
                        template,
                        id,
                        to_drawable,
                        d.ir_particle_sys_bone.clone(),
                    );
                }
                let sound = if found_someone {
                    d.loud_ping_sound.event_name.clone()
                } else {
                    d.ping_sound.event_name.clone()
                };
                world.emit(ObjectClientEvent::AudioAddEvent {
                    name: sound,
                    object_id: id,
                    position: None,
                    player_index: None,
                });
            }
        }
        Ok(update_sleep(d.update_rate))
    }
}

impl UpdateBehavior for StealthDetectorUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.update_inner(world, me) {
            Ok(t) => t,
            Err(u) => {
                Self::undefined(world, me, u);
                update_sleep(self.data.update_rate)
            }
        }
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        // start these guys with random phasings so that we don't have all of 'em check on the same frame.
        let wake = if self.enabled {
            update_sleep(
                world
                    .random
                    .game_logic_random_value(1, self.data.update_rate as i32)
                    as u32,
            )
        } else {
            UPDATE_SLEEP_FOREVER
        };
        update_behavior::set_wake_frame(world, wake);
    }
    /// The field after the base: `m_enabled`.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        if crate::logic::modules::plant(1976) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.enabled)
    }
    /// `setSDEnabled` and `isSDEnabled` (`BattlePlanUpdate`'s calls).
    fn other_call(
        &mut self,
        world: &mut World,
        _me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::StealthDetectorSetEnabled(enabled) => {
                self.set_sd_enabled(world, enabled);
                ModuleReply::Done
            }
            ModuleCall::StealthDetectorIsEnabled => ModuleReply::Bool(self.enabled),
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::StealthDetectorUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::StealthDetectorUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for StealthDetectorUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the field.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `StealthDetectorUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::StealthDetectorUpdate(Arc::new(
        StealthDetectorUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
