// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/AcademyStats.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/AcademyStats.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Westwood Studios Pacific, Steven Johnson, October 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `AcademyStats`: what a player has done, in the terms of the "academy"
//! advice screen, which tells a player what they have not tried yet. Each
//! player keeps counters of the things it has built, researched, used and
//! done; the advice is chosen from those counters at the end of a match
//! (or when asked).
//!
//! # How the advice is chosen (and why the random draws are part of it)
//!
//! Upstream evaluates the tips in three tiers, and each tier twice: a
//! **counting** pass finds how many tips apply, and then, when any apply, a
//! **choosing** pass draws one client random number per applicable tip and
//! keeps the tip when the draw lands in the open slots. The counting pass
//! also draws (with arguments that make no sense for counting: `0, -2` and
//! on), and both passes update the same counters (the idle and power-out
//! maxima, the income gap). So the draws, their arguments and their order are
//! part of the result, and this module makes them in upstream's order from the
//! same [`RandomValues`] (`GameClientRandomValue`).
//!
//! # Cut points
//!
//! - **The advice text.** `TheGameText->fetch(label)` is the client's. The
//!   advice is returned as the GameText labels, in order
//!   ([`AcademyAdvice::tips`]); the header that upstream puts in front of the
//!   text (`"\n\n"`) is the client's formatting.
//! - **Other players' statistics**, which the advice reads: the local player's
//!   mines and the neutral player's vehicles sniped
//!   ([`AdviceInputs`]), and `TheGlobalData->m_useAlternateMouse`.
//! - **The player's objects** (`iterateObjects`, the first update) are the
//!   [`AcademyObject`]s passed to `update`: the template's kind-of mask, and
//!   whether the object's container is a tunnel network.
//! - **The player's money and power** (`countMoney`, `hasSufficientPower`) are
//!   passed to `update` as values.
//! - **The dozer's command set** and the supply center's cost (`init`): see
//!   the next section.
//! - **`getPlayer()`**: the owning player is layer 7's business; this struct
//!   does not hold it.
//!
//! # A defect kept as it is: the dozer's command set is never found
//!
//! `AcademyStats::init` looks for the supply-center and command-center
//! templates through the command set of the player's first dozer. The
//! callback that does the search, `findDozerCommandSet`, assigns its result to
//! a local copy of its `userData` argument and returns nothing, so
//! `m_dozerCommandSet` stays `NULL`, and the loop that classifies the buttons
//! never runs. Upstream therefore always has no supply-center template (the
//! supply-center cost is the fixed 1000) and no command-center template. This
//! port keeps that: [`AcademyStats::init`] does not search, and the
//! differential test drives a player with a dozer and a command set that has
//! both templates, and checks that the cost is still 1000 and the command
//! center is still unknown. Fixing it changes which advice is given, so it is
//! for the maintainer to decide; it is not a fix to make quietly.
//!
//! # Other upstream defects kept as they are
//!
//! - `recordIncome` and tier 3 item 30 compute `m_lastIncomeFrame - now` (the
//!   operands are the wrong way round), so the "gap between incomes" is the
//!   wrapped difference, which is huge unless the income is in the future.
//!   Ported with the same operands.
//! - Tier 3 item 31 (mines cleared by dozers) shows the advice label
//!   `ACADEMY:NoIncome`, the label of item 30 (a copy-paste). Ported.
//!
//! # Integer semantics
//!
//! Frames and counters are `UnsignedInt`s, and upstream wraps them: the frame
//! differences are `wrapping_sub`, and the counters `wrapping_add`.
//!
//! # Deliberate differences
//!
//! - The constructor leaves every field unset until `init` (upstream);
//!   `Default` zeroes them, and `init` sets them all, so no value is read
//!   before it is set.
//! - `crc` is empty upstream and is not translated.

use crate::common::academy_classification::AcademyClassificationType;
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::kind_of::{
    KindOfMaskType, KINDOF_DISGUISER, KINDOF_DOZER, KINDOF_FS_ADVANCED_TECH, KINDOF_FS_BARRACKS, KINDOF_FS_BLACK_MARKET,
    KINDOF_FS_STRATEGY_CENTER, KINDOF_FS_SUPPLY_CENTER, KINDOF_FS_SUPPLY_DROPZONE, KINDOF_FS_WARFACTORY, KINDOF_HARVESTER,
    KINDOF_HERO, KINDOF_INFANTRY, KINDOF_MONEY_HACKER, KINDOF_VEHICLE,
};
use crate::common::random_value::RandomValues;
use crate::common::xfer::{Xfer, XferResult, XferVersion};

/// `MAX_ADVICE_TIPS`: one tip per advice screen.
pub const MAX_ADVICE_TIPS: i32 = 1;

/// `FRAMES_BETWEEN_UPDATES`.
pub const FRAMES_BETWEEN_UPDATES: u32 = 30;

/// `AcademyAdviceInfo` as the client gets it: the GameText labels of the
/// tips chosen, in order (at most [`MAX_ADVICE_TIPS`]). Empty is no advice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AcademyAdvice {
    /// `advice[0..numTips]`, as `TheGameText->fetch` labels.
    pub tips: Vec<&'static str>,
}

impl AcademyAdvice {
    /// `numTips`.
    pub fn num_tips(&self) -> usize {
        self.tips.len()
    }
}

