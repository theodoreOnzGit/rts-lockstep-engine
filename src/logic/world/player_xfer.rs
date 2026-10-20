// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp
//   (`Player::crc`, `Player::xfer`, `Player::loadPostProcess`)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Slice 3 of `Player` (`p1w10-sonnet`): the save, load and CRC of a player, as free functions over
//! [`PlayerXferHost`]. Read `player_ops.rs` first.
//!
//! `Player::xfer` reaches the upgrade templates and the name keys (sciences, upgrade masks), the team factory (the
//! prototypes and the default team), the score keeper and the AI, so it is a function over a host like
//! `team_xfer.rs`'s, not a `Snapshot`. A nested `xferSnapshot` is kept the way `team_xfer.rs` keeps it: through the
//! xfer's own `xfer_snapshot` (on a load that counts one post-process request), by lending it a one-off snapshot that
//! holds the host for the duration of the call ([`xfer_nested`]).
//!
//! # Where upstream throws
//!
//! `SC_INVALID_DATA` ([`PlayerError::InvalidData`]): an upgrade or a team prototype the world does not have, an AI,
//! resource gathering manager or tunnel tracker present in the file and not in the player (or the other way round), a
//! kind-of production change or special power timer list that is not empty on a load, a squad count that is not
//! `NUM_HOTKEY_SQUADS`, a squad that is `NULL`.
//!
//! # Upstream quirks kept
//!
//! - **`Player::crc` writes the battle plan bonuses, the skill points and the purchase points only**, and tests the
//!   bonuses' presence with a `Bool`, so a CRC of two players that differ in anything else is equal.
//! - **The upgrade count is a 16-bit `UnsignedShort`** while a save writes every upgrade; a load of an upgrade adds
//!   it with `UPGRADE_STATUS_INVALID` (so no mask changes) and then reads its status.
//! - **A load reads the current selection only if the file has one**, and a player that has one the file does not
//!   keeps it.
//! - **A load into a player that has a battle plan bonus the file does not replaces it by nothing**, and a file with
//!   one replaces it by a new one whose fields are all read.
//! - **`Squad::xfer` on a load pushes onto the ids the squad holds** and refuses a squad whose `getAllObjects` cache
//!   is left filled (see `player_squads.rs`).

use crate::common::science::{xfer_science_vec, ScienceXferError};
use crate::common::unicode_string::UnicodeString;
use crate::common::upgrade::{xfer_upgrade_mask, Upgrade, UpgradeCenter, XferUpgradeMaskError, UPGRADE_STATUS_INVALID};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus};
use crate::logic::sides_list::BuildListInfo;

use super::ids::{PlayerIndex, TEAM_ID_INVALID};
use super::player::*;
use super::player_ops::*;
use super::player_squads::*;
use super::team_ops::*;
use crate::common::name_key_generator::NameKeyGenerator;

/// What `Player::xfer` asks beyond [`PlayerSquadHost`].
pub trait PlayerXferHost: PlayerSquadHost {
    /// `TheUpgradeCenter` and the name keys together (`xferUpgradeMask` reads the one and may add to the other).
    fn upgrade_center_and_names(&mut self) -> (&UpgradeCenter, &mut NameKeyGenerator);
    /// `xfer->xferSnapshot( &m_scoreKeeper )`'s body: `m_scoreKeeper.xfer(xfer)` (the score keeper reaches the thing
    /// templates the world knows, so it is the host's to run).
    fn xfer_score_keeper(&mut self, p: PlayerIndex, xfer: &mut dyn Xfer) -> PResult<()>;
    /// `xfer->xferSnapshot( m_ai )`'s body: `m_ai->xfer(xfer)`.
    fn xfer_ai(&mut self, p: PlayerIndex, xfer: &mut dyn Xfer) -> PResult<()>;
}

fn pl<H: PlayerXferHost>(h: &H, p: PlayerIndex) -> PResult<&Player> {
    h.players().get_nth(p).ok_or(PlayerError::NoPlayer)
}

