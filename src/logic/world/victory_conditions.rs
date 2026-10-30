// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/VictoryConditions.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/VictoryConditions.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `VictoryConditions` (`TheVictoryConditions`): who has won and lost a multiplayer game. The players of interest (not the neutral one,
//! not the civilians, not observers) are cached once the players exist; each frame, while the game is multiplayer and has a local player
//! or an observer, the conditions check whether a single alliance remains (the frame it happened is the game's end), eliminate the
//! players who have no objects left (no units, no buildings that count, or neither, as the conditions say: their map revealed, the
//! client told, an AI's slot marked, the player killed), and notice the local player's defeat (the radar forced on, chat to everyone).
//!
//! The players and the world are reached through [`VictoryHost`]: the player's questions are `Player`'s (`hasAnyObjects`,
//! `hasAnyUnits`, `hasAnyBuildings(mask)`, `getRelationship`, `killPlayer`, translated in `player_ops` and `player_objects`), the rest
//! the recorder, the player list and templates, the game info's slots and the client.
//!
//! # Upstream facts kept
//!
//! - `update` with no local player and no observer returns at once; with an observer and no local player it reads
//!   `m_players[m_localSlotNum]` only after `m_isObserver` is checked, so never.
//! - `isLocalAlliedVictory` and `isLocalAlliedDefeat` without a local player (`m_localSlotNum` -1, not an observer: only before
//!   `cachePlayerPtrs`) read `m_players[-1]` (`Undefined::ArrayIndexOutOfRange`).
//! - The defeat sound is one function-local `static AudioEventRTS`, made the first time a player is eliminated after frame 1.
//!
//! Differential-verified in `tests/differential_victory_v1.rs` (the file unchanged, on stand-ins): 20,000 programs, 14,329 compared,
//! 0 mismatches; plants 3831-3835 caught.
//!
//! `p1w13-sonnet`, layer 11, batch V1, 2026-10-30.

use crate::common::game_common::{Relationship, MAX_PLAYER_COUNT};
use crate::common::ini::{IniError, Undefined};
use crate::common::name_key_generator::NameKeyType;
use crate::logic::modules::plant;
use crate::logic::world::ids::PlayerIndex;

/// `VICTORY_NOBUILDINGS`.
pub const VICTORY_NOBUILDINGS: i32 = 1;
/// `VICTORY_NOUNITS`.
pub const VICTORY_NOUNITS: i32 = 2;
/// `MAX_SLOTS` (GameNetwork/NetworkDefs.h).
pub const MAX_SLOTS: i32 = 8;

const N: usize = MAX_PLAYER_COUNT as usize;

/// What the conditions do to the world outside the players' questions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VictoryEffect {
    /// `ThePartitionManager->revealMapForPlayerPermanently(index)`.
    RevealMapForPlayerPermanently(PlayerIndex),
    /// `TheGameClient->updateFakeDrawables()`.
    UpdateFakeDrawables,
    /// `TheInGameUI->message("GUI:PlayerHasBeenDefeated", player->getPlayerDisplayName().str())`.
    PlayerHasBeenDefeatedMessage(PlayerIndex),
    /// `TheAudio->addAudioEvent(&leftGameSound)` ("GUIMessageReceived").
    LeftGameSound,
    /// `slot->setLastFrameInGame(frame)` of the game info's slot.
    SlotLastFrameInGame { slot: i32, frame: u32 },
    /// `PopulateInGameDiplomacyPopup()`.
    PopulateInGameDiplomacyPopup,
    /// `TheRadar->forceOn(TRUE)`.
    RadarForceOn,
    /// `SetInGameChatType(INGAME_CHAT_EVERYONE)`.
    InGameChatEveryone,
}

