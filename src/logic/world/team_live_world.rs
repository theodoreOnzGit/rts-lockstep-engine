// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp (the calls `Object` and its modules make into `Team`,
//   `Player`, `Energy`, `ScoreKeeper` and `AcademyStats`)
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Energy.cpp (`addPowerBonus`, `removePowerBonus`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I3b: the world's own team and player answers. `Object` and its modules ask about teams and players
//! (a team's controlling player, a player's mask, colour, relationships, upgrades) and tell them things (a new member, a
//! deposit, a kill's skill points). Until batch I3a the world could not run the team and player code, so those were cut
//! points answered from tables in [`ObjectCuts`](super::object_cuts::ObjectCuts). With `cuts.live_teams` on (off by
//! default, so the differential tests that drive the tables are unchanged) they are the team and player lists':
//!
//! - **The questions** go through [`World::tq`], a read-only view whose methods have the names of the tables' (the
//!   view keeps them from shadowing the trait methods of the same names the world implements); the three that change
//!   state (`findTeam` can make a singleton team, `getProductionVeterancyLevel` can make a name key, the member list)
//!   are world methods: [`World::find_team_by_name`], [`World::player_production_veterancy_level`],
//!   [`World::team_member_list_contains`], [`World::team_member_list_remove`], [`World::team_member_list_prepend`].
//! - **The notifications** ([`CutCall`]'s `Player*`, `Team*` and `AcademyStats*` calls) are still logged, and with
//!   `live_teams` [`World::cut`] also runs them on the lists ([`World::live_team_call`]).
//!
//! The attack priority question (`TheScriptEngine->getAttackInfo(name)`) stays a table: the script engine comes in
//! with batch I8.
//!
//! `p1w13-sonnet`, integration batch I3b, 2026-10-31.

use crate::common::base_type::Coord3D;
use crate::common::game_common::{PlayerMaskType, Relationship, VeterancyLevel};
use crate::common::name_key_generator::NameKeyType;
use crate::common::science::ScienceType;
use crate::common::upgrade::UpgradeMaskType;
use crate::logic::world::academy_stats::AcademyObject;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{ObjectId, PlayerIndex, TeamId};
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::player_objects::{
    player_apply_battle_plan_bonuses_for_object, player_becoming_team_member, player_do_bounty_for_kill,
    player_friend_apply_difficulty_bonuses_for_object, player_get_num_battle_plans_active, player_money_deposit, player_money_withdraw,
    player_on_structure_construction_complete, player_set_units_vision_spied,
};
use crate::logic::world::player_ops::{
    player_add_kind_of_production_cost_change, player_add_radar, player_add_skill_points, player_add_skill_points_for_kill,
    player_deliver_brown_out_notice, player_get_production_veterancy_level, player_get_relationship, player_grant_science,
    player_allowed_to_build, player_get_ai_base_center, player_heal_all_objects, player_is_playable_side, player_is_player_active,
    player_is_skirmish_ai_player, player_on_power_brown_out_change, player_on_unit_created, player_remove_kind_of_production_cost_change,
    player_remove_radar, PResult,
};
use crate::logic::world::player_squads::{player_add_upgrade, player_has_upgrade_complete_mask, player_remove_upgrade};
use crate::logic::world::score_keeper::ScoredObject;
use crate::logic::world::team_ops::{factory_find_team, team_get_controlling_player, team_get_relationship, team_notify_team_of_object_death};

/// The world's team and player questions, read-only. See the module docs.
#[derive(Clone, Copy)]
pub struct TeamQueries<'a> {
    w: &'a World,
}

