// SPDX-License-Identifier: GPL-3.0-only
//
// Shape of the events that game logic hands to the client (the cut point of
// docs/phase1-porting-graph.md, "Cut points: the client interface"). The
// client-facing singletons upstream (`TheAudio`, `TheInGameUI`, ...) are not
// reachable from logic; logic pushes one of these onto a queue it owns, and
// the client drains the queue after each logic frame.
//
// Translation-owned file (this repository's own design, not upstream code):
// it names the upstream calls each variant replaces, and nothing else.

//! Events that logic emits for the client. Only the variants needed by the
//! layer-4 static data are defined here (the others arrive with their
//! callers); each names the upstream call it replaces.
//!
//! This is the one client-notification interface: `logic::world` owns the
//! per-frame queue of these (`world::events::ClientEventQueue`) and
//! `TerrainVisualEvent` nests inside as [`ClientEvent::Terrain`]. See
//! `docs/world-design.md`.

// --- p1w5-world ---
use crate::logic::terrain_visual::TerrainVisualEvent;
// --- end p1w5-world ---
// --- p1w6-partition ---
use crate::logic::world::partition::PartitionClientEvent;
// --- end p1w6-partition ---

/// Which of `MiscAudio`'s money sounds an event asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoneySound {
    /// `MiscAudio::m_moneyDepositSound`.
    Deposit,
    /// `MiscAudio::m_moneyWithdrawSound`.
    Withdraw,
}

/// One event for the client.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientEvent {
    /// `TheAudio->addAudioEvent(&event)`, where `event` is the money sound
    /// of `MiscAudio` with `setPlayerIndex(player_index)`. The client looks
    /// up the sound's name in its own `MiscAudio`, so logic never holds a
    /// `AudioEventRTS`.
    MoneySound { sound: MoneySound, player_index: i32 },
    // --- p1w4-thingtemplate ---
    /// `TheGameClient->friend_createDrawable(tmplate, statusBits)`, which
    /// `ThingFactory::newDrawable` calls (and `GameLogic::registerObject`
    /// for every object): the client makes the drawable of a thing made from
    /// the template with this id, with these `DrawableStatus` bits. Logic
    /// never holds the `Drawable`. `object_id` is the object the drawable is
    /// bound to (`GameLogic::sendObjectCreated` binds them), so the client
    /// can pair a drawable with its object; `ObjectDestroyed` is the matching
    /// end (added by `p1w6-object`, lead decision 2026-10-18).
    ObjectCreated { template_id: u16, drawable_status_bits: u32, object_id: i32 },
    // --- end p1w4-thingtemplate ---
    // --- p1w5-world ---
    /// `TheGameClient->destroyDrawable(obj->getDrawable())`, which
    /// `GameLogic::sendObjectDestroyed` calls from `~Object`: the client
    /// destroys the drawable it keeps for this object id (a drawable is a
    /// client-side mapping from `ObjectId`, see `logic::world`).
    ObjectDestroyed { object_id: i32 },
    /// `TheAudio->addAudioEvent(&event)` where `event` names a sound, is
    /// attached to an object (`setObjectID`) and has a player
    /// (`setPlayerIndex`, -1 for none). The client resolves the name in its
    /// own audio tables. Position, volume and the other `AudioEventRTS`
    /// setters are added by the first caller that needs them.
    Audio { event_name: Vec<u8>, object_id: i32, player_index: i32 },
    /// The `TheTerrainVisual` writes of `TerrainLogic`
    /// ([`TerrainVisualEvent`]), nested: `TerrainLogic` still queues its own
    /// events (its differential tests read them) and
    /// `World::flush_terrain_events` moves them here, in order.
    Terrain(TerrainVisualEvent),
    // --- end p1w5-world ---
    // --- p1w6-object ---
    /// What `Object` tells `Drawable`, `TheAudio`, `TheRadar`, `TheInGameUI`,
    /// `TheControlBar`, `TheEva` and `TheGameClient` (see
    /// `logic::world::object_events`).
    Object(crate::logic::world::object_events::ObjectClientEvent),
    // --- end p1w6-object ---
    // --- p1w6-partition ---
    /// The partition manager's calls on `TheDisplay`, `TheRadar` and
    /// `TheGhostObjectManager` ([`PartitionClientEvent`]), nested;
    /// `World::flush_partition_events` moves them here, in order.
    Partition(PartitionClientEvent),
    // --- end p1w6-partition ---
    // --- p1w10-sonnet ---
    /// What `Player` tells `TheAudio`, `TheControlBar` and `TheEva` ([`PlayerClientEvent`]), nested.
    Player(crate::logic::world::player_ops::PlayerClientEvent),
    // --- end p1w10-sonnet ---
    /// GitHub #51: the script actions' calls on `TheAudio` (ScriptActions.cpp) and the script engine's music track played
    /// again after a load (`ScriptEngine::loadPostProcess`), nested ([`ScriptAudioEvent`]).
    ScriptAudio(ScriptAudioEvent),
}