/// What a player's objects contribute to the academy counters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcademyObject {
    /// The template's kind-of mask.
    pub kind_of: KindOfMaskType,
    /// `getContain() && getContain()->isTunnelContain()`.
    pub tunnel_contain: bool,
}

/// The values of other players and of the global data that the advice reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdviceInputs {
    /// `ThePlayerList->getLocalPlayer()->getAcademyStats()->getMines()`.
    pub local_mines: u32,
    /// `ThePlayerList->getNeutralPlayer()->getAcademyStats()->getVehiclesSniped()`.
    pub neutral_vehicles_sniped: u32,
    /// `TheGlobalData->m_useAlternateMouse`.
    pub use_alternate_mouse: bool,
}

/// `m_supplyCenterCost` when there is no supply-center template (always, see
/// the module documentation).
const DEFAULT_SUPPLY_CENTER_COST: u32 = 1000;

/// `class AcademyStats`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcademyStats {
    /// `m_nextUpdateFrame`.
    next_update_frame: u32,
    /// `m_firstUpdate`: the objects are recorded on the first update only.
    first_update: bool,
    /// `m_unknownSide`: not USA, China or GLA (or no template): no advice.
    unknown_side: bool,
    /// The player's base side (`getPlayerTemplate()->getBaseSide()`), copied at
    /// `init`; not saved (upstream reads it from the player, which is not part of
    /// the statistics).
    side: Option<Vec<u8>>,

    // Tier 1 (basic advice)
    /// `m_spentCashBeforeBuildingSupplyCenter`.
    spent_cash_before_building_supply_center: bool,
    /// `m_supplyCentersBuilt`.
    supply_centers_built: u32,
    /// `m_supplyCenterCost`.
    supply_center_cost: u32,
    /// `m_researchedRadar`.
    researched_radar: bool,
    /// `m_peonsBuilt`.
    peons_built: u32,
    /// `m_structuresCaptured`.
    structures_captured: u32,
    /// `m_generalsPointsSpent`.
    generals_points_spent: u32,
    /// `m_specialPowersUsed`.
    special_powers_used: u32,
    /// `m_structuresGarrisoned`.
    structures_garrisoned: u32,
    /// `m_idleBuildingUnitsMaxFrames`.
    idle_building_units_max_frames: u32,
    /// `m_lastUnitBuiltFrame`.
    last_unit_built_frame: u32,
    /// `m_dragSelectUnits`.
    drag_select_units: u32,
    /// `m_upgradesPurchased`.
    upgrades_purchased: u32,
    /// `m_powerOutMaxFrames`.
    power_out_max_frames: u32,
    /// `m_oldestPowerOutFrame`.
    oldest_power_out_frame: u32,
    /// `m_hadPowerLastCheck`.
    had_power_last_check: bool,
    /// `m_gatherersBuilt`.
    gatherers_built: u32,
    /// `m_heroesBuilt`.
    heroes_built: u32,

    // Tier 2 (intermediate advice)
    /// `m_hadAStrategyCenter`.
    had_a_strategy_center: bool,
    /// `m_choseAStrategyForCenter`.
    chose_a_strategy_for_center: bool,
    /// `m_unitsEnteredTunnelNetwork`.
    units_entered_tunnel_network: u32,
    /// `m_hadATunnelNetwork`.
    had_a_tunnel_network: bool,
    /// `m_controlGroupsUsed`.
    control_groups_used: u32,
    /// `m_secondaryIncomeUnitsBuilt`.
    secondary_income_units_built: u32,
    /// `m_clearedGarrisonedBuildings`.
    cleared_garrisoned_buildings: u32,
    /// `m_salvageCollected`.
    salvage_collected: u32,
    /// `m_guardAbilityUsedCount`.
    guard_ability_used_count: u32,

    // Tier 3 (advanced advice)
    /// `m_doubleClickAttackMoveOrdersGiven`.
    double_click_attack_move_orders_given: u32,
    /// `m_builtBarracksWithinFiveMinutes`.
    built_barracks_within_five_minutes: bool,
    /// `m_builtWarFactoryWithinTenMinutes`.
    built_war_factory_within_ten_minutes: bool,
    /// `m_builtTechStructureWithinFifteenMinutes`.
    built_tech_structure_within_fifteen_minutes: bool,
    /// `m_lastIncomeFrame`.
    last_income_frame: u32,
    /// `m_maxFramesBetweenIncome`.
    max_frames_between_income: u32,
    /// `m_mines`: a neutral-player statistic (mines recorded).
    mines: u32,
    /// `m_minesCleared`.
    mines_cleared: u32,
    /// `m_vehiclesRecovered`.
    vehicles_recovered: u32,
    /// `m_vehiclesSniped`.
    vehicles_sniped: u32,
    /// `m_disguisableVehiclesBuilt`.
    disguisable_vehicles_built: u32,
    /// `m_vehiclesDisguised`.
    vehicles_disguised: u32,
    /// `m_firestormsCreated`.
    firestorms_created: u32,
}