impl World {
    /// The team and player questions (see the module docs).
    pub fn tq(&self) -> TeamQueries<'_> {
        TeamQueries { w: self }
    }

    /// `TheTeamFactory->findTeam(name)`: the team of that name (a singleton prototype's team is made if it has none).
    pub fn find_team_by_name(&mut self, name: &[u8]) -> Option<TeamId> {
        if !self.cuts.live_teams {
            return self.cuts.find_team_by_name(name);
        }
        factory_find_team(self, name).ok().flatten()
    }

    /// `Player::getProductionVeterancyLevel(templateName)`.
    pub fn player_production_veterancy_level(&mut self, p: PlayerIndex, template_name: &[u8]) -> VeterancyLevel {
        if !self.cuts.live_teams {
            return self.cuts.player_production_veterancy_level(p, template_name);
        }
        player_get_production_veterancy_level(self, p, template_name).unwrap_or(VeterancyLevel::Regular)
    }

    /// `team->isInList_TeamMemberList(obj)`.
    pub fn team_member_list_contains(&self, team: TeamId, obj: ObjectId) -> bool {
        if !self.cuts.live_teams {
            return self.cuts.team_member_list_contains(team, obj);
        }
        self.teams.is_in_member_list(team, obj)
    }

    /// `team->removeFrom_TeamMemberList(obj)`.
    pub fn team_member_list_remove(&mut self, team: TeamId, obj: ObjectId) {
        if !self.cuts.live_teams {
            return self.cuts.team_member_list_remove(team, obj);
        }
        self.teams.remove_member(team, obj);
    }

    /// `team->prependTo_TeamMemberList(obj)`.
    pub fn team_member_list_prepend(&mut self, team: TeamId, obj: ObjectId) {
        if !self.cuts.live_teams {
            return self.cuts.team_member_list_prepend(team, obj);
        }
        self.teams.prepend_member(team, obj);
    }

    /// The scored view of an object (`ScoreKeeper`'s reads of it).
    fn live_scored_object(&self, o: ObjectId) -> Option<ScoredObject> {
        let obj = self.find_object(o)?;
        Some(ScoredObject {
            template_name: obj.template().get_name().to_vec(),
            kind_of: *obj.template().kindof(),
            controller: self.get_controlling_player(o).unwrap_or(-1),
            under_construction: obj.test_status(crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION),
        })
    }

    /// `ScoreKeeper::add*(obj)` of player `p`: `f` with the player's score keeper, the object's view and whether scoring is on.
    fn live_score(&mut self, p: PlayerIndex, o: ObjectId, f: impl FnOnce(&mut crate::logic::world::score_keeper::ScoreKeeper, &ScoredObject, bool)) {
        let scoring = self.game_logic.scoring_enabled;
        let Some(so) = self.live_scored_object(o) else { return };
        if let Some(pl) = self.players.get_nth_mut(p) {
            f(&mut pl.score_keeper, &so, scoring);
        }
    }

    /// `player->getAcademyStats()->...` of player `p`.
    fn live_academy(&mut self, p: PlayerIndex, f: impl FnOnce(&mut crate::logic::world::academy_stats::AcademyStats, u32)) {
        let frame = self.frame();
        if let Some(pl) = self.players.get_nth_mut(p) {
            f(&mut pl.academy_stats, frame);
        }
    }

    /// `Player::addPowerBonus(obj)` / `removePowerBonus(obj)`: `m_energy.addPowerBonus(obj)`, the object's template's
    /// energy bonus, and the brown-out notice the energy gives its owner.
    fn live_power_bonus(&mut self, p: PlayerIndex, o: ObjectId, add: bool) -> PResult<()> {
        let Some(bonus) = self.find_object(o).map(|x| x.template().get_energy_bonus()) else { return Ok(()) };
        let frame = self.frame();
        let Some(pl) = self.players.get_nth_mut(p) else { return Ok(()) };
        let notice = if add { pl.energy.add_power_bonus(frame, bonus) } else { pl.energy.remove_power_bonus(frame, bonus) };
        player_deliver_brown_out_notice(self, p, notice)
    }

    /// Integration batch I36: `Energy::objectEnteringInfluence(obj)` / `objectLeavingInfluence(obj)` of player `p`'s energy: the object's
    /// template's `EnergyProduction` joins or leaves the sums, and the energy's brown-out notice goes to its owner.
    fn live_energy_influence(&mut self, p: PlayerIndex, o: ObjectId, entering: bool) -> PResult<()> {
        let Some(production) = self.find_object(o).map(|x| x.template().get_energy_production()) else { return Ok(()) };
        let frame = self.frame();
        let Some(pl) = self.players.get_nth_mut(p) else { return Ok(()) };
        let notice = if entering { pl.energy.object_entering_influence(frame, production) } else { pl.energy.object_leaving_influence(frame, production) };
        player_deliver_brown_out_notice(self, p, notice)
    }

    /// A `Player*`, `Team*` or `AcademyStats*` cut call run on the team and player lists (`cuts.live_teams`). Calls
    /// whose answer the caller reads from a table keep that table answer; the errors of the player code (upstream's
    /// undefined behaviour) are recorded in `cuts.team_live_errors`.
    pub(crate) fn live_team_call(&mut self, call: &CutCall) {
        let r: PResult<()> = match call.clone() {
            CutCall::PlayerBecomingTeamMember { player, object, incoming } => player_becoming_team_member(self, player, Some(object), incoming),
            CutCall::PlayerScoreAddObjectCaptured { player, object } => {
                self.live_score(player, object, |sk, so, s| sk.add_object_captured(so, s));
                Ok(())
            }
            CutCall::PlayerScoreAddObjectLost { player, object } => {
                self.live_score(player, object, |sk, so, s| sk.add_object_lost(so, s));
                Ok(())
            }
            CutCall::PlayerScoreAddObjectDestroyed { player, object } => {
                self.live_score(player, object, |sk, so, s| sk.add_object_destroyed(so, s));
                Ok(())
            }
            CutCall::PlayerAddSkillPointsForKill { player, killer, victim } => player_add_skill_points_for_kill(self, player, Some(killer), Some(victim)).map(|_| ()),
            CutCall::PlayerDoBountyForKill { player, killer, victim } => player_do_bounty_for_kill(self, player, Some(killer), Some(victim)),
            CutCall::PlayerApplyDifficultyBonusesForObject { player, object, apply } => player_friend_apply_difficulty_bonuses_for_object(self, player, object, apply),
            CutCall::PlayerApplyBattlePlanBonusesForObject { player, object } => player_apply_battle_plan_bonuses_for_object(self, player, object),
            CutCall::PlayerRemoveRadar { player, disable_proof } => player_remove_radar(self, player, disable_proof),
            CutCall::PlayerAddRadar { player, disable_proof } => player_add_radar(self, player, disable_proof),
            CutCall::PlayerGrantScience { player, science } => player_grant_science(self, player, ScienceType(science)).map(|_| ()),
            CutCall::PlayerAcademyRecordUpgrade { player, upgrade, granted } => {
                if let Some(c) = self.statics.upgrade_center.find_upgrade_by_key(NameKeyType(upgrade)).map(|u| u.academy_classification_type()) {
                    self.live_academy(player, |a, _| a.record_upgrade(c, granted));
                }
                Ok(())
            }
            CutCall::PlayerAddKindOfProductionCostChange { player, kind_of, percent } => player_add_kind_of_production_cost_change(self, player, kind_of, percent),
            CutCall::PlayerRemoveKindOfProductionCostChange { player, kind_of, percent } => player_remove_kind_of_production_cost_change(self, player, kind_of, percent),
            // (integration batch I36: the player's energy, `getControllingPlayer()->getEnergy()->...`; they were records only, so a
            // player's production and consumption stayed 0 and no power plant or consumer counted)
            CutCall::EnergyObjectEnteringInfluence { player, object } => self.live_energy_influence(player, object, true),
            CutCall::EnergyObjectLeavingInfluence { player, object } => self.live_energy_influence(player, object, false),
            CutCall::EnergyAdjustPower { player, amount, add } => {
                let frame = self.frame();
                match self.players.get_nth_mut(player) {
                    Some(pl) => {
                        let notice = pl.energy.adjust_power(frame, amount, add);
                        player_deliver_brown_out_notice(self, player, notice)
                    }
                    None => Ok(()),
                }
            }
            CutCall::PlayerAddPowerBonus { player, object } => self.live_power_bonus(player, object, true),
            CutCall::PlayerRemovePowerBonus { player, object } => self.live_power_bonus(player, object, false),
            CutCall::PlayerOnStructureConstructionComplete { player, builder, structure, is_rebuild } => {
                player_on_structure_construction_complete(self, player, Some(builder).filter(|b| *b != 0), structure, is_rebuild)
            }
            CutCall::PlayerHealAllObjects { player } => player_heal_all_objects(self, player),
            CutCall::PlayerMoneyDeposit { player, amount } => player_money_deposit(self, player, amount, true),
            CutCall::PlayerMoneyWithdraw { player, amount } => player_money_withdraw(self, player, amount, true).map(|_| ()),
            CutCall::PlayerScoreAddMoneyEarned { player, amount } => {
                if let Some(pl) = self.players.get_nth_mut(player) {
                    pl.score_keeper.add_money_earned(amount);
                }
                Ok(())
            }
            CutCall::PlayerScoreAddMoneySpent { player, amount } => {
                if let Some(pl) = self.players.get_nth_mut(player) {
                    pl.score_keeper.add_money_spent(amount);
                }
                Ok(())
            }
            CutCall::PlayerSetPowerSabotagedTillFrame { player, frame } => {
                if let Some(pl) = self.players.get_nth_mut(player) {
                    pl.energy.set_power_sabotaged_till_frame(frame);
                }
                Ok(())
            }
            CutCall::PlayerOnPowerBrownOutChange { player, brown_out } => player_on_power_brown_out_change(self, player, brown_out),
            CutCall::PlayerAcademyRecordSalvageCollected { player } => {
                self.live_academy(player, |a, _| a.record_salvage_collected());
                Ok(())
            }
            CutCall::PlayerAcademyRecordSpecialPowerUsed { player, template } => {
                if let Some(c) = self.statics.special_power_store.find_special_power_template_by_id(template).map(|t| t.academy_classification_type()) {
                    self.live_academy(player, |a, _| a.record_special_power_used(c));
                }
                Ok(())
            }
            CutCall::PlayerAddSkillPoints { player, points } => player_add_skill_points(self, player, points).map(|_| ()),
            CutCall::PlayerAcademyRecordBuildingCapture { player } => {
                self.live_academy(player, |a, _| a.record_building_capture());
                Ok(())
            }
            CutCall::PlayerSetUnitsVisionSpied { player, setting, which_units, by_whom } => player_set_units_vision_spied(self, player, setting, &which_units, by_whom),
            CutCall::TeamNotifyTeamOfObjectDeath { team } => team_notify_team_of_object_death(self, team).map_err(crate::logic::world::player_ops::PlayerError::Team),
            CutCall::TeamSetEnteredExited { team } => {
                // `Team::setEnteredExited()`: `m_enteredOrExited = true`
                if let Some(t) = self.teams.team_mut(team) {
                    t.entered_or_exited = true;
                }
                Ok(())
            }
            CutCall::AcademyStatsRecordMine => {
                // `ThePlayerList->getNeutralPlayer()->getAcademyStats()->recordMine()`
                self.live_academy(0, |a, _| a.record_mine());
                Ok(())
            }
            CutCall::AcademyStatsRecordMineCleared { player } => {
                self.live_academy(player, |a, _| a.record_mine_cleared());
                Ok(())
            }
            CutCall::PlayerOnUnitCreated { player, factory, unit } => player_on_unit_created(self, player, Some(factory).filter(|f| *f != 0), Some(unit).filter(|u| *u != 0)),
            CutCall::PlayerAddUpgrade { player, upgrade, status } => player_add_upgrade(self, player, NameKeyType(upgrade), status).map(|_| ()),
            CutCall::PlayerRemoveUpgrade { player, upgrade } => player_remove_upgrade(self, player, NameKeyType(upgrade)),
            CutCall::PlayerAcademyRecordProduction { player, object, constructor: _ } => {
                let Some(kind_of) = self.find_object(object).map(|x| *x.template().kindof()) else { return };
                let tunnel_contain = self.get_contain(object).is_some_and(|c| self.module_call(object, c, ModuleCall::IsTunnelContain).as_bool());
                self.live_academy(player, |a, frame| a.record_production(frame, &AcademyObject { kind_of, tunnel_contain }));
                Ok(())
            }
            // (run by the battle plan module itself, on the bonuses it owns and `changeBattlePlan` rewrites)
            CutCall::PlayerChangeBattlePlan { .. } => Ok(()),
            CutCall::AcademyStatsRecordBattlePlanSelected { player } => {
                self.live_academy(player, |a, _| a.record_battle_plan_selected());
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = r {
            self.cuts.team_live_errors.push(format!("{call:?}: {e:?}"));
        }
    }

    /// `player->hasUpgradeComplete(upgrade)`, by the upgrade's mask.
    pub(crate) fn live_player_has_upgrade_complete(&self, p: PlayerIndex, mask: &UpgradeMaskType) -> bool {
        player_has_upgrade_complete_mask(self, p, mask).unwrap_or(false)
    }

    /// `player->hasUpgradeInProduction(upgrade)`, by the upgrade's mask (`m_upgradesInProgress.testForAll(mask)`).
    pub(crate) fn live_player_has_upgrade_in_production(&self, p: PlayerIndex, mask: &UpgradeMaskType) -> bool {
        self.players.get_nth(p).is_some_and(|x| x.upgrades_in_progress.test_for_all(mask))
    }

    /// `player->allowedToBuild(tmplate)`.
    pub(crate) fn live_player_allowed_to_build(&self, p: PlayerIndex, template: u16) -> bool {
        player_allowed_to_build(self, p, crate::logic::world::team_ops::TemplateRef::from(template)).unwrap_or(false)
    }

    /// `player->getAiBaseCenter(&pos)`: `None` for `FALSE`.
    pub(crate) fn live_player_get_ai_base_center(&mut self, p: PlayerIndex) -> Option<Coord3D> {
        let mut pos = Coord3D::default();
        player_get_ai_base_center(self, p, &mut pos).ok().filter(|b| *b).map(|_| pos)
    }
}

impl TeamQueries<'_> {
    fn live(&self) -> bool {
        self.w.cuts.live_teams
    }

    /// `Team::getControllingPlayer()`: `None` for a team no player controls.
    pub fn team_player(&self, team: TeamId) -> Option<PlayerIndex> {
        if !self.live() {
            return self.w.cuts.team_player(team);
        }
        team_get_controlling_player(self.w, team).ok().flatten()
    }

    /// `Team::getName()`: the prototype's name; empty if unknown.
    pub fn team_name(&self, team: TeamId) -> Vec<u8> {
        if !self.live() {
            return self.w.cuts.team_name(team);
        }
        let teams = &self.w.teams;
        teams.team(team).and_then(|t| teams.prototype(t.proto)).map(|p| p.name.clone()).unwrap_or_default()
    }

    /// `Player::isPlayerActive()`.
    pub fn player_is_active(&self, p: PlayerIndex) -> bool {
        if !self.live() {
            return self.w.cuts.player_is_active(p);
        }
        player_is_player_active(self.w, p).unwrap_or(false)
    }

    /// `Player::isPlayableSide()`.
    pub fn player_is_playable_side(&self, p: PlayerIndex) -> bool {
        if !self.live() {
            return self.w.cuts.player_is_playable_side(p);
        }
        player_is_playable_side(self.w, p).unwrap_or(false)
    }

    /// `Player::isSkirmishAIPlayer()`.
    pub fn player_is_skirmish_ai(&self, p: PlayerIndex) -> bool {
        if !self.live() {
            return self.w.cuts.player_is_skirmish_ai(p);
        }
        player_is_skirmish_ai_player(self.w, p).unwrap_or(false)
    }

    /// `Player::getDefaultTeam()`: 0 if unset.
    pub fn player_default_team(&self, p: PlayerIndex) -> TeamId {
        if !self.live() {
            return self.w.cuts.player_default_team(p);
        }
        self.w.players.get_nth(p).and_then(|x| x.default_team).unwrap_or(0)
    }

    /// `Team::getRelationship(that)`: `None` is `NULL`.
    pub fn team_relationship(&self, team: TeamId, that: Option<TeamId>) -> Relationship {
        if !self.live() {
            return self.w.cuts.team_relationship(team, that);
        }
        team_get_relationship(self.w, team, that).unwrap_or(Relationship::Neutral)
    }

    /// `Player::getRelationship(thatTeam)`: `None` is `NULL`.
    pub fn player_relationship_to_team(&self, p: PlayerIndex, that: Option<TeamId>) -> Relationship {
        if !self.live() {
            return self.w.cuts.player_relationship_to_team(p, that);
        }
        player_get_relationship(self.w, p, that).unwrap_or(Relationship::Neutral)
    }

    /// `p->getRelationship(q->getDefaultTeam())`.
    pub fn player_relationship_to_player_default_team(&self, p: PlayerIndex, q: PlayerIndex) -> Relationship {
        if !self.live() {
            return self.w.cuts.player_relationship_to_player_default_team(p, q);
        }
        let team = self.player_default_team(q);
        self.player_relationship_to_team(p, if team != 0 { Some(team) } else { None })
    }

    /// `Player::getPlayerMask()`: `1 << m_playerIndex`.
    pub fn player_mask(&self, p: PlayerIndex) -> PlayerMaskType {
        if !self.live() {
            return self.w.cuts.player_mask(p);
        }
        1u16.checked_shl(p as u32).unwrap_or(0)
    }

    /// `Player::getPlayerColor()`.
    pub fn player_color(&self, p: PlayerIndex) -> i32 {
        if !self.live() {
            return self.w.cuts.player_color(p);
        }
        self.w.players.get_nth(p).map_or(0, |x| x.color as i32)
    }

    /// `Player::getPlayerNightColor()`.
    pub fn player_night_color(&self, p: PlayerIndex) -> i32 {
        if !self.live() {
            return self.w.cuts.player_night_color(p);
        }
        self.w.players.get_nth(p).map_or(0, |x| x.night_color as i32)
    }

    /// `Player::getPlayerTemplate()->getSide()`; `None` for no template.
    pub fn player_side(&self, p: PlayerIndex) -> Option<Vec<u8>> {
        if !self.live() {
            return self.w.cuts.player_side(p);
        }
        let t = self.w.players.get_nth(p)?.template?;
        self.w.statics.player_template_store.nth_player_template(t as i32).map(|t| t.side().to_vec())
    }

    /// `Player::getCompletedUpgradeMask()`.
    pub fn player_completed_upgrade_mask(&self, p: PlayerIndex) -> UpgradeMaskType {
        if !self.live() {
            return self.w.cuts.player_completed_upgrade_mask(p);
        }
        self.w.players.get_nth(p).map(|x| x.upgrades_completed).unwrap_or_default()
    }

    /// `Player::getNumBattlePlansActive()`.
    pub fn player_num_battle_plans_active(&self, p: PlayerIndex) -> i32 {
        if !self.live() {
            return self.w.cuts.player_num_battle_plans_active(p);
        }
        player_get_num_battle_plans_active(self.w, p).unwrap_or(0)
    }

    /// `proto && proto->getTemplateInfo()`: the team has a prototype.
    pub fn team_has_template_info(&self, team: TeamId) -> bool {
        if !self.live() {
            return self.w.cuts.team_has_template_info(team);
        }
        self.w.teams.team(team).is_some_and(|t| self.w.teams.prototype(t.proto).is_some())
    }

    /// The team prototype's attack priority names an existing attack info with a name (the script engine's: a table
    /// until batch I8, live or not).
    pub fn team_attack_info(&self, team: TeamId) -> bool {
        self.w.cuts.team_attack_info(team)
    }
}

