// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/SelectionInfo.cpp (SelectionInfo, contextCommandForNewSelection)
//   GeneralsMD/Code/GameEngine/Include/GameClient/SelectionInfo.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SelectionInfo` and `contextCommandForNewSelection` (C2a, GitHub #6): what a click or a drag selection would pick, counted
//! by whose it is, and whether it is a context command (an order to the current selection) rather than a new selection.
//!
//! ~~**The context command itself is not translated yet:** `TheGameClient->evaluateContextCommand(draw, pos, EVALUATE_ONLY)` is
//! `CommandXlat.cpp`'s (C2b). Here it is [`SelectionInfoHost::context_command_is_valid`]; until C2b the client answers false
//! (`MSG_INVALID`), so a click on a unit selects it rather than ordering the selection.~~ **Corrected (C2b):**
//! `TheGameClient->evaluateContextCommand(draw, pos, EVALUATE_ONLY) != MSG_INVALID` is the `valid` argument; the selection
//! translator passes the command translator's
//! [`evaluate_context_command`](crate::client::command_xlat::CommandTranslator::evaluate_context_command).

use crate::client::in_game_ui::DrawableId;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{KindOfType, KINDOF_CRATE, KINDOF_INFANTRY, KINDOF_STRUCTURE};
use crate::common::game_common::Relationship;

/// What `contextCommandForNewSelection` asks of the game.
pub trait SelectionInfoHost {
    /// `draw->getObject()`'s id.
    fn drawable_object(&self, draw: DrawableId) -> Option<ObjectId>;
    /// `obj->isLocallyControlled()`.
    fn is_locally_controlled(&self, obj: ObjectId) -> bool;
    /// `obj->isKindOf(k)`.
    fn is_kind_of(&self, obj: ObjectId, k: KindOfType) -> bool;
    /// `localPlayer->getRelationship(obj->getTeam())`.
    fn relationship_to_local_player(&self, obj: ObjectId) -> Relationship;
    /// `TheActionManager->canPlayerGarrison(localPlayer, obj, CMD_FROM_PLAYER)`.
    fn can_local_player_garrison(&self, obj: ObjectId) -> bool;
    /// `TheInGameUI->isInForceAttackMode()` and `isInForceMoveToMode()`.
    fn force_fire_or_move(&self) -> bool;
    /// `TheInGameUI->isInPreferSelectionMode()`.
    fn prefer_selection(&self) -> bool;
    /// `TheGlobalData->m_useAlternateMouse`.
    fn use_alternate_mouse(&self) -> bool;
}

/// `struct SelectionInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectionInfo {
    pub current_count_enemies: i32,
    pub current_count_civilians: i32,
    pub current_count_mine: i32,
    pub current_count_mine_infantry: i32,
    pub current_count_mine_buildings: i32,
    pub current_count_friends: i32,
    pub new_count_enemies: i32,
    pub new_count_civilians: i32,
    pub new_count_crates: i32,
    pub new_count_mine: i32,
    pub new_count_mine_buildings: i32,
    pub new_count_friends: i32,
    pub new_count_garrisonable_buildings: i32,
    pub select_enemies: bool,
    pub select_civilians: bool,
    pub select_mine: bool,
    pub select_mine_buildings: bool,
    pub select_friends: bool,
}