impl Default for AcademyStats {
    /// `AcademyStats()` (upstream sets nothing until `init`; here all zero).
    fn default() -> Self {
        AcademyStats {
            next_update_frame: 0,
            first_update: false,
            unknown_side: false,
            side: None,
            spent_cash_before_building_supply_center: false,
            supply_centers_built: 0,
            supply_center_cost: DEFAULT_SUPPLY_CENTER_COST,
            researched_radar: false,
            peons_built: 0,
            structures_captured: 0,
            generals_points_spent: 0,
            special_powers_used: 0,
            structures_garrisoned: 0,
            idle_building_units_max_frames: 0,
            last_unit_built_frame: 0,
            drag_select_units: 0,
            upgrades_purchased: 0,
            power_out_max_frames: 0,
            oldest_power_out_frame: 0,
            had_power_last_check: false,
            gatherers_built: 0,
            heroes_built: 0,
            had_a_strategy_center: false,
            chose_a_strategy_for_center: false,
            units_entered_tunnel_network: 0,
            had_a_tunnel_network: false,
            control_groups_used: 0,
            secondary_income_units_built: 0,
            cleared_garrisoned_buildings: 0,
            salvage_collected: 0,
            guard_ability_used_count: 0,
            double_click_attack_move_orders_given: 0,
            built_barracks_within_five_minutes: false,
            built_war_factory_within_ten_minutes: false,
            built_tech_structure_within_fifteen_minutes: false,
            last_income_frame: 0,
            max_frames_between_income: 0,
            mines: 0,
            mines_cleared: 0,
            vehicles_recovered: 0,
            vehicles_sniped: 0,
            disguisable_vehicles_built: 0,
            vehicles_disguised: 0,
            firestorms_created: 0,
        }
    }
}

/// `strcmp`-style `compareNoCase(s) == 0`.
fn side_is(side: &Option<Vec<u8>>, name: &str) -> bool {
    side.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(name.as_bytes()))
}

/// One pass of the advice: whether it is choosing, how many tips are still
/// open to the draw (`numAvailableTips`), and how many apply (`availableTips`).
struct Pass {
    choosing: bool,
    num_available: i32,
    available: i32,
}

impl Pass {
    /// `evaluateTierN(info, numAvailableTips)`: `None` is the counting pass
    /// (`-1`), `Some(n)` the choosing pass over `n` applicable tips.
    fn new(num_available: Option<i32>) -> Pass {
        Pass { choosing: num_available.is_some(), num_available: num_available.unwrap_or(-1), available: 0 }
    }

    /// One tip's block of upstream: it applies, so it is counted, a client
    /// random number is drawn for it (`0 .. numAvailable - 1`), and it is taken
    /// when the draw is below the open slots.
    fn offer(&mut self, info: &mut AcademyAdvice, rng: &mut RandomValues, label: &'static str) {
        self.available += 1;
        let rand = rng.game_client_random_value(0, self.num_available - 1);
        let limit = MAX_ADVICE_TIPS - info.num_tips() as i32;
        if self.choosing && rand < limit {
            info.tips.push(label);
        }
        self.num_available -= 1;
    }
}

impl AcademyStats {
    /// `AcademyStats::init(player)`: the frame of the first update, the side
    /// (`None`: no player template) and every counter cleared. The dozer's
    /// command set is not searched (see the module documentation).
    pub fn init(&mut self, frame: u32, side: Option<&[u8]>) {
        self.next_update_frame = frame.wrapping_add(FRAMES_BETWEEN_UPDATES);
        self.first_update = true;
        self.side = side.map(|s| s.to_vec());
        self.unknown_side = !(side_is(&self.side, "USA") || side_is(&self.side, "China") || side_is(&self.side, "GLA"));
        // The supply-center template is never found upstream: the cost is the default.
        self.supply_center_cost = DEFAULT_SUPPLY_CENTER_COST;
        self.spent_cash_before_building_supply_center = false;
        self.supply_centers_built = 0;
        self.researched_radar = false;
        self.peons_built = 0;
        self.structures_captured = 0;
        self.generals_points_spent = 0;
        self.special_powers_used = 0;
        self.structures_garrisoned = 0;
        self.idle_building_units_max_frames = 0;
        self.last_unit_built_frame = 0;
        self.drag_select_units = 0;
        self.upgrades_purchased = 0;
        self.power_out_max_frames = 0;
        self.oldest_power_out_frame = 0;
        self.had_power_last_check = false;
        self.gatherers_built = 0;
        self.heroes_built = 0;
        self.had_a_strategy_center = false;
        self.chose_a_strategy_for_center = false;
        self.had_a_tunnel_network = false;
        self.units_entered_tunnel_network = 0;
        self.control_groups_used = 0;
        self.secondary_income_units_built = 0;
        self.cleared_garrisoned_buildings = 0;
        self.salvage_collected = 0;
        self.guard_ability_used_count = 0;
        self.double_click_attack_move_orders_given = 0;
        self.built_barracks_within_five_minutes = false;
        self.built_war_factory_within_ten_minutes = false;
        self.built_tech_structure_within_fifteen_minutes = false;
        self.last_income_frame = 0;
        self.max_frames_between_income = 0;
        self.mines = 0;
        self.mines_cleared = 0;
        self.vehicles_recovered = 0;
        self.vehicles_sniped = 0;
        self.disguisable_vehicles_built = 0;
        self.vehicles_disguised = 0;
        self.firestorms_created = 0;
    }