/// GitHub #51: one of the scripts' calls on `TheAudio`. Upstream calls the audio manager directly from the logic's
/// script actions; here the logic pushes the call, and the client's audio manager makes it
/// (`client::client_audio::ClientAudio::apply_event`).
///
/// **Not simulation state.** Pushing one changes nothing the logic reads, saves or puts in its CRC: it is not given a
/// handle from the logic's made-up audio handles (`cuts.last_audio_handle`, which the modules keep and save)~~, and a
/// logical event's random file choice is the client's (`client::audio_event`, GitHub #50), never the World's generator~~.
/// **Corrected 2026-11-06 (GitHub #50):** a *logical* event's `GameLogicRandomValue` draws are the logic's own, made by the
/// World from its generator before the event is pushed, as upstream's `addAudioEvent` makes them
/// ([`crate::logic::logical_audio`]); they are simulation state, and the values drawn travel in
/// [`ScriptAudioEvent::AddEvent`] for the client to play.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptAudioEvent {
    /// `TheAudio->addAudioEvent(&event)` of the event the action built (`doPlaySoundEffect`, `doPlaySoundEffectAt`,
    /// `doSoundPlayFromNamed`, `doSpeechPlay`, `doMusicTrackChange`, `doUnitSelectAndPlay`; and
    /// `ScriptEngine::loadPostProcess`'s current music track).
    ///
    /// GitHub #50: with the values of the logic's draws for a logical event (none for one that is not logical), which the
    /// client's `AudioEventRTS` uses in place of its own `GameLogicRandomValue` draws.
    AddEvent(crate::logic::world::script_actions::AudioEvent, crate::logic::logical_audio::LogicalAudioPicks),
    /// `TheAudio->removeAudioEvent(handle)`: only `AHSV_StopTheMusic` / `AHSV_StopTheMusicFade` (`doMusicTrackChange`).
    RemoveEventHandle(i32),
    /// `TheAudio->pauseAmbient(pausing)` (`doAmbientSoundsPause`).
    PauseAmbient(bool),
    /// `TheAudio->setVolume(volume, whichToAffect)` (`doAudioSetVolume`: already divided by 100 and clamped to [0, 1]).
    SetVolume { volume: f32, affect: i32 },
    /// `TheAudio->setAudioEventEnabled(name, enable)` (an empty name: all).
    SetEventEnabled { name: Vec<u8>, enable: bool },
    /// `TheAudio->removeDisabledEvents()`.
    RemoveDisabledEvents,
    /// `TheAudio->removeAudioEvent(name)`.
    RemoveEventNamed(Vec<u8>),
    /// `TheAudio->setAudioEventVolumeOverride(name, volume)` (-1 restores; an empty name: all).
    SetEventVolumeOverride { name: Vec<u8>, volume: f32 },
    /// `TheAudio->pauseAudio(affect)` (`pause`) or `resumeAudio(affect)` (`SUSPEND_BACKGROUND_SOUNDS`,
    /// `RESUME_BACKGROUND_SOUNDS`).
    Pause { pause: bool, affect: i32 },
}
