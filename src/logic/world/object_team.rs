// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//   (`setTeam`, `setTemporaryTeam`, `setOrRestoreTeam`, `restoreOriginalTeam`,
//   `getRelationship`, `getControllingPlayer`, the indicator colours,
//   `onCapture`, `defect`, `goInvulnerable`, `friend_adjustPowerForPlayer`,
//   `setReceivingDifficultyBonus`, the radar entry points, producer, builder
//   and group)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Team and player membership of an `Object`. Teams and players are ids;
//! everything `Team` and `Player` do for the object (member lists, scores,
//! energy, relationships) is a cut point listed in `object_cuts.rs`.

use crate::common::game_common::Relationship;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{KINDOF_CAPTURABLE, KINDOF_MINE};
use crate::common::object_status_types::{OBJECT_STATUS_IS_CARBOMB, OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::logic::object_script_status_bits::OBJECT_STATUS_SCRIPT_UNSELLABLE;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{PlayerIndex, TeamId};
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{ObjectClientEvent, ObjectSound, RadarCall};

/// `RADAR_PRIORITY_NOT_ON_RADAR` (`Radar.h`).
pub const RADAR_PRIORITY_NOT_ON_RADAR: u8 = 1;
/// `RADAR_PRIORITY_STRUCTURE`.
pub const RADAR_PRIORITY_STRUCTURE: u8 = 2;
/// `RADAR_PRIORITY_UNIT`.
pub const RADAR_PRIORITY_UNIT: u8 = 3;

impl World {
    // ---- team ---------------------------------------------------------------------------------

    /// `getTeam()`: the team id, `None` for no team.
    pub fn get_team(&self, id: ObjectId) -> Option<TeamId> {
        self.objects.find(id).map(|o| o.team).filter(|&t| t != 0)
    }

    /// `getControllingPlayer()`: the controlling player of the object's team.
    pub fn get_controlling_player(&self, id: ObjectId) -> Option<PlayerIndex> {
        let team = self.get_team(id)?;
        self.cuts.team_player(team)
    }

    /// `getRelationship(that)`: `NEUTRAL` without a team; an undetected
    /// defector (this one) is `NEUTRAL` to everyone ("so my AI does not give
    /// away my position by auto acquire") and (the other one) `ALLIES` to
    /// everyone ("so I treat undetecteddefectors like they were my very own").
    pub fn get_relationship(&self, id: ObjectId, that: ObjectId) -> Relationship {
        let (Some(me), Some(other)) = (self.objects.find(id), self.objects.find(that)) else {
            return Relationship::Neutral;
        };
        if me.team == 0 {
            return Relationship::Neutral;
        }
        if me.is_undetected_defector() {
            return Relationship::Neutral;
        }
        if other.is_undetected_defector() {
            return Relationship::Allies;
        }
        self.cuts.team_relationship(me.team, Some(other.team).filter(|&t| t != 0))
    }

    /// `isLocallyControlled()`: the controlling player is the local player
    /// (two `NULL`s are equal).
    pub fn is_locally_controlled(&self, id: ObjectId) -> bool {
        self.get_controlling_player(id) == self.players.local_index()
    }

    /// `isNeutralControlled()`.
    pub fn is_neutral_controlled(&self, id: ObjectId) -> bool {
        self.get_controlling_player(id) == Some(0)
    }

    /// `setTeam(team)`: sets the team and the original team. "In order to
    /// prevent spawning useful units for a player after he dies, we just
    /// assign objects to the neutral player if we try to misbehave."
    pub fn set_team(&mut self, id: ObjectId, team: Option<TeamId>) {
        let mut team = team;
        if let Some(t) = team {
            if let Some(p) = self.cuts.team_player(t) {
                if !self.cuts.player_is_active(p) {
                    team = Some(self.cuts.neutral_default_team);
                }
            }
        }
        self.set_temporary_team(id, team);
        let name = match self.get_team(id) {
            Some(t) => self.cuts.team_name(t),
            None => Vec::new(),
        };
        if let Some(o) = self.objects.find_mut(id) {
            o.original_team_name = name;
        }
    }

    /// `setTemporaryTeam(team)`: the team but not the original team.
    pub fn set_temporary_team(&mut self, id: ObjectId, team: Option<TeamId>) {
        self.set_or_restore_team(id, team, false);
    }

    /// `restoreOriginalTeam()`: back to the team of birth, if it still exists
    /// and is not the current one.
    pub fn restore_original_team(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find(id) else { return };
        if o.team == 0 || o.original_team_name.is_empty() {
            return;
        }
        let name = o.original_team_name.clone();
        let current = o.team;
        let Some(orig) = self.cuts.find_team_by_name(&name) else {
            return; // "Object original team could not be found or created!"
        };
        if current == orig {
            return; // "Object appears to still be on its original team, so why are we attempting to restore it?"
        }
        self.set_team(id, Some(orig));
    }

    /// `setOrRestoreTeam(team, restoring)`: leave the old team's member list,
    /// join the new team's, adjust the AI's attitude to the new team, tell the
    /// drawable, run `onCapture` (unless restoring), refresh the radar entry
    /// and tell the UI the object changed hands.
    pub fn set_or_restore_team(&mut self, id: ObjectId, team: Option<TeamId>, restoring: bool) {
        let new_team = team.unwrap_or(0);
        let Some(o) = self.objects.find(id) else { return };
        // don't do anything if the team hasn't changed
        if o.team == new_team {
            return;
        }
        let old_team = o.team;

        // Before Switch //////////////////////////
        if old_team != 0 && self.cuts.team_member_list_contains(old_team, id) {
            self.cuts.team_member_list_remove(old_team, id);
            if let Some(p) = self.cuts.team_player(old_team) {
                self.cut(CutCall::PlayerBecomingTeamMember { player: p, object: id, incoming: false });
            }
        }

        // Switch //////////////////////////
        if let Some(o) = self.objects.find_mut(id) {
            o.team = new_team;
        }

        // After Switch //////////////////////////
        if new_team != 0 {
            if !self.cuts.team_member_list_contains(new_team, id) {
                self.cuts.team_member_list_prepend(new_team, id);
                if let Some(p) = self.cuts.team_player(new_team) {
                    self.cut(CutCall::PlayerBecomingTeamMember { player: p, object: id, incoming: true });
                }
            }
            // now, adjust the attitude of the unit to its new team.
            if self.cuts.team_has_template_info(new_team) && self.objects.find(id).is_some_and(|o| o.ai.is_some()) {
                self.cut(CutCall::AiSetAttitude { object: id, team: new_team });
                if self.cuts.team_attack_info(new_team) {
                    self.cut(CutCall::AiSetAttackInfo { object: id, team: new_team });
                }
            }
            // emit message announcing object's new alliance
            if self.has_drawable(id) {
                self.emit(ObjectClientEvent::DrawableChangedTeam { object_id: id });
            }
        }

        // This can't just go in ::defect, because some things just do setTeam. The act of
        // setting a new team needs to tell the modules and do other important stuff.
        // And it needs to happen after the switch.
        if old_team != 0 && new_team != 0 && !restoring {
            if let (Some(old_p), Some(new_p)) = (self.cuts.team_player(old_team), self.cuts.team_player(new_team)) {
                self.on_capture(id, old_p, new_p);
            }
        }

        // the team changed we have a change in priorities on the radar if we are
        // a candidate for the radar as it is
        if self.objects.find(id).is_some_and(|o| o.on_radar) {
            // removing it and adding it will cause a resort to happen
            self.radar_remove_object(id);
            self.radar_add_object(id);
        }

        // Tell TheInGameUI that the object has changed hands
        let old_player_index = if old_team != 0 { self.cuts.team_player(old_team).unwrap_or(-1) } else { -1 };
        let new_player_index = if new_team != 0 { self.cuts.team_player(new_team).unwrap_or(-1) } else { -1 };
        if old_player_index != new_player_index {
            self.emit(ObjectClientEvent::ObjectChangedTeam { object_id: id, old_player: old_player_index, new_player: new_player_index });
        }
    }

    /// `hasDrawable`: `m_drawable != NULL`.
    pub fn has_drawable(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID)
    }

    // ---- indicator colours ----------------------------------------------------------------------

    /// `getIndicatorColor()`: the custom colour, else the controlling
    /// player's, else opaque black.
    pub fn get_indicator_color(&self, id: ObjectId) -> i32 {
        let Some(o) = self.objects.find(id) else { return 0 };
        if o.indicator_color == 0 {
            if let Some(p) = self.get_controlling_player(id) {
                return self.cuts.player_color(p);
            }
            return crate::common::ini::game_make_color(0, 0, 0, 255);
        }
        o.indicator_color
    }

    /// `getNightIndicatorColor()`: "used to make blue/purple easier to see on night models".
    pub fn get_night_indicator_color(&self, id: ObjectId) -> i32 {
        let Some(o) = self.objects.find(id) else { return 0 };
        if o.indicator_color == 0 {
            if let Some(p) = self.get_controlling_player(id) {
                return self.cuts.player_night_color(p);
            }
            return crate::common::ini::game_make_color(0, 0, 0, 255);
        }
        o.indicator_color
    }

    /// `setCustomIndicatorColor(c)`: the drawable is told on a change.
    pub fn set_custom_indicator_color(&mut self, id: ObjectId, c: i32) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if o.indicator_color != c {
            o.indicator_color = c;
            if self.has_drawable(id) {
                self.emit(ObjectClientEvent::DrawableChangedTeam { object_id: id });
            }
        }
    }

    /// `removeCustomIndicatorColor()`.
    pub fn remove_custom_indicator_color(&mut self, id: ObjectId) {
        self.set_custom_indicator_color(id, 0);
    }

    // ---- capture and defection ------------------------------------------------------------------------

    /// `onCapture(oldOwner, newOwner)`: "central point for onCapture logic".
    /// The AI goes idle ("everybody dhills when they captured so they don't
    /// keep doing something the new player might not want him to be doing"),
    /// the new owner scores the capture, the modules are told, the object
    /// relooks for its new masters and becomes sellable ("design needs the
    /// player to be able to sell buildings he steals from the AI's build
    /// list"), the control bar is dirtied, and a skirmish AI sells a captured
    /// faction structure at once.
    pub fn on_capture(&mut self, id: ObjectId, old_owner: PlayerIndex, new_owner: PlayerIndex) {
        if self.objects.find(id).is_some_and(|o| o.ai.is_some()) && old_owner != new_owner {
            self.cut(CutCall::AiIdle { object: id });
        }
        // this gets the new owner some points
        self.cut(CutCall::PlayerScoreAddObjectCaptured { player: new_owner, object: id });
        // rip through the behavior modules and call the onCapture for any modules that care
        self.call_all_modules(id, ModuleCall::OnCapture { old_owner, new_owner });
        // We have to undo our look for the old team and redo it for the new.
        // onCapture is used now, so it better be called after ownership changes and not before.
        self.handle_partition_cell_maintenance(id);
        // Design needs the player to be able to sell buildings he steals from the AI's build list...
        self.clear_script_status(id, OBJECT_STATUS_SCRIPT_UNSELLABLE);
        // mark the command bar to redraw
        self.emit(ObjectClientEvent::ControlBarMarkUiDirty);
        if old_owner != new_owner && self.cuts.player_is_skirmish_ai(new_owner) {
            // The skirmish ai doesn't know what to do with captured faction buildings except sell them.
            if self.is_faction_structure(id) {
                self.cut(CutCall::BuildAssistantSellObject { object: id });
            }
        }
    }

    /// `isFactionStructure()`: `isAnyKindOf(KINDOFMASK_FS)`.
    pub fn is_faction_structure(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_any_kind_of(&crate::common::kind_of::kindofmask_fs()))
    }

    /// `isNonFactionStructure()`.
    pub fn is_non_faction_structure(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_kind_of(crate::common::kind_of::KINDOF_STRUCTURE)) && !self.is_faction_structure(id)
    }

    /// `defect(newTeam, detectionTime)`: change sides in secret. Not while
    /// contained, without a player, onto the player's own default team, under
    /// construction or being sold. Production is cancelled and refunded; the
    /// radar warns of an infiltration between two playable sides; the object
    /// is an undetected defector for `detectionTime` frames; it changes team,
    /// relooks, goes idle, and says so; a container that kicks out on capture
    /// empties; parked units defect with it; and so do the mines it laid.
    pub fn defect(&mut self, id: ObjectId, new_team: TeamId, detection_time: u32) {
        // --- p1w11-sonnet --- (the call itself, for the tests that compare it with upstream's: a trace line only a test that asks for effect lines sees)
        if self.cuts.probe_effect_trace {
            if let Some(t) = &mut self.cuts.trace {
                t.push(format!("defect {id} {new_team} {detection_time}"));
            }
        }
        // --- end p1w11-sonnet ---
        if self.is_contained(id) {
            return; // @todo make contained units unselectable, until then... lorenzen
        }
        let Some(player) = self.get_controlling_player(id) else { return };
        let my_team = self.cuts.player_default_team(player);
        if my_team == new_team {
            return; // can't defect from my own team, that would be silly
        }
        // things that are under construction, or sold, cannot defect.
        {
            let o = self.objects.find(id).expect("get_controlling_player found it");
            if o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) || o.test_status(OBJECT_STATUS_SOLD) {
                return;
            }
        }

        // Before switch ////////////////////////////////////////
        // Design says:
        if let Some(production) = self.get_production_update_interface(id) {
            self.module_call(id, production, ModuleCall::CancelAndRefundAllProduction);
        }

        // pop it up on the radar, so as to warn those who care
        // do this first, since after setTeam() the infiltrator becomes the controllingplayer, not me
        // But don't do this is if the new team is not a real team.
        if self.objects.find(id).is_some_and(|o| o.on_radar) {
            let new_playable = self.cuts.team_player(new_team).is_some_and(|p| self.cuts.player_is_playable_side(p));
            let mine_playable = self.cuts.team_player(my_team).is_some_and(|p| self.cuts.player_is_playable_side(p));
            if new_playable && mine_playable {
                self.emit(ObjectClientEvent::Radar { call: RadarCall::TryInfiltrationEvent, object_id: id, position: None });
            }
        }

        self.friend_set_undetected_defector(id, detection_time > 0);
        if let Some(h) = self.objects.find(id).and_then(|o| o.helpers.defection) {
            self.module_call(id, h, ModuleCall::StartDefectionTimer { time: detection_time, with_defector_fx: true });
        }

        // Switch ////////////////////////////////////////
        self.set_team(id, Some(new_team));

        // After switch ////////////////////////////////////////
        let ai = self.objects.find(id).is_some_and(|o| o.ai.is_some());
        self.handle_partition_cell_maintenance(id); // to clear the shroud for my new master
        if ai {
            self.cut(CutCall::AiIdle { object: id });
        }

        // Play our sound indicating we've been defected. (weird verbage, but true.)
        self.emit(ObjectClientEvent::Sound { sound: ObjectSound::VoiceDefect, object_id: id, position: None });

        // make the new recruit the only selected thing, awaiting new command to move, attack, etc...
        if self.has_drawable(id) {
            // This is the first of several flashes which get cue'd by doDefectorUpdateStuff()
            self.emit(ObjectClientEvent::DrawableFlashAsSelected { object_id: id });
            self.emit(ObjectClientEvent::Sound { sound: ObjectSound::DefectorTimerTick, object_id: id, position: None });
        }

        if let Some(ct) = self.get_contain(id) {
            if self.module_call(id, ct, ModuleCall::IsKickOutOnCapture).as_bool() {
                // Caves really really don't want to do this.
                self.module_call(id, ct, ModuleCall::RemoveAllContained { expose_stealth_units: true });
            }
        }

        // if it has parking places, defect anything parked there.
        for idx in self.all_modules(id) {
            if self.module_call(id, idx, ModuleCall::HasParkingPlaceInterface).as_bool() {
                self.module_call(id, idx, ModuleCall::DefectAllParkedUnits { new_team, detection_time });
                break;
            }
        }

        // defect any mines that are owned by this structure, right now.
        // unfortunately, structures don't keep list of mines they own, so we must do
        // this the hard way :-( [fortunately, this doens't happen very often]
        for mine in self.objects.ids_in_order() {
            let Some(m) = self.objects.find(mine) else { continue };
            if m.is_kind_of(KINDOF_MINE) && m.producer_id == id {
                self.set_team(mine, Some(new_team));
            }
        }
    }

    /// `goInvulnerable(time)`: an undetected defector for `time` frames,
    /// without the defector effects.
    pub fn go_invulnerable(&mut self, id: ObjectId, time: u32) {
        self.friend_set_undetected_defector(id, time > 0);
        if let Some(h) = self.objects.find(id).and_then(|o| o.helpers.defection) {
            self.module_call(id, h, ModuleCall::StartDefectionTimer { time, with_defector_fx: false });
        }
    }

    /// `friend_adjustPowerForPlayer(incoming)`: the object's power joins or
    /// leaves its player's grid. A disabled producer does neither ("disabledness
    /// only affects Producers, not Consumers").
    pub fn friend_adjust_power_for_player(&mut self, id: ObjectId, incoming: bool) {
        let Some(o) = self.objects.find(id) else { return };
        if o.is_disabled() && o.template().get_energy_production() > 0 {
            return;
        }
        let Some(player) = self.get_controlling_player(id) else { return };
        if incoming {
            self.cut(CutCall::EnergyObjectEnteringInfluence { player, object: id });
        } else {
            self.cut(CutCall::EnergyObjectLeavingInfluence { player, object: id });
        }
    }

    /// `setReceivingDifficultyBonus(receive)`.
    pub fn set_receiving_difficulty_bonus(&mut self, id: ObjectId, receive: bool) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if receive == o.is_receiving_difficulty_bonus {
            return;
        }
        o.is_receiving_difficulty_bonus = receive;
        if let Some(player) = self.get_controlling_player(id) {
            self.cut(CutCall::PlayerApplyDifficultyBonusesForObject { player, object: id, apply: receive });
        }
    }

    // ---- producer, builder, group ---------------------------------------------------------------------------

    /// `setProducer(obj)`: the id of the object that produced this one
    /// (`INVALID_ID` for `NULL`).
    pub fn set_producer(&mut self, id: ObjectId, producer: ObjectId) {
        if let Some(o) = self.objects.find_mut(id) {
            o.producer_id = producer;
        }
    }

    /// `setBuilder(obj)`.
    pub fn set_builder(&mut self, id: ObjectId, builder: ObjectId) {
        if let Some(o) = self.objects.find_mut(id) {
            o.builder_id = builder;
        }
    }

    /// `enterGroup(group)`: leave the current group first.
    pub fn enter_group(&mut self, id: ObjectId, group: u32) {
        self.leave_group(id);
        if let Some(o) = self.objects.find_mut(id) {
            o.group = group;
        }
    }

    /// `leaveGroup()`: "to avoid recursion, set m_group to NULL before removing".
    pub fn leave_group(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if o.group != 0 {
            let group = o.group;
            o.group = 0;
            self.cut(CutCall::AiGroupRemove { group, object: id });
        }
    }

    // ---- radar -----------------------------------------------------------------------------------------------

    /// `getRadarPriority()`: the template's, else structure for a garrisonable
    /// or capturable object; a car bomb is always a unit.
    pub fn get_radar_priority(&mut self, id: ObjectId) -> u8 {
        let Some(o) = self.objects.find(id) else { return crate::common::thing_template::RADAR_PRIORITY_INVALID };
        // first, get the priority at the thing template level
        let mut priority = o.template().get_default_radar_priority();
        // there are some objects that we want to show up on the radar when they have certain properties...
        if priority == crate::common::thing_template::RADAR_PRIORITY_INVALID {
            // objects that are "garrisonable" show up on the radar
            if let Some(cmi) = self.get_contain(id) {
                if self.module_call(id, cmi, ModuleCall::IsGarrisonable).as_bool() {
                    priority = RADAR_PRIORITY_STRUCTURE;
                }
            }
            // objects that are "capturable" show up on the radar
            if self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_CAPTURABLE)) {
                priority = RADAR_PRIORITY_STRUCTURE;
            }
        }
        // Carbombs will show up as units regardless of their default priority
        if self.objects.find(id).is_some_and(|o| o.test_status(OBJECT_STATUS_IS_CARBOMB)) {
            priority = RADAR_PRIORITY_UNIT;
        }
        priority
    }

    /// `TheRadar->addObject(obj)`: an object whose priority is visible gets
    /// radar data (`m_radarData`) and the radar is told. (The radar's own
    /// priority-sorted lists are the client's.)
    pub fn radar_add_object(&mut self, id: ObjectId) {
        let priority = self.get_radar_priority(id);
        // isPriorityVisible: INVALID and NOT_ON_RADAR are not
        if priority == crate::common::thing_template::RADAR_PRIORITY_INVALID || priority == RADAR_PRIORITY_NOT_ON_RADAR {
            return;
        }
        if let Some(o) = self.objects.find_mut(id) {
            o.on_radar = true;
            self.emit(ObjectClientEvent::Radar { call: RadarCall::AddObject, object_id: id, position: None });
        }
    }

    /// `TheRadar->removeObject(obj)`: nothing for an object with no radar data.
    pub fn radar_remove_object(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if !o.on_radar {
            return;
        }
        o.on_radar = false;
        self.emit(ObjectClientEvent::Radar { call: RadarCall::RemoveObject, object_id: id, position: None });
    }
}
