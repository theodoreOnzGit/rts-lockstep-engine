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
    // --- p1w11-sonnet ---
    /// `MiscAudio::m_crateHeal` (`HealCrateCollide`).
    CrateHeal,
    /// `MiscAudio::m_crateShroud` (`ShroudCrateCollide`).
    CrateShroud,
    /// `MiscAudio::m_crateMoney` (`MoneyCrateCollide`).
    CrateMoney,
    /// `MiscAudio::m_crateFreeUnit` (`UnitCrateCollide`, batch R13b).
    CrateFreeUnit,
    /// `MiscAudio::m_crateSalvage` (`SalvageCrateCollide`, batch R13c).
    CrateSalvage,
    /// `MiscAudio::m_defectorTimerDingSound` (`ObjectDefectionHelper`, when the period ends).
    DefectorTimerDing,
    /// batch R13: `MiscAudio::m_sabotageResetTimerBuilding` (the command centre and superweapon sabotage).
    SabotageResetTimerBuilding,
    /// batch R13: `MiscAudio::m_moneyWithdrawSound` (the supply centre and drop zone sabotage).
    MoneyWithdraw,
    /// batch R13: `MiscAudio::m_sabotageShutDownBuilding` (the other sabotage).
    SabotageShutDownBuilding,
    // --- end p1w11-sonnet ---
}

/// The `TheEva->setShouldPlay(..)` messages `Object::onDie` sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaMessage {
    /// `EVA_BuldingLost`.
    BuildingLost,
    /// `EVA_UnitLost`.
    UnitLost,
    /// batch R13: `EVA_BuildingSabotaged`.
    BuildingSabotaged,
    /// batch R13: `EVA_CashStolen`.
    CashStolen,
    /// batch R13e: `EVA_VehicleStolen`.
    VehicleStolen,
}

// --- p1w11-sonnet ---
/// The superweapons `SpecialPowerModule::aboutToDoSpecialPower` announces with `TheEva->setShouldPlay(EVA_SuperweaponLaunched_<who>_<weapon>)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaSuperweapon {
    /// The particle uplink cannon.
    ParticleCannon,
    /// The neutron missile.
    Nuke,
    /// The scud storm.
    ScudStorm,
    /// The GPS scrambler.
    GpsScrambler,
    /// The sneak attack.
    SneakAttack,
}

/// Whose superweapon it is, to the local player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaWhose {
    /// The local player's own.
    Own,
    /// An ally's (anything that is not an enemy's).
    Ally,
    /// An enemy's.
    Enemy,
}
// --- end p1w11-sonnet ---

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

// --- p1w13-sonnet ---
/// batch R26: what a `StealthUpdate` does to its object's drawable. The decisions that only change what is drawn are the client's: whether the
/// drawable is selected (the control bar, the new drawable's selection) and whether it is hidden (a disguise restored after a load).
#[derive(Clone, Debug, PartialEq)]
pub enum StealthClientOp {
    /// `draw->setEffectiveOpacity(opacity, explicit)` (`explicit` -1: the default argument).
    EffectiveOpacity { opacity: f32, explicit: f32 },
    /// `draw->setSecondMaterialPassOpacity(opacity)`.
    SecondMaterialPassOpacity(f32),
    /// `draw->setStealthLook(look)` (a `StealthLookType` number).
    StealthLook(i32),
    /// `changeVisualDisguise`'s swap: the client destroys the drawable and makes one of `template` with the old one's position, orientation and
    /// model condition flags (then `updateDrawable`).
    ReplaceDrawable { template: Vec<u8> },
    /// `draw->setIndicatorColor(color)` (the new drawable's).
    IndicatorColor(i32),
    /// `if( selected ) TheInGameUI->selectDrawable( draw );`: the new drawable is selected if the old one was.
    SelectIfWasSelected,
    /// `if( wasHidden && draw ) draw->setDrawableHidden( TRUE );`: after a disguise restored after a load, the new drawable is hidden if the old one
    /// was.
    HideIfWasHidden,
    /// `if( draw->isSelected() ) TheControlBar->markUIDirty();` (`disguiseAsObject`).
    MarkUiDirtyIfSelected,
}