fn plm<H: PlayerXferHost>(h: &mut H, p: PlayerIndex) -> PResult<&mut Player> {
    h.players_mut().get_nth_mut(p).ok_or(PlayerError::NoPlayer)
}

fn science_err(e: ScienceXferError) -> PlayerError {
    match e {
        ScienceXferError::Xfer(s) => PlayerError::Xfer(s),
        ScienceXferError::NameKeysExhausted => PlayerError::NameKeysExhausted,
    }
}

fn upgrade_mask_err(e: XferUpgradeMaskError) -> PlayerError {
    match e {
        XferUpgradeMaskError::Xfer(s) => PlayerError::Xfer(s),
        XferUpgradeMaskError::NameKeysExhausted => PlayerError::NameKeysExhausted,
    }
}

/// `xferSnapshot` of a snapshot that needs the host (see the module docs).
pub fn xfer_nested<H, F>(host: &mut H, xfer: &mut dyn Xfer, f: F) -> PResult<()>
where
    F: FnMut(&mut H, &mut dyn Xfer) -> PResult<()>,
{
    struct Nested<'a, H, F> {
        host: &'a mut H,
        f: F,
        err: Option<PlayerError>,
    }
    impl<'a, H, F: FnMut(&mut H, &mut dyn Xfer) -> PResult<()>> Snapshot for Nested<'a, H, F> {
        fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
            Ok(())
        }
        fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
            match (self.f)(self.host, xfer) {
                Ok(()) => Ok(()),
                Err(e) => {
                    self.err = Some(e);
                    Err(XferStatus::ErrorUnknown)
                }
            }
        }
        fn load_post_process(&mut self) {}
    }
    let mut n = Nested { host, f, err: None };
    let r = xfer.xfer_snapshot(&mut n);
    match n.err.take() {
        Some(e) => Err(e),
        None => r.map_err(PlayerError::Xfer),
    }
}

fn xfer_user_u32(xfer: &mut dyn Xfer, v: &mut u32) -> PResult<()> {
    let mut b = v.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    *v = u32::from_le_bytes(b);
    Ok(())
}

/// The fields `crc` and `xfer` both write for the battle plan bonuses (`m_armorScalar` ... `m_invalidKindOf`).
fn xfer_battle_plan_bonus_fields(xfer: &mut dyn Xfer, b: &mut BattlePlanBonuses) -> PResult<()> {
    if !plant(301) { xfer.xfer_real(&mut b.armor_scalar)?; }
    if !plant(302) { xfer.xfer_real(&mut b.sight_range_scalar)?; }
    if !plant(303) { xfer.xfer_int(&mut b.bombardment)?; }
    if !plant(304) { xfer.xfer_int(&mut b.hold_the_line)?; }
    if !plant(305) { xfer.xfer_int(&mut b.search_and_destroy)?; }
    b.valid_kind_of.xfer(xfer)?;
    b.invalid_kind_of.xfer(xfer)?;
    Ok(())
}

/// `Player::crc(xfer)`.
pub fn player_crc<H: PlayerXferHost>(h: &mut H, xfer: &mut dyn Xfer, p: PlayerIndex) -> PResult<()> {
    // Player battle plan bonuses
    let mut battle_plan_bonus = pl(h, p)?.battle_plan_bonuses.is_some();
    if !plant(306) { xfer.xfer_bool(&mut battle_plan_bonus)?; }
    if let Some(b) = plm(h, p)?.battle_plan_bonuses.as_mut() {
        xfer_battle_plan_bonus_fields(xfer, b)?;
    }
    let me = plm(h, p)?;
    if !plant(307) { if plant(388) { xfer.xfer_int(&mut me.science_purchase_points)?; } else { xfer.xfer_int(&mut me.skill_points)?; } }
    if !plant(308) { if plant(388) { xfer.xfer_int(&mut me.skill_points)?; } else { xfer.xfer_int(&mut me.science_purchase_points)?; } }
    Ok(())
}