#[cfg(test)]
mod tests {
    //! `live_teams` on: the world's team and player answers are the lists', and the notifications run the player code;
    //! off (the default): the tables, as before.
    use super::*;
    use crate::common::base_type::Coord3D;
    use crate::common::dict::Dict;
    use crate::logic::world::player_host_world::tests::{make, world};
    use crate::logic::world::team_ops::factory_init_team;

    const AT: Coord3D = Coord3D { x: 100.0, y: 100.0, z: 0.0 };

    /// `setTeam` keeps the team arena's member lists, and tells the player (`becomingTeamMember`), whose objects are
    /// then the team's members.
    #[test]
    fn with_live_teams_set_team_keeps_the_member_lists() {
        let (mut w, team1) = world();
        factory_init_team(&mut w, b"team1b", b"Player1", true, &Dict::new()).unwrap();
        assert_eq!(w.find_team_by_name(b"team1b"), None); // the table's answer
        w.cuts.live_teams = true;
        let team1b = w.find_team_by_name(b"team1b").expect("the second team");
        let tank = make(&mut w, b"Tank", team1, AT);
        assert_eq!(w.teams.members(team1), vec![tank]);
        assert_eq!((w.get_team(tank), w.get_controlling_player(tank)), (Some(team1), Some(1)));
        w.set_team(tank, Some(team1b));
        assert_eq!((w.teams.members(team1), w.teams.members(team1b)), (vec![], vec![tank]));
        assert_eq!(w.player_iterate_objects(1), vec![tank]);
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }

