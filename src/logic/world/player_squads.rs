// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Player.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp
//   (slice 2b: the upgrade list, the hotkey squads and the team messages that fill them)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Squad.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/Squad.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Slice 2b of `Player` (`p1w10-sonnet`): the upgrade list and the hotkey squads, over [`PlayerSquadHost`]. Read
//! `player_objects.rs` and `player_ops.rs` first.

use crate::common::game_type::ObjectId;
use crate::common::name_key_generator::NameKeyType;
use crate::common::xfer::{Xfer, XferMode};
use crate::common::upgrade::{UpgradeMaskType, UPGRADE_STATUS_COMPLETE, UPGRADE_STATUS_IN_PRODUCTION};
use crate::logic::client_event::ClientEvent;

use super::ids::{PlayerIndex, TeamId};
use super::player::*;
use super::player_objects::*;
use super::player_ops::*;
use super::team_ops::*;

/// What the upgrades and the hotkey squads ask beyond [`PlayerObjectHost`].
pub trait PlayerSquadHost: PlayerObjectHost {
    /// `upgradeTemplate->getUpgradeMask()` for the template named by its name key.
    fn upgrade_mask(&self, t: NameKeyType) -> UpgradeMaskType;
    /// `obj->updateUpgradeModules()`.
    fn obj_update_upgrade_modules(&mut self, o: ObjectId);
    /// `obj->isSelectable()`.
    fn obj_is_selectable(&self, o: ObjectId) -> bool;
    /// `group->getAllIDs()`.
    fn ai_group_all_ids(&self, group: GroupRef) -> Vec<ObjectId>;
    /// `msg->getArgumentCount()`.
    fn message_argument_count(&self, msg: MessageRef) -> u8;
    /// `msg->getArgument(i)->objectID`.
    fn message_argument_object_id(&self, msg: MessageRef, i: u8) -> ObjectId;
    /// `getAcademyStats()->recordControlGroupsUsed()`.
    fn academy_record_control_groups_used(&mut self, p: PlayerIndex);
}

/// A `GameMessage*`.
pub type MessageRef = u32;

fn pl<H: PlayerSquadHost>(h: &H, p: PlayerIndex) -> PResult<&Player> {
    h.players().get_nth(p).ok_or(PlayerError::NoPlayer)
}

fn plm<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex) -> PResult<&mut Player> {
    h.players_mut().get_nth_mut(p).ok_or(PlayerError::NoPlayer)
}

// ---- upgrades ----------------------------------------------------------------------------------------------------

/// `Player::findUpgrade(upgradeTemplate)`: the index in the list.
pub fn player_find_upgrade<H: PlayerSquadHost>(h: &H, p: PlayerIndex, upgrade_template: NameKeyType) -> PResult<Option<usize>> {
    Ok(pl(h, p)?.upgrades.iter().position(|u| u.template == upgrade_template))
}

/// `Player::hasUpgradeComplete(upgradeTemplate)`.
pub fn player_has_upgrade_complete<H: PlayerSquadHost>(h: &H, p: PlayerIndex, upgrade_template: NameKeyType) -> PResult<bool> {
    let mask = h.upgrade_mask(upgrade_template);
    player_has_upgrade_complete_mask(h, p, &mask)
}

/// `Player::hasUpgradeComplete(testMask)`: every bit of the mask is complete.
pub fn player_has_upgrade_complete_mask<H: PlayerSquadHost>(h: &H, p: PlayerIndex, test_mask: &UpgradeMaskType) -> PResult<bool> {
    Ok(if plant(210) { pl(h, p)?.upgrades_completed.test_for_any(test_mask) } else { pl(h, p)?.upgrades_completed.test_for_all(test_mask) })
}

