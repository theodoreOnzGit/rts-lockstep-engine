// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the events `Object` (Object.cpp) sends to the client singletons. Each
// variant names the upstream call it replaces.

//! The client notifications of `Object` (layer 8), nested in
//! [`ClientEvent::Object`](crate::logic::client_event::ClientEvent::Object).
//!
//! Upstream's `Object` talks to `Drawable`, `TheAudio`, `TheRadar`,
//! `TheInGameUI`, `TheControlBar`, `TheEva` and `TheGameClient` (37 calls in
//! `Object.cpp`). None of them is read back by logic, so each is an event;
//! logic never holds a `Drawable` (the object keeps the client's
//! [`DrawableId`](crate::common::game_type::DrawableId)). The order of the
//! events is the order of the calls.
//!
//! **A call on `m_drawable` is an event only while the object is bound to a
//! drawable** (upstream tests `if (m_drawable)` first), so an unbound object
//! (under construction, or after `~Object`) sends none.

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;

/// The sounds `Object.cpp` asks `TheAudio` for. The client looks the name up
/// in its own `MiscAudio` (or, for [`ObjectSound::VoiceDefect`], in the
/// object's template: `getTemplate()->getVoiceDefect()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectSound {
    /// `MiscAudio::m_splatterVehiclePilotsBrain` (`setDisabledUntil`, `DISABLED_UNMANNED`).
    SplatterVehiclePilotsBrain,
    /// `MiscAudio::m_buildingDisabled`.
    BuildingDisabled,
    /// `MiscAudio::m_vehicleDisabled`.
    VehicleDisabled,
    /// `MiscAudio::m_buildingReenabled`.
    BuildingReenabled,
    /// `MiscAudio::m_vehicleReenabled`.
    VehicleReenabled,
    /// `MiscAudio::m_unitPromoted` (`onVeterancyLevelChanged`).
    UnitPromoted,
    /// `MiscAudio::m_defectorTimerTickSound` (`defect`).
    DefectorTimerTick,
    /// `getTemplate()->getVoiceDefect()` (`defect`).
    VoiceDefect,
}

/// The `TheEva->setShouldPlay(..)` messages `Object::onDie` sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaMessage {
    /// `EVA_BuldingLost`.
    BuildingLost,
    /// `EVA_UnitLost`.
    UnitLost,
}

/// The `TheRadar` calls of `Object.cpp`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadarCall {
    /// `TheRadar->addObject(obj)`.
    AddObject,
    /// `TheRadar->removeObject(obj)`.
    RemoveObject,
    /// `TheRadar->tryUnderAttackEvent(obj)`.
    TryUnderAttackEvent,
    /// `TheRadar->tryInfiltrationEvent(obj)`.
    TryInfiltrationEvent,
    /// `TheRadar->tryEvent(RADAR_EVENT_FAKE, pos)`.
    TryFakeEvent,
}

/// What `Drawable` is asked to do to its model-condition state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelConditionOp {
    /// `setModelConditionState(a)` / `setModelConditionFlags(set)`.
    Set(Vec<i32>),
    /// `clearModelConditionState(a)` / `clearModelConditionFlags(clr)`.
    Clear(Vec<i32>),
    /// `clearAndSetModelConditionState` / `clearAndSetModelConditionFlags`.
    ClearAndSet { clear: Vec<i32>, set: Vec<i32> },
}