    /// `isFirstUpdate()`.
    pub fn is_first_update(&self) -> bool {
        self.first_update
    }

    /// `setFirstUpdate(set)`.
    pub fn set_first_update(&mut self, set: bool) {
        self.first_update = set;
    }

    /// `hadASupplyCenter()`.
    pub fn had_a_supply_center(&self) -> bool {
        self.supply_centers_built > 0
    }

    /// `getCommandCenterTemplate()`: always `None` (upstream's search for it
    /// never runs, see the module documentation).
    pub fn command_center_template(&self) -> Option<&[u8]> {
        None
    }

    /// `getMines()`.
    pub fn mines(&self) -> u32 {
        self.mines
    }

    /// `getVehiclesSniped()`.
    pub fn vehicles_sniped(&self) -> u32 {
        self.vehicles_sniped
    }

    /// `update()`: every [`FRAMES_BETWEEN_UPDATES`] frames, on the first
    /// update the objects are recorded; then the money check (no supply center
    /// yet and less money than a supply center costs) and the power check
    /// (each change of sufficiency, and how long a brown-out lasted).
    /// `money` and `has_power` are `None` when the player has no money or no
    /// energy object (upstream skips the check).
    pub fn update(&mut self, frame: u32, money: Option<u32>, has_power: Option<bool>, objects: &[AcademyObject]) {
        if self.unknown_side {
            return;
        }
        let now = frame;
        if self.next_update_frame >= now {
            self.next_update_frame = now.wrapping_add(FRAMES_BETWEEN_UPDATES);
            if self.first_update {
                for o in objects {
                    self.record_production(frame, o);
                }
            }
            // 2) Did player run out of money before building a supply center?
            if self.supply_centers_built == 0 && !self.spent_cash_before_building_supply_center {
                if let Some(amount) = money {
                    if amount < self.supply_center_cost {
                        self.spent_cash_before_building_supply_center = true;
                    }
                }
            }
            // 12) Was player out of power for more than 10 minutes?
            if let Some(has) = has_power {
                if has != self.had_power_last_check {
                    if !has {
                        // We just lost power.
                        self.oldest_power_out_frame = now;
                    } else {
                        // We just gained power... see how long we were without.
                        let frames = now.wrapping_sub(self.oldest_power_out_frame);
                        if frames > self.power_out_max_frames {
                            self.power_out_max_frames = frames;
                        }
                    }
                    self.had_power_last_check = has;
                }
            }
            if self.is_first_update() {
                self.set_first_update(false);
            }
        }
    }

    /// `recordProduction(obj, constructer)`: an object was built. The
    /// constructor is not read upstream either.
    pub fn record_production(&mut self, frame: u32, obj: &AcademyObject) {
        let k = &obj.kind_of;
        let now = frame;
        // 22) more than one supply center; 2) money before one.
        if k.test(KINDOF_FS_SUPPLY_CENTER) {
            self.supply_centers_built = self.supply_centers_built.wrapping_add(1);
        }
        // 4) Did player build any dozers/workers?
        if k.test(KINDOF_DOZER) {
            self.peons_built = self.peons_built.wrapping_add(1);
        }
        // 9) How idle was the player in building military units?
        if (k.test(KINDOF_INFANTRY) || k.test(KINDOF_VEHICLE)) && !k.test(KINDOF_DOZER) && !k.test(KINDOF_HARVESTER) {
            // How long has it been since we built our last unit?
            let idle_frames = now.wrapping_sub(self.last_unit_built_frame);
            // If it was longer than our max time... then record it.
            if idle_frames > self.idle_building_units_max_frames {
                self.idle_building_units_max_frames = idle_frames;
            }
            // Record the frame we built our unit.
            self.last_unit_built_frame = now;
        }
        // 13) Extra gatherers built?
        if k.test(KINDOF_HARVESTER) {
            self.gatherers_built = self.gatherers_built.wrapping_add(1);
        }
        // 14) Heroes built?
        if k.test(KINDOF_HERO) {
            self.heroes_built = self.heroes_built.wrapping_add(1);
        }
        // 15) Had a strategy center?
        if k.test(KINDOF_FS_STRATEGY_CENTER) {
            self.had_a_strategy_center = true;
        }
        // 16) Placed units inside tunnel network?
        if obj.tunnel_contain {
            self.had_a_tunnel_network = true;
        }
        // 18) Built secondary income building?
        if k.test(KINDOF_MONEY_HACKER) || k.test(KINDOF_FS_BLACK_MARKET) || k.test(KINDOF_FS_SUPPLY_DROPZONE) {
            self.secondary_income_units_built = self.secondary_income_units_built.wrapping_add(1);
        }
        // 27) Built barracks within 5 minutes?
        if k.test(KINDOF_FS_BARRACKS) && now <= 300 * LOGICFRAMES_PER_SECOND as u32 {
            self.built_barracks_within_five_minutes = true;
        }
        // 28) Built war factory within 10 minutes?
        if k.test(KINDOF_FS_WARFACTORY) && now <= 600 * LOGICFRAMES_PER_SECOND as u32 {
            self.built_war_factory_within_ten_minutes = true;
        }
        // 29) Built tech structure within 15 minutes?
        if k.test(KINDOF_FS_ADVANCED_TECH) && now <= 900 * LOGICFRAMES_PER_SECOND as u32 {
            self.built_tech_structure_within_fifteen_minutes = true;
        }
        // 33) Built a "disguisable" unit?
        if k.test(KINDOF_DISGUISER) {
            self.disguisable_vehicles_built = self.disguisable_vehicles_built.wrapping_add(1);
        }
    }