/// `Player::hasUpgradeInProduction(upgradeTemplate)`.
pub fn player_has_upgrade_in_production<H: PlayerSquadHost>(h: &H, p: PlayerIndex, upgrade_template: NameKeyType) -> PResult<bool> {
    let mask = h.upgrade_mask(upgrade_template);
    Ok(if plant(211) { pl(h, p)?.upgrades_completed.test_for_all(&mask) } else { pl(h, p)?.upgrades_in_progress.test_for_all(&mask) })
}

/// `Player::addUpgrade(upgradeTemplate, status)`: the index of the upgrade (a new one goes to the head).
pub fn player_add_upgrade<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, upgrade_template: NameKeyType, status: i32) -> PResult<usize> {
    let idx = match player_find_upgrade(h, p, upgrade_template)? {
        Some(i) => i,
        None => {
            let pos = if plant(201) { pl(h, p)?.upgrades.len() } else { 0 };
            plm(h, p)?.upgrades.insert(pos, PlayerUpgrade { template: upgrade_template, status: 0 });
            pos
        }
    };
    plm(h, p)?.upgrades[idx].status = status;
    let new_mask = h.upgrade_mask(upgrade_template);
    if status == UPGRADE_STATUS_IN_PRODUCTION {
        if plant(202) {
            plm(h, p)?.upgrades_completed.set_mask(&new_mask);
        } else {
            plm(h, p)?.upgrades_in_progress.set_mask(&new_mask);
        }
    } else if status == UPGRADE_STATUS_COMPLETE {
        let me = plm(h, p)?;
        if !plant(203) {
            me.upgrades_in_progress.clear_mask(&new_mask);
        }
        if !plant(204) {
            me.upgrades_completed.set_mask(&new_mask);
        }
        if !plant(205) {
            player_on_upgrade_completed(h, p, upgrade_template)?;
        }
    }
    if h.players().local_index() == Some(p) || plant(206) {
        h.client_event(ClientEvent::Player(PlayerClientEvent::ControlBarDirty));
    }
    Ok(idx)
}

/// `Player::onUpgradeCompleted(upgradeTemplate)`: every object rechecks its upgrade modules.
pub fn player_on_upgrade_completed<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, _upgrade_template: NameKeyType) -> PResult<()> {
    for_each_member(h, p, |h, obj| h.obj_update_upgrade_modules(obj))
}

/// `Player::removeUpgrade(upgradeTemplate)`.
pub fn player_remove_upgrade<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, upgrade_template: NameKeyType) -> PResult<()> {
    if let Some(i) = player_find_upgrade(h, p, upgrade_template)? {
        let old_mask = h.upgrade_mask(upgrade_template);
        let me = plm(h, p)?;
        let _status = me.upgrades.remove(i).status;
        if !plant(207) {
            me.upgrades_in_progress.clear_mask(&old_mask);
        }
        if !plant(208) {
            me.upgrades_completed.clear_mask(&old_mask);
        }
        // if the status was COMPLETE: onUpgradeRemoved(), which does nothing
        if h.players().local_index() == Some(p) || plant(209) {
            h.client_event(ClientEvent::Player(PlayerClientEvent::ControlBarDirty));
        }
    }
    Ok(())
}

// ---- the hotkey squads ---------------------------------------------------------------------------------------------

/// `Squad::addObject(objectToAdd)`.
pub fn squad_add_object(s: &mut Squad, o: Option<ObjectId>) {
    if let Some(o) = o {
        s.object_ids.push(o);
    }
}

/// `Squad::addObjectID(objectID)`.
pub fn squad_add_object_id(s: &mut Squad, id: ObjectId) {
    s.object_ids.push(id);
}

/// `Squad::removeObject(objectToRemove)`: the first match.
pub fn squad_remove_object(s: &mut Squad, o: Option<ObjectId>) {
    if let Some(o) = o {
        if let Some(i) = if plant(212) { s.object_ids.iter().rposition(|x| *x == o) } else { s.object_ids.iter().position(|x| *x == o) } {
            s.object_ids.remove(i);
        }
    }
}