/// `xferScienceVec` on one of the player's science lists (selected by `which`: 0 `m_sciences`, 1 `m_sciencesDisabled`,
/// 2 `m_sciencesHidden`). The list keeps what was read before a failure, as upstream's does.
fn xfer_player_science_vec<H: PlayerXferHost>(h: &mut H, xfer: &mut dyn Xfer, p: PlayerIndex, which: u8) -> PResult<()> {
    let mut v = {
        let me = plm(h, p)?;
        match which {
            0 => std::mem::take(&mut me.sciences),
            1 => std::mem::take(&mut me.sciences_disabled),
            _ => std::mem::take(&mut me.sciences_hidden),
        }
    };
    let r = xfer_science_vec(xfer, h.names().0, &mut v);
    let me = plm(h, p)?;
    match which {
        0 => me.sciences = v,
        1 => me.sciences_disabled = v,
        _ => me.sciences_hidden = v,
    }
    r.map_err(science_err)
}

/// `xferUpgradeMask` on `m_upgradesInProgress` (`which` 0) or `m_upgradesCompleted` (1).
fn xfer_player_upgrade_mask<H: PlayerXferHost>(h: &mut H, xfer: &mut dyn Xfer, p: PlayerIndex, which: u8) -> PResult<()> {
    let mut mask = {
        let me = plm(h, p)?;
        if which == 0 { me.upgrades_in_progress } else { me.upgrades_completed }
    };
    let r = {
        let (center, names) = h.upgrade_center_and_names();
        xfer_upgrade_mask(xfer, &mut mask, center, names)
    };
    let me = plm(h, p)?;
    if which == 0 {
        me.upgrades_in_progress = mask;
    } else {
        me.upgrades_completed = mask;
    }
    r.map_err(upgrade_mask_err)
}