/// batch R27: which feedback a `StealthDetectorUpdate` gives (see [`ObjectClientEvent::StealthDetectorFeedback`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StealthFeedback {
    /// The local player's detector discovered an enemy's stealthed unit (`RADAR_EVENT_STEALTH_DISCOVERED`).
    Discovered,
    /// The local player's stealthed unit was revealed (`RADAR_EVENT_STEALTH_NEUTRALIZED`).
    Neutralized,
}

/// What a module does to one `RadiusDecal` (see [`ObjectClientEvent::GridDecal`]).
#[derive(Clone, Debug, PartialEq)]
pub enum GridDecalOp {
    /// `clear()`.
    Clear,
    /// `tmpl.createRadiusDecal(pos, radius, owningPlayer, decal)`; `player` is the owning player's index (`None`: `NULL`).
    Create { template: crate::logic::radius_decal::RadiusDecalTemplate, radius: f32, position: Coord3D, player: Option<i32> },
    /// `setPosition(pos)`.
    SetPosition(Coord3D),
    /// `setOpacity(o)`.
    SetOpacity(f32),
    /// `update()` (batch R20, `RadiusDecalUpdate`): the client throbs the opacity of the decal it drew, if it drew one.
    Update,
}
// --- end p1w13-sonnet ---

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
    /// `getDrawable()->setTerrainDecalSize(x, y)` (`CreateCrateDie`, batch R14c).
    DrawableSetTerrainDecalSize { object_id: ObjectId, x: f32, y: f32 },
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
    // --- p1w10-sonnet ---
    /// `draw->showSubObject(name, show)` (`SubObjectsUpgrade`).
    DrawableShowSubObject { object_id: ObjectId, name: Vec<u8>, show: bool },
    /// `draw->updateSubObjects()` (`SubObjectsUpgrade`).
    DrawableUpdateSubObjects { object_id: ObjectId },
    // --- end p1w10-sonnet ---
    // --- p1w12-sonnet ---
    /// `TheAudio->removeAudioEvent(handle)` (`FlammableUpdate`'s burning sound).
    AudioRemoveEvent { handle: u32 },
    // --- end p1w12-sonnet ---
    // --- p1w13-sonnet ---
    /// One call on the `index`th grid decal of an object's `DynamicShroudClearingRangeUpdate` (a `RadiusDecal`, which the client's shadow manager draws).
    GridDecal { object_id: ObjectId, index: u32, op: GridDecalOp },
    /// `FloatUpdate`: the drawable's instance matrix is set to a tilt that depends on the frame (`sin(frame * 0.0291) * 0.05` of yaw, `sin(frame * 0.0515) * 0.05` of
    /// pitch, keeping its Z rotation); the client's maths, since the matrix is client data.
    DrawableFloatTilt { object_id: ObjectId, frame: u32 },
    /// `draw->reactToBodyDamageStateChange(state)` (`ActiveBody::evaluateVisualCondition`), the state a `BodyDamageType`.
    DrawableReactToBodyDamageStateChange { object_id: ObjectId, state: i32 },
    // --- end p1w13-sonnet ---
    // --- p1w11-sonnet ---
    /// `TheAudio->addAudioEvent(&event)` for an event copied from the template's per-unit sounds (`EjectPilotDie`): the event's name, the object
    /// it is attached to (`INVALID_ID` for none), the position it was set to and the player index it was set to, if any.
    AudioAddEvent { name: Vec<u8>, object_id: ObjectId, position: Option<Coord3D>, player_index: Option<i32> },
    // --- end p1w11-sonnet ---
    // --- p1w13-sonnet ---
    /// `m_drawable->setShadowsEnabled(enabled)`.
    DrawableSetShadowsEnabled { object_id: ObjectId, enabled: bool },
    // --- end p1w13-sonnet ---
    /// `m_drawable->setTerrainDecalFadeTarget(target, rate)`.
    DrawableTerrainDecalFadeTarget { object_id: ObjectId, target: f32, rate: f32 },
    /// `m_drawable->setDrawableHidden(hidden)` (`TunnelTracker::loadPostProcess`).
    DrawableSetHidden { object_id: ObjectId, hidden: bool },
    /// `m_drawable->flashAsSelected()`.
    DrawableFlashAsSelected { object_id: ObjectId },
    // --- p1w13-sonnet --- (batch R5: RiderChangeContain moves the selection between bike and rider)
    /// `TheMessageStream->appendMessage(MSG_CREATE_SELECTED_GROUP)` with `appendBooleanArgument(createNew)` and `appendObjectIDArgument(object)`.
    MessageCreateSelectedGroup { create_new: bool, object_id: ObjectId },
    /// `TheMessageStream->appendMessage(MSG_REMOVE_FROM_SELECTED_GROUP)` with `appendObjectIDArgument(object)`.
    MessageRemoveFromSelectedGroup { object_id: ObjectId },
    /// batch R24: `TheInGameUI->message(label)` (the string manager's label, shown to the local player).
    UiMessage { label: Vec<u8> },
    /// batch R24: `TheRadar->createEvent(&pos, type)` (a `RadarEventType` number; the default four seconds to live).
    RadarCreateEvent { position: Coord3D, event_type: i32 },
    /// `TheInGameUI->selectDrawable(object's drawable)`.
    UiSelectDrawable { object_id: ObjectId },
    /// `TheInGameUI->deselectDrawable(object's drawable)`.
    UiDeselectDrawable { object_id: ObjectId },
    /// `TheInGameUI->setDisplayedMaxWarning(on)`.
    UiSetDisplayedMaxWarning { on: bool },
    /// batch R26: what a `StealthUpdate` does to the object's drawable (see [`StealthClientOp`]).
    Stealth { object_id: ObjectId, op: StealthClientOp },
    /// batch R27: a `StealthDetectorUpdate`'s feedback to the local player for a detection. With `try_radar_event`, `TheRadar->tryEvent(type,
    /// &position)` decides whether the rest plays; without, `TheRadar->createEvent(&position, type)` and the rest always plays: the misc audio's
    /// `m_stealthDiscoveredSound` or `m_stealthNeutralizedSound` with `setPlayerIndex(player_index)` (`None`: upstream reads a `NULL` player
    /// there), `TheInGameUI->message(TheGameText->fetch("MESSAGE:StealthDiscovered"))` or `"MESSAGE:StealthNeutralized"`, and, unless `eva` is
    /// `EVA_Invalid` (-1), `TheEva->setShouldPlay(eva)`.
    StealthDetectorFeedback { feedback: StealthFeedback, try_radar_event: bool, position: Coord3D, player_index: Option<i32>, eva: i32 },
    /// batch R27: `TheParticleSystemManager->createParticleSystem(template)` (the system the client numbers `id`), attached to the object's
    /// drawable (or the object), at the position of the drawable's pristine bone `bone` (the IR ping's default, `{-1.66, 5.5, 15}`, if it has
    /// none).
    ParticleSystemAtBone { template: Vec<u8>, id: u32, object_id: ObjectId, attach_to_drawable: bool, bone: Vec<u8> },
    // --- end p1w13-sonnet ---
    // --- p1w11-sonnet ---
    /// `draw->flashAsSelected(&color)` (`ObjectDefectionHelper`'s white flash when the period ends).
    DrawableFlashAsSelectedColor { object_id: ObjectId, r: f32, g: f32, b: f32 },
    /// `TheEva->setShouldPlay(EVA_SuperweaponLaunched_<whose>_<weapon>)` (`SpecialPowerModule::aboutToDoSpecialPower`).
    EvaSuperweaponLaunched { weapon: EvaSuperweapon, whose: EvaWhose },
    /// `TheInGameUI->addSuperweapon(playerIndex, powerName, id, powerTemplate)` (`SpecialPowerModule`'s constructor, `onSpecialPowerCreation`
    /// and `loadPostProcess`); the template by id.
    UiAddSuperweapon { player_index: i32, power_name: Vec<u8>, object_id: ObjectId, template: u32 },
    // --- end p1w11-sonnet ---
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
    // --- p1w11-sonnet ---
    /// `FXList::doFXObj(fx, primary, secondary)` with a secondary object (`FXListDie`: the damage dealer; `INVALID_ID` for none).
    FxListDoFxObjSecondary { fx: Vec<u8>, object_id: ObjectId, secondary: ObjectId },
    /// `FXList::doFXPos(fx, &position)` (`FXListDie` with `OrientToObject = No`).
    FxListDoFxPos { fx: Vec<u8>, position: Coord3D },
    // --- end p1w11-sonnet ---
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
