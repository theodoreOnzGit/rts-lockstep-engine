// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/VictoryConditions.cpp (the calls the conditions make out of
//   `VictoryConditions`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I5: the world as `VictoryHost`, and `TheVictoryConditions` in the world ([`World::victory`]).
//! `GameLogic::update`'s `TheVictoryConditions->update()` and `Player`'s `hasSinglePlayerBeenDefeated` are now the
//! conditions' own.
//!
//! - **The world:** the frame, the player list and its players (template, observer, local player, objects, units,
//!   buildings that count for victory, relationships, name keys, `killPlayer`), the player template store, the name keys,
//!   and the partition manager's `revealMapForPlayerPermanently` (with `cuts.live_partition`).
//! - **Cut points:** `TheRecorder->isMultiplayer()` and `TheGameInfo`'s slots ([`GameLoopCuts`](super::game_logic_host_world::GameLoopCuts)),
//!   and the client's effects (the fake drawables, the in-game UI message, the sound, the slot's last frame, the
//!   diplomacy popup, the radar, the chat type), each logged as [`CutCall::Host`].
//!
//! `p1w13-sonnet`, integration batch I5, 2026-10-31.

use crate::common::game_common::Relationship;
use crate::common::ini::IniError;
use crate::common::kind_of::{KindOfMaskType, KINDOF_MP_COUNT_FOR_VICTORY};
use crate::common::name_key_generator::NameKeyType;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::PlayerIndex;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::player_objects::{player_kill_player, PlayerObjectHost};
#[cfg(test)]
use crate::logic::world::player_ops::PlayerHost;
use crate::logic::world::player_ops::{
    player_get_relationship, player_has_any_buildings_kind_of, player_has_any_objects, player_has_any_units, player_is_local_player,
    player_is_player_observer,
};
use crate::logic::world::victory_conditions::{VictoryEffect, VictoryHost};

impl World {
    /// `TheVictoryConditions->update()`.
    pub fn victory_update(&mut self) -> Result<(), IniError> {
        let mut v = std::mem::take(&mut self.victory);
        let r = v.update(self);
        self.victory = v;
        r
    }

    /// `TheVictoryConditions->cachePlayerPtrs()` (once the players exist, at the start of a game).
    pub fn victory_cache_player_ptrs(&mut self) {
        let mut v = std::mem::take(&mut self.victory);
        v.cache_player_ptrs(self);
        self.victory = v;
    }

    /// Logs one of the conditions' effects on the client.
    fn victory_cut(&mut self, call: &'static str, args: &[i64]) {
        self.cut(CutCall::Host { call, args: args.to_vec(), text: Vec::new() });
    }
}

impl VictoryHost for World {
    fn victory_is_multiplayer(&self) -> bool {
        self.cuts.game_loop.recorder_multiplayer
    }
    fn victory_frame(&self) -> u32 {
        self.frame()
    }
    fn victory_nth_player(&self, i: i32) -> Option<PlayerIndex> {
        self.players.get_nth(i).map(|p| p.player_index)
    }
    fn victory_is_neutral_player(&self, p: PlayerIndex) -> bool {
        p == 0
    }
    fn victory_player_template(&self, p: PlayerIndex) -> Option<NameKeyType> {
        let t = self.players.get_nth(p)?.template?;
        self.statics.player_template_store.nth_player_template(t as i32).map(|t| t.name_key())
    }
    fn victory_civilian_template(&mut self) -> Option<NameKeyType> {
        let key = self.names.name_to_key(b"FactionCivilian");
        let t = self.player_template_find(key)?;
        self.statics.player_template_store.nth_player_template(t as i32).map(|t| t.name_key())
    }
    fn victory_is_player_observer(&self, p: PlayerIndex) -> bool {
        player_is_player_observer(self, p).unwrap_or(false)
    }
    fn victory_is_local_player(&self, p: PlayerIndex) -> bool {
        player_is_local_player(self, p)
    }
    fn victory_has_any_objects(&self, p: PlayerIndex) -> bool {
        player_has_any_objects(self, p).unwrap_or(false)
    }
    fn victory_has_any_units(&self, p: PlayerIndex) -> bool {
        player_has_any_units(self, p).unwrap_or(false)
    }
    fn victory_has_any_buildings_counting_for_victory(&self, p: PlayerIndex) -> bool {
        player_has_any_buildings_kind_of(self, p, &KindOfMaskType::from_indices(&[KINDOF_MP_COUNT_FOR_VICTORY])).unwrap_or(false)
    }
    fn victory_relationship_to_default_team(&self, p: PlayerIndex, q: PlayerIndex) -> Relationship {
        let team = self.players.get_nth(q).and_then(|x| x.default_team);
        player_get_relationship(self, p, team).unwrap_or(Relationship::Neutral)
    }
    fn victory_player_name_key(&self, p: PlayerIndex) -> NameKeyType {
        NameKeyType(self.players.get_nth(p).map_or(0, |x| x.name_key as u32))
    }
    fn victory_name_key(&mut self, name: &[u8]) -> NameKeyType {
        self.names.name_to_key(name)
    }
    fn victory_slot_is_ai(&self, idx: i32) -> Option<bool> {
        let slots = self.cuts.game_loop.game_info_slots_ai.as_ref()?;
        usize::try_from(idx).ok().and_then(|i| slots.get(i)).copied()
    }
    fn victory_kill_player(&mut self, p: PlayerIndex) -> Result<(), IniError> {
        if let Err(e) = player_kill_player(self, p) {
            self.cuts.team_live_errors.push(format!("Player::killPlayer: {e:?}"));
        }
        Ok(())
    }
    fn victory_effect(&mut self, e: VictoryEffect) {
        match e {
            VictoryEffect::RevealMapForPlayerPermanently(p) => {
                self.victory_cut("PartitionManager::revealMapForPlayerPermanently", &[i64::from(p)]);
                if self.cuts.live_partition {
                    let mut pm = std::mem::take(&mut self.partition);
                    pm.reveal_map_for_player_permanently(self, p);
                    self.partition = pm;
                }
            }
            VictoryEffect::UpdateFakeDrawables => self.victory_cut("GameClient::updateFakeDrawables", &[]),
            VictoryEffect::PlayerHasBeenDefeatedMessage(p) => self.victory_cut("InGameUI::message(GUI:PlayerHasBeenDefeated)", &[i64::from(p)]),
            VictoryEffect::LeftGameSound => self.victory_cut("AudioManager::addAudioEvent(GUIMessageReceived)", &[]),
            VictoryEffect::SlotLastFrameInGame { slot, frame } => self.victory_cut("GameSlot::setLastFrameInGame", &[i64::from(slot), i64::from(frame)]),
            VictoryEffect::PopulateInGameDiplomacyPopup => self.victory_cut("PopulateInGameDiplomacyPopup", &[]),
            VictoryEffect::RadarForceOn => self.victory_cut("Radar::forceOn", &[]),
            VictoryEffect::InGameChatEveryone => self.victory_cut("SetInGameChatType(INGAME_CHAT_EVERYONE)", &[]),
        }
    }
}