    /// `recordUpgrade(upgrade, granted)`: a radar upgrade was researched; an
    /// upgrade bought (not granted for free) counts.
    pub fn record_upgrade(&mut self, classification: AcademyClassificationType, granted: bool) {
        if classification == AcademyClassificationType::UpgradeRadar {
            self.researched_radar = true;
        }
        if !granted {
            self.upgrades_purchased = self.upgrades_purchased.wrapping_add(1);
        }
    }

    /// `recordSpecialPowerUsed(spTemplate)`: counts a superweapon's use.
    pub fn record_special_power_used(&mut self, classification: AcademyClassificationType) {
        if classification == AcademyClassificationType::Superpower {
            self.special_powers_used = self.special_powers_used.wrapping_add(1);
        }
    }

    /// `recordIncome()`. The gap is computed as upstream computes it, with the
    /// operands the wrong way round (see the module documentation).
    pub fn record_income(&mut self, frame: u32) {
        let now = frame;
        let delta = self.last_income_frame.wrapping_sub(now);
        if delta > self.max_frames_between_income {
            self.max_frames_between_income = delta;
        }
        self.last_income_frame = now;
    }

    /// `recordBuildingCapture()`.
    pub fn record_building_capture(&mut self) {
        self.structures_captured = self.structures_captured.wrapping_add(1);
    }

    /// `recordGeneralsPointsSpent(points)`: `UnsignedInt += Int`.
    pub fn record_generals_points_spent(&mut self, points: i32) {
        self.generals_points_spent = self.generals_points_spent.wrapping_add(points as u32);
    }

    /// `recordBuildingGarrisoned()`.
    pub fn record_building_garrisoned(&mut self) {
        self.structures_garrisoned = self.structures_garrisoned.wrapping_add(1);
    }

    /// `recordDragSelection()`.
    pub fn record_drag_selection(&mut self) {
        self.drag_select_units = self.drag_select_units.wrapping_add(1);
    }

    /// `recordStrategyCenter()`.
    pub fn record_strategy_center(&mut self) {
        self.had_a_strategy_center = true;
    }

    /// `recordBattlePlanSelected()`.
    pub fn record_battle_plan_selected(&mut self) {
        self.chose_a_strategy_for_center = true;
    }

    /// `recordUnitEnteredTunnelNetwork()`.
    pub fn record_unit_entered_tunnel_network(&mut self) {
        self.units_entered_tunnel_network = self.units_entered_tunnel_network.wrapping_add(1);
    }

    /// `recordControlGroupsUsed()`.
    pub fn record_control_groups_used(&mut self) {
        self.control_groups_used = self.control_groups_used.wrapping_add(1);
    }

    /// `recordClearedGarrisonedBuilding()`.
    pub fn record_cleared_garrisoned_building(&mut self) {
        self.cleared_garrisoned_buildings = self.cleared_garrisoned_buildings.wrapping_add(1);
    }

    /// `recordVehicleDisguised()`.
    pub fn record_vehicle_disguised(&mut self) {
        self.vehicles_disguised = self.vehicles_disguised.wrapping_add(1);
    }

    /// `recordFirestormCreated()`.
    pub fn record_firestorm_created(&mut self) {
        self.firestorms_created = self.firestorms_created.wrapping_add(1);
    }

    /// `recordGuardAbilityUsed()`.
    pub fn record_guard_ability_used(&mut self) {
        self.guard_ability_used_count = self.guard_ability_used_count.wrapping_add(1);
    }

    /// `recordSalvageCollected()`.
    pub fn record_salvage_collected(&mut self) {
        self.salvage_collected = self.salvage_collected.wrapping_add(1);
    }

    /// `recordDoubleClickAttackMoveOrderGiven()`.
    pub fn record_double_click_attack_move_order_given(&mut self) {
        self.double_click_attack_move_orders_given = self.double_click_attack_move_orders_given.wrapping_add(1);
    }

    /// `recordMineCleared()`.
    pub fn record_mine_cleared(&mut self) {
        self.mines_cleared = self.mines_cleared.wrapping_add(1);
    }

    /// `recordVehicleSniped()`: a neutral-player statistic.
    pub fn record_vehicle_sniped(&mut self) {
        self.vehicles_sniped = self.vehicles_sniped.wrapping_add(1);
    }

    /// `recordMine()`: a neutral-player statistic.
    pub fn record_mine(&mut self) {
        self.mines = self.mines.wrapping_add(1);
    }

    /// `calculateAcademyAdvice(info)`: up to [`MAX_ADVICE_TIPS`] tips, from
    /// tier 1, then (when there is room) tier 2, then tier 3. Nothing is given
    /// for an unknown side. `rng` is the client random stream (`GameClientRandomValue`).
    pub fn calculate_academy_advice(&mut self, frame: u32, inputs: &AdviceInputs, rng: &mut RandomValues) -> AcademyAdvice {
        let mut info = AcademyAdvice::default();
        if self.unknown_side {
            return info;
        }
        self.evaluate_tier1(frame, &mut info, rng, None);
        if info.num_tips() < MAX_ADVICE_TIPS as usize {
            self.evaluate_tier2(&mut info, rng, None);
            if info.num_tips() < MAX_ADVICE_TIPS as usize {
                self.evaluate_tier3(frame, inputs, &mut info, rng, None);
            }
        }
        info
    }

