// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarCommand.cpp (ControlBar::populateCommand,
//     populateBuildQueue, updateContextCommand, getCommandAvailability, getRappellerCount's use)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (setControlCommand: the images and the
//     tooltip text of a command button; #131: its hot key)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (CommandAvailability, CommandOption, MAX_BUILD_QUEUE_BUTTONS)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! What the control bar shows for the selected object (GitHub #12): its command set's buttons (which ones, their
//! images, their text, enabled or not, a reload or charge clock) and its build queue with the first entry's progress.
//!
//! # As upstream does it
//!
//! `populateCommand(obj)` looks up the object's command set; without one every command window is hidden. Each of the
//! 18 windows `ControlBar.wnd:ButtonCommand01..18` gets the set's button in that slot, or is hidden: a `SCRIPT_ONLY`
//! button is hidden, an exit-container button is left to the transport's inventory, and the others are shown, enabled
//! and given the button (`setControlCommand`: the `ButtonImage` as the enabled image; `TextLabel`, through the game
//! text, is the tooltip's). A `NEED_SPECIAL_POWER_SCIENCE` button whose power needs a science the player lacks is
//! hidden; one whose science vector the player has (part of) takes the images and text of the matching
//! purchase-science button of the player template's rank 1, 3 or 8 command sets. Then `updateContextCommand` asks
//! `getCommandAvailability` of every shown button: hidden, restricted (disabled), not ready (disabled, the clock of the
//! reload or charge), can't afford (disabled, drawn in colour), active (a check-like button checked) or available.
//!
//! The build queue (`populateBuildQueue`) shows up to 9 entries of the producer's queue, each with the produced
//! thing's or upgrade's button image; `updateContextCommand` draws the first entry's progress as an inverse clock.
//!
//! # Upstream quirks that are kept
//!
//! - `getCommandAvailability` of a disabled object asks itself again with `forceDisabledEvaluation` to tell hidden from
//!   restricted; only sell, evacuate, exit, beacon delete, rally point, stop and switch weapon stay usable.
//! - Several "can't afford" answers are restricted (the code returns `COMMAND_RESTRICTED` with the comment
//!   `COMMAND_CANT_AFFORD`), and an upgrade already owned or in production is `COMMAND_CANT_AFFORD` (the swap the
//!   comments show): kept as written.
//! - The science images replace the command button's own (`copyImagesFrom` on a `const` button upstream, which then
//!   keeps them for every object using it): here the slot shows them, the logic's button is not changed.
//!
//! # Where this differs from upstream
//!
//! The windows are not changed in place: [`populate_command`] returns what each slot shows ([`CommandSlot`]); the
//! drawing applies it. The veterancy overlay, ~~hot keys,~~ the rally point display, the transport inventory
//! (`doTransportInventoryUI`) and the multi-select context are not translated yet. **Corrected 2026-11-11 (#131):** the
//! hot keys are ([`populate_command_with_hot_keys`], [`crate::client::hot_key`]). Every question about the World is
//! asked of a [`ControlBarStateHost`]; the client's host answers from the World, read only.

use crate::client::control_bar::gui;
use crate::common::game_type::ObjectId;
use crate::logic::command_button::{CommandButton, MAX_COMMANDS_PER_SET};

/// `MAX_BUILD_QUEUE_BUTTONS`.
pub const MAX_BUILD_QUEUE_BUTTONS: usize = 9;

/// The `CommandOption` bits read here.
pub mod opt {
    pub const NEED_UPGRADE: u32 = 0x0000_0040;
    pub const NEED_SPECIAL_POWER_SCIENCE: u32 = 0x0000_0080;
    pub const CHECK_LIKE: u32 = 0x0000_0400;
    pub const OPTION_ONE: u32 = 0x0000_2000;
    pub const NOT_QUEUEABLE: u32 = 0x0001_0000;
    pub const SCRIPT_ONLY: u32 = 0x0008_0000;
    pub const IGNORES_UNDERPOWERED: u32 = 0x0010_0000;
    pub const USES_MINE_CLEARING_WEAPONSET: u32 = 0x0020_0000;
    pub const MUST_BE_STOPPED: u32 = 0x0080_0000;
}

/// `enum CommandAvailability`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandAvailability {
    Available,
    Restricted,
    Active,
    Hidden,
    NotReady,
    CantAfford,
}

/// `BuildableStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Buildable {
    Yes,
    IgnorePrerequisites,
    No,
    OnlyByAi,
}