/// `Squad::clearSquad()`.
pub fn squad_clear(s: &mut Squad) {
    if !plant(214) {
        s.object_ids.clear();
    }
    if !plant(213) {
        s.objects_cached.clear();
    }
}

/// `Squad::getAllObjects()`: the objects that still exist; the ones that do not are dropped from the squad. The result
/// is also left in the squad's cache (`m_objectsCached`), as upstream does, and returned as a copy.
pub fn squad_get_all_objects<H: PlayerSquadHost>(h: &H, s: &mut Squad) -> Vec<ObjectId> {
    s.objects_cached.clear();
    let mut cached = Vec::new();
    s.object_ids.retain(|id| match h.find_object_by_id(*id) {
        Some(o) => {
            cached.push(o);
            true
        }
        None => plant(215),
    });
    s.objects_cached = cached.clone();
    cached
}

/// `Squad::getLiveObjects()`: those of them that are selectable (the cache is left holding exactly these).
pub fn squad_get_live_objects<H: PlayerSquadHost>(h: &H, s: &mut Squad) -> Vec<ObjectId> {
    squad_get_all_objects(h, s);
    if !plant(217) {
        s.objects_cached.retain(|o| h.obj_is_selectable(*o) != plant(216));
    }
    s.objects_cached.clone()
}

/// `Squad::crc`: nothing.
pub fn squad_crc(_s: &mut Squad, _xfer: &mut dyn Xfer) -> PResult<()> {
    Ok(())
}

/// `Squad::xfer`, version 1: the count (`UnsignedShort`, the size truncated to 16 bits while a save writes every id),
/// then the ids. A load pushes onto whatever the squad holds (upstream does not clear it first) and refuses a squad
/// whose `m_objectsCached` is not empty with `throw SC_INVALID_DATA` ([`PlayerError::InvalidData`]).
pub fn squad_xfer(s: &mut Squad, xfer: &mut dyn Xfer) -> PResult<()> {
    let current_version = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut object_count = s.object_ids.len() as u16 + plant(234) as u16;
    xfer.xfer_unsigned_short(&mut object_count)?;

    if xfer.get_xfer_mode() == XferMode::Save {
        for id in s.object_ids.iter() {
            let mut object_id = *id;
            if !plant(237) {
                xfer.xfer_object_id(&mut object_id)?;
            }
        }
    } else {
        // the cached objects list should be empty
        if !s.objects_cached.is_empty() && !plant(235) {
            return Err(PlayerError::InvalidData);
        }
        for _ in 0..object_count {
            let mut object_id: ObjectId = 0;
            xfer.xfer_object_id(&mut object_id)?;
            if plant(236) {
                s.object_ids.insert(0, object_id);
            } else {
                s.object_ids.push(object_id);
            }
        }
    }
    Ok(())
}

/// `Squad::getSizeOfGroup()`.
pub fn squad_get_size_of_group(s: &Squad) -> i32 {
    s.object_ids.len() as i32
}

/// `Squad::isOnSquad(objToTest)`.
pub fn squad_is_on_squad(s: &Squad, o: ObjectId) -> bool {
    s.object_ids.contains(&o)
}

/// `Squad::squadFromTeam(fromTeam, clearSquadFirst)`.
pub fn squad_from_team<H: PlayerSquadHost>(h: &H, s: &mut Squad, from_team: Option<TeamId>, clear_squad_first: bool) {
    let Some(t) = from_team else { return };
    if clear_squad_first && !plant(231) {
        s.object_ids.clear();
    }
    s.object_ids.extend(h.teams().members(t));
}