    /// `evaluateTier1Advice(info, numAvailableTips)`. The counting pass runs
    /// first (`None`); if any tip applies, the choosing pass over those runs.
    fn evaluate_tier1(&mut self, frame: u32, info: &mut AcademyAdvice, rng: &mut RandomValues, num_available: Option<i32>) {
        let now = frame;
        let mut pass = Pass::new(num_available);
        // 2) Did player run out of money before building a supply center?
        if self.spent_cash_before_building_supply_center {
            pass.offer(info, rng, "ACADEMY:BuildSupplyCenterEarlier");
        }
        // 3) Did player build radar (if applicable)?
        if !self.researched_radar {
            pass.offer(info, rng, "ACADEMY:TryBuildingRadar");
        }
        // 4) Did player build any dozers/workers?
        if self.peons_built < 2 {
            pass.offer(info, rng, "ACADEMY:BuildMorePeons");
        }
        // 5) Did player ever capture a structure?
        if self.structures_captured == 0 {
            pass.offer(info, rng, "ACADEMY:TryCapturingStructures");
        }
        // 6) Did player spend any generals points?
        if self.generals_points_spent == 0 {
            pass.offer(info, rng, "ACADEMY:SpendGeneralsPoints");
        }
        // 7) Did player ever use a generals power or superweapon?
        if self.special_powers_used == 0 {
            pass.offer(info, rng, "ACADEMY:TryUsingSuperweapons");
        }
        // 8) Did player garrison any structures?
        if self.structures_garrisoned == 0 {
            pass.offer(info, rng, "ACADEMY:TryGarrisoningAStructure");
        }
        // 9) How idle was the player in building military units?
        let idle_frames = now.wrapping_sub(self.last_unit_built_frame);
        if idle_frames > self.idle_building_units_max_frames {
            self.idle_building_units_max_frames = idle_frames;
        }
        if self.idle_building_units_max_frames > 300 * LOGICFRAMES_PER_SECOND as u32 || self.last_unit_built_frame == 0 {
            pass.offer(info, rng, "ACADEMY:IdleBuildingUnits");
        }
        // 10) Did player drag select units?
        if self.drag_select_units == 0 {
            pass.offer(info, rng, "ACADEMY:TryDragSelectingUnits");
        }
        // 11) Did player upgrade anything?
        if self.upgrades_purchased == 0 {
            pass.offer(info, rng, "ACADEMY:ResearchUpgrades");
        }
        // 12) Was player out of power for more than 10 minutes?
        if !self.had_power_last_check {
            let frames = now.wrapping_sub(self.oldest_power_out_frame);
            if frames > self.power_out_max_frames {
                self.power_out_max_frames = frames;
            }
        }
        if self.power_out_max_frames > 600 * LOGICFRAMES_PER_SECOND as u32 {
            pass.offer(info, rng, "ACADEMY:RanOutOfPower");
        }
        // 13) Extra gatherers built? (free ones with supply centers do not count)
        if self.gatherers_built == 0 {
            pass.offer(info, rng, "ACADEMY:BuildMoreGatherers");
        }
        // 14) Heroes built?
        if self.heroes_built == 0 {
            pass.offer(info, rng, "ACADEMY:BuildAHero");
        }
        if !pass.choosing && pass.available > 0 {
            let available = pass.available;
            self.evaluate_tier1(frame, info, rng, Some(available));
        }
    }

    /// `evaluateTier2Advice(info, numAvailableTips)`.
    fn evaluate_tier2(&mut self, info: &mut AcademyAdvice, rng: &mut RandomValues, num_available: Option<i32>) {
        let mut pass = Pass::new(num_available);
        // 15) Selected a strategy center battle plan?
        if self.had_a_strategy_center && !self.chose_a_strategy_for_center {
            pass.offer(info, rng, "ACADEMY:PickStrategyCenterPlan");
        }
        // 16) Placed units inside tunnel network?
        if self.had_a_tunnel_network && self.units_entered_tunnel_network == 0 {
            pass.offer(info, rng, "ACADEMY:UseTunnelNetwork");
        }
        // 17) Player used control groups?
        if self.control_groups_used == 0 {
            pass.offer(info, rng, "ACADEMY:UseControlGroups");
        }
        // 18) Built secondary income building?
        if self.secondary_income_units_built == 0 {
            pass.offer(info, rng, "ACADEMY:UseSecondaryIncomeMethods");
        }
        // 19) Cleared out garrisoned buildings?
        if self.cleared_garrisoned_buildings == 0 {
            pass.offer(info, rng, "ACADEMY:ClearBuildings");
        }
        // 20) Did the Player pick up salvage (as GLA)?
        if side_is(&self.side, "GLA") && self.salvage_collected == 0 {
            pass.offer(info, rng, "ACADEMY:PickUpSalvage");
        }
        // 21) Did the player ever use the "Guard" ability?
        if self.guard_ability_used_count == 0 {
            pass.offer(info, rng, "ACADEMY:UseGuardAbility");
        }
        // 22) Did the player build more than one Supply Center?
        if self.supply_centers_built < 2 {
            pass.offer(info, rng, "ACADEMY:MultipleSupplyCenters");
        }
        if !pass.choosing && pass.available > 0 {
            let available = pass.available;
            self.evaluate_tier2(info, rng, Some(available));
        }
    }