/// `Player::xfer(xfer)`, version 8 (2: the skill point modifier; 3: the score screen flag; 4: the shared special power
/// timers; 5: the sciences as names; 6: `m_unitsShouldHunt`; 7: the preorder flag; 8: the disabled and hidden sciences).
pub fn player_xfer<H: PlayerXferHost>(h: &mut H, xfer: &mut dyn Xfer, p: PlayerIndex) -> PResult<()> {
    // version
    let current_version = 8;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    // money
    if !plant(381) {
        xfer_nested(h, xfer, |h, x| plm(h, p)?.money.xfer(x).map_err(PlayerError::Xfer))?;
    }

    // upgrade list count
    let mut upgrade_count = pl(h, p)?.upgrades.len() as u16 + plant(387) as u16;
    if !plant(309) { xfer.xfer_unsigned_short(&mut upgrade_count)?; }

    if (version >= 7) != plant(351) {
        // preorder info
        let mut v = pl(h, p)?.is_preorder;
        if !plant(310) { xfer.xfer_bool(&mut v)?; }
        plm(h, p)?.is_preorder = v;
    }

    if (version >= 8) != plant(352) {
        xfer_player_science_vec(h, xfer, p, if plant(386) { 2 } else { 1 })?;
        xfer_player_science_vec(h, xfer, p, 2)?;
    }

    // xfer upgrade instances
    if (xfer.get_xfer_mode() == XferMode::Save) != plant(353) {
        for i in 0..pl(h, p)?.upgrades.len() {
            // write upgrade name
            let up = pl(h, p)?.upgrades[i];
            let mut upgrade_name = {
                let (center, _) = h.upgrade_center_and_names();
                center.find_upgrade_by_key(up.template).ok_or(PlayerError::Null)?.upgrade_name().to_vec()
            };
            if !plant(311) { xfer.xfer_ascii_string(&mut upgrade_name)?; }

            // xfer upgrade data
            let mut u = Upgrade::new(up.template);
            u.set_status(up.status);
            xfer.xfer_snapshot(&mut u)?;
        }
    } else {
        for _ in 0..upgrade_count {
            // read name
            let mut upgrade_name = Vec::new();
            xfer.xfer_ascii_string(&mut upgrade_name)?;

            // find template for this upgrade
            let key = {
                let (center, names) = h.upgrade_center_and_names();
                center.find_upgrade(names, &upgrade_name).map_err(|_| PlayerError::NameKeysExhausted)?.map(|t| t.upgrade_name_key())
            };
            // sanity
            let Some(key) = key else {
                return Err(if plant(354) { PlayerError::Null } else { PlayerError::InvalidData });
            };

            // add upgrade to player, the status is invalid, but that's OK cause we're about to xfer it
            let idx = player_add_upgrade(h, p, key, if plant(374) { crate::common::upgrade::UPGRADE_STATUS_COMPLETE } else { UPGRADE_STATUS_INVALID })?;

            // xfer upgrade data
            let mut u = Upgrade::new(key);
            u.set_status(pl(h, p)?.upgrades[idx].status);
            let r = xfer.xfer_snapshot(&mut u);
            plm(h, p)?.upgrades[idx].status = u.status();
            r?;
        }
    }

    // radar info
    {
        let me = plm(h, p)?;
        if !plant(312) { xfer.xfer_int(&mut me.radar_count)?; }
        if !plant(313) { xfer.xfer_bool(&mut me.is_player_dead)?; }
        if !plant(314) { xfer.xfer_int(&mut me.disable_proof_radar_count)?; }
        if !plant(315) { xfer.xfer_bool(&mut me.radar_disabled)?; }
    }

    // upgrades in progress, upgrades complete
    xfer_player_upgrade_mask(h, xfer, p, if plant(385) { 1 } else { 0 })?;
    xfer_player_upgrade_mask(h, xfer, p, 1)?;

    // energy info
    if !plant(382) {
        xfer_nested(h, xfer, |h, x| plm(h, p)?.energy.xfer(x).map_err(PlayerError::Xfer))?;
    }

    //
    // team prototypes ... this is only the fact that team prototypes are on this player
    // it is not the team prototype data itself
    //
    let mut prototype_count = pl(h, p)?.team_prototypes.len() as u16;
    if !plant(316) { xfer.xfer_unsigned_short(&mut prototype_count)?; }
    if (xfer.get_xfer_mode() == XferMode::Save) != plant(355) {
        for i in 0..pl(h, p)?.team_prototypes.len() {
            let mut prototype_id = pl(h, p)?.team_prototypes[i];
            xfer_user_u32(xfer, &mut prototype_id)?;
        }
    } else {
        // empty the list right now
        if !plant(375) {
            plm(h, p)?.team_prototypes.clear();
        }

        // read all the data
        for _ in 0..prototype_count {
            // read id
            let mut prototype_id = 0u32;
            xfer_user_u32(xfer, &mut prototype_id)?;

            // find prototype
            let Some(prototype) = factory_find_team_prototype_by_id(h, prototype_id) else {
                return Err(if plant(356) { PlayerError::Null } else { PlayerError::InvalidData });
            };

            // put in list
            plm(h, p)?.team_prototypes.push(prototype);
        }
    }

    // build list info
    let mut build_list_info_count = pl(h, p)?.build_list.len() as u16;
    if !plant(317) { xfer.xfer_unsigned_short(&mut build_list_info_count)?; }
    if (xfer.get_xfer_mode() == XferMode::Save) != plant(357) {
        // xfer each build list info
        for i in 0..pl(h, p)?.build_list.len() {
            let mut e = pl(h, p)?.build_list[i].clone();
            xfer.xfer_snapshot(&mut e)?;
        }
    } else {
        //
        // destroy any build list that we got from loading the bare bones map, note that deleting
        // the head of these structures automatically deletes any links attached
        //
        if !plant(376) {
            plm(h, p)?.build_list.clear();
        }

        // read each build list info
        for _ in 0..build_list_info_count {
            // allocate new build list, attach to the *end* of the list in the player
            let mut e = BuildListInfo::new();
            // xfer data
            let r = xfer.xfer_snapshot(&mut e);
            plm(h, p)?.build_list.push(e);
            r?;
        }
    }

    // ai player data
    let mut ai_player_present = pl(h, p)?.ai.is_some();
    if !plant(318) { xfer.xfer_bool(&mut ai_player_present)?; }
    if ai_player_present != pl(h, p)?.ai.is_some() && !plant(358) {
        return Err(PlayerError::InvalidData);
    }
    if ai_player_present
        && !plant(384) {
            xfer_nested(h, xfer, |h, x| h.xfer_ai(p, x))?;
        }

    // resource gathering manager
    let mut present = pl(h, p)?.resource_gathering_manager.is_some();
    if !plant(319) { xfer.xfer_bool(&mut present)?; }
    if present != pl(h, p)?.resource_gathering_manager.is_some() && !plant(359) {
        return Err(PlayerError::InvalidData);
    }
    if present {
        let mut m = plm(h, p)?.resource_gathering_manager.take().ok_or(PlayerError::Null)?;
        let r = xfer.xfer_snapshot(&mut m);
        plm(h, p)?.resource_gathering_manager = Some(m);
        r?;
    }

    // tunnel tracking system
    let mut present = pl(h, p)?.tunnel_system.is_some();
    if !plant(320) { xfer.xfer_bool(&mut present)?; }
    if present != pl(h, p)?.tunnel_system.is_some() && !plant(360) {
        return Err(PlayerError::InvalidData);
    }
    if present {
        let mut t = plm(h, p)?.tunnel_system.take().ok_or(PlayerError::Null)?;
        let r = xfer.xfer_snapshot(&mut t);
        plm(h, p)?.tunnel_system = Some(t);
        r?;
    }

    // default team
    let mut team_id = pl(h, p)?.default_team.unwrap_or(TEAM_ID_INVALID);
    xfer_user_u32(xfer, &mut team_id)?;
    if xfer.get_xfer_mode() == XferMode::Load && !plant(361) {
        plm(h, p)?.default_team = if plant(377) { None } else { factory_find_team_by_id(h, team_id) };
    }

    // sciences
    if (version >= 5) != plant(362) {
        // m_sciences will contain some intrinsic sciences and stuff, which we don't want.
        // so nuke 'em for load.
        if xfer.get_xfer_mode() == XferMode::Load && !plant(363) {
            plm(h, p)?.sciences.clear();
        }
        xfer_player_science_vec(h, xfer, p, 0)?;
    } else {
        // This code is WRONG WRONG WRONG and must not be used or mimicked; it
        // is present for backwards "compatibility" only. (srj)
        let mut science_count = pl(h, p)?.sciences.len() as u16;
        xfer.xfer_unsigned_short(&mut science_count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for i in 0..pl(h, p)?.sciences.len() {
                let mut b = pl(h, p)?.sciences[i].0.to_le_bytes();
                xfer.xfer_user(&mut b)?;
            }
        } else {
            for _ in 0..science_count {
                let mut b = [0u8; 4];
                xfer.xfer_user(&mut b)?;
                plm(h, p)?.sciences.push(crate::common::science::ScienceType(i32::from_le_bytes(b)));
            }
        }
    }

    {
        let me = plm(h, p)?;
        // rank level, skill points, science purchase points, level up, level down
        if !plant(321) { xfer.xfer_int(&mut me.rank_level)?; }
        if !plant(322) { xfer.xfer_int(&mut me.skill_points)?; }
        if !plant(323) { xfer.xfer_int(&mut me.science_purchase_points)?; }
        if !plant(324) { xfer.xfer_int(&mut me.level_up)?; }
        if !plant(325) { xfer.xfer_int(&mut me.level_down)?; }

        // general name
        let mut name: Vec<u16> = me.general_name.str().to_vec();
        if !plant(326) { xfer.xfer_unicode_string(&mut name)?; }
        let mut s = UnicodeString::default();
        s.set(&name).map_err(|_| PlayerError::Xfer(XferStatus::InvalidParameters))?;
        me.general_name = s;
    }

    // player relations, team relations
    {
        let mut m = std::mem::take(&mut plm(h, p)?.player_relations);
        let r = xfer.xfer_snapshot(&mut m);
        plm(h, p)?.player_relations = m;
        r?;
        let mut m = std::mem::take(&mut plm(h, p)?.team_relations);
        let r = xfer.xfer_snapshot(&mut m);
        plm(h, p)?.team_relations = m;
        r?;
    }

    {
        let me = plm(h, p)?;
        // can build units, can build base, observer
        if !plant(327) { xfer.xfer_bool(&mut me.can_build_units)?; }
        if !plant(328) { xfer.xfer_bool(&mut me.can_build_base)?; }
        if !plant(329) { xfer.xfer_bool(&mut me.observer)?; }

        if (version >= 2) != plant(364) {
            // current skill point modifier value
            if !plant(330) { xfer.xfer_real(&mut me.skill_points_modifier)?; }
        } else {
            me.skill_points_modifier = 1.0;
        }

        if (version >= 3) != plant(365) {
            if !plant(331) { xfer.xfer_bool(&mut me.list_in_score_screen)?; }
        } else {
            me.list_in_score_screen = true;
        }

        // attacked by
        let mut bytes = [0u8; MAX_PLAYER_COUNT as usize];
        for (i, b) in me.attacked_by.iter().enumerate() {
            bytes[i] = *b as u8;
        }
        if !plant(332) { xfer.xfer_user(&mut bytes)?; }
        for (i, b) in me.attacked_by.iter_mut().enumerate() {
            *b = bytes[i] != 0 && !plant(380);
        }

        // cash bounty percent
        if !plant(333) { xfer.xfer_real(&mut me.cash_bounty_percent)?; }
    }

    // score keeper
    if !plant(383) {
        xfer_nested(h, xfer, |h, x| h.xfer_score_keeper(p, x))?;
    }

    // size of and data for kindof percent production change list
    let mut percent_production_change_count = pl(h, p)?.kind_of_production_changes.len() as u16;
    if !plant(334) { xfer.xfer_unsigned_short(&mut percent_production_change_count)?; }
    if xfer.get_xfer_mode() == XferMode::Save {
        // save each item
        for i in 0..pl(h, p)?.kind_of_production_changes.len() {
            let me = plm(h, p)?;
            let entry = &mut me.kind_of_production_changes[i];
            entry.kind_of.xfer(xfer)?;
            if !plant(335) { xfer.xfer_real(&mut entry.percent)?; }
            if !plant(336) { xfer.xfer_unsigned_int(&mut entry.ref_count)?; }
        }
    } else {
        // sanity, list must be empty right now
        if !pl(h, p)?.kind_of_production_changes.is_empty() && !plant(366) {
            return Err(PlayerError::InvalidData);
        }

        // read each entry
        for _ in 0..percent_production_change_count {
            let mut entry = KindOfPercentProductionChange { kind_of: Default::default(), percent: 0.0, ref_count: 0 };
            // read data
            entry.kind_of.xfer(xfer)?;
            if !plant(337) { xfer.xfer_real(&mut entry.percent)?; }
            if !plant(338) { xfer.xfer_unsigned_int(&mut entry.ref_count)?; }
            // put at end of list
            plm(h, p)?.kind_of_production_changes.push(entry);
        }
    }

    if (version < 4) != plant(367) {
        plm(h, p)?.special_power_timers.clear();
    } else {
        let mut timer_list_size = pl(h, p)?.special_power_timers.len() as u16;
        if !plant(339) { xfer.xfer_unsigned_short(&mut timer_list_size)?; }
        if xfer.get_xfer_mode() == XferMode::Save {
            for i in 0..pl(h, p)?.special_power_timers.len() {
                let timer = &mut plm(h, p)?.special_power_timers[i];
                if !plant(340) { xfer.xfer_unsigned_int(&mut timer.template_id)?; }
                if !plant(341) { xfer.xfer_unsigned_int(&mut timer.ready_frame)?; }
            }
        } else {
            // sanity, list must be empty right now
            if !pl(h, p)?.special_power_timers.is_empty() && !plant(368) {
                return Err(PlayerError::InvalidData);
            }
            // read each entry
            for _ in 0..timer_list_size {
                let mut timer = SpecialPowerReadyTimer { template_id: 0, ready_frame: 0 };
                if !plant(342) { xfer.xfer_unsigned_int(&mut timer.template_id)?; }
                if !plant(343) { xfer.xfer_unsigned_int(&mut timer.ready_frame)?; }
                plm(h, p)?.special_power_timers.push(timer);
            }
        }
    }

    // squads
    let mut squad_count = NUM_HOTKEY_SQUADS as u16;
    if !plant(344) { xfer.xfer_unsigned_short(&mut squad_count)?; }
    if squad_count as usize != NUM_HOTKEY_SQUADS && !plant(369) {
        return Err(PlayerError::InvalidData);
    }
    for i in 0..squad_count as usize {
        if pl(h, p)?.squads.get(i).is_none() && !plant(370) {
            return Err(PlayerError::InvalidData);
        }
        let mut sq = std::mem::take(&mut plm(h, p)?.squads[i]);
        let r = xfer_nested(&mut sq, xfer, |s, x| squad_xfer(s, x));
        plm(h, p)?.squads[i] = sq;
        r?;
    }

    // current squad selection
    let mut current_selection_present = pl(h, p)?.current_selection.is_some();
    if !plant(345) { xfer.xfer_bool(&mut current_selection_present)?; }
    if current_selection_present {
        // allocate squad if needed
        if (pl(h, p)?.current_selection.is_none() || plant(371)) && xfer.get_xfer_mode() == XferMode::Load {
            plm(h, p)?.current_selection = Some(Squad::default());
        }
        if plant(379) {
            plm(h, p)?.current_selection = None;
            return Ok(());
        }
        // xfer
        let mut sq = plm(h, p)?.current_selection.take().ok_or(PlayerError::Null)?;
        let r = xfer_nested(&mut sq, xfer, |s, x| squad_xfer(s, x));
        plm(h, p)?.current_selection = Some(sq);
        r?;
    }

    // Player battle plan bonuses
    let mut battle_plan_bonus = pl(h, p)?.battle_plan_bonuses.is_some();
    if !plant(346) { xfer.xfer_bool(&mut battle_plan_bonus)?; } // If we're loading, it just replaces the bool
    if xfer.get_xfer_mode() == XferMode::Load && !plant(372) {
        let me = plm(h, p)?;
        if !plant(378) {
            me.battle_plan_bonuses = None;
        }
        if battle_plan_bonus {
            me.battle_plan_bonuses = Some(BattlePlanBonuses {
                armor_scalar: 0.0,
                bombardment: 0,
                search_and_destroy: 0,
                hold_the_line: 0,
                sight_range_scalar: 0.0,
                valid_kind_of: Default::default(),
                invalid_kind_of: Default::default(),
            });
        }
    }
    if let Some(b) = plm(h, p)?.battle_plan_bonuses.as_mut() {
        xfer_battle_plan_bonus_fields(xfer, b)?;
    }
    {
        let me = plm(h, p)?;
        if !plant(347) { xfer.xfer_int(&mut me.bombard_battle_plans)?; }
        if !plant(348) { xfer.xfer_int(&mut me.hold_the_line_battle_plans)?; }
        if !plant(349) { xfer.xfer_int(&mut me.search_and_destroy_battle_plans)?; }

        if (version >= 6) != plant(373) {
            if !plant(350) { xfer.xfer_bool(&mut me.units_should_hunt)?; }
        } else {
            me.units_should_hunt = false;
        }
    }
    Ok(())
}

/// `Player::loadPostProcess()`: nothing.
pub fn player_load_post_process<H: PlayerXferHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    pl(h, p).map(|_| ())
}
