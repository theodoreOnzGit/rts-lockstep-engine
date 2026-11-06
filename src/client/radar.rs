// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Radar.h (RADAR_CELL_WIDTH/HEIGHT, RadarEventType, RadarObject,
//     RadarPriorityType, Radar)
//   GeneralsMD/Code/GameEngine/Source/Common/System/Radar.cpp (RadarObject::RadarObject, Radar::Radar,
//     clearAllEvents, reset, update, newMap, addObject, deleteFromList, removeObject, radarToWorld2D, radarToWorld,
//     worldToRadar, localPixelToRadar, screenPixelToWorld, objectUnderRadarPixel, searchListForRadarLocationMatch,
//     findDrawPositions, radarColorLookupTable, createEvent, createPlayerEvent, internalCreateEvent, getLastEventLoc,
//     tryUnderAttackEvent, tryInfiltrationEvent, tryEvent, refreshTerrain, queueTerrainRefresh, crc,
//     isPriorityVisible)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarCallback.cpp (LeftHUDInput's radar test:
//     isRadarForced, isRadarHidden, hasRadar)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The radar, `TheRadar` (GitHub #35): the objects it shows, its events (the pings), its coordinates, and whether it
//! is hidden or forced on. What it looks like is [`crate::client::w3d_radar`].
//!
//! # Logic state or client state?
//!
//! **Client state.** Upstream's `Radar` lives in `Common/System` and the logic calls it (`Object.cpp`'s `addObject`,
//! `removeObject` and `tryUnderAttackEvent`, the crates' `tryInfiltrationEvent`, the scripts' `hide` and `forceOn`), but:
//!
//! - **`Radar::crc` writes nothing** (Radar.cpp:1376), nor does `RadarObject::crc`: the radar is not in the logic's CRC,
//!   so nothing here moves gate (a).
//! - **`Radar::xfer` is saved** (`CHUNK_Radar`, `GameState.cpp`): the hidden and forced flags, both object lists (each
//!   entry its object's id and colour), the 64 events and the two event indices. ~~This port's save game does not carry
//!   the block yet (GitHub #35's gap note); [`Radar`] holds every field it would write.~~ **Corrected 2026-11-06 (GitHub
//!   #4):** the client's save (`crate::client::save_game`) writes and reads it, every field.
//! - **Nothing in the logic reads the radar back**, except one bit the logic already keeps: whether an object has radar
//!   data (`m_radarData != NULL`, read by `Object::attemptDamage` before `tryUnderAttackEvent`), which is the World's
//!   `on_radar`. `tryEvent`'s answer gates only the stealth detectors' sound, message and EVA (`StealthDetectorUpdate`),
//!   all client feedback.
//!
//! So the logic emits the calls as client events (`RadarCall` of `object_events`, `PlayerClientEvent::RadarAddObject`
//! and `RadarRemoveObject`, `ClientCall::RadarInformation`, `client_event::RadarCall` for the scripts and the victory
//! conditions) and this radar takes them ([`Radar::take_event`]), reading the World through [`RadarHost`] and never
//! writing it.
//!
//! # Where this differs from upstream
//!
//! - **Objects are ids.** A `RadarObject` holds its object's id, not a pointer; an object the World no longer has is
//!   dropped from the lists when its `ObjectDestroyed` arrives (upstream removes it in `~Object`, which the logic's
//!   `RemoveObject` event already mirrors; the drop is a guard).
//! - **`addObject` of an object already listed** replaces it (upstream asserts in a debug build and links a second entry
//!   in a release one).
//! - **The radar window** is a rectangle the client gives ([`Radar::set_radar_window`]), where upstream finds
//!   `ControlBar.wnd:LeftHUD` in `newMap`.
//! - **What `tryUnderAttackEvent` and `tryInfiltrationEvent` do besides the event** (`TheControlBar->triggerRadarAttackGlow`,
//!   `TheInGameUI->message`, `TheAudio->addAudioEvent`, `TheEva->setShouldPlay`) is returned as [`RadarFeedback`] for the
//!   control bar, the UI messages, the audio and EVA to carry out.
//! - **`newMap`'s terrain read** asks the host (`isUnderwater`), which reads the logic's terrain without counting.
//!
//! # Upstream quirks that are kept
//!
//! - **`tryEvent`'s distance** is written `a.x - p.x * a.x - p.x + a.y - p.y * a.y - p.y` (no parentheses), so its
//!   "within 250 units" test passes for nearly any two points of a map: within ten seconds of an event of a type, no
//!   other event of that type is made anywhere. The test is translated as written.
//! - **`tryUnderAttackEvent`** plays the structure's sound for a unit (`m_radarStructureUnderAttackSound`; the unit's
//!   is never played), and its ally branch is unreachable: the logic calls it only for the local player's objects.
//! - **`drawSingleGenericEvent`** (in `w3d_radar`) adds a size in pixels to a position in radar cells.

use crate::common::base_type::{int_to_real, real_to_int, Coord3D, ICoord2D, RGBAColorInt, Region3D};
use crate::common::game_common::{Relationship, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::ObjectId;
use crate::common::ini::game_make_color;
use crate::common::kind_of::{KindOfType, KINDOF_DISGUISER, KINDOF_HARVESTER, KINDOF_INFANTRY, KINDOF_MP_COUNT_FOR_VICTORY, KINDOF_STRUCTURE, KINDOF_VEHICLE};
use crate::common::thing_template::RADAR_PRIORITY_INVALID;
use crate::logic::world::object_team::RADAR_PRIORITY_NOT_ON_RADAR;
use crate::logic::client_event::ClientEvent;
use crate::logic::world::game_logic_dispatch::ClientCall;
use crate::logic::world::object_events::{ObjectClientEvent, RadarCall, StealthFeedback};
use crate::logic::world::player_ops::PlayerClientEvent;

/// `RADAR_CELL_WIDTH`: "radar created at this horz resolution".
pub const RADAR_CELL_WIDTH: i32 = 128;
/// `RADAR_CELL_HEIGHT`.
pub const RADAR_CELL_HEIGHT: i32 = 128;

/// `RadarEventType`.
pub type RadarEventType = i32;
pub const RADAR_EVENT_INVALID: RadarEventType = 0;
pub const RADAR_EVENT_CONSTRUCTION: RadarEventType = 1;
pub const RADAR_EVENT_UPGRADE: RadarEventType = 2;
pub const RADAR_EVENT_UNDER_ATTACK: RadarEventType = 3;
pub const RADAR_EVENT_INFORMATION: RadarEventType = 4;
pub const RADAR_EVENT_BEACON_PULSE: RadarEventType = 5;
/// "for defection, hijacking, hacking, carbombing, and other sneaks".
pub const RADAR_EVENT_INFILTRATION: RadarEventType = 6;
pub const RADAR_EVENT_BATTLE_PLAN: RadarEventType = 7;
/// "we discovered a stealth unit".
pub const RADAR_EVENT_STEALTH_DISCOVERED: RadarEventType = 8;
/// "our stealth unit has been revealed".
pub const RADAR_EVENT_STEALTH_NEUTRALIZED: RadarEventType = 9;
/// "Internally creates a radar event, but doesn't notify the player".
pub const RADAR_EVENT_FAKE: RadarEventType = 10;
pub const RADAR_EVENT_NUM_EVENTS: RadarEventType = 11;

/// `RadarPriorityType`'s `RADAR_PRIORITY_LOCAL_UNIT_ONLY`: "unit priority, but only on the radar if controlled by the
/// local player".
pub const RADAR_PRIORITY_LOCAL_UNIT_ONLY: u8 = 4;

/// `MAX_RADAR_EVENTS`.
pub const MAX_RADAR_EVENTS: usize = 64;

/// `RADAR_QUEUE_TERRAIN_REFRESH_DELAY`: `LOGICFRAMES_PER_SECOND * 3.0f`.
pub const RADAR_QUEUE_TERRAIN_REFRESH_DELAY: f32 = LOGICFRAMES_PER_SECOND as f32 * 3.0;

/// `class RadarObject`: an object on the radar and the colour it is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadarObject {
    /// `m_object`, by id.
    pub object: ObjectId,
    /// `m_color` (`GameMakeColor(255, 255, 255, 255)` at first).
    pub color: i32,
}