    /// `evaluateTier3Advice(info, numAvailableTips)`.
    fn evaluate_tier3(&mut self, frame: u32, inputs: &AdviceInputs, info: &mut AcademyAdvice, rng: &mut RandomValues, num_available: Option<i32>) {
        let now = frame;
        let mut pass = Pass::new(num_available);
        // 25) Did the player use the new alternate interface in the options?
        if !inputs.use_alternate_mouse {
            pass.offer(info, rng, "ACADEMY:AlternateMouseInterface");
        }
        // 26) Did the player use the "double click location attack move/guard"?
        if self.double_click_attack_move_orders_given == 0 {
            pass.offer(info, rng, "ACADEMY:DoubleClickAttackMoveGuard");
        }
        // 27) Built barracks within 5 minutes?
        if !self.built_barracks_within_five_minutes {
            pass.offer(info, rng, "ACADEMY:BuildBarracksSooner");
        }
        // 28) Built war factory within 10 minutes?
        if !self.built_war_factory_within_ten_minutes {
            pass.offer(info, rng, "ACADEMY:BuildWarFactorySooner");
        }
        // 29) Built tech structure within 15 minutes?
        if !self.built_tech_structure_within_fifteen_minutes {
            pass.offer(info, rng, "ACADEMY:BuildTechStructureSooner");
        }
        // 30) No income for 2 minutes? (the operands as upstream has them)
        let delta = self.last_income_frame.wrapping_sub(now);
        if delta > self.max_frames_between_income {
            self.max_frames_between_income = delta;
        }
        if self.max_frames_between_income > LOGICFRAMES_PER_SECOND as u32 * 120 || self.last_income_frame == 0 {
            pass.offer(info, rng, "ACADEMY:NoIncome");
        }
        // 31) Did the Player ever use Dozers/Workers to clear out mines? (the
        // local player's mines; upstream shows the NoIncome label here)
        if inputs.local_mines > 0 && self.mines_cleared == 0 {
            pass.offer(info, rng, "ACADEMY:NoIncome");
        }
        // 32) Captured any sniped vehicles?
        if inputs.neutral_vehicles_sniped > 0 && self.vehicles_recovered == 0 {
            pass.offer(info, rng, "ACADEMY:UnmannedVehicles");
        }
        // 33) Did the player build a "disguisable" unit and never disguise one?
        if self.disguisable_vehicles_built > 0 && self.vehicles_disguised == 0 {
            pass.offer(info, rng, "ACADEMY:DisguisedUnits");
        }
        // 35) Did the player ever create a "Firestorm"? (China only)
        if side_is(&self.side, "China") && self.firestorms_created == 0 {
            pass.offer(info, rng, "ACADEMY:Firestorm");
        }
        if !pass.choosing && pass.available > 0 {
            let available = pass.available;
            self.evaluate_tier3(frame, inputs, info, rng, Some(available));
        }
    }