/// `Squad::squadFromAIGroup(fromAIGroup, clearSquadFirst)`: the squad takes the group's ids (a replacement, whatever
/// `clearSquadFirst` says).
pub fn squad_from_ai_group<H: PlayerSquadHost>(h: &H, s: &mut Squad, from_group: Option<GroupRef>, clear_squad_first: bool) {
    let Some(g) = from_group else { return };
    if clear_squad_first {
        s.object_ids.clear();
    }
    if plant(232) {
        s.object_ids.extend(h.ai_group_all_ids(g));
    } else {
        s.object_ids = h.ai_group_all_ids(g);
    }
}

/// `Squad::aiGroupFromSquad(aiGroupToFill)`.
pub fn squad_ai_group_from_squad<H: PlayerSquadHost>(h: &mut H, s: &mut Squad, group: Option<GroupRef>) {
    let Some(g) = group else { return };
    let live = if plant(233) { squad_get_all_objects(h, s) } else { squad_get_live_objects(h, s) };
    for o in live {
        h.ai_group_add(g, o);
    }
}

fn hotkey_in_range(hotkey_num: i32) -> bool {
    (if plant(220) { 1 } else { 0 }..NUM_HOTKEY_SQUADS as i32).contains(&hotkey_num)
}

/// `Player::processCreateTeamGameMessage(hotkeyNum, msg)`.
pub fn player_process_create_team_game_message<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, hotkey_num: i32, msg: MessageRef) -> PResult<()> {
    if !hotkey_in_range(hotkey_num) {
        return Ok(()); // processCreateTeamGameMessage got an invalid hotkey number
    }
    squad_clear(plm(h, p)?.squads.get_mut(hotkey_num as usize).ok_or(PlayerError::NoPlayer)?); // m_squads[n] is NULL before init
    let num_args = h.message_argument_count(msg);
    for i in 0..num_args {
        let obj_id = h.message_argument_object_id(msg, i);
        if let Some(obj) = h.find_object_by_id(obj_id).or(if plant(219) { Some(obj_id) } else { None }) {
            if !plant(218) {
                player_remove_object_from_hotkey_squad(h, p, Some(obj))?;
            }
            squad_add_object(plm(h, p)?.squads.get_mut(hotkey_num as usize).ok_or(PlayerError::NoPlayer)?, Some(obj));
        }
    }
    Ok(())
}

/// `Player::processSelectTeamGameMessage(hotkeyNum, msg)`.
pub fn player_process_select_team_game_message<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, hotkey_num: i32, _msg: MessageRef) -> PResult<()> {
    if !hotkey_in_range(hotkey_num) {
        return Ok(());
    }
    if plm(h, p)?.squads.is_empty() {
        return Ok(()); // `if (m_squads[hotkeyNum] == NULL) return;`: before init all are NULL
    }
    let mut squad = std::mem::take(&mut plm(h, p)?.squads[hotkey_num as usize]);
    let object_list = if plant(238) { squad_get_all_objects(h, &mut squad) } else { squad_get_live_objects(h, &mut squad) };
    plm(h, p)?.squads[hotkey_num as usize] = squad;
    let sel = plm(h, p)?.current_selection.as_mut().ok_or(PlayerError::NoPlayer)?; // clearSquad() on NULL
    if !plant(222) {
        squad_clear(sel);
    }
    for o in object_list.iter() {
        squad_add_object(sel, Some(*o));
    }
    if !object_list.is_empty() || plant(221) {
        h.academy_record_control_groups_used(p);
    }
    Ok(())
}

/// `Player::processAddTeamGameMessage(hotkeyNum, msg)`.
pub fn player_process_add_team_game_message<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, hotkey_num: i32, _msg: MessageRef) -> PResult<()> {
    if !hotkey_in_range(hotkey_num) {
        return Ok(());
    }
    if plm(h, p)?.squads.is_empty() {
        return Ok(()); // `if (m_squads[hotkeyNum] == NULL) return;`
    }
    if plm(h, p)?.current_selection.is_none() {
        plm(h, p)?.current_selection = Some(Squad::default());
    }
    let mut squad = std::mem::take(&mut plm(h, p)?.squads[hotkey_num as usize]);
    let object_list = if plant(223) { squad_get_all_objects(h, &mut squad) } else { squad_get_live_objects(h, &mut squad) };
    plm(h, p)?.squads[hotkey_num as usize] = squad;
    let sel = plm(h, p)?.current_selection.as_mut().ok_or(PlayerError::NoPlayer)?;
    for o in object_list {
        squad_add_object(sel, Some(o));
    }
    Ok(())
}