/// One notification of `Object`. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectClientEvent {
    /// `TheAudio->addAudioEvent(&sound)` with `sound.setObjectID(id)` or
    /// `sound.setPosition(getPosition())` (whichever the call site does).
    Sound { sound: ObjectSound, object_id: ObjectId, position: Option<Coord3D> },
    /// `TheRadar->...(obj)`; `position` for [`RadarCall::TryFakeEvent`].
    Radar { call: RadarCall, object_id: ObjectId, position: Option<Coord3D> },
    /// `TheEva->setShouldPlay(message)`.
    Eva { message: EvaMessage },
    /// `TheInGameUI->objectChangedTeam(obj, oldPlayerIndex, newPlayerIndex)`.
    ObjectChangedTeam { object_id: ObjectId, old_player: i32, new_player: i32 },
    /// `TheInGameUI->removeIdleWorker(obj, playerIndex)`.
    RemoveIdleWorker { object_id: ObjectId, player_index: i32 },
    /// `TheInGameUI->addWorldAnimation(levelGainAnimation, pos, ..)`
    /// (`onVeterancyLevelChanged`); `position` already includes the health
    /// box offset. The animation's name and timing are global data the
    /// client reads.
    LevelGainAnimation { object_id: ObjectId, position: Coord3D },
    /// `TheControlBar->markUIDirty()`.
    ControlBarMarkUiDirty,
    // --- p1l9-upgrades ---
    /// `obj->getDrawable()->setTerrainDecal(decal)` (`ArmorUpgrade`).
    DrawableSetTerrainDecal { object_id: ObjectId, decal: i32 },
    // --- end p1l9-upgrades ---
    /// `TheGameClient->notifyTerrainObjectMoved(obj)`.
    TerrainObjectMoved { object_id: ObjectId },
    /// `m_drawable->changedTeam()`.
    DrawableChangedTeam { object_id: ObjectId },
    /// `m_drawable->reactToGeometryChange()`.
    DrawableReactToGeometryChange { object_id: ObjectId },
    /// `m_drawable->setSelectable(selectable)`.
    DrawableSetSelectable { object_id: ObjectId, selectable: bool },
    /// `m_drawable->setTintStatus(status)` (`on`) or `clearTintStatus(status)`.
    DrawableTint { object_id: ObjectId, status: i32, on: bool },
    /// The model-condition calls on `m_drawable`.
    DrawableModelCondition { object_id: ObjectId, op: ModelConditionOp },
    /// `m_drawable->setTerrainDecalFadeTarget(target, rate)`.
    DrawableTerrainDecalFadeTarget { object_id: ObjectId, target: f32, rate: f32 },
    /// `m_drawable->setDrawableHidden(hidden)` (`TunnelTracker::loadPostProcess`).
    DrawableSetHidden { object_id: ObjectId, hidden: bool },
    /// `m_drawable->flashAsSelected()`.
    DrawableFlashAsSelected { object_id: ObjectId },
    /// `draw->setID(drawableID)` (`Object::xfer` on a load): the drawable of
    /// this object takes the id it had when it was saved.
    DrawableSetId { object_id: ObjectId, drawable_id: i32 },
    /// The ambient-sound customisation of `updateObjValuesFromMapProperties`
    /// (the `objectSoundAmbient*` properties of the map object): the client
    /// builds its `DynamicAudioEventInfo` from these. `None` = property absent.
    DrawableAmbientSoundProperties { object_id: ObjectId, properties: AmbientSoundProperties },
    /// `m_drawable->clearAndSetModelConditionFlags(..)` for the day/night and
    /// weather properties of the map object (`objectTime`, `objectWeather`).
    DrawableTimeOrWeather { object_id: ObjectId, night: Option<bool>, snow: Option<bool> },
    // --- p1l9-example ---
    /// `FXList::doFXObj(fx, obj)`: an effect list played on the object (FX
    /// lists are client data; logic only names it). `UpgradeMux::performUpgradeFX`
    /// is the first caller.
    FxListDoFxObj { fx: Vec<u8>, object_id: ObjectId },
    // --- end p1l9-example ---
}

/// The `objectSoundAmbient*` properties of a map object, as read from its
/// `Dict` (`None` = the key is absent).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AmbientSoundProperties {
    /// `objectSoundAmbient`: the event name (empty = turn the sound off).
    pub name: Option<Vec<u8>>,
    /// `objectSoundAmbientCustomized`.
    pub customized: Option<bool>,
    /// `objectSoundAmbientEnabled`.
    pub enabled: Option<bool>,
    /// `objectSoundAmbientLooping`.
    pub looping: Option<bool>,
    /// `objectSoundAmbientLoopCount`.
    pub loop_count: Option<i32>,
    /// `objectSoundAmbientMinVolume`.
    pub min_volume: Option<f32>,
    /// `objectSoundAmbientVolume`.
    pub volume: Option<f32>,
    /// `objectSoundAmbientMinRange`.
    pub min_range: Option<f32>,
    /// `objectSoundAmbientMaxRange`.
    pub max_range: Option<f32>,
    /// `objectSoundAmbientPriority`.
    pub priority: Option<i32>,
}

impl AmbientSoundProperties {
    /// Whether the map object sets none of the properties.
    pub fn is_empty(&self) -> bool {
        *self == AmbientSoundProperties::default()
    }
}