/// The world the conditions reach.
pub trait VictoryHost {
    /// `TheRecorder->isMultiplayer()`.
    fn victory_is_multiplayer(&self) -> bool;
    /// `TheGameLogic->getFrame()`.
    fn victory_frame(&self) -> u32;
    /// `ThePlayerList->getNthPlayer(i)` (`None`: `NULL`).
    fn victory_nth_player(&self, i: i32) -> Option<PlayerIndex>;
    /// Whether the player is `ThePlayerList->getNeutralPlayer()`.
    fn victory_is_neutral_player(&self, p: PlayerIndex) -> bool;
    /// `player->getPlayerTemplate()`, as the template's name key (`None`: `NULL`).
    fn victory_player_template(&self, p: PlayerIndex) -> Option<NameKeyType>;
    /// `ThePlayerTemplateStore->findPlayerTemplate(NAMEKEY("FactionCivilian"))`, as its name key (`None`: `NULL`).
    fn victory_civilian_template(&mut self) -> Option<NameKeyType>;
    /// `player->isPlayerObserver()`.
    fn victory_is_player_observer(&self, p: PlayerIndex) -> bool;
    /// `player->isLocalPlayer()`.
    fn victory_is_local_player(&self, p: PlayerIndex) -> bool;
    /// `player->hasAnyObjects()`.
    fn victory_has_any_objects(&self, p: PlayerIndex) -> bool;
    /// `player->hasAnyUnits()`.
    fn victory_has_any_units(&self, p: PlayerIndex) -> bool;
    /// `player->hasAnyBuildings(mask)` with `KINDOF_MP_COUNT_FOR_VICTORY`.
    fn victory_has_any_buildings_counting_for_victory(&self, p: PlayerIndex) -> bool;
    /// `p->getRelationship(q->getDefaultTeam())`.
    fn victory_relationship_to_default_team(&self, p: PlayerIndex, q: PlayerIndex) -> Relationship;
    /// `player->getPlayerNameKey()`.
    fn victory_player_name_key(&self, p: PlayerIndex) -> NameKeyType;
    /// `NAMEKEY(name)`.
    fn victory_name_key(&mut self, name: &[u8]) -> NameKeyType;
    /// `TheGameInfo ? TheGameInfo->getSlot(idx) : NULL`, and whether it is an AI's (`None`: no slot).
    fn victory_slot_is_ai(&self, idx: i32) -> Option<bool>;
    /// `player->killPlayer()`.
    fn victory_kill_player(&mut self, p: PlayerIndex) -> Result<(), IniError>;
    /// What the conditions do to the rest of the world.
    fn victory_effect(&mut self, e: VictoryEffect);
}

/// `class VictoryConditions : public VictoryConditionsInterface`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VictoryConditions {
    /// `m_victoryConditions` (the interface's).
    victory_conditions: i32,
    players: [Option<PlayerIndex>; N],
    local_slot_num: i32,
    end_frame: u32,
    is_defeated: [bool; N],
    local_player_defeated: bool,
    single_alliance_remaining: bool,
    is_observer: bool,
}

impl Default for VictoryConditions {
    fn default() -> Self {
        Self::new()
    }
}

/// `areAllies(p1, p2)`: two players each allied to the other's default team.
fn are_allies<H: VictoryHost>(h: &H, p1: PlayerIndex, p2: PlayerIndex) -> bool {
    p1 != p2
        && h.victory_relationship_to_default_team(p1, p2) == Relationship::Allies
        && (plant(3831) || h.victory_relationship_to_default_team(p2, p1) == Relationship::Allies)
}

impl VictoryConditions {
    /// `createVictoryConditions()`: the constructor's `reset()`.
    pub fn new() -> Self {
        let mut v = VictoryConditions {
            victory_conditions: 0,
            players: [None; N],
            local_slot_num: -1,
            end_frame: 0,
            is_defeated: [false; N],
            local_player_defeated: false,
            single_alliance_remaining: false,
            is_observer: false,
        };
        v.reset();
        v
    }

    /// `init()`.
    pub fn init(&mut self) {
        self.reset();
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        self.players = [None; N];
        self.is_defeated = [false; N];
        self.local_slot_num = -1;
        self.local_player_defeated = false;
        self.single_alliance_remaining = false;
        self.is_observer = false;
        self.end_frame = 0;
        self.victory_conditions = VICTORY_NOBUILDINGS | VICTORY_NOUNITS;
    }

