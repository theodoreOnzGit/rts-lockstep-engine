// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/OCLUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/OCLUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `OCLUpdate`: creates an object creation list on a random timer (`MinDelay` to `MaxDelay` frames), at the object or at the closest point of the map's
//! edge (`CreateAtEdge`), or, if `FactionTriggered`, the list of the faction (the side of the controlling player's template) once a playable player owns the
//! object, restarting the timer whenever the owner's colour changes.
//!
//! The list is a name (`OCL`, or `FactionOCL = Faction:<side> OCL:<list>` for each side); `None` is `NULL`, and a name that is no list makes nothing
//! ([`CutCall::OclCreateAt`]). `TheTerrainLogic->findClosestEdgePoint` is [`World::terrain_find_closest_edge_point`]; `isPlayableSide`, `getPlayerColor` and
//! the template's side are the player tables of `ObjectCuts`.
//!
//! # Upstream facts kept
//!
//! - `parseFactionObjectCreationList` compares the word after the faction name with `stricmp(token, "OCL")` without testing it for `NULL`
//!   ([`Undefined::NullTokenCompared`]).
//! - `MinDelay`/`MaxDelay` are `UnsignedInt`s passed to `GameLogicRandomValue(Int, Int)`.
//! - A disabled object only pushes the next creation frame back by one each update.
//! - `getCountdownPercent` divides by `nextCreationFrame - timerStartedFrame` as a `float`, which is 0 for a timer started this frame.
//!
//! `p1w12-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
    SEPS_COLON,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `struct OCLUpdateModuleData::FactionOCLInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactionOclInfo {
    /// `m_factionName`.
    pub faction_name: Vec<u8>,
    /// `m_ocl`, by name (`None` for `NULL`).
    pub ocl: Option<Vec<u8>>,
}

/// `class OCLUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct OclUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_minDelay` (frames).
    pub min_delay: u32,
    /// `m_maxDelay` (frames).
    pub max_delay: u32,
    /// `m_ocl`, by name (`None` for `NULL`).
    pub ocl: Option<Vec<u8>>,
    /// `m_factionOCL`, in file order.
    pub faction_ocl: Vec<FactionOclInfo>,
    /// `m_isCreateAtEdge`.
    pub is_create_at_edge: bool,
    /// `m_isFactionTriggered`.
    pub is_faction_triggered: bool,
}

impl Default for OclUpdateModuleData {
    /// `OCLUpdateModuleData()`.
    fn default() -> Self {
        OclUpdateModuleData {
            update: UpdateModuleData::new(),
            min_delay: 0,
            max_delay: 0,
            ocl: None,
            faction_ocl: Vec::new(),
            is_create_at_edge: false,
            is_faction_triggered: false,
        }
    }
}

/// `INI::parseObjectCreationList`: `None` is no list; any other name is looked up when it is used.
fn parse_ocl(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

/// `parseFactionObjectCreationList(ini, instance, store, userData)`: `Faction:<side> OCL:<list>`, appended to `store`.
pub fn parse_faction_object_creation_list(
    ini: &mut Ini<'_>,
    store: &mut Vec<FactionOclInfo>,
) -> IniResult<()> {
    let mut info = FactionOclInfo {
        faction_name: Vec::new(),
        ocl: None,
    };
    let token = ini.get_next_token(Some(SEPS_COLON))?;
    if token.eq_ignore_ascii_case(b"Faction") {
        let token = ini
            .get_next_token_or_null(Some(SEPS_COLON))
            .ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        info.faction_name = token;
    } else {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }

    let token = ini
        .get_next_token_or_null(Some(SEPS_COLON))
        .ok_or_else(|| IniError::undefined(Undefined::NullTokenCompared))?;
    if token.eq_ignore_ascii_case(b"OCL") {
        info.ocl = parse_ocl(ini)?;
    } else {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }

    // Insert the info into the ocl hashmap
    store.push(info);
    Ok(())
}

/// `OCLUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<OclUpdateModuleData>; 7] = [
    FieldParse::new(
        "OCL",
        |ini, d, _| {
            d.ocl = parse_ocl(ini)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "FactionOCL",
        |ini, d, _| parse_faction_object_creation_list(ini, &mut d.faction_ocl),
        UserData::None,
    ),
    FieldParse::new(
        "MinDelay",
        |ini, d, _| {
            d.min_delay = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "MaxDelay",
        |ini, d, _| {
            d.max_delay = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "CreateAtEdge",
        |ini, d, _| {
            d.is_create_at_edge = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "FactionTriggered",
        |ini, d, _| {
            d.is_faction_triggered = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl OclUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, OclUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<OclUpdateModuleData> {
        let mut data = OclUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, OclUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class OCLUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct OclUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getOCLUpdateModuleData()`.
    data: Arc<OclUpdateModuleData>,
    /// `m_nextCreationFrame`.
    next_creation_frame: u32,
    /// `m_timerStartedFrame`.
    timer_started_frame: u32,
    /// `m_isFactionNeutral`.
    is_faction_neutral: bool,
    /// `m_currentPlayerColor`.
    current_player_color: i32,
}

impl PartialEq for OclUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.next_creation_frame == other.next_creation_frame
            && self.timer_started_frame == other.timer_started_frame
            && self.is_faction_neutral == other.is_faction_neutral
            && self.current_player_color == other.current_player_color
    }
}

impl OclUpdate {
    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<OclUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(OclUpdate {
            update,
            data,
            next_creation_frame: 0,
            timer_started_frame: 0,
            is_faction_neutral: true,
            current_player_color: 0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getOCLUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<OclUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.next_creation_frame,
            self.timer_started_frame,
            u32::from(self.is_faction_neutral),
            self.current_player_color as u32,
        ]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
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

    /// `xfer` with the scheduling half the slot holds: the version, that half, then the fields.
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

    /// `resetTimer()`.
    pub fn reset_timer(&mut self, world: &mut World) {
        self.set_next_creation_frame(world);
    }

    /// `shouldCreate()`.
    fn should_create(&self, world: &World, me: ObjectId) -> bool {
        if world.frame() < self.next_creation_frame {
            return false; // too soon
        }
        if world
            .objects
            .find(me)
            .is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION))
            && !crate::logic::modules::plant(1001)
        {
            return false; // not built yet
        }
        true
    }

    /// `setNextCreationFrame()`.
    fn set_next_creation_frame(&mut self, world: &mut World) {
        let delay = world.random.game_logic_random_value(
            self.data.min_delay as i32,
            if crate::logic::modules::plant(1002) {
                self.data.min_delay as i32
            } else {
                self.data.max_delay as i32
            },
        );
        self.timer_started_frame = world.frame();
        self.next_creation_frame = self.timer_started_frame.wrapping_add(delay as u32);
    }

    /// `getCountdownPercent()`.
    pub fn get_countdown_percent(&self, world: &World) -> f32 {
        let now = world.frame();
        1.0f32
            - (self.next_creation_frame.wrapping_sub(now) as f32
                / self
                    .next_creation_frame
                    .wrapping_sub(self.timer_started_frame) as f32)
    }

    /// `getRemainingFrames()`.
    pub fn get_remaining_frames(&self, world: &World) -> u32 {
        self.next_creation_frame.wrapping_sub(world.frame())
    }
}

impl UpdateBehavior for OclUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if world
            .objects
            .find(me.object)
            .is_some_and(|o| o.is_disabled())
        {
            if !crate::logic::modules::plant(1004) {
                self.next_creation_frame = self.next_creation_frame.wrapping_add(1);
            }
            return UPDATE_SLEEP_NONE;
        }

        let data = self.data.clone();

        // Test if the OCL update is faction dependant. If so, check for faction changes
        if data.is_faction_triggered {
            let player = world.get_controlling_player(me.object);

            // Test for when a player captures the building
            if self.is_faction_neutral {
                if let Some(p) = player.filter(|p| {
                    world.cuts.player_is_playable_side(*p) || crate::logic::modules::plant(1007)
                }) {
                    self.current_player_color = world.cuts.player_color(p);
                    self.is_faction_neutral = false;
                    self.set_next_creation_frame(world);
                }
            }
            // Test for when the building has been made neutral or when it changes faction
            else {
                // If this is no longer under player control, then we set the faction to neutral
                match player.filter(|p| world.cuts.player_is_playable_side(*p)) {
                    None => self.is_faction_neutral = true,
                    // If another player has taken control, reset the timer
                    Some(p) => {
                        if world.cuts.player_color(p) != self.current_player_color
                            && !crate::logic::modules::plant(1008)
                        {
                            self.current_player_color = world.cuts.player_color(p);
                            self.set_next_creation_frame(world);
                        }
                    }
                }
            }

            // If the building is neutal, skip futher update
            if self.is_faction_neutral {
                return UPDATE_SLEEP_NONE;
            }
        }

        if self.should_create(world, me.object) {
            if self.next_creation_frame == 0 && !crate::logic::modules::plant(1009) {
                // You don't get to actually spread the first try, you start on a timer, then go
                self.set_next_creation_frame(world);
                return UPDATE_SLEEP_NONE;
            }

            self.set_next_creation_frame(world);

            let position = world
                .objects
                .find(me.object)
                .map(|o| o.get_position())
                .unwrap_or_default();
            let creation_coord = if data.is_create_at_edge && !crate::logic::modules::plant(1006) {
                world.terrain_find_closest_edge_point(position)
            } else {
                position
            };
            let angle = world
                .objects
                .find(me.object)
                .map_or(0.0, |o| o.thing.orientation());

            // If this is faction triggered, search through the faction specific OCLs to find the match
            if data.is_faction_triggered {
                let Some(player) = world.get_controlling_player(me.object) else {
                    return UPDATE_SLEEP_NONE;
                };
                let Some(side) = world.cuts.player_side(player) else {
                    return UPDATE_SLEEP_NONE;
                };

                // Loop through the list of faction ocls to find the matching faction that triggeres the specific ocls
                for info in &data.faction_ocl {
                    if side == info.faction_name
                        || (crate::logic::modules::plant(1005)
                            && side.eq_ignore_ascii_case(&info.faction_name))
                    {
                        if let Some(ocl) = info.ocl.clone() {
                            world.cut(CutCall::OclCreateAt {
                                primary: me.object,
                                ocl,
                                primary_pos: creation_coord,
                                secondary_pos: position,
                                angle,
                            });
                        }
                        break;
                    }
                }
            }
            // Use the non faction OCL information
            else if let Some(ocl) = data.ocl.clone() {
                world.cut(CutCall::OclCreateAt {
                    primary: me.object,
                    ocl,
                    primary_pos: creation_coord,
                    secondary_pos: position,
                    angle,
                });
            }
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.next_creation_frame)?;
        xfer.xfer_unsigned_int(&mut self.timer_started_frame)?;
        xfer.xfer_bool(&mut self.is_faction_neutral)?;
        if crate::logic::modules::plant(1003) {
            return Ok(());
        }
        xfer.xfer_int(&mut self.current_player_color)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::OclUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::OclUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(
        &mut self,
        world: &mut World,
        _me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::OclUpdateResetTimer => {
                self.reset_timer(world);
                ModuleReply::Done
            }
            ModuleCall::OclUpdateGetCountdownPercent => {
                ModuleReply::Real(self.get_countdown_percent(world))
            }
            ModuleCall::OclUpdateGetRemainingFrames => {
                ModuleReply::Int(self.get_remaining_frames(world) as i32)
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for OclUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the fields.
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

/// The factory's `NewModuleDataProc` for `OCLUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::OclUpdate(Arc::new(
        OclUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