/// A weapon's state, for the fire weapon buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponState {
    /// `getClipReloadTime(obj)`.
    pub clip_reload_time: u32,
    /// `getStatus() == READY_TO_FIRE`, `== RELOADING_CLIP`.
    pub ready_to_fire: bool,
    pub reloading_clip: bool,
    /// `getPossibleNextShotFrame()`.
    pub possible_next_shot_frame: u32,
    /// `getPercentReadyToFire()`.
    pub percent_ready: f32,
}

/// A special power module's state, for the special power buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpecialPowerState {
    /// No module for the button's power.
    NoModule,
    /// Not ready, `getPercentReady()`.
    Charging(f32),
    /// Ready; whether a special ability update has the power in use, and whether the battle plan's option matches.
    Ready { ability_in_use: Option<bool>, battle_plan_option_match: bool },
}

/// What the control bar asks of the game (upstream reaches the objects and players directly).
pub trait ControlBarStateHost {
    /// `obj->getCommandSetString()`'s set: its buttons' places in the World's list, by slot.
    fn command_set_buttons(&self, obj: ObjectId) -> Option<[Option<usize>; MAX_COMMANDS_PER_SET]>;
    /// The button at a place.
    fn button(&self, index: usize) -> Option<CommandButton>;
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `findMostReadyShortcutSpecialPowerOfType` for the button's power.
    fn shortcut_source(&mut self, button: &CommandButton) -> Option<ObjectId>;
    /// `OBJECT_STATUS_SCRIPT_DISABLED` or `_UNPOWERED`.
    fn script_disabled_or_unpowered(&self, obj: ObjectId) -> bool;
    /// `isDisabledByType(DISABLED_UNMANNED)`, `DISABLED_SUBDUED`.
    fn disabled_unmanned(&self, obj: ObjectId) -> bool;
    fn disabled_subdued(&self, obj: ObjectId) -> bool;
    /// `hasSingleUseCommandBeenUsed()`.
    fn single_use_used(&self, obj: ObjectId) -> bool;
    /// `getAI() && getAI()->isMoving()`.
    fn is_moving(&mut self, obj: ObjectId) -> bool;
    /// `isDisabled()`, and whether only `DISABLED_UNDERPOWERED` is set.
    fn disabled(&self, obj: ObjectId) -> (bool, bool);
    /// Whether the button's upgrade is a player upgrade (else an object upgrade); `None` without one.
    fn upgrade_is_player(&self, upgrade: u32) -> Option<bool>;
    /// `player->hasUpgradeComplete(u)`, `hasUpgradeInProduction(u)`.
    fn player_has_upgrade_complete(&self, obj: ObjectId, upgrade: u32) -> bool;
    fn player_has_upgrade_in_production(&self, obj: ObjectId, upgrade: u32) -> bool;
    /// `obj->hasUpgrade(u)`, `affectedByUpgrade(u)`.
    fn obj_has_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool;
    fn affected_by_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool;
    /// The production queue: `None` without a production interface.
    fn production_count(&mut self, obj: ObjectId) -> Option<i32>;
    /// `pu->isUpgradeInQueue(u)`.
    fn is_upgrade_in_queue(&mut self, obj: ObjectId, upgrade: u32) -> bool;
    /// `getBuildable()` of a template; whether the object's player is a computer player.
    fn buildable(&self, template: u16) -> Buildable;
    fn player_is_computer(&self, obj: ObjectId) -> bool;
    /// `isKindOf(KINDOF_DOZER)`; whether it has a dozer AI with a build task pending (`None`: no dozer AI).
    fn is_dozer(&self, obj: ObjectId) -> bool;
    fn dozer_build_pending(&mut self, obj: ObjectId) -> Option<bool>;
    /// `player->canBuild(t)`, `canAffordBuild(t)`.
    fn player_can_build(&mut self, obj: ObjectId, template: Option<u16>) -> bool;
    fn player_can_afford_build(&mut self, obj: ObjectId, template: Option<u16>) -> bool;
    /// `TheBuildAssistant->canMakeUnit(obj, t)`.
    fn can_make_unit(&mut self, obj: ObjectId, template: Option<u16>) -> crate::logic::world::build_assistant::CanMakeType;
    /// `TheUpgradeCenter->canAffordUpgrade(player, u)` (silently).
    fn can_afford_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool;
    /// `player->hasScience(st)`.
    fn player_has_science(&mut self, obj: ObjectId, st: i32) -> bool;
    /// `OBJECT_STATUS_SCRIPT_UNSELLABLE`.
    fn unsellable(&self, obj: ObjectId) -> bool;
    /// The weapon in a slot; `None` without one.
    fn weapon(&mut self, obj: ObjectId, slot: i32) -> Option<WeaponState>;
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `testWeaponSetFlag(WEAPONSET_MINE_CLEARING_DETAIL)`.
    fn mine_clearing_weapon_set(&self, obj: ObjectId) -> bool;
    /// `getRappellerCount(obj)`.
    fn rappeller_count(&mut self, obj: ObjectId) -> i32;
    /// `getContain()->getContainCount()` (`None`: no contain).
    fn contain_count(&mut self, obj: ObjectId) -> Option<u32>;
    /// `getAIUpdateInterface() != NULL` (#33: the fire weapon buttons are restricted without an AI).
    fn has_ai_update(&self, obj: ObjectId) -> bool;
    /// `getDockUpdateInterface()->isDockOpen()` (`None`: no dock).
    fn dock_open(&mut self, obj: ObjectId) -> Option<bool>;
    /// The special power module of the button's power.
    fn special_power(&mut self, obj: ObjectId, button: &CommandButton) -> SpecialPowerState;
    /// An overcharge behaviour that is active.
    fn overcharge_active(&mut self, obj: ObjectId) -> bool;
    /// The selected, locally controlled objects' current weapon slots (switch weapon buttons).
    fn selected_current_weapon_slots(&mut self) -> Vec<i32>;
    /// A hack internet AI that is hacking, packing or unpacking.
    fn hacking_packing_or_unpacking(&mut self, obj: ObjectId) -> bool;
    /// A battle plan update and whether its active plan is the bombardment (`None`: no update).
    fn battle_plan_is_bombardment(&mut self, obj: ObjectId) -> Option<bool>;
    /// The purchase-science buttons of the player's template rank 1, 3 and 8 sets, in order (`None` if a set or name is
    /// missing).
    fn purchase_science_buttons(&self, obj: ObjectId) -> Option<Vec<usize>>;
    /// `power->getRequiredScience()` of the button's power (`None`: `SCIENCE_INVALID` or no power).
    fn special_power_required_science(&self, button: &CommandButton) -> Option<i32>;
}