    /// `setVictoryConditions(victoryConditions)`.
    pub fn set_victory_conditions(&mut self, v: i32) {
        self.victory_conditions = v;
    }

    /// `getVictoryConditions()`.
    pub fn get_victory_conditions(&self) -> i32 {
        self.victory_conditions
    }

    /// `amIObserver()`.
    pub fn am_i_observer(&self) -> bool {
        self.is_observer
    }

    /// `getEndFrame()`.
    pub fn get_end_frame(&self) -> u32 {
        self.end_frame
    }

    /// The cached players, the local slot, the end frame, the eliminations and the three flags (for the tests).
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = vec![self.victory_conditions as u32];
        v.extend(self.players.iter().map(|p| p.map_or(u32::MAX, |p| p as u32)));
        v.push(self.local_slot_num as u32);
        v.push(self.end_frame);
        v.extend(self.is_defeated.iter().map(|&b| u32::from(b)));
        v.extend([u32::from(self.local_player_defeated), u32::from(self.single_alliance_remaining), u32::from(self.is_observer)]);
        v
    }

    fn isset(&self, bit: i32) -> bool {
        self.victory_conditions & bit != 0
    }

    /// `m_players[m_localSlotNum]` (-1 reads before the array).
    fn local_player(&self) -> Result<Option<PlayerIndex>, IniError> {
        usize::try_from(self.local_slot_num).ok().and_then(|i| self.players.get(i).copied()).ok_or(IniError::undefined(Undefined::ArrayIndexOutOfRange))
    }

    /// `update()`.
    pub fn update<H: VictoryHost>(&mut self, h: &mut H) -> Result<(), IniError> {
        if !h.victory_is_multiplayer() || (self.local_slot_num == -1 && !self.is_observer) {
            return Ok(());
        }
        // "Check for a single winning alliance"
        if !self.single_alliance_remaining {
            let mut multiple_alliances = false;
            let mut alive: Option<PlayerIndex> = None;
            for i in 0..N {
                let Some(player) = self.players[i] else { continue };
                if !self.has_single_player_been_defeated(h, Some(player)) {
                    match alive {
                        // "check to verify they are on the same team"
                        Some(a) => {
                            if !are_allies(h, a, player) {
                                multiple_alliances = true;
                                break;
                            }
                        }
                        None => alive = Some(player), // "save this pointer to check against"
                    }
                }
            }
            if !multiple_alliances {
                self.single_alliance_remaining = true; // "don't check again"
                self.end_frame = h.victory_frame();
            }
        }
        // "check for player eliminations"
        for i in 0..N {
            let Some(p) = self.players[i] else { continue };
            if !self.is_defeated[i] && self.has_single_player_been_defeated(h, Some(p)) {
                self.is_defeated[i] = true;
                if h.victory_frame() > if plant(3832) { 0 } else { 1 } {
                    h.victory_effect(VictoryEffect::RevealMapForPlayerPermanently(p));
                    h.victory_effect(VictoryEffect::UpdateFakeDrawables);
                    h.victory_effect(VictoryEffect::PlayerHasBeenDefeatedMessage(p));
                    // "People are boneheads. Also play a sound"
                    h.victory_effect(VictoryEffect::LeftGameSound);
                }
                for idx in 0..MAX_SLOTS {
                    let name = format!("player{idx}");
                    if h.victory_player_name_key(p) == h.victory_name_key(name.as_bytes()) && h.victory_slot_is_ai(idx) == Some(true) {
                        let frame = h.victory_frame();
                        h.victory_effect(VictoryEffect::SlotLastFrameInGame { slot: idx, frame });
                    }
                }
                // "destroy any remaining units (infantry if its a short game, for example)"
                h.victory_kill_player(p)?;
                h.victory_effect(VictoryEffect::PopulateInGameDiplomacyPopup);
            }
        }
        // "Check if the local player has been eliminated"
        if !self.local_player_defeated && !self.is_observer {
            let local = self.local_player()?;
            if self.has_single_player_been_defeated(h, local) {
                self.local_player_defeated = !plant(3833); // "don't check again"
                h.victory_effect(VictoryEffect::RadarForceOn);
                // "can't chat to allies after death.  Only to other observers."
                h.victory_effect(VictoryEffect::InGameChatEveryone);
            }
        }
        Ok(())
    }

    /// `hasAchievedVictory(player)`: a player of the remaining alliance.
    pub fn has_achieved_victory<H: VictoryHost>(&self, h: &H, player: Option<PlayerIndex>) -> bool {
        let Some(player) = player else { return false };
        if self.single_alliance_remaining {
            for &q in self.players.iter().flatten() {
                if !self.has_single_player_been_defeated(h, Some(q)) && (player == q || are_allies(h, q, player)) {
                    return true;
                }
            }
        }
        false
    }

    /// `hasBeenDefeated(player)`: once one alliance remains, anyone not of it.
    pub fn has_been_defeated<H: VictoryHost>(&self, h: &H, player: Option<PlayerIndex>) -> bool {
        if player.is_none() {
            return false;
        }
        self.single_alliance_remaining && !self.has_achieved_victory(h, player)
    }

    /// `hasSinglePlayerBeenDefeated(player)`: nothing left of what the conditions count.
    pub fn has_single_player_been_defeated<H: VictoryHost>(&self, h: &H, player: Option<PlayerIndex>) -> bool {
        let Some(player) = player else { return false };
        if self.isset(VICTORY_NOUNITS) && self.isset(VICTORY_NOBUILDINGS) {
            if !h.victory_has_any_objects(player) {
                return true;
            }
        } else if self.isset(VICTORY_NOUNITS) {
            if !h.victory_has_any_units(player) {
                return true;
            }
        } else if self.isset(VICTORY_NOBUILDINGS) && !h.victory_has_any_buildings_counting_for_victory(player) && !plant(3834) {
            return true;
        }
        false
    }

    /// `cachePlayerPtrs()`: the players of interest, in list order; without a local player the conditions observe.
    pub fn cache_player_ptrs<H: VictoryHost>(&mut self, h: &mut H) {
        if !h.victory_is_multiplayer() {
            return;
        }
        let mut player_count = 0usize;
        let civ_template = h.victory_civilian_template();
        for i in 0..MAX_PLAYER_COUNT {
            let Some(player) = h.victory_nth_player(i) else { continue };
            let template = h.victory_player_template(player);
            if !h.victory_is_neutral_player(player) && template.is_some() && (template != civ_template || plant(3835)) && !h.victory_is_player_observer(player) {
                self.players[player_count] = Some(player);
                if h.victory_is_local_player(player) {
                    self.local_slot_num = player_count as i32;
                }
                player_count += 1;
            }
        }
        while player_count < N {
            self.players[player_count] = None;
            player_count += 1;
        }
        if self.local_slot_num < 0 {
            self.local_player_defeated = true; // "if we have no local player, don't check for defeat"
            h.victory_effect(VictoryEffect::RadarForceOn);
            self.is_observer = true;
        }
    }

    /// `isLocalAlliedVictory()`.
    pub fn is_local_allied_victory<H: VictoryHost>(&self, h: &H) -> Result<bool, IniError> {
        if self.is_observer {
            return Ok(false);
        }
        Ok(self.has_achieved_victory(h, self.local_player()?))
    }

    /// `isLocalAlliedDefeat()`.
    pub fn is_local_allied_defeat<H: VictoryHost>(&self, h: &H) -> Result<bool, IniError> {
        if self.is_observer {
            return Ok(self.single_alliance_remaining);
        }
        Ok(self.has_been_defeated(h, self.local_player()?))
    }

    /// `isLocalDefeat()`.
    pub fn is_local_defeat(&self) -> bool {
        !self.is_observer && self.local_player_defeated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_constructor_resets() {
        let v = VictoryConditions::new();
        assert_eq!(v.get_victory_conditions(), VICTORY_NOBUILDINGS | VICTORY_NOUNITS);
        assert_eq!((v.local_slot_num, v.am_i_observer(), v.get_end_frame(), v.is_local_defeat()), (-1, false, 0, false));
    }
}