    /// `AcademyStats::xfer`, version 1: every counter and flag above, in
    /// upstream's order, and the unknown-side flag. The side and the tips are
    /// not saved.
    pub fn xfer<X: Xfer + ?Sized>(&mut self, xfer: &mut X) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_unsigned_int(&mut self.next_update_frame)?;
        xfer.xfer_bool(&mut self.first_update)?;
        xfer.xfer_bool(&mut self.unknown_side)?;
        // Tier 1
        xfer.xfer_bool(&mut self.spent_cash_before_building_supply_center)?;
        xfer.xfer_unsigned_int(&mut self.supply_centers_built)?;
        xfer.xfer_unsigned_int(&mut self.supply_center_cost)?;
        xfer.xfer_bool(&mut self.researched_radar)?;
        xfer.xfer_unsigned_int(&mut self.peons_built)?;
        xfer.xfer_unsigned_int(&mut self.structures_captured)?;
        xfer.xfer_unsigned_int(&mut self.generals_points_spent)?;
        xfer.xfer_unsigned_int(&mut self.special_powers_used)?;
        xfer.xfer_unsigned_int(&mut self.structures_garrisoned)?;
        xfer.xfer_unsigned_int(&mut self.idle_building_units_max_frames)?;
        xfer.xfer_unsigned_int(&mut self.last_unit_built_frame)?;
        xfer.xfer_unsigned_int(&mut self.drag_select_units)?;
        xfer.xfer_unsigned_int(&mut self.upgrades_purchased)?;
        xfer.xfer_unsigned_int(&mut self.power_out_max_frames)?;
        xfer.xfer_unsigned_int(&mut self.oldest_power_out_frame)?;
        xfer.xfer_bool(&mut self.had_power_last_check)?;
        xfer.xfer_unsigned_int(&mut self.gatherers_built)?;
        xfer.xfer_unsigned_int(&mut self.heroes_built)?;
        // Tier 2
        xfer.xfer_bool(&mut self.had_a_strategy_center)?;
        xfer.xfer_bool(&mut self.chose_a_strategy_for_center)?;
        xfer.xfer_unsigned_int(&mut self.units_entered_tunnel_network)?;
        xfer.xfer_bool(&mut self.had_a_tunnel_network)?;
        xfer.xfer_unsigned_int(&mut self.control_groups_used)?;
        xfer.xfer_unsigned_int(&mut self.secondary_income_units_built)?;
        xfer.xfer_unsigned_int(&mut self.cleared_garrisoned_buildings)?;
        xfer.xfer_unsigned_int(&mut self.salvage_collected)?;
        xfer.xfer_unsigned_int(&mut self.guard_ability_used_count)?;
        // Tier 3
        xfer.xfer_unsigned_int(&mut self.double_click_attack_move_orders_given)?;
        xfer.xfer_bool(&mut self.built_barracks_within_five_minutes)?;
        xfer.xfer_bool(&mut self.built_war_factory_within_ten_minutes)?;
        xfer.xfer_bool(&mut self.built_tech_structure_within_fifteen_minutes)?;
        xfer.xfer_unsigned_int(&mut self.last_income_frame)?;
        xfer.xfer_unsigned_int(&mut self.max_frames_between_income)?;
        xfer.xfer_unsigned_int(&mut self.mines)?;
        xfer.xfer_unsigned_int(&mut self.mines_cleared)?;
        xfer.xfer_unsigned_int(&mut self.vehicles_recovered)?;
        xfer.xfer_unsigned_int(&mut self.vehicles_sniped)?;
        xfer.xfer_unsigned_int(&mut self.disguisable_vehicles_built)?;
        xfer.xfer_unsigned_int(&mut self.vehicles_disguised)?;
        xfer.xfer_unsigned_int(&mut self.firestorms_created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(kinds: &[i32]) -> AcademyObject {
        AcademyObject { kind_of: KindOfMaskType::from_indices(kinds), tunnel_contain: false }
    }

    #[test]
    fn an_unknown_side_gets_no_advice_and_no_draws() {
        let mut a = AcademyStats::default();
        a.init(0, Some(b"Civilian"));
        let mut rng = RandomValues::new(1);
        let before = rng.clone();
        assert_eq!(a.calculate_academy_advice(0, &AdviceInputs::default(), &mut rng).num_tips(), 0);
        assert_eq!(rng, before, "no client random number is drawn");
    }

    #[test]
    fn a_new_usa_player_is_offered_one_tip_from_tier_one() {
        let mut a = AcademyStats::default();
        a.init(0, Some(b"usa"));
        let mut rng = RandomValues::new(7);
        let advice = a.calculate_academy_advice(10, &AdviceInputs::default(), &mut rng);
        assert_eq!(advice.num_tips(), 1);
        assert!(advice.tips[0].starts_with("ACADEMY:"));
    }

    #[test]
    fn a_player_who_has_done_everything_is_left_only_the_income_tip() {
        // Upstream's income gap is `last - now` (see the module documentation), so the
        // "no income for 2 minutes" tip is always offered: it is the one tip left.
        let mut a = AcademyStats::default();
        a.init(0, Some(b"GLA"));
        a.record_production(1, &obj(&[KINDOF_FS_SUPPLY_CENTER]));
        a.record_production(1, &obj(&[KINDOF_FS_SUPPLY_CENTER]));
        a.record_upgrade(AcademyClassificationType::UpgradeRadar, false);
        a.record_special_power_used(AcademyClassificationType::Superpower);
        a.record_building_garrisoned();
        a.record_drag_selection();
        a.record_building_capture();
        a.record_generals_points_spent(3);
        a.record_control_groups_used();
        a.record_cleared_garrisoned_building();
        a.record_salvage_collected();
        a.record_guard_ability_used();
        a.record_double_click_attack_move_order_given();
        a.record_strategy_center();
        a.record_battle_plan_selected();
        a.record_mine_cleared();
        a.record_vehicle_disguised();
        a.record_firestorm_created();
        a.record_production(1, &obj(&[KINDOF_DOZER]));
        a.record_production(1, &obj(&[KINDOF_DOZER]));
        a.record_production(1, &obj(&[KINDOF_HARVESTER, KINDOF_VEHICLE]));
        a.record_production(1, &obj(&[KINDOF_HERO, KINDOF_INFANTRY]));
        a.record_production(1, &obj(&[KINDOF_DISGUISER]));
        a.record_production(1, &obj(&[KINDOF_FS_BARRACKS, KINDOF_FS_WARFACTORY, KINDOF_FS_ADVANCED_TECH]));
        a.record_production(1, &obj(&[KINDOF_MONEY_HACKER]));
        let mut rng = RandomValues::new(3);
        let inputs = AdviceInputs { use_alternate_mouse: true, ..AdviceInputs::default() };
        assert_eq!(a.calculate_academy_advice(5, &inputs, &mut rng).tips, vec!["ACADEMY:NoIncome"]);
    }

    #[test]
    fn running_out_of_money_before_a_supply_center_is_noticed_once() {
        let mut a = AcademyStats::default();
        a.init(0, Some(b"China"));
        a.update(30, Some(10), None, &[]);
        assert!(a.spent_cash_before_building_supply_center);
        assert!(!a.is_first_update());
    }
}