/// `Player::getCurrentSelectionAsAIGroup(group)`.
pub fn player_get_current_selection_as_ai_group<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, group: Option<GroupRef>) -> PResult<()> {
    if let Some(mut sel) = if plant(224) { None } else { plm(h, p)?.current_selection.take() } {
        squad_ai_group_from_squad(h, &mut sel, group);
        plm(h, p)?.current_selection = Some(sel);
    }
    Ok(())
}

/// `Player::setCurrentlySelectedAIGroup(group)`.
pub fn player_set_currently_selected_ai_group<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, group: Option<GroupRef>) -> PResult<()> {
    if plm(h, p)?.current_selection.is_none() {
        plm(h, p)?.current_selection = Some(Squad::default());
    }
    let mut sel = plm(h, p)?.current_selection.take().unwrap_or_default();
    if !plant(225) {
        squad_clear(&mut sel);
    }
    if group.is_some() && !plant(226) {
        squad_from_ai_group(h, &mut sel, group, true);
    }
    plm(h, p)?.current_selection = Some(sel);
    Ok(())
}

/// `Player::getHotkeySquad(squadNumber)`: the squad (a copy), or `None` outside the table.
pub fn player_get_hotkey_squad<H: PlayerSquadHost>(h: &H, p: PlayerIndex, squad_number: i32) -> PResult<Option<Squad>> {
    if (0..NUM_HOTKEY_SQUADS as i32).contains(&squad_number) {
        return Ok(pl(h, p)?.squads.get(squad_number as usize).cloned());
    }
    Ok(None)
}

/// `Player::getSquadNumberForObject(objToFind)`: `NO_HOTKEY_SQUAD` (-1) if it is on none.
pub fn player_get_squad_number_for_object<H: PlayerSquadHost>(h: &H, p: PlayerIndex, obj_to_find: ObjectId) -> PResult<i32> {
    let me = pl(h, p)?;
    let mut last = -1;
    for i in 0..NUM_HOTKEY_SQUADS {
        let sq = me.squads.get(i).ok_or(PlayerError::NoPlayer)?; // m_squads[i]->isOnSquad on a NULL squad
        if squad_is_on_squad(sq, obj_to_find) {
            if plant(227) {
                last = i as i32;
            } else {
                return Ok(i as i32);
            }
        }
    }
    Ok(last)
}

/// `Player::removeObjectFromHotkeySquad(objToRemove)`.
pub fn player_remove_object_from_hotkey_squad<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, obj_to_remove: Option<ObjectId>) -> PResult<()> {
    for sq in plm(h, p)?.squads.iter_mut().take(if plant(229) { 5 } else { 99 }) {
        squad_remove_object(sq, obj_to_remove);
    }
    Ok(())
}

/// `Player::addAIGroupToCurrentSelection(group)`.
pub fn player_add_ai_group_to_current_selection<H: PlayerSquadHost>(h: &mut H, p: PlayerIndex, group: Option<GroupRef>) -> PResult<()> {
    let Some(g) = group else { return Ok(()) };
    if plm(h, p)?.current_selection.is_none() {
        plm(h, p)?.current_selection = Some(Squad::default());
    }
    let mut ids = h.ai_group_all_ids(g);
    if plant(230) {
        ids.reverse();
    }
    let sel = plm(h, p)?.current_selection.as_mut().ok_or(PlayerError::NoPlayer)?;
    for id in ids {
        squad_add_object_id(sel, id);
    }
    Ok(())
}