/// `ControlBar::getCommandAvailability(command, obj, win, applyToWin, forceDisabledEvaluation)`: the availability, and
/// the inverse clock's percent drawn on the button (`GadgetButtonDrawInverseClock`), if any.
pub fn get_command_availability(button: &CommandButton, obj: Option<ObjectId>, force_disabled: bool, h: &mut impl ControlBarStateHost) -> (CommandAvailability, Option<i32>) {
    use CommandAvailability::*;
    let t = button.command;
    let obj = if t == gui::SPECIAL_POWER_FROM_SHORTCUT || t == gui::SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT { h.shortcut_source(button) } else { obj };
    let Some(obj) = obj else { return (Hidden, None) };
    if h.script_disabled_or_unpowered(obj) || h.disabled_unmanned(obj) {
        return (Hidden, None);
    }
    if h.single_use_used(obj) {
        return (Restricted, None);
    }
    let o = button.options;
    if o & opt::MUST_BE_STOPPED != 0 && h.is_moving(obj) {
        return (Restricted, None);
    }
    let (mut disabled, only_underpowered) = h.disabled(obj);
    if disabled && o & opt::IGNORES_UNDERPOWERED != 0 && only_underpowered {
        disabled = false;
    }
    if disabled && !force_disabled {
        let usable = [gui::SELL, gui::EVACUATE, gui::EXIT_CONTAINER, gui::BEACON_DELETE, gui::SET_RALLY_POINT, gui::STOP, gui::SWITCH_WEAPON];
        if !usable.contains(&t) {
            if get_command_availability(button, Some(obj), true, h).0 == Hidden {
                return (Hidden, None);
            }
            return (Restricted, None);
        }
    }
    if o & opt::NEED_UPGRADE != 0 {
        if let Some(u) = button.upgrade.map(|k| k.0) {
            match h.upgrade_is_player(u) {
                Some(true) if !h.player_has_upgrade_complete(obj, u) => return (Restricted, None),
                Some(false) if !h.obj_has_upgrade(obj, u) => return (Restricted, None),
                _ => {}
            }
        }
    }
    let count = h.production_count(obj);
    if count.is_some_and(|c| c > 0) && o & opt::NOT_QUEUEABLE != 0 {
        return (Restricted, None);
    }
    let queue_maxed = count == Some(MAX_BUILD_QUEUE_BUTTONS as i32);
    let hidden_by_buildable = |h: &mut dyn FnMut() -> (Buildable, bool)| {
        let (b, computer) = h();
        b == Buildable::No || (b == Buildable::OnlyByAi && !computer)
    };
    use crate::logic::world::build_assistant::CanMakeType;
    match t {
        gui::DOZER_CONSTRUCT => {
            if let Some(tt) = button.thing_template {
                if hidden_by_buildable(&mut || (h.buildable(tt), h.player_is_computer(obj))) {
                    return (Hidden, None);
                }
            }
            if !h.is_dozer(obj) {
                return (Restricted, None);
            }
            match h.dozer_build_pending(obj) {
                None | Some(true) => return (Restricted, None),
                Some(false) => {}
            }
            if !h.player_can_build(obj, button.thing_template) || !h.player_can_afford_build(obj, button.thing_template) {
                return (Restricted, None);
            }
        }
        gui::SELL => {
            if h.unsellable(obj) {
                return (Hidden, None);
            }
            if h.disabled_subdued(obj) {
                return (Restricted, None);
            }
        }
        gui::UNIT_BUILD => {
            if let Some(tt) = button.thing_template {
                if hidden_by_buildable(&mut || (h.buildable(tt), h.player_is_computer(obj))) {
                    return (Hidden, None);
                }
            }
            if queue_maxed || !h.player_can_build(obj, button.thing_template) {
                return (Restricted, None);
            }
            match h.can_make_unit(obj, button.thing_template) {
                CanMakeType::MaxedOutForPlayer | CanMakeType::ParkingPlacesFull | CanMakeType::NoMoney => return (Restricted, None),
                _ => {}
            }
        }
        gui::PLAYER_UPGRADE => {
            if queue_maxed {
                return (Restricted, None);
            }
            let u = button.upgrade.map_or(0, |k| k.0);
            if h.player_has_upgrade_complete(obj, u) || h.player_has_upgrade_in_production(obj, u) {
                return (CantAfford, None);
            }
            if !h.can_afford_upgrade(obj, u) {
                return (Restricted, None);
            }
            for &st in &button.science {
                if !h.player_has_science(obj, st.0) {
                    return (Restricted, None);
                }
            }
        }
        gui::OBJECT_UPGRADE => {
            if queue_maxed || count.is_none() {
                return (Restricted, None);
            }
            let u = button.upgrade.map_or(0, |k| k.0);
            if h.obj_has_upgrade(obj, u) || h.is_upgrade_in_queue(obj, u) || !h.affected_by_upgrade(obj, u) {
                return (CantAfford, None);
            }
            if !h.can_afford_upgrade(obj, u) {
                return (Restricted, None);
            }
            for &st in &button.science {
                if !h.player_has_science(obj, st.0) {
                    return (Restricted, None);
                }
            }
        }
        gui::FIRE_WEAPON => {
            // "no ai, can't possibly fire weapon" (**Corrected 2026-11-06 (#33):** the check was missing here)
            if !h.has_ai_update(obj) {
                return (Restricted, None);
            }
            let w = h.weapon(obj, button.weapon_slot);
            if let Some(w) = w {
                if w.clip_reload_time == 0 {
                    return (Available, None);
                }
            }
            let now = h.frame();
            let not_ready = match w {
                None => true,
                Some(w) => !w.ready_to_fire || w.possible_next_shot_frame == now || w.possible_next_shot_frame == now.wrapping_sub(1),
            };
            if not_ready {
                return match w {
                    Some(w) => (NotReady, w.reloading_clip.then_some((w.percent_ready * 100.0) as i32)),
                    None => {
                        if o & opt::USES_MINE_CLEARING_WEAPONSET != 0 && !h.mine_clearing_weapon_set(obj) {
                            (Available, None)
                        } else {
                            (Restricted, None)
                        }
                    }
                };
            }
        }
        gui::COMBATDROP => {
            if h.rappeller_count(obj) <= 0 {
                return (Restricted, None);
            }
        }
        gui::EXIT_CONTAINER => {
            if h.disabled_subdued(obj) {
                return (Restricted, None);
            }
        }
        gui::EVACUATE => {
            if h.contain_count(obj).is_none_or(|c| c == 0) || h.disabled_subdued(obj) {
                return (Restricted, None);
            }
        }
        gui::EXECUTE_RAILED_TRANSPORT => {
            if h.dock_open(obj) != Some(true) {
                return (Restricted, None);
            }
        }
        gui::SPECIAL_POWER | gui::SPECIAL_POWER_FROM_SHORTCUT | gui::SPECIAL_POWER_CONSTRUCT | gui::SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT => {
            match h.special_power(obj, button) {
                SpecialPowerState::NoModule => {}
                SpecialPowerState::Charging(p) => return (NotReady, Some((p * 100.0) as i32)),
                SpecialPowerState::Ready { ability_in_use: Some(true), .. } => return (Restricted, None),
                SpecialPowerState::Ready { ability_in_use: None, battle_plan_option_match: true } => return (Active, None),
                SpecialPowerState::Ready { .. } => {}
            }
        }
        gui::TOGGLE_OVERCHARGE => {
            if h.overcharge_active(obj) {
                return (Active, None);
            }
        }
        gui::SWITCH_WEAPON => {
            if h.weapon(obj, button.weapon_slot).is_none() {
                return (Restricted, None);
            }
            if h.selected_current_weapon_slots().iter().any(|&s| s != button.weapon_slot) {
                return (Available, None);
            }
            return (Active, None);
        }
        gui::HACK_INTERNET => {
            if h.hacking_packing_or_unpacking(obj) {
                return (Restricted, None);
            }
            return (Available, None);
        }
        gui::STOP => {
            if o & opt::OPTION_ONE == 0 {
                return (Available, None);
            }
            if h.battle_plan_is_bombardment(obj) == Some(false) {
                return (Restricted, None);
            }
            return (Available, None);
        }
        _ => {}
    }
    (Available, None)
}