    /// The questions are the lists' with `live_teams`, the tables' without.
    #[test]
    fn the_questions_follow_the_switch() {
        let (mut w, team1) = world();
        w.players.get_nth_mut(1).unwrap().color = 0x00ff_0000;
        w.cuts.player_colors.insert(1, 7);
        w.cuts.team_players.insert(team1, 0);
        assert_eq!((w.tq().player_color(1), w.tq().team_player(team1), w.tq().player_mask(1)), (7, Some(0), 2));
        w.cuts.live_teams = true;
        assert_eq!((w.tq().player_color(1), w.tq().team_player(team1), w.tq().player_mask(3)), (0x00ff_0000, Some(1), 8));
        assert_eq!(w.tq().team_name(team1), b"team1");
        assert_eq!(w.tq().player_default_team(1), 0);
        assert!(w.tq().team_has_template_info(team1) && !w.tq().team_has_template_info(999));
    }

    /// A deposit, a withdrawal and the score of earned money reach the player with `live_teams`; without it they are
    /// only logged.
    #[test]
    fn with_live_teams_the_notifications_reach_the_player() {
        let (mut w, _) = world();
        w.cut(CutCall::PlayerMoneyDeposit { player: 1, amount: 500 });
        assert_eq!(w.players.get_nth(1).unwrap().money.count_money(), 0);
        w.cuts.live_teams = true;
        w.cut(CutCall::PlayerMoneyDeposit { player: 1, amount: 500 });
        w.cut(CutCall::PlayerMoneyWithdraw { player: 1, amount: 200 });
        w.cut(CutCall::PlayerScoreAddMoneyEarned { player: 1, amount: 500 });
        let p = w.players.get_nth(1).unwrap();
        assert_eq!((p.money.count_money(), p.score_keeper.total_money_earned()), (300, 500));
        assert_eq!(w.cuts.log.as_ref().unwrap().iter().filter(|c| matches!(c, CutCall::PlayerMoneyDeposit { .. })).count(), 2);
    }

    /// The player code's errors (upstream's undefined behaviour: here a player index past the list) are recorded.
    #[test]
    fn a_failing_player_call_is_recorded() {
        let (mut w, _) = world();
        w.cuts.live_teams = true;
        w.cut(CutCall::PlayerHealAllObjects { player: 99 });
        assert_eq!(w.cuts.team_live_errors.len(), 1);
    }
}