/// `struct Radar::RadarEvent`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RadarEvent {
    /// `type`.
    pub event_type: RadarEventType,
    /// "TRUE when event is "active", otherwise it's just historical information".
    pub active: bool,
    pub create_frame: u32,
    pub die_frame: u32,
    /// "start fading out on this frame".
    pub fade_frame: u32,
    pub color1: RGBAColorInt,
    pub color2: RGBAColorInt,
    pub world_loc: Coord3D,
    pub radar_loc: ICoord2D,
    /// "TRUE when we have played the radar sound for this".
    pub sound_played: bool,
}

/// `isUnderwater(x, y, &waterZ, &terrainZ)`'s answer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RadarUnderwater {
    pub underwater: bool,
    /// Written only when there is water at the point.
    pub water_z: Option<f32>,
    pub terrain_z: f32,
}

/// What the radar reads of the World (and of the client: the local player). All reads; the `&mut` ones call a module
/// that only answers (`getRadarPriority`'s garrisonable test, the disguise, the container's apparent player).
pub trait RadarHost {
    /// `TheGameLogic->getFrame()`.
    fn radar_frame(&self) -> u32;
    /// `ThePlayerList->getLocalPlayer()`.
    fn radar_local_player(&self) -> Option<i32>;
    /// `obj->getRadarPriority()` (`RADAR_PRIORITY_INVALID` for an object that is gone).
    fn radar_priority(&mut self, obj: ObjectId) -> u8;
    /// `obj->getPosition()`.
    fn radar_object_position(&self, obj: ObjectId) -> Option<Coord3D>;
    /// `obj->isLocallyControlled()`.
    fn radar_is_locally_controlled(&self, obj: ObjectId) -> bool;
    /// `obj->getControllingPlayer()`.
    fn radar_controlling_player(&self, obj: ObjectId) -> Option<i32>;
    /// `obj->getIndicatorColor()`.
    fn radar_indicator_color(&self, obj: ObjectId) -> i32;
    /// `obj->isKindOf(kind)`.
    fn radar_is_kind_of(&self, obj: ObjectId, kind: KindOfType) -> bool;
    /// `obj->getStealth()`, `isDisguised()` and `getDisguisedPlayerIndex()`: the disguise's player when disguised.
    fn radar_disguised_player(&mut self, obj: ObjectId) -> Option<i32>;
    /// `obj->getContain()`: `None` without one; else `getApparentControllingPlayer(observer)`.
    fn radar_contain_apparent_player(&mut self, obj: ObjectId, observer: Option<i32>) -> Option<Option<i32>>;
    /// `player->getPlayerColor()`.
    fn radar_player_color(&self, player: i32) -> i32;
    /// Whether the player exists (`getNthPlayer(i) != NULL`).
    fn radar_player_exists(&self, player: i32) -> bool;
    /// `player->getRelationship(other->getDefaultTeam())`.
    fn radar_player_relationship_to_player(&self, player: i32, other: i32) -> Relationship;
    /// `player->getRelationship(obj->getTeam())`.
    fn radar_player_relationship_to_object(&self, player: i32, obj: ObjectId) -> Relationship;
    /// `player->isPlayerActive()`.
    fn radar_is_player_active(&self, player: i32) -> bool;
    /// `player->hasRadar()`.
    fn radar_player_has_radar(&self, player: i32) -> bool;
    /// `TheTerrainLogic->getExtent(&extent)`.
    fn radar_map_extent(&self) -> Region3D;
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn radar_ground_height(&self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->isUnderwater(x, y, &waterZ, &terrainZ)`.
    fn radar_is_underwater(&self, x: f32, y: f32) -> RadarUnderwater;
}

/// Which of `MiscAudio`'s sounds a radar call plays (`setPlayerIndex(localPlayer)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadarSound {
    /// `m_radarHarvesterUnderAttackSound`.
    HarvesterUnderAttack,
    /// `m_radarStructureUnderAttackSound` (upstream's unit, structure and other under-attack sound alike).
    StructureUnderAttack,
    /// `m_radarInfiltrationSound`.
    Infiltration,
}

/// What a radar call does besides the radar: for the control bar, the UI messages, the audio and EVA.
#[derive(Clone, Debug, PartialEq)]
pub enum RadarFeedback {
    /// `TheControlBar->triggerRadarAttackGlow()`.
    AttackGlow,
    /// `TheInGameUI->message(label)`.
    Message(&'static [u8]),
    /// `TheAudio->addAudioEvent(&sound)` with `setPlayerIndex(player)`.
    Sound { sound: RadarSound, player_index: Option<i32> },
    /// `TheEva->setShouldPlay(message)`, by name (`EVA_BaseUnderAttack`, `EVA_AllyUnderAttack`).
    Eva(&'static str),
}

/// What [`Radar::take_events`] made of a frame's events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RadarFrame {
    /// The feedback, in order.
    pub feedback: Vec<RadarFeedback>,
    /// The indices of the `StealthDetectorFeedback` events whose `tryEvent` refused: their sound, message and EVA do not
    /// play (`StealthDetectorUpdate`'s `doFeedback`).
    pub refused: Vec<usize>,
}

/// `radarColorLookupTable`: each event type's two colours.
const RADAR_COLOR_LOOKUP_TABLE: [(RadarEventType, [u32; 4], [u32; 4]); 11] = [
    (RADAR_EVENT_CONSTRUCTION, [128, 128, 255, 255], [128, 255, 255, 255]),
    (RADAR_EVENT_UPGRADE, [128, 0, 64, 255], [255, 185, 220, 255]),
    (RADAR_EVENT_UNDER_ATTACK, [255, 0, 0, 255], [255, 128, 128, 255]),
    (RADAR_EVENT_INFORMATION, [255, 255, 0, 255], [255, 255, 128, 255]),
    (RADAR_EVENT_BEACON_PULSE, [255, 255, 0, 255], [255, 255, 128, 255]),
    (RADAR_EVENT_INFILTRATION, [0, 255, 255, 255], [128, 255, 255, 255]),
    (RADAR_EVENT_BATTLE_PLAN, [255, 255, 255, 255], [255, 255, 255, 255]),
    (RADAR_EVENT_STEALTH_DISCOVERED, [0, 255, 0, 255], [0, 128, 0, 255]),
    (RADAR_EVENT_STEALTH_NEUTRALIZED, [0, 255, 0, 255], [0, 128, 0, 255]),
    (RADAR_EVENT_FAKE, [0, 0, 0, 0], [0, 0, 0, 0]),
    (RADAR_EVENT_INVALID, [0, 0, 0, 0], [0, 0, 0, 0]),
];

fn rgba(c: [u32; 4]) -> RGBAColorInt {
    RGBAColorInt { red: c[0], green: c[1], blue: c[2], alpha: c[3] }
}

/// `class Radar` (the device-independent half; [`crate::client::w3d_radar`] is `W3DRadar`'s).
#[derive(Clone, Debug, PartialEq)]
pub struct Radar {
    /// `m_radarHidden`: "true when radar is not visible".
    pub radar_hidden: bool,
    /// `m_radarForceOn`: "true when radar is forced to be on".
    pub radar_force_on: bool,
    /// `m_objectList`: every object not the local player's, grouped by priority (lowest first).
    pub object_list: Vec<RadarObject>,
    /// `m_localObjectList`: the local player's, "sorted in exactly the same priority as the regular object list".
    pub local_object_list: Vec<RadarObject>,
    /// `m_terrainAverageZ`.
    pub terrain_average_z: f32,
    /// `m_waterAverageZ`.
    pub water_average_z: f32,
    /// `m_xSample`.
    pub x_sample: f32,
    /// `m_ySample`.
    pub y_sample: f32,
    /// `m_event`.
    pub event: [RadarEvent; MAX_RADAR_EVENTS],
    /// `m_nextFreeRadarEvent`.
    pub next_free_radar_event: i32,
    /// `m_lastRadarEvent` (-1: none).
    pub last_radar_event: i32,
    /// `m_radarWindow`'s screen position and size (`winGetScreenPosition`, `winGetSize`); `None` without the window.
    pub radar_window: Option<(ICoord2D, ICoord2D)>,
    /// `m_mapExtent`.
    pub map_extent: Region3D,
    /// `m_queueTerrainRefreshFrame`.
    pub queue_terrain_refresh_frame: u32,
    /// Set when `refreshTerrain` runs (by `update` after a queued refresh, or a script): the device's terrain texture is
    /// to be rebuilt (`W3DRadar::refreshTerrain` extends it with `buildTerrainTexture`). Cleared by the device.
    pub terrain_refresh_wanted: bool,
}

impl Default for Radar {
    fn default() -> Self {
        Self::new()
    }
}

impl Radar {
    /// `Radar::Radar()`: no window, empty lists, not hidden, not forced, a zero extent, and every event cleared.
    pub fn new() -> Self {
        let mut r = Radar {
            radar_hidden: false,
            radar_force_on: false,
            object_list: Vec::new(),
            local_object_list: Vec::new(),
            terrain_average_z: 0.0,
            water_average_z: 0.0,
            x_sample: 0.0,
            y_sample: 0.0,
            event: [RadarEvent::default(); MAX_RADAR_EVENTS],
            next_free_radar_event: 0,
            last_radar_event: -1,
            radar_window: None,
            map_extent: Region3D::default(),
            queue_terrain_refresh_frame: 0,
            terrain_refresh_wanted: false,
        };
        r.clear_all_events();
        r
    }

    /// `clearAllEvents()`: the next free index 0, no last event, every event zeroed.
    pub fn clear_all_events(&mut self) {
        self.next_free_radar_event = 0;
        self.last_radar_event = -1;
        self.event = [RadarEvent::default(); MAX_RADAR_EVENTS];
    }

    /// `deleteListResources()`: both lists emptied.
    pub fn delete_list_resources(&mut self) {
        self.local_object_list.clear();
        self.object_list.clear();
    }

    /// `reset()`: the lists and events cleared, and the radar no longer forced on (hidden stays as it was).
    pub fn reset(&mut self) {
        self.delete_list_resources();
        self.clear_all_events();
        self.radar_force_on = false;
    }

    /// `update()`: an active event past its die frame becomes inactive; a queued terrain refresh runs once
    /// `RADAR_QUEUE_TERRAIN_REFRESH_DELAY` frames have passed.
    pub fn update(&mut self, host: &impl RadarHost) {
        let this_frame = host.radar_frame();
        for e in self.event.iter_mut() {
            if e.active && e.create_frame != 0 && this_frame > e.die_frame {
                e.active = false;
            }
        }
        if self.queue_terrain_refresh_frame != 0 && (this_frame.wrapping_sub(self.queue_terrain_refresh_frame) as f32) > RADAR_QUEUE_TERRAIN_REFRESH_DELAY {
            self.refresh_terrain();
        }
    }

    /// `newMap(terrain)`: a reset; the map's extent and the sample spacing (`width / RADAR_CELL_WIDTH`); and the average
    /// terrain and water heights over every second cell of every second row (`isUnderwater`'s terrain Z, each average
    /// over at least one sample). Upstream also finds the radar window here ([`Radar::set_radar_window`]).
    pub fn new_map(&mut self, host: &impl RadarHost) {
        self.reset();
        self.map_extent = host.radar_map_extent();
        self.x_sample = self.map_extent.width() / RADAR_CELL_WIDTH as f32;
        self.y_sample = self.map_extent.height() / RADAR_CELL_HEIGHT as f32;
        let (mut terrain_samples, mut water_samples) = (0i32, 0i32);
        self.terrain_average_z = 0.0;
        self.water_average_z = 0.0;
        // "since we're averaging let's skip every second sample..."
        let mut world_point = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        let mut y = 0;
        while y < RADAR_CELL_HEIGHT {
            world_point.x = 0.0;
            let mut x = 0;
            while x < RADAR_CELL_WIDTH {
                let u = host.radar_is_underwater(world_point.x, world_point.y);
                if u.underwater {
                    self.water_average_z += u.terrain_z;
                    water_samples += 1;
                } else {
                    self.terrain_average_z += u.terrain_z;
                    terrain_samples += 1;
                }
                x += 2;
                world_point.x += 2.0 * self.x_sample;
            }
            y += 2;
            world_point.y += 2.0 * self.y_sample;
        }
        // "avoid divide by zeros"
        if terrain_samples == 0 {
            terrain_samples = 1;
        }
        if water_samples == 0 {
            water_samples = 1;
        }
        self.terrain_average_z /= int_to_real(terrain_samples);
        self.water_average_z /= int_to_real(water_samples);
    }

    /// `m_radarWindow` (`ControlBar.wnd:LeftHUD`): its screen position and size, or none.
    pub fn set_radar_window(&mut self, window: Option<(ICoord2D, ICoord2D)>) {
        self.radar_window = window;
    }

    /// `isRadarWindow(window)`'s question, for a screen pixel: whether it is inside the radar window.
    pub fn is_in_radar_window(&self, pixel: &ICoord2D) -> bool {
        self.radar_window.is_some_and(|(p, s)| pixel.x >= p.x && pixel.y >= p.y && pixel.x < p.x + s.x && pixel.y < p.y + s.y)
    }

    /// `isPriorityVisible(priority)`: not `INVALID` or `NOT_ON_RADAR`.
    pub fn is_priority_visible(priority: u8) -> bool {
        !matches!(priority, RADAR_PRIORITY_INVALID | RADAR_PRIORITY_NOT_ON_RADAR)
    }

    /// `hide(hide)`.
    pub fn hide(&mut self, hide: bool) {
        self.radar_hidden = hide;
    }

    /// `forceOn(force)`.
    pub fn force_on(&mut self, force: bool) {
        self.radar_force_on = force;
    }

    /// Whether the radar is on for the local player: `isRadarForced() || (!isRadarHidden() && player->hasRadar())`
    /// (`ControlBarCallback.cpp`'s `LeftHUDInput`, `W3DControlBar.cpp`'s draw, `InGameUI.cpp`).
    pub fn is_on_for(&self, host: &impl RadarHost) -> bool {
        self.radar_force_on || (!self.radar_hidden && host.radar_local_player().is_some_and(|p| host.radar_player_has_radar(p)))
    }

    /// `addObject(obj)`: an object of a visible priority is listed with its colour (see the module docs for the
    /// disguise and the garrison), at the head of its priority's group, in the local player's list or the other.
    pub fn add_object(&mut self, obj: ObjectId, host: &mut impl RadarHost) {
        let new_priority = host.radar_priority(obj);
        if !Self::is_priority_visible(new_priority) {
            return;
        }
        // (upstream asserts no radar data here; an id already listed is replaced)
        self.remove_object(obj);
        let mut new_obj = RadarObject { object: obj, color: game_make_color(255, 255, 255, 255) };
        let mut player = host.radar_controlling_player(obj);
        let mut use_indicator_color = true;
        let local = host.radar_local_player();
        if host.radar_is_kind_of(obj, KINDOF_DISGUISER) {
            // "Because we have support for disguised units pretending to be units from another team, we need to
            // intercept it here and make sure it's rendered appropriately based on which client is rendering it."
            if let Some(disguised) = host.radar_disguised_player(obj) {
                if let (Some(p), Some(client)) = (player, local) {
                    if host.radar_player_relationship_to_player(p, client) != Relationship::Allies && host.radar_is_player_active(client) {
                        // "Neutrals and enemies will see this disguised unit as the team it's disguised as."
                        player = host.radar_player_exists(disguised).then_some(disguised);
                        if player.is_some() {
                            use_indicator_color = false;
                        }
                    }
                }
            }
        }
        if let Some(apparent) = host.radar_contain_apparent_player(obj, local) {
            // "To handle Stealth garrison, ask containers what color they are drawing with to the local player."
            player = apparent;
            if player.is_some() {
                use_indicator_color = false;
            }
        }
        new_obj.color = match player {
            Some(p) if !use_indicator_color => host.radar_player_color(p),
            _ => host.radar_indicator_color(obj),
        };
        let local_list = host.radar_is_locally_controlled(obj);
        // "link object to master list at the head of it's priority section"
        let priorities: Vec<u8> = {
            let list = if local_list { &self.local_object_list } else { &self.object_list };
            list.iter().map(|r| r.object).collect::<Vec<_>>().into_iter().map(|o| host.radar_priority(o)).collect()
        };
        let list = if local_list { &mut self.local_object_list } else { &mut self.object_list };
        let mut at = list.len();
        let mut prev_priority = RADAR_PRIORITY_INVALID;
        for (i, &curr_priority) in priorities.iter().enumerate() {
            if (i == 0 || prev_priority < new_priority) && curr_priority >= new_priority {
                at = i;
                break;
            }
            prev_priority = curr_priority;
        }
        list.insert(at, new_obj);
    }

    /// `deleteFromList(obj, list)`: whether the object was found (and removed).
    fn delete_from_list(obj: ObjectId, list: &mut Vec<RadarObject>) -> bool {
        match list.iter().position(|r| r.object == obj) {
            Some(i) => {
                list.remove(i);
                true
            }
            None => false,
        }
    }

    /// `removeObject(obj)`: from the local list, else the other (nothing for an object not listed).
    pub fn remove_object(&mut self, obj: ObjectId) {
        if !Self::delete_from_list(obj, &mut self.local_object_list) {
            Self::delete_from_list(obj, &mut self.object_list);
        }
    }

    /// `radarToWorld2D(radar, world)`: the cell clamped onto the radar, times the sample spacing (no Z).
    pub fn radar_to_world_2d(&self, radar: &ICoord2D) -> Coord3D {
        let x = radar.x.clamp(0, RADAR_CELL_WIDTH - 1);
        let y = radar.y.clamp(0, RADAR_CELL_HEIGHT - 1);
        Coord3D { x: x as f32 * self.x_sample, y: y as f32 * self.y_sample, z: 0.0 }
    }

    /// `radarToWorld(radar, world)`: [`Radar::radar_to_world_2d`] on the ground.
    pub fn radar_to_world(&self, radar: &ICoord2D, host: &impl RadarHost) -> Coord3D {
        let mut w = self.radar_to_world_2d(radar);
        w.z = host.radar_ground_height(w.x, w.y);
        w
    }

    /// `worldToRadar(world, radar)`: the point over the sample spacing (truncated, as the C conversion), clamped onto
    /// the radar. Upstream's off-map check is commented out ("an insanity check"), so every point translates.
    pub fn world_to_radar(&self, world: &Coord3D) -> ICoord2D {
        let x = (world.x / self.x_sample) as i32;
        let y = (world.y / self.y_sample) as i32;
        ICoord2D { x: x.clamp(0, RADAR_CELL_WIDTH - 1), y: y.clamp(0, RADAR_CELL_HEIGHT - 1) }
    }

    /// `findDrawPositions(startX, startY, width, height, &ul, &lr)`: where the terrain image sits inside the radar area
    /// so that the map keeps its aspect ratio (bars above and below a wide map, beside a tall one).
    pub fn find_draw_positions(&self, start_x: i32, start_y: i32, width: i32, height: i32) -> (ICoord2D, ICoord2D) {
        let ratio_width = self.map_extent.width() / (width as f32 * 1.0);
        let ratio_height = self.map_extent.height() / (height as f32 * 1.0);
        let (mut ul, mut lr);
        if ratio_width >= ratio_height {
            let rx = self.map_extent.width() / ratio_width;
            let ry = self.map_extent.height() / ratio_width;
            ul = ICoord2D { x: 0, y: ((height as f32 - ry) / 2.0) as i32 };
            lr = ICoord2D { x: rx as i32, y: 0 };
            lr.y = height - ul.y;
        } else {
            let rx = self.map_extent.width() / ratio_height;
            let ry = self.map_extent.height() / ratio_height;
            ul = ICoord2D { x: ((width as f32 - rx) / 2.0) as i32, y: 0 };
            lr = ICoord2D { x: 0, y: ry as i32 };
            lr.x = width - ul.x;
        }
        // "make them pixel positions"
        ul.x += start_x;
        ul.y += start_y;
        lr.x += start_x;
        lr.y += start_y;
        (ul, lr)
    }

    /// `localPixelToRadar(pixel, radar)`: a pixel of the radar window (its top left (0, 0)) to the radar cell under it,
    /// `None` outside the map's image. Y is inverted ("+x = right, -y = down").
    pub fn local_pixel_to_radar(&self, pixel: &ICoord2D) -> Option<ICoord2D> {
        let (_, size) = self.radar_window?;
        let (ul, lr) = self.find_draw_positions(0, 0, size.x, size.y);
        let scaled_width = lr.x - ul.x;
        let scaled_height = lr.y - ul.y;
        if pixel.x < ul.x || pixel.x > lr.x || pixel.y < ul.y || pixel.y > lr.y {
            return None;
        }
        // (upstream divides by zero for an empty window or map)
        if scaled_width == 0 || scaled_height == 0 || size.y == 0 || size.x == 0 {
            return None;
        }
        let mut radar = ICoord2D::default();
        if scaled_width >= scaled_height {
            radar.x = (pixel.x - ul.x) * RADAR_CELL_WIDTH / scaled_width;
            radar.y = real_to_int(((pixel.y - ul.y) as f32 / int_to_real(scaled_height)) * size.y as f32);
            radar.y = (size.y - radar.y) * RADAR_CELL_HEIGHT / size.y;
        } else {
            radar.x = real_to_int(((pixel.x - ul.x) as f32 / int_to_real(scaled_width)) * size.x as f32);
            radar.x = radar.x * RADAR_CELL_WIDTH / size.x;
            radar.y = (size.y - pixel.y) * RADAR_CELL_HEIGHT / size.y;
        }
        Some(radar)
    }

    /// `screenPixelToWorld(pixel, world)`: a screen pixel in the radar window to the ground point it shows.
    pub fn screen_pixel_to_world(&self, pixel: &ICoord2D, host: &impl RadarHost) -> Option<Coord3D> {
        let (pos, _) = self.radar_window?;
        let local = ICoord2D { x: pixel.x - pos.x, y: pixel.y - pos.y };
        let radar = self.local_pixel_to_radar(&local)?;
        Some(self.radar_to_world(&radar, host))
    }

    /// `objectUnderRadarPixel(pixel)` (a pixel of the radar window): the first object of the local list, else of the
    /// other, whose radar cell is within one of the pixel's.
    pub fn object_under_radar_pixel(&self, pixel: &ICoord2D, host: &impl RadarHost) -> Option<ObjectId> {
        let radar = self.local_pixel_to_radar(pixel)?;
        self.search_list_for_radar_location_match(&self.local_object_list, &radar, host)
            .or_else(|| self.search_list_for_radar_location_match(&self.object_list, &radar, host))
    }

    /// `searchListForRadarLocationMatch(listHead, radarMatch)`.
    fn search_list_for_radar_location_match(&self, list: &[RadarObject], m: &ICoord2D, host: &impl RadarHost) -> Option<ObjectId> {
        for r in list {
            let Some(pos) = host.radar_object_position(r.object) else { continue };
            let radar = self.world_to_radar(&pos);
            if radar.x >= m.x - 1 && radar.x <= m.x + 1 && radar.y >= m.y - 1 && radar.y <= m.y + 1 {
                return Some(r.object);
            }
        }
        None
    }

    /// `createEvent(world, type, secondsToLive)`: an event in its type's colours (white for a type not in the table).
    pub fn create_event(&mut self, world: &Coord3D, event_type: RadarEventType, seconds_to_live: f32, host: &impl RadarHost) {
        let (c1, c2) = RADAR_COLOR_LOOKUP_TABLE
            .iter()
            .take_while(|e| e.0 != RADAR_EVENT_INVALID)
            .find(|e| e.0 == event_type)
            .map_or(([255, 255, 255, 255], [255, 255, 255, 255]), |e| (e.1, e.2));
        self.internal_create_event(world, event_type, seconds_to_live, &rgba(c1), &rgba(c2), host);
    }

    /// `createPlayerEvent(player, world, type, secondsToLive)`: an event in the player's colour and a darker one (each
    /// channel less three quarters of itself).
    pub fn create_player_event(&mut self, player: i32, world: &Coord3D, event_type: RadarEventType, seconds_to_live: f32, host: &impl RadarHost) {
        if !host.radar_player_exists(player) {
            return;
        }
        let c = host.radar_player_color(player) as u32;
        let c1 = RGBAColorInt { red: (c >> 16) & 0xff, green: (c >> 8) & 0xff, blue: c & 0xff, alpha: (c >> 24) & 0xff };
        // "the second will be a darker color 1"
        let dark_scale = 0.75f32;
        let darker = |v: u32| (v as i32 - real_to_int(v as f32 * dark_scale)).max(0) as u32;
        let c2 = RGBAColorInt { red: darker(c1.red), green: darker(c1.green), blue: darker(c1.blue), alpha: c1.alpha };
        self.internal_create_event(world, event_type, seconds_to_live, &c1, &c2, host);
    }

    /// `internalCreateEvent(world, type, secondsToLive, color1, color2)`: the next slot (wrapping after 64) takes the
    /// event, dying `secondsToLive` seconds on and fading for the last half second; it is the last event unless it is
    /// a beacon's pulse.
    fn internal_create_event(&mut self, world: &Coord3D, event_type: RadarEventType, seconds_to_live: f32, color1: &RGBAColorInt, color2: &RGBAColorInt, host: &impl RadarHost) {
        // "this many seconds before we hit the die frame we start to fade away"
        let seconds_before_die_to_fade = 0.5f32;
        let radar = self.world_to_radar(world);
        let frame = host.radar_frame();
        // (`getFrame() + LOGICFRAMES_PER_SECOND * secondsToLive`: an unsigned plus a Real, in Real)
        let die_frame = (frame as f32 + LOGICFRAMES_PER_SECOND as f32 * seconds_to_live) as u32;
        let fade_frame = (die_frame as f32 - LOGICFRAMES_PER_SECOND as f32 * seconds_before_die_to_fade) as u32;
        let i = self.next_free_radar_event as usize;
        self.event[i] = RadarEvent {
            event_type,
            active: true,
            create_frame: frame,
            die_frame,
            fade_frame,
            color1: *color1,
            color2: *color2,
            world_loc: *world,
            radar_loc: radar,
            sound_played: false,
        };
        // "record the index of this, our "last" radar event."
        if event_type != RADAR_EVENT_BEACON_PULSE {
            self.last_radar_event = self.next_free_radar_event;
        }
        self.next_free_radar_event += 1;
        if self.next_free_radar_event >= MAX_RADAR_EVENTS as i32 {
            self.next_free_radar_event = 0;
        }
    }

    /// `getLastEventLoc(&eventPos)`: where the last event (not a beacon's pulse) was.
    pub fn get_last_event_loc(&self) -> Option<Coord3D> {
        (self.last_radar_event != -1).then(|| self.event[self.last_radar_event as usize].world_loc)
    }

    /// `tryEvent(event, pos)`: no new event while one of the type was made less than ten seconds ago "within 250
    /// units" (the test as upstream writes it: see the module docs); else the event, and `true`.
    pub fn try_event(&mut self, event: RadarEventType, pos: &Coord3D, host: &impl RadarHost) -> bool {
        if event <= RADAR_EVENT_INVALID || event >= RADAR_EVENT_NUM_EVENTS {
            return false;
        }
        let current_frame = host.radar_frame();
        let close_enough_distance_sq = 250.0f32 * 250.0f32;
        let frames_between_events = (LOGICFRAMES_PER_SECOND * 10) as u32;
        for e in &self.event {
            if e.event_type == event {
                let w = &e.world_loc;
                // (upstream's expression, with C's precedence: `a.x - (p.x * a.x) - p.x + a.y - (p.y * a.y) - p.y`)
                #[allow(clippy::suspicious_operation_groupings)]
                let dist_squared = w.x - pos.x * w.x - pos.x + w.y - pos.y * w.y - pos.y;
                if dist_squared <= close_enough_distance_sq && current_frame.wrapping_sub(e.create_frame) < frames_between_events {
                    return false;
                }
            }
        }
        self.create_event(pos, event, 4.0, host);
        true
    }

    /// `tryUnderAttackEvent(obj)`: an under-attack event if [`Radar::try_event`] takes it, and then the attack glow and,
    /// "even if we don't have a radar", the message and the sound for a unit (a harvester's own), a victory structure
    /// (with EVA: base under attack for the local player's, ally under attack for an ally's) or anything else.
    pub fn try_under_attack_event(&mut self, obj: ObjectId, host: &impl RadarHost) -> Vec<RadarFeedback> {
        let mut out = Vec::new();
        let Some(pos) = host.radar_object_position(obj) else { return out };
        if !self.try_event(RADAR_EVENT_UNDER_ATTACK, &pos, host) {
            return out;
        }
        out.push(RadarFeedback::AttackGlow);
        let player = host.radar_local_player();
        let kind = |k| host.radar_is_kind_of(obj, k);
        if kind(KINDOF_INFANTRY) || kind(KINDOF_VEHICLE) {
            if kind(KINDOF_HARVESTER) {
                out.push(RadarFeedback::Message(b"RADAR:HarvesterUnderAttack"));
                out.push(RadarFeedback::Sound { sound: RadarSound::HarvesterUnderAttack, player_index: player });
            } else {
                out.push(RadarFeedback::Message(b"RADAR:UnitUnderAttack"));
                out.push(RadarFeedback::Sound { sound: RadarSound::StructureUnderAttack, player_index: player });
            }
        } else if kind(KINDOF_STRUCTURE) && kind(KINDOF_MP_COUNT_FOR_VICTORY) {
            // "play EVA. If its our object, play Base under attack."
            let owner = host.radar_controlling_player(obj);
            if owner.is_some() && owner == player {
                out.push(RadarFeedback::Eva("BaseUnderAttack"));
            } else if player.is_some_and(|p| host.radar_player_relationship_to_object(p, obj) == Relationship::Allies) {
                out.push(RadarFeedback::Eva("AllyUnderAttack"));
            }
            out.push(RadarFeedback::Message(b"RADAR:StructureUnderAttack"));
            out.push(RadarFeedback::Sound { sound: RadarSound::StructureUnderAttack, player_index: player });
        } else {
            out.push(RadarFeedback::Message(b"RADAR:UnderAttack"));
            out.push(RadarFeedback::Sound { sound: RadarSound::StructureUnderAttack, player_index: player });
        }
        out
    }

    /// `tryInfiltrationEvent(obj)`: for the local player's object only, an infiltration event (always made), the message
    /// and the sound.
    pub fn try_infiltration_event(&mut self, obj: ObjectId, host: &impl RadarHost) -> Vec<RadarFeedback> {
        let player = host.radar_local_player();
        // "We should only be warned against infiltrations that are taking place against us."
        if host.radar_controlling_player(obj) != player {
            return Vec::new();
        }
        let Some(pos) = host.radar_object_position(obj) else { return Vec::new() };
        self.create_event(&pos, RADAR_EVENT_INFILTRATION, 4.0, host);
        vec![RadarFeedback::Message(b"RADAR:Infiltration"), RadarFeedback::Sound { sound: RadarSound::Infiltration, player_index: player }]
    }

    /// `refreshTerrain(terrain)`: "no future queue is valid now"; the device rebuilds its terrain texture.
    pub fn refresh_terrain(&mut self) {
        self.queue_terrain_refresh_frame = 0;
        self.terrain_refresh_wanted = true;
    }

    /// `queueTerrainRefresh()`: a refresh `RADAR_QUEUE_TERRAIN_REFRESH_DELAY` frames from now (an earlier one queued is
    /// "simply just forgotten").
    pub fn queue_terrain_refresh(&mut self, host: &impl RadarHost) {
        self.queue_terrain_refresh_frame = host.radar_frame();
    }

    /// `Radar::crc(xfer)`: nothing (as upstream: the radar is not in the logic's CRC).
    pub fn crc(&self) {}

    /// One of the logic's client events, if it is a radar call (see the module docs): returns the feedback, and for a
    /// stealth detector's feedback whether it plays (`doFeedback`).
    pub fn take_event(&mut self, ev: &ClientEvent, host: &mut impl RadarHost) -> (Vec<RadarFeedback>, bool) {
        let mut feedback = Vec::new();
        let mut plays = true;
        match ev {
            ClientEvent::Object(ObjectClientEvent::Radar { call, object_id, position }) => match call {
                RadarCall::AddObject => self.add_object(*object_id, host),
                RadarCall::RemoveObject => self.remove_object(*object_id),
                RadarCall::TryUnderAttackEvent => feedback = self.try_under_attack_event(*object_id, host),
                RadarCall::TryInfiltrationEvent => feedback = self.try_infiltration_event(*object_id, host),
                RadarCall::TryFakeEvent => {
                    if let Some(p) = position.or_else(|| host.radar_object_position(*object_id)) {
                        self.try_event(RADAR_EVENT_FAKE, &p, host);
                    }
                }
            },
            ClientEvent::Object(ObjectClientEvent::RadarCreateEvent { position, event_type }) => self.create_event(position, *event_type, 4.0, host),
            // (`createPlayerEvent`'s sanity: nothing for a `NULL` player)
            ClientEvent::Object(ObjectClientEvent::RadarCreatePlayerEvent { player: Some(p), position, event_type }) => {
                self.create_player_event(*p, position, *event_type, 4.0, host)
            }
            ClientEvent::Object(ObjectClientEvent::RadarRefreshTerrain) => self.refresh_terrain(),
            // (`TerrainLogic::setActiveBoundary`: the device's terrain is rebuilt with the new extent)
            ClientEvent::Object(ObjectClientEvent::RadarNewMap) => {
                self.new_map(host);
                self.refresh_terrain();
            }
            ClientEvent::Object(ObjectClientEvent::StealthDetectorFeedback { feedback: f, try_radar_event, position, .. }) => {
                let (t, label): (RadarEventType, &'static [u8]) = match f {
                    StealthFeedback::Discovered => (RADAR_EVENT_STEALTH_DISCOVERED, b"MESSAGE:StealthDiscovered"),
                    StealthFeedback::Neutralized => (RADAR_EVENT_STEALTH_NEUTRALIZED, b"MESSAGE:StealthNeutralized"),
                };
                if *try_radar_event {
                    plays = self.try_event(t, position, host);
                } else {
                    self.create_event(position, t, 4.0, host);
                }
                if plays {
                    feedback.push(RadarFeedback::Message(label));
                }
            }
            ClientEvent::Player(PlayerClientEvent::RadarRemoveObject { object }) => self.remove_object(*object),
            ClientEvent::Player(PlayerClientEvent::RadarAddObject { object }) => self.add_object(*object, host),
            ClientEvent::Dispatch(ClientCall::RadarInformation { pos }) => self.create_event(pos, RADAR_EVENT_INFORMATION, 4.0, host),
            ClientEvent::Radar(r) => match r {
                crate::logic::client_event::RadarCall::CreateEvent { position, event_type } => self.create_event(position, *event_type, 4.0, host),
                crate::logic::client_event::RadarCall::Hide(h) => self.hide(*h),
                crate::logic::client_event::RadarCall::ForceOn(f) => self.force_on(*f),
                crate::logic::client_event::RadarCall::RefreshTerrain => self.refresh_terrain(),
            },
            ClientEvent::ObjectDestroyed { object_id } => self.remove_object(*object_id),
            _ => {}
        }
        (feedback, plays)
    }

    /// A frame's events ([`Radar::take_event`] each, in order), then `update()`.
    pub fn take_events(&mut self, events: &[ClientEvent], host: &mut impl RadarHost) -> RadarFrame {
        let mut out = RadarFrame::default();
        for (i, e) in events.iter().enumerate() {
            let (f, plays) = self.take_event(e, host);
            out.feedback.extend(f);
            if !plays {
                out.refused.push(i);
            }
        }
        self.update(host);
        out
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A World of a few objects on a flat 1000 x 500 map (heights 10), player 1 local, 2 its enemy, 3 its ally.
    #[derive(Default)]
    pub(crate) struct Host {
        pub frame: u32,
        pub local: Option<i32>,
        /// id -> (position, player, priority, kinds)
        pub objects: BTreeMap<ObjectId, (Coord3D, Option<i32>, u8, Vec<KindOfType>)>,
        pub has_radar: bool,
        pub extent: Region3D,
    }

    impl Host {
        pub(crate) fn new() -> Self {
            Host { local: Some(1), extent: Region3D { lo: Coord3D { x: 0.0, y: 0.0, z: 0.0 }, hi: Coord3D { x: 1000.0, y: 500.0, z: 40.0 } }, ..Default::default() }
        }
        pub(crate) fn put(&mut self, id: ObjectId, x: f32, y: f32, player: i32, priority: u8, kinds: &[KindOfType]) {
            self.objects.insert(id, (Coord3D { x, y, z: 10.0 }, Some(player), priority, kinds.to_vec()));
        }
    }

    impl RadarHost for Host {
        fn radar_frame(&self) -> u32 {
            self.frame
        }
        fn radar_local_player(&self) -> Option<i32> {
            self.local
        }
        fn radar_priority(&mut self, obj: ObjectId) -> u8 {
            self.objects.get(&obj).map_or(RADAR_PRIORITY_INVALID, |o| o.2)
        }
        fn radar_object_position(&self, obj: ObjectId) -> Option<Coord3D> {
            self.objects.get(&obj).map(|o| o.0)
        }
        fn radar_is_locally_controlled(&self, obj: ObjectId) -> bool {
            self.radar_controlling_player(obj) == self.local
        }
        fn radar_controlling_player(&self, obj: ObjectId) -> Option<i32> {
            self.objects.get(&obj).and_then(|o| o.1)
        }
        fn radar_indicator_color(&self, obj: ObjectId) -> i32 {
            self.radar_controlling_player(obj).map_or(0, |p| self.radar_player_color(p))
        }
        fn radar_is_kind_of(&self, obj: ObjectId, kind: KindOfType) -> bool {
            self.objects.get(&obj).is_some_and(|o| o.3.contains(&kind))
        }
        fn radar_disguised_player(&mut self, _obj: ObjectId) -> Option<i32> {
            None
        }
        fn radar_contain_apparent_player(&mut self, _obj: ObjectId, _observer: Option<i32>) -> Option<Option<i32>> {
            None
        }
        fn radar_player_color(&self, player: i32) -> i32 {
            game_make_color(player as u8 * 60, 100, 200, 255)
        }
        fn radar_player_exists(&self, player: i32) -> bool {
            (0..4).contains(&player)
        }
        fn radar_player_relationship_to_player(&self, player: i32, other: i32) -> Relationship {
            if player == other || player + other == 4 {
                Relationship::Allies
            } else {
                Relationship::Enemies
            }
        }
        fn radar_player_relationship_to_object(&self, player: i32, obj: ObjectId) -> Relationship {
            self.radar_controlling_player(obj).map_or(Relationship::Neutral, |o| self.radar_player_relationship_to_player(player, o))
        }
        fn radar_is_player_active(&self, _player: i32) -> bool {
            true
        }
        fn radar_player_has_radar(&self, _player: i32) -> bool {
            self.has_radar
        }
        fn radar_map_extent(&self) -> Region3D {
            self.extent
        }
        fn radar_ground_height(&self, _x: f32, _y: f32) -> f32 {
            10.0
        }
        fn radar_is_underwater(&self, x: f32, _y: f32) -> RadarUnderwater {
            // a lake over the western fifth, its floor at 2
            if x < 200.0 {
                RadarUnderwater { underwater: true, water_z: Some(5.0), terrain_z: 2.0 }
            } else {
                RadarUnderwater { underwater: false, water_z: None, terrain_z: 10.0 }
            }
        }
    }

    fn radar(host: &Host) -> Radar {
        let mut r = Radar::new();
        r.new_map(host);
        r
    }

    /// #35: `worldToRadar` and `radarToWorld` on a 1000 x 500 map. Predicted from Radar.cpp before the run: the samples
    /// are 1000 / 128 = 7.8125 and 500 / 128 = 3.90625 (exact in binary); a cell's corner goes to the world and back to
    /// the same cell for every one of the 128 x 128; a world point goes to the cell whose corner is at or below it, so
    /// the round trip world -> radar -> world moves a point down by less than one sample; points off the map clamp to
    /// the edge cells (-50 -> 0, 1e6 -> 127); `radarToWorld` puts the point on the ground (z 10). `newMap`'s averages:
    /// 26 of the 64 sampled columns (x = 0, 15.6, ... 195.3) are under the lake, so the water average is 2 and the
    /// terrain's 10.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn world_to_radar_and_back() {
        let host = Host::new();
        let r = radar(&host);
        assert_eq!((r.x_sample, r.y_sample), (7.8125, 3.90625));
        assert_eq!((r.terrain_average_z, r.water_average_z), (10.0, 2.0));
        for y in 0..RADAR_CELL_HEIGHT {
            for x in 0..RADAR_CELL_WIDTH {
                let c = ICoord2D { x, y };
                assert_eq!(r.world_to_radar(&r.radar_to_world_2d(&c)), c);
            }
        }
        for &(x, y) in &[(0.0f32, 0.0f32), (3.0, 499.0), (511.7, 250.2), (999.9, 1.0), (7.8125, 3.90625)] {
            let p = Coord3D { x, y, z: 0.0 };
            let back = r.radar_to_world(&r.world_to_radar(&p), &host);
            assert!(back.x <= x && x - back.x < r.x_sample && back.y <= y && y - back.y < r.y_sample, "{p:?} -> {back:?}");
            assert_eq!(back.z, 10.0);
        }
        assert_eq!(r.world_to_radar(&Coord3D { x: -50.0, y: 1e6, z: 0.0 }), ICoord2D { x: 0, y: 127 });
        assert_eq!(r.radar_to_world_2d(&ICoord2D { x: 500, y: -3 }), Coord3D { x: 127.0 * 7.8125, y: 0.0, z: 0.0 });
    }

    /// #35: the radar window's pixels. Predicted before the run: a 200 x 200 window over the 2:1 map draws the map 200 x
    /// 100 between y 50 and 150 (`findDrawPositions`); a pixel above or below that is no cell; the top-left corner of
    /// the image (0, 50) is cell (0, 128) (y inverted: `(200 - 0) * 128 / 200`; upstream does not clamp here, and
    /// `radarToWorld` clamps it to row 127), the bottom-right (200, 150) is (128, 0); the centre (100, 100) is (64, 64).
    /// `screenPixelToWorld` subtracts the window's screen position (300, 400) and so gives the same point as the local
    /// pixel; the centre is (500, 250) on the ground.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn radar_window_pixels_map_onto_the_map() {
        let host = Host::new();
        let mut r = radar(&host);
        r.set_radar_window(Some((ICoord2D { x: 300, y: 400 }, ICoord2D { x: 200, y: 200 })));
        assert_eq!(r.find_draw_positions(0, 0, 200, 200), (ICoord2D { x: 0, y: 50 }, ICoord2D { x: 200, y: 150 }));
        assert_eq!(r.local_pixel_to_radar(&ICoord2D { x: 100, y: 49 }), None);
        assert_eq!(r.local_pixel_to_radar(&ICoord2D { x: 100, y: 151 }), None);
        assert_eq!(r.local_pixel_to_radar(&ICoord2D { x: 0, y: 50 }), Some(ICoord2D { x: 0, y: 128 }));
        assert_eq!(r.local_pixel_to_radar(&ICoord2D { x: 200, y: 150 }), Some(ICoord2D { x: 128, y: 0 }));
        assert_eq!(r.local_pixel_to_radar(&ICoord2D { x: 100, y: 100 }), Some(ICoord2D { x: 64, y: 64 }));
        assert_eq!(r.screen_pixel_to_world(&ICoord2D { x: 400, y: 500 }, &host), Some(Coord3D { x: 500.0, y: 250.0, z: 10.0 }));
        assert!(r.is_in_radar_window(&ICoord2D { x: 300, y: 599 }) && !r.is_in_radar_window(&ICoord2D { x: 500, y: 500 }));
    }

    /// #35: the lists. Predicted before the run: objects go in priority order (structure 2 before unit 3) with a new
    /// object at the head of its priority's group; the local player's go to the local list; a `NOT_ON_RADAR` object is
    /// not listed; the colour is the indicator colour (the player's here); removing takes it from whichever list holds
    /// it; `objectUnderRadarPixel` finds the local object within a cell of the pixel's cell first.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn objects_are_listed_by_priority() {
        let mut host = Host::new();
        host.put(10, 100.0, 100.0, 2, 3, &[]);
        host.put(11, 120.0, 100.0, 2, 2, &[]);
        host.put(12, 140.0, 100.0, 2, 3, &[]);
        host.put(13, 500.0, 250.0, 1, 3, &[]);
        host.put(14, 600.0, 250.0, 2, RADAR_PRIORITY_NOT_ON_RADAR, &[]);
        let mut r = radar(&host);
        for id in [10, 11, 12, 13, 14] {
            r.add_object(id, &mut host);
        }
        assert_eq!(r.object_list.iter().map(|o| o.object).collect::<Vec<_>>(), vec![11, 12, 10]);
        assert_eq!(r.local_object_list, vec![RadarObject { object: 13, color: host.radar_player_color(1) }]);
        r.set_radar_window(Some((ICoord2D::default(), ICoord2D { x: 128, y: 128 })));
        // (500, 250) is cell (64, 64); the window's image is 128 x 64 from y 32: pixel (64, 64) is cell (64, 64)
        assert_eq!(r.object_under_radar_pixel(&ICoord2D { x: 64, y: 64 }, &host), Some(13));
        r.remove_object(13);
        r.remove_object(10);
        assert!(r.local_object_list.is_empty());
        assert_eq!(r.object_list.iter().map(|o| o.object).collect::<Vec<_>>(), vec![11, 12]);
    }

    /// #35: event pings and their expiry. Predicted before the run: an event made at frame 100 for four seconds dies at
    /// frame 220 and starts fading at 205; `update` keeps it active through frame 220 and not at 221; the under-attack
    /// colours are red and light red; `getLastEventLoc` is the last event's place, and a beacon's pulse does not take that
    /// over; the 65th event overwrites the first slot.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn events_ping_and_expire() {
        let mut host = Host::new();
        let mut r = radar(&host);
        host.frame = 100;
        let at = Coord3D { x: 250.0, y: 125.0, z: 0.0 };
        r.create_event(&at, RADAR_EVENT_UNDER_ATTACK, 4.0, &host);
        let e = r.event[0];
        assert_eq!((e.create_frame, e.die_frame, e.fade_frame, e.radar_loc), (100, 220, 205, ICoord2D { x: 32, y: 32 }));
        assert_eq!((e.color1, e.color2), (rgba([255, 0, 0, 255]), rgba([255, 128, 128, 255])));
        host.frame = 220;
        r.update(&host);
        assert!(r.event[0].active);
        host.frame = 221;
        r.update(&host);
        assert!(!r.event[0].active);
        assert_eq!(r.get_last_event_loc(), Some(at));
        r.create_event(&Coord3D { x: 9.0, y: 9.0, z: 0.0 }, RADAR_EVENT_BEACON_PULSE, 1.0, &host);
        assert_eq!(r.get_last_event_loc(), Some(at));
        for _ in 0..63 {
            r.create_event(&Coord3D { x: 1.0, y: 2.0, z: 0.0 }, RADAR_EVENT_INFORMATION, 4.0, &host);
        }
        assert_eq!(r.next_free_radar_event, 1);
        assert_eq!(r.event[0].event_type, RADAR_EVENT_INFORMATION);
        // a player's event: the player's colour and a quarter of it
        r.create_player_event(2, &at, RADAR_EVENT_BATTLE_PLAN, 4.0, &host);
        let c = r.event[1];
        assert_eq!((c.color1.red, c.color2.red, c.color1.green, c.color2.green, c.color1.blue, c.color2.blue), (120, 30, 100, 25, 200, 50));
    }