/// What one command window shows.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandSlot {
    /// The button's place in the World's list (`GadgetButtonGetData`).
    pub button: usize,
    /// The enabled image (`ButtonImage`, or the matching purchase-science button's).
    pub image: Option<Vec<u8>>,
    /// The tooltip's text (`TextLabel` through the game text), and the description (`DescriptLabel`).
    pub label: Vec<u16>,
    pub description: Vec<u16>,
    pub availability: CommandAvailability,
    /// The inverse clock's percent.
    pub clock: Option<i32>,
    /// `winEnable`.
    pub enabled: bool,
    /// `WIN_STATUS_NOT_READY`, `WIN_STATUS_ALWAYS_COLOR`.
    pub not_ready: bool,
    pub always_color: bool,
    /// `CHECK_LIKE` and checked.
    pub check_like: bool,
    pub checked: bool,
}

/// `populateCommand(obj)` then the command windows' part of `updateContextCommand()`: what each of the 18 command windows
/// shows (`None`: hidden).
pub fn populate_command(obj: ObjectId, h: &mut impl ControlBarStateHost) -> [Option<CommandSlot>; MAX_COMMANDS_PER_SET] {
    populate_command_with_hot_keys(obj, h).0
}

/// The 18 command windows' slots ([`populate_command`]).
pub type CommandSlots = [Option<CommandSlot>; MAX_COMMANDS_PER_SET];