/// `contextCommandForNewSelection(current, new, &info, isPoint)`: fills `info` with the counts of the current and the new
/// drawables, and answers whether the click is a context command for the current selection (true) rather than a selection.
/// `valid(h, draw)` is `TheGameClient->evaluateContextCommand(draw, draw->getPosition(), EVALUATE_ONLY) != MSG_INVALID`.
pub fn context_command_for_new_selection<H: SelectionInfoHost>(
    current: &[DrawableId],
    new: &[DrawableId],
    info: &mut SelectionInfo,
    selection_is_point: bool,
    h: &mut H,
    valid: &mut impl FnMut(&mut H, DrawableId) -> bool,
) -> bool {
    if h.force_fire_or_move() {
        return false;
    }
    for &d in current {
        let Some(obj) = h.drawable_object(d) else { continue };
        if h.is_locally_controlled(obj) {
            info.current_count_mine += 1;
            if h.is_kind_of(obj, KINDOF_INFANTRY) {
                info.current_count_mine_infantry += 1;
            } else if h.is_kind_of(obj, KINDOF_STRUCTURE) {
                info.current_count_mine_buildings += 1;
            }
        } else {
            match h.relationship_to_local_player(obj) {
                Relationship::Allies => info.current_count_friends += 1,
                Relationship::Enemies => info.current_count_enemies += 1,
                Relationship::Neutral => info.current_count_civilians += 1,
            }
        }
    }
    let (mut new_mine, mut new_friendly, mut new_enemy, mut new_civilian) = (None, None, None, None);
    for &d in new {
        let Some(obj) = h.drawable_object(d) else { continue };
        if h.can_local_player_garrison(obj) {
            info.new_count_garrisonable_buildings += 1;
        }
        if h.is_kind_of(obj, KINDOF_CRATE) {
            info.new_count_crates += 1;
        }
        if h.is_locally_controlled(obj) {
            info.new_count_mine += 1;
            new_mine = Some(d);
            if h.is_kind_of(obj, KINDOF_STRUCTURE) {
                info.new_count_mine_buildings += 1;
            }
        } else {
            match h.relationship_to_local_player(obj) {
                Relationship::Allies => {
                    new_friendly = Some(d);
                    info.new_count_friends += 1;
                }
                Relationship::Enemies => {
                    new_enemy = Some(d);
                    info.new_count_enemies += 1;
                }
                Relationship::Neutral => {
                    new_civilian = Some(d);
                    info.new_count_civilians += 1;
                }
            }
        }
    }
    if info.current_count_enemies > 0 || info.current_count_friends > 0 || info.current_count_civilians > 0 {
        return false;
    }
    if h.use_alternate_mouse() {
        // context sensitive commands never apply when selecting in alternate mouse mode
        return false;
    }
    if info.current_count_mine > 0 {
        if info.new_count_enemies > 0 {
            if info.new_count_enemies == 1 && selection_is_point {
                return new_enemy.is_some_and(|d| valid(h, d));
            }
            return selection_is_point;
        }
        if info.new_count_mine > 0 {
            if info.new_count_mine == 1 && selection_is_point && !h.prefer_selection() {
                return new_mine.is_some_and(|d| valid(h, d));
            }
            return false;
        }
        if info.new_count_friends > 0 {
            if info.new_count_friends == 1 && selection_is_point {
                return new_friendly.is_some_and(|d| valid(h, d));
            }
            return false;
        }
        if info.current_count_mine_infantry > 0 && info.new_count_garrisonable_buildings == 1 {
            return true;
        }
        if info.new_count_civilians > 0 {
            if info.new_count_civilians == 1 && selection_is_point {
                return new_civilian.is_some_and(|d| valid(h, d));
            }
            return false;
        }
        if info.new_count_crates > 0 {
            return info.new_count_crates == 1 && selection_is_point;
        }
    }
    if info.current_count_mine == 0 {
        return false;
    }
    selection_is_point
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Drawables 1..: drawable `d`'s object is `d`; `mine` are locally controlled, `rel` the others' relationships.
    #[derive(Default)]
    struct Host {
        mine: Vec<ObjectId>,
        rel: BTreeMap<ObjectId, Relationship>,
        infantry: Vec<ObjectId>,
        context_valid: Vec<DrawableId>,
    }

    impl SelectionInfoHost for Host {
        fn drawable_object(&self, draw: DrawableId) -> Option<ObjectId> {
            Some(draw)
        }
        fn is_locally_controlled(&self, obj: ObjectId) -> bool {
            self.mine.contains(&obj)
        }
        fn is_kind_of(&self, obj: ObjectId, k: KindOfType) -> bool {
            k == KINDOF_INFANTRY && self.infantry.contains(&obj)
        }
        fn relationship_to_local_player(&self, obj: ObjectId) -> Relationship {
            self.rel.get(&obj).copied().unwrap_or(Relationship::Neutral)
        }
        fn can_local_player_garrison(&self, _obj: ObjectId) -> bool {
            false
        }
        fn force_fire_or_move(&self) -> bool {
            false
        }
        fn prefer_selection(&self) -> bool {
            false
        }
        fn use_alternate_mouse(&self) -> bool {
            false
        }
    }

    fn valid(h: &mut Host, draw: DrawableId) -> bool {
        h.context_valid.contains(&draw)
    }

    /// C2a: `contextCommandForNewSelection`'s decision. Predicted from SelectionInfo.cpp, with my units 1 and 2 and an enemy 9:
    /// - nothing selected, clicking unit 1: counted as one new mine, and not a command (nothing to command);
    /// - unit 1 selected, clicking the enemy: a command only if the context command is valid (attack, C2b): false without it,
    ///   true with it;
    /// - unit 1 selected, drag over the enemy (not a point): false;
    /// - unit 1 selected, clicking my unit 2: a command only if valid; with no valid command it is a selection.
    #[test]
    fn a_click_is_a_selection_unless_it_is_a_context_command() {
        let mut h = Host { mine: vec![1, 2], ..Default::default() };
        h.rel.insert(9, Relationship::Enemies);
        let mut si = SelectionInfo::default();
        assert!(!context_command_for_new_selection(&[], &[1], &mut si, true, &mut h, &mut valid));
        assert_eq!((si.new_count_mine, si.current_count_mine), (1, 0));
        let mut si = SelectionInfo::default();
        assert!(!context_command_for_new_selection(&[1], &[9], &mut si, true, &mut h, &mut valid));
        assert_eq!((si.current_count_mine, si.new_count_enemies), (1, 1));
        h.context_valid.push(9);
        assert!(context_command_for_new_selection(&[1], &[9], &mut SelectionInfo::default(), true, &mut h, &mut valid));
        assert!(!context_command_for_new_selection(&[1], &[9], &mut SelectionInfo::default(), false, &mut h, &mut valid));
        assert!(!context_command_for_new_selection(&[1], &[2], &mut SelectionInfo::default(), true, &mut h, &mut valid));
    }
}