#[cfg(test)]
mod tests {
    //! The conditions themselves are checked against the compiled VictoryConditions.cpp by
    //! `tests/differential_victory_v1.rs`; these check them on the world: the players of interest, an elimination, and
    //! the game logic's frame running them.
    use super::*;
    use crate::common::base_type::Coord3D;
    use crate::common::dict::Dict;
    use crate::logic::world::player_host_world::tests::{make, world};
    use crate::logic::world::team_ops::{factory_find_team, factory_init_team};

    fn hosts(w: &World) -> Vec<&'static str> {
        w.cuts.log.as_ref().unwrap().iter().filter_map(|c| if let CutCall::Host { call, .. } = c { Some(*call) } else { None }).collect()
    }

    /// A multiplayer game of America (player 1, local, with a tank) against America (player 2, with nothing) and the
    /// civilians (player 3, never of interest): at frame 5 player 2 is eliminated (its map revealed, the client told,
    /// the player killed), one alliance remains, and player 1 has won.
    #[test]
    fn a_player_with_nothing_left_is_eliminated() {
        let (mut w, team1) = world();
        w.players.set_player_count(4);
        for (i, name, template) in [(1, &b"Player1"[..], 0u32), (2, b"Player2", 0), (3, b"Player3", 1)] {
            let key = w.names.name_to_key(name);
            let p = w.players.get_nth_mut(i).unwrap();
            p.name_key = key.0 as i32;
            p.template = Some(template);
        }
        factory_init_team(&mut w, b"team2", b"Player2", true, &Dict::new()).unwrap();
        assert!(factory_find_team(&mut w, b"team2").unwrap().is_some());
        let _ = w.players.set_local_player(Some(1));
        w.cuts.game_loop.recorder_multiplayer = true;
        w.victory_cache_player_ptrs();
        assert_eq!(&w.victory.state_words()[1..4], &[1, 2, u32::MAX]);
        make(&mut w, b"Tank", team1, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        w.set_frame(5);
        w.cuts.log = Some(Vec::new());
        w.victory_update().unwrap();
        assert!(w.victory.has_single_player_been_defeated(&w, Some(2)));
        assert!(!w.victory.has_single_player_been_defeated(&w, Some(1)));
        assert!(w.victory.has_achieved_victory(&w, Some(1)) && w.victory.has_been_defeated(&w, Some(2)));
        assert_eq!(w.victory.get_end_frame(), 5);
        assert_eq!(
            hosts(&w),
            [
                "PartitionManager::revealMapForPlayerPermanently",
                "GameClient::updateFakeDrawables",
                "InGameUI::message(GUI:PlayerHasBeenDefeated)",
                "AudioManager::addAudioEvent(GUIMessageReceived)",
                "PopulateInGameDiplomacyPopup",
            ]
        );
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        // the player host's question is the conditions' answer
        assert!(w.victory_has_single_player_been_defeated(2) && !w.victory_has_single_player_been_defeated(1));
    }
}