/// #131: [`populate_command`], and the hot keys `setControlCommand` adds on the way (`searchHotKey` of each button's text
/// label, [`crate::client::hot_key`]): `(command window slot, key)` in the order added, for every button the loop gives a
/// window to (all but the script-only ones and the transport's exit buttons), **including** those then hidden for a
/// science the player lacks or by their availability. The caller resets `TheHotKeyManager` first (`switchToContext`)
/// and adds them in order (a key already mapped keeps its first window).
pub fn populate_command_with_hot_keys(obj: ObjectId, h: &mut impl ControlBarStateHost) -> (CommandSlots, Vec<(usize, Vec<u8>)>) {
    let mut slots: CommandSlots = Default::default();
    let mut hot_keys = Vec::new();
    let Some(set) = h.command_set_buttons(obj) else { return (slots, hot_keys) };
    for (i, b) in set.iter().enumerate() {
        let Some(bi) = *b else { continue };
        let Some(button) = h.button(bi) else { continue };
        if button.options & opt::SCRIPT_ONLY != 0 || button.command == gui::EXIT_CONTAINER {
            continue;
        }
        // (#131) setControlCommand's hot key: `searchHotKey(commandButton->getTextLabel())`, the button's own label (the
        // purchase-science text below comes after it, upstream's `copyButtonTextFrom`)
        if !button.labels[0].is_empty() {
            let text = h.fetch_text(&button.labels[0]);
            let key = crate::client::hot_key::HotKeyManager::search_hot_key(&text);
            if !key.is_empty() {
                hot_keys.push((i, key));
            }
        }
        // setControlCommand
        let mut image = (!button.images[0].is_empty()).then(|| button.images[0].clone());
        let mut label_src = button.labels[0].clone();
        let mut descript_src = button.labels[1].clone();
        if button.options & opt::NEED_SPECIAL_POWER_SCIENCE != 0 {
            if let Some(req) = h.special_power_required_science(&button) {
                if button.command != gui::PURCHASE_SCIENCE && button.command != gui::PLAYER_UPGRADE && button.command != gui::OBJECT_UPGRADE {
                    if !h.player_has_science(obj, req) {
                        continue;
                    }
                    // the best science of the vector the player has, in order
                    let mut best = None;
                    for &st in &button.science {
                        if h.player_has_science(obj, st.0) {
                            best = Some(st.0);
                        } else {
                            break;
                        }
                    }
                    if let (Some(science), Some(purchase)) = (best, h.purchase_science_buttons(obj)) {
                        for pi in purchase {
                            let Some(pb) = h.button(pi) else { continue };
                            if pb.command == gui::PURCHASE_SCIENCE && pb.science.first().map(|s| s.0) == Some(science) {
                                // copyImagesFrom(command, TRUE), copyButtonTextFrom(command, FALSE, TRUE): the image; the
                                // text and description labels where the purchase button has them
                                image = (!pb.images[0].is_empty()).then(|| pb.images[0].clone());
                                if !pb.labels[0].is_empty() {
                                    label_src = pb.labels[0].clone();
                                }
                                if !pb.labels[1].is_empty() {
                                    descript_src = pb.labels[1].clone();
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
        let fetch = |h: &mut dyn FnMut(&[u8]) -> Vec<u16>, l: &[u8]| if l.is_empty() { Vec::new() } else { h(l) };
        let label = fetch(&mut |l| h.fetch_text(l), &label_src);
        let description = fetch(&mut |l| h.fetch_text(l), &descript_src);
        // updateContextCommand
        let (availability, clock) = get_command_availability(&button, Some(obj), false, h);
        if availability == CommandAvailability::Hidden {
            continue;
        }
        let check_like = button.options & opt::CHECK_LIKE != 0;
        slots[i] = Some(CommandSlot {
            button: bi,
            image,
            label,
            description,
            availability,
            clock,
            enabled: matches!(availability, CommandAvailability::Available | CommandAvailability::Active),
            not_ready: availability == CommandAvailability::NotReady,
            always_color: availability == CommandAvailability::CantAfford,
            check_like,
            checked: check_like && availability == CommandAvailability::Active,
        });
    }
    (slots, hot_keys)
}

/// One build queue entry as the queue window shows it (`m_queueData`).
#[derive(Clone, Debug, PartialEq)]
pub struct QueueSlot {
    /// `PRODUCTION_UNIT` (else an upgrade).
    pub unit: bool,
    /// The production id (a unit) or the upgrade's name key, for the cancel button.
    pub id: u32,
    /// The thing's or upgrade's button image.
    pub image: Option<Vec<u8>>,
}

/// `populateBuildQueue(producer)` and `updateContextCommand`'s clock: the entries (at most 9), and the first's percent
/// complete (`GadgetButtonDrawInverseClock` of `getPercentComplete()`).
pub fn populate_build_queue(entries: &[(bool, u32, Option<Vec<u8>>, f32)]) -> (Vec<QueueSlot>, Option<i32>) {
    let slots = entries.iter().take(MAX_BUILD_QUEUE_BUTTONS).map(|(unit, id, image, _)| QueueSlot { unit: *unit, id: *id, image: image.clone() }).collect();
    (slots, entries.first().map(|e| e.3 as i32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::world::build_assistant::CanMakeType;

    /// A host whose object 1 holds a set of five buttons.
    struct H {
        buttons: Vec<CommandButton>,
        disabled: bool,
        money: bool,
        charging: Option<f32>,
        /// (#33) the AI, and a weapon with no clip reload in every slot
        ai: bool,
        weapon: bool,
    }
    impl ControlBarStateHost for H {
        fn command_set_buttons(&self, _: ObjectId) -> Option<[Option<usize>; MAX_COMMANDS_PER_SET]> {
            let mut s = [None; MAX_COMMANDS_PER_SET];
            for i in 0..self.buttons.len() {
                s[i] = Some(i);
            }
            Some(s)
        }
        fn button(&self, i: usize) -> Option<CommandButton> {
            self.buttons.get(i).cloned()
        }
        fn fetch_text(&self, l: &[u8]) -> Vec<u16> {
            [b"T:".as_slice(), l].concat().iter().map(|&b| u16::from(b)).collect()
        }
        fn shortcut_source(&mut self, _: &CommandButton) -> Option<ObjectId> {
            None
        }
        fn script_disabled_or_unpowered(&self, _: ObjectId) -> bool {
            false
        }
        fn disabled_unmanned(&self, _: ObjectId) -> bool {
            false
        }
        fn disabled_subdued(&self, _: ObjectId) -> bool {
            false
        }
        fn single_use_used(&self, _: ObjectId) -> bool {
            false
        }
        fn is_moving(&mut self, _: ObjectId) -> bool {
            false
        }
        fn disabled(&self, _: ObjectId) -> (bool, bool) {
            (self.disabled, false)
        }
        fn upgrade_is_player(&self, _: u32) -> Option<bool> {
            None
        }
        fn player_has_upgrade_complete(&self, _: ObjectId, _: u32) -> bool {
            false
        }
        fn player_has_upgrade_in_production(&self, _: ObjectId, _: u32) -> bool {
            false
        }
        fn obj_has_upgrade(&mut self, _: ObjectId, _: u32) -> bool {
            false
        }
        fn affected_by_upgrade(&mut self, _: ObjectId, _: u32) -> bool {
            true
        }
        fn production_count(&mut self, _: ObjectId) -> Option<i32> {
            Some(0)
        }
        fn is_upgrade_in_queue(&mut self, _: ObjectId, _: u32) -> bool {
            false
        }
        fn buildable(&self, t: u16) -> Buildable {
            if t == 99 {
                Buildable::No
            } else {
                Buildable::Yes
            }
        }
        fn player_is_computer(&self, _: ObjectId) -> bool {
            false
        }
        fn is_dozer(&self, _: ObjectId) -> bool {
            false
        }
        fn dozer_build_pending(&mut self, _: ObjectId) -> Option<bool> {
            None
        }
        fn player_can_build(&mut self, _: ObjectId, _: Option<u16>) -> bool {
            true
        }
        fn player_can_afford_build(&mut self, _: ObjectId, _: Option<u16>) -> bool {
            self.money
        }
        fn can_make_unit(&mut self, _: ObjectId, _: Option<u16>) -> CanMakeType {
            if self.money {
                CanMakeType::Ok
            } else {
                CanMakeType::NoMoney
            }
        }
        fn can_afford_upgrade(&mut self, _: ObjectId, _: u32) -> bool {
            self.money
        }
        fn player_has_science(&mut self, _: ObjectId, _: i32) -> bool {
            false
        }
        fn unsellable(&self, _: ObjectId) -> bool {
            false
        }
        fn weapon(&mut self, _: ObjectId, _: i32) -> Option<WeaponState> {
            self.weapon.then_some(WeaponState { clip_reload_time: 0, ready_to_fire: true, reloading_clip: false, possible_next_shot_frame: 0, percent_ready: 1.0 })
        }
        fn frame(&self) -> u32 {
            10
        }
        fn mine_clearing_weapon_set(&self, _: ObjectId) -> bool {
            false
        }
        fn rappeller_count(&mut self, _: ObjectId) -> i32 {
            0
        }
        fn has_ai_update(&self, _: ObjectId) -> bool {
            self.ai
        }
        fn contain_count(&mut self, _: ObjectId) -> Option<u32> {
            None
        }
        fn dock_open(&mut self, _: ObjectId) -> Option<bool> {
            None
        }
        fn special_power(&mut self, _: ObjectId, _: &CommandButton) -> SpecialPowerState {
            match self.charging {
                Some(p) => SpecialPowerState::Charging(p),
                None => SpecialPowerState::Ready { ability_in_use: None, battle_plan_option_match: false },
            }
        }
        fn overcharge_active(&mut self, _: ObjectId) -> bool {
            false
        }
        fn selected_current_weapon_slots(&mut self) -> Vec<i32> {
            Vec::new()
        }
        fn hacking_packing_or_unpacking(&mut self, _: ObjectId) -> bool {
            false
        }
        fn battle_plan_is_bombardment(&mut self, _: ObjectId) -> Option<bool> {
            None
        }
        fn purchase_science_buttons(&self, _: ObjectId) -> Option<Vec<usize>> {
            None
        }
        fn special_power_required_science(&self, _: &CommandButton) -> Option<i32> {
            None
        }
    }

    fn button(command: i32, image: &str, label: &str) -> CommandButton {
        let mut b = CommandButton::new(b"B");
        b.command = command;
        b.images[0] = image.as_bytes().to_vec();
        b.labels[0] = label.as_bytes().to_vec();
        b
    }

    /// #12: the command windows. Predicted from ControlBarCommand.cpp before the run, for object 1 whose set is
    /// [build tank, stop, script-only sell, special power, build template 99 (not buildable)]:
    /// - with money: slot 0 shows `ImgTank`, label `T:CONTROLBAR:Tank`, enabled, available; slot 1 stop available; slot 2
    ///   hidden (script only); slot 3 the power, charging 25 %: not ready, clock 25, disabled; slot 4 hidden;
    /// - without money: slot 0 restricted (the "can't afford" that returns restricted), disabled, not drawn in colour;
    /// - disabled (EMP): slot 0 restricted, slot 1 (stop) still available;
    /// - the build queue of three entries: three slots, the first one's clock its percent (40).
    #[test]
    fn the_command_windows_show_as_upstream() {
        let mut sell = button(gui::SELL, "ImgSell", "");
        sell.options = opt::SCRIPT_ONLY;
        let mut tank = button(gui::UNIT_BUILD, "ImgTank", "CONTROLBAR:Tank");
        tank.thing_template = Some(5);
        let mut nope = button(gui::UNIT_BUILD, "ImgNope", "");
        nope.thing_template = Some(99);
        let mut h = H { buttons: vec![tank, button(gui::STOP, "ImgStop", ""), sell, button(gui::SPECIAL_POWER, "ImgPower", ""), nope], disabled: false, money: true, charging: Some(0.25), ai: true, weapon: false };
        let s = populate_command(1, &mut h);
        let a = s[0].as_ref().unwrap();
        let t: Vec<u16> = "T:CONTROLBAR:Tank".encode_utf16().collect();
        assert_eq!((a.image.as_deref(), &a.label, a.enabled, a.availability), (Some(&b"ImgTank"[..]), &t, true, CommandAvailability::Available));
        assert_eq!(s[1].as_ref().unwrap().availability, CommandAvailability::Available);
        assert!(s[2].is_none());
        let p = s[3].as_ref().unwrap();
        assert_eq!((p.availability, p.clock, p.enabled, p.not_ready), (CommandAvailability::NotReady, Some(25), false, true));
        assert!(s[4].is_none());
        h.money = false;
        let s = populate_command(1, &mut h);
        let a = s[0].as_ref().unwrap();
        assert_eq!((a.availability, a.enabled, a.always_color), (CommandAvailability::Restricted, false, false));
        h.money = true;
        h.disabled = true;
        let s = populate_command(1, &mut h);
        assert_eq!(s[0].as_ref().unwrap().availability, CommandAvailability::Restricted);
        assert_eq!(s[1].as_ref().unwrap().availability, CommandAvailability::Available);
        let (q, clock) = populate_build_queue(&[(true, 1, Some(b"ImgTank".to_vec()), 40.0), (true, 2, None, 0.0), (false, 77, None, 0.0)]);
        assert_eq!((q.len(), clock, q[2].unit), (3, Some(40), false));
    }

    /// #33, the fire weapon button and the AI (`getCommandAvailability`: "no ai, can't possibly fire weapon"). Predicted:
    /// a weapon with no clip reload time is `AVAILABLE` with an AI ("anything with a 0 clip reload time"), `RESTRICTED`
    /// without one (the check the #12 translation had missed); no weapon in the slot is `RESTRICTED` either way.
    #[test]
    fn a_fire_weapon_button_needs_an_ai() {
        let fire = button(gui::FIRE_WEAPON, "ImgFire", "");
        let mut h = H { buttons: vec![fire.clone()], disabled: false, money: true, charging: None, ai: true, weapon: true };
        assert_eq!(get_command_availability(&fire, Some(1), false, &mut h), (CommandAvailability::Available, None));
        h.ai = false;
        assert_eq!(get_command_availability(&fire, Some(1), false, &mut h), (CommandAvailability::Restricted, None));
        h.ai = true;
        h.weapon = false;
        assert_eq!(get_command_availability(&fire, Some(1), false, &mut h), (CommandAvailability::Restricted, None));
    }
}