    /// #35: the under-attack EVA and its throttling. Predicted before the run, from Radar.cpp's `tryEvent` as written
    /// (its distance test passes for any two points of this map): the local player's victory structure attacked at frame
    /// 10 makes an event, the glow, `EVA_BaseUnderAttack`, `RADAR:StructureUnderAttack` and the structure sound; attacked
    /// again at frame 100 (3 seconds later) and its other structure 800 units away at frame 200 make nothing; at frame
    /// 311 (more than 10 seconds after 10) the far structure makes an event again; a harvester attacked at frame 312
    /// makes nothing (the same type); an infantry unit at frame 700 gets `RADAR:UnitUnderAttack` with the structure
    /// sound; an infiltration of an enemy's object makes nothing, of mine always an event.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn under_attack_eva_is_throttled() {
        let mut host = Host::new();
        host.put(1, 100.0, 100.0, 1, 2, &[KINDOF_STRUCTURE, KINDOF_MP_COUNT_FOR_VICTORY]);
        host.put(2, 900.0, 400.0, 1, 2, &[KINDOF_STRUCTURE, KINDOF_MP_COUNT_FOR_VICTORY]);
        host.put(3, 500.0, 200.0, 1, 3, &[KINDOF_VEHICLE, KINDOF_HARVESTER]);
        host.put(4, 510.0, 200.0, 1, 3, &[KINDOF_INFANTRY]);
        host.put(5, 520.0, 200.0, 2, 3, &[KINDOF_INFANTRY]);
        let mut r = radar(&host);
        let sound = RadarFeedback::Sound { sound: RadarSound::StructureUnderAttack, player_index: Some(1) };
        host.frame = 10;
        assert_eq!(
            r.try_under_attack_event(1, &host),
            vec![RadarFeedback::AttackGlow, RadarFeedback::Eva("BaseUnderAttack"), RadarFeedback::Message(b"RADAR:StructureUnderAttack"), sound.clone()]
        );
        host.frame = 100;
        assert!(r.try_under_attack_event(1, &host).is_empty());
        host.frame = 200;
        assert!(r.try_under_attack_event(2, &host).is_empty());
        host.frame = 311;
        assert_eq!(r.try_under_attack_event(2, &host).len(), 4);
        host.frame = 312;
        assert!(r.try_under_attack_event(3, &host).is_empty());
        host.frame = 700;
        assert_eq!(r.try_under_attack_event(4, &host), vec![RadarFeedback::AttackGlow, RadarFeedback::Message(b"RADAR:UnitUnderAttack"), sound]);
        assert_eq!(r.event.iter().filter(|e| e.event_type == RADAR_EVENT_UNDER_ATTACK).count(), 3);
        assert!(r.try_infiltration_event(5, &host).is_empty());
        assert_eq!(r.try_infiltration_event(4, &host).len(), 2);
        assert_eq!(r.try_infiltration_event(4, &host).len(), 2);
        assert_eq!(r.event.iter().filter(|e| e.event_type == RADAR_EVENT_INFILTRATION).count(), 2);
    }

    /// #35: the logic's events. Predicted before the run: a script's hide, force on and event reach the radar; an
    /// object's add and remove events list and unlist it; a stealth detector's `tryEvent` feedback plays the first time
    /// and is refused the second (index 1 of the second batch), while one without `tryEvent` always plays. Added with the
    /// battle plan's and the wave guide's events: a player event of player 2 takes the player's colour (red 120); one of a
    /// `NULL` player makes nothing; a terrain refresh asks the device to rebuild.
    ///
    /// *Results (2026-11-06):* as predicted, first run (the player and refresh lines: first run after they were added).
    #[test]
    fn the_logics_events_reach_the_radar() {
        use crate::logic::client_event::RadarCall as Script;
        let mut host = Host::new();
        host.put(7, 300.0, 300.0, 2, 3, &[]);
        let mut r = radar(&host);
        host.frame = 5;
        let stealth = |t: bool| ClientEvent::Object(ObjectClientEvent::StealthDetectorFeedback { feedback: StealthFeedback::Discovered, try_radar_event: t, position: Coord3D { x: 50.0, y: 50.0, z: 0.0 }, player_index: Some(1), eva: -1 });
        let f = r.take_events(
            &[
                ClientEvent::Radar(Script::Hide(true)),
                ClientEvent::Radar(Script::ForceOn(true)),
                ClientEvent::Object(ObjectClientEvent::Radar { call: RadarCall::AddObject, object_id: 7, position: None }),
                stealth(true),
            ],
            &mut host,
        );
        assert!(r.radar_hidden && r.radar_force_on && r.is_on_for(&host));
        assert_eq!(r.object_list.len(), 1);
        assert_eq!(f.feedback, vec![RadarFeedback::Message(b"MESSAGE:StealthDiscovered")]);
        assert!(f.refused.is_empty());
        let f = r.take_events(&[ClientEvent::Object(ObjectClientEvent::Radar { call: RadarCall::RemoveObject, object_id: 7, position: None }), stealth(true), stealth(false)], &mut host);
        assert_eq!(f.refused, vec![1]);
        assert!(r.object_list.is_empty());
        let next = r.next_free_radar_event as usize;
        r.terrain_refresh_wanted = false;
        r.take_events(
            &[
                ClientEvent::Object(ObjectClientEvent::RadarCreatePlayerEvent { player: Some(2), position: Coord3D { x: 10.0, y: 10.0, z: 0.0 }, event_type: RADAR_EVENT_BATTLE_PLAN }),
                ClientEvent::Object(ObjectClientEvent::RadarCreatePlayerEvent { player: None, position: Coord3D { x: 10.0, y: 10.0, z: 0.0 }, event_type: RADAR_EVENT_BATTLE_PLAN }),
                ClientEvent::Object(ObjectClientEvent::RadarRefreshTerrain),
            ],
            &mut host,
        );
        assert_eq!((r.event[next].event_type, r.event[next].color1.red), (RADAR_EVENT_BATTLE_PLAN, 120));
        assert_eq!(r.next_free_radar_event as usize, next + 1);
        assert!(r.terrain_refresh_wanted);
        r.take_events(&[ClientEvent::Radar(Script::ForceOn(false)), ClientEvent::Radar(Script::Hide(false))], &mut host);
        assert!(!r.is_on_for(&host));
        host.has_radar = true;
        assert!(r.is_on_for(&host));
    }
}
