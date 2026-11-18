//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (ControlBar::populatePurchaseScience,
//!   updateContextPurchaseScience, CommandSet::friend_addToList, newCommandButton, newCommandSet, newCommandSetOverride,
//!   processContextSensitiveButtonTransition, setControlBarSchemeByName, hideCommunicator, updatePurchaseScience,
//!   updateCommandMarkerImage, getForegroundMarkerPos, getBackgroundMarkerPos, drawTransitionHandler,
//!   populateSpecialPowerShortcut, hasAnyShortcutSelection, updateSpecialPowerShortcut,
//!   drawSpecialPowerShortcutMultiplierText)
//!
//! The verified `crate::client::control_bar*` is the control bar's command logic (what a click does, the availability of
//! a command) and the scheme; the windows of the purchase-science screen and the shortcut bar are not in it. They are
//! reached here through [`ControlBarWindowsHost`], by window handle.
//!
//! Sweep 2 notes:
//! - `ControlBar::updatePurchaseScience` (ControlBar.cpp:2931), `updateCommandMarkerImage` (:3090) and
//!   `drawTransitionHandler` (:3183): empty bodies upstream (drafted as such).

use crate::client::control_bar::gui;
use crate::common::game_type::ObjectId;

/// `MAX_PURCHASE_SCIENCE_RANK_1`, `_3`, `_8` (ControlBar.h).
pub const MAX_PURCHASE_SCIENCE_RANK_1: usize = 4;
pub const MAX_PURCHASE_SCIENCE_RANK_3: usize = 15;
pub const MAX_PURCHASE_SCIENCE_RANK_8: usize = 4;
/// `MAX_SPECIAL_POWER_SHORTCUTS`.
pub const MAX_SPECIAL_POWER_SHORTCUTS: usize = 11;
/// `WIN_STATUS_ALWAYS_COLOR`, `WIN_STATUS_NOT_READY`.
pub const WIN_STATUS_ALWAYS_COLOR: u32 = crate::client::window_script::win_status::ALWAYS_COLOR;
pub const WIN_STATUS_NOT_READY: u32 = crate::client::window_script::win_status::NOT_READY;
/// `MAX_COMMANDS_PER_SET` (ControlBar.h).
pub const MAX_COMMANDS_PER_SET: usize = 18;
/// `SCIENCE_INVALID`.
pub const SCIENCE_INVALID: i32 = -1;

/// A command button's fields these functions read.
#[derive(Clone, Debug, Default)]
pub struct CommandButtonInfo {
    /// `getName()`.
    pub name: Vec<u8>,
    /// `getOptions()`.
    pub options: u32,
    /// `getCommandType()` (`crate::client::control_bar::gui`).
    pub command_type: i32,
    /// `getScienceVec()`.
    pub science_vec: Vec<i32>,
    /// `getSpecialPowerTemplate()`: its `getSpecialPowerType()` and `getRequiredScience()`; `None`: no template.
    pub special_power: Option<(i32, i32)>,
    /// `getUpgradeTemplate()`'s mask; `None`: no template.
    pub upgrade_mask: Option<u64>,
    /// `getThingTemplate()`'s id.
    pub thing_template: Option<u16>,
}

/// `CommandOption` bits read here (ControlBar.h).
pub mod command_option {
    pub const NEED_UPGRADE: u32 = 0x0000_0040;
    pub const NEED_SPECIAL_POWER_SCIENCE: u32 = 0x0000_0080;
    pub const SCRIPT_ONLY: u32 = 0x0008_0000;
}

/// What these functions ask of the control bar's windows, the local player and the science store.
pub trait ControlBarWindowsHost {
    // ---- windows, by handle ----
    fn win_hide(&mut self, win: u32, hide: bool);
    fn win_enable(&mut self, win: u32, enable: bool);
    fn win_is_hidden(&self, win: u32) -> bool;
    fn win_set_status(&mut self, win: u32, bits: u32);
    fn win_clear_status(&mut self, win: u32, bits: u32);
    /// `TheWindowManager->winGetWindowFromId(parent, nameToKey(name))`.
    fn win_get_window_from_name(&self, parent: u32, name: &[u8]) -> Option<u32>;
    /// `GadgetStaticTextSetText(win, text)`.
    fn static_text_set_text(&mut self, win: u32, text: &[u16]);
    /// `GadgetProgressBarSetProgress(win, progress)`.
    fn progress_bar_set_progress(&mut self, win: u32, progress: i32);
    /// `GadgetButtonGetData(win)`: the command the button holds.
    fn button_data(&self, win: u32) -> Option<CommandButtonInfo>;
    /// `GadgetButtonSetText(win, text)`.
    fn button_set_text(&mut self, win: u32, text: &[u16]);
    /// `GadgetButtonSetAltSound(win, name)`.
    fn button_set_alt_sound(&mut self, win: u32, name: &[u8]);
    /// `setControlCommand(win, commandButton)`.
    fn set_control_command(&mut self, win: u32, command_button: &CommandButtonInfo);
    // ---- the control bar's own state ----
    /// `m_contextParent[CP_PURCHASE_SCIENCE]`, `m_contextParent[CP_MASTER]`.
    fn context_parent_purchase_science(&self) -> Option<u32>;
    fn context_parent_master(&self) -> Option<u32>;
    /// `m_sciencePurchaseWindowsRank1[i]` ... `Rank3`, `Rank8`.
    fn science_purchase_window(&self, rank: u32, i: usize) -> u32;
    /// `m_specialPowerShortcutButtons[i]`, `m_specialPowerShortcutButtonParents[i]` (`None`: NULL), the parent window,
    /// and `m_currentlyUsedSpecialPowersButtons`.
    fn shortcut_button(&self, i: usize) -> Option<u32>;
    fn shortcut_button_parent(&self, i: usize) -> Option<u32>;
    fn shortcut_parent(&self) -> Option<u32>;
    fn shortcut_buttons_exist(&self) -> bool;
    fn currently_used_special_powers_buttons(&self) -> usize;
    /// `m_commandWindows[i]`.
    fn command_window(&self, i: usize) -> u32;
    /// `m_shortcutDisplayStrings[i]` exists.
    fn shortcut_display_string_exists(&self, i: usize) -> bool;
    /// `showSpecialPowerShortcut()`, `hideSpecialPowerShortcut()`, `animateSpecialPowerShortcut(forward)`,
    /// `m_animateWindowManagerForGenShortcuts->isFinished()`.
    fn show_special_power_shortcut(&mut self);
    fn hide_special_power_shortcut(&mut self);
    fn animate_special_power_shortcut(&mut self, forward: bool);
    fn gen_shortcuts_animation_finished(&self) -> bool;
    /// `findCommandSet(name)` then `getCommandButton(i)`: the button `i` of the named command set, `None` for no set (the
    /// outer) or no button (the inner).
    fn command_set_button(&self, set_name: &[u8], i: usize) -> Option<Option<CommandButtonInfo>>;
    /// `getCommandAvailability(button, obj, win[, applyToWin])`: a `CommandAvailability` (`crate::client::control_bar_command`).
    fn get_command_availability(&mut self, button: &CommandButtonInfo, obj: Option<ObjectId>, win: u32, apply_to_win: Option<u32>) -> CommandAvailability;
    // ---- the local player ----
    fn script_engine_is_game_ending(&self) -> bool;
    fn player_exists(&self) -> bool;
    /// `player->getPlayerTemplate()`'s purchase science command set names of ranks 1, 3, 8, and the special power
    /// shortcut command set name; `None`: no template.
    fn purchase_science_command_set_names(&self) -> Option<[Vec<u8>; 3]>;
    fn special_power_shortcut_command_set_name(&self) -> Option<Vec<u8>>;
    fn player_is_local(&self) -> bool;
    fn player_is_active(&self) -> bool;
    fn player_has_science(&self, science: i32) -> bool;
    fn player_is_science_disabled(&self, science: i32) -> bool;
    fn player_is_science_hidden(&self, science: i32) -> bool;
    fn player_has_upgrade_complete(&self, mask: u64) -> bool;
    fn player_science_purchase_points(&self) -> i32;
    fn player_skill_points(&self) -> i32;
    fn player_skill_points_level_down(&self) -> i32;
    fn player_skill_points_level_up(&self) -> i32;
    fn player_rank_level(&self) -> i32;
    /// `TheScienceStore->playerHasPrereqsForScience`, `getSciencePurchaseCost`, `playerHasRootPrereqsForScience`.
    fn science_player_has_prereqs(&self, science: i32) -> bool;
    fn science_purchase_cost(&self, science: i32) -> i32;
    fn science_player_has_root_prereqs(&self, science: i32) -> bool;
    /// `TheGameText->fetch(label)`.
    fn game_text_fetch(&self, label: &[u8]) -> Vec<u16>;
    /// `getLocalPlayer()->findMostReadyShortcutSpecialPowerOfType(type)`.
    fn find_most_ready_shortcut_special_power_of_type(&self, special_power_type: i32) -> Option<ObjectId>;
    /// `findAnyExistingObjectWithThingTemplate(template)`.
    fn find_any_existing_object_with_thing_template(&self, template: u16) -> Option<ObjectId>;
    /// `findMostReadyShortcutSpecialPowerForThing(template, mostReadyPercentage)`.
    fn find_most_ready_shortcut_special_power_for_thing(&self, template: u16) -> Option<ObjectId>;
    /// `hasAnyShortcutSpecialPower()`, `countReadyShortcutSpecialPowersOfType(type)`.
    fn has_any_shortcut_special_power(&self) -> bool;
    fn count_ready_shortcut_special_powers_of_type(&self, special_power_type: i32) -> i32;
    /// `obj->getCommandSetString()`.
    fn object_command_set_string(&self, obj: ObjectId) -> Vec<u8>;
    /// `commandButton->copyImagesFrom(command, TRUE)` and `copyButtonTextFrom(command, TRUE, TRUE)`.
    fn copy_images_and_text_from(&mut self, to: &CommandButtonInfo, from: &CommandButtonInfo);
}

/// `CommandAvailability` (`COMMAND_HIDDEN`, `COMMAND_RESTRICTED`, `COMMAND_NOT_READY`, `COMMAND_CANT_AFFORD`, and the
/// others which enable the window).
pub use crate::client::control_bar_command::CommandAvailability;

fn bit_test(options: u32, bit: u32) -> bool {
    options & bit != 0
}

/// One rank's loop of `populatePurchaseScience`. `kind` follows the three upstream loops' differences: rank 1 does
/// nothing to a button without a science, rank 3 takes `SCIENCE_INVALID` for it, rank 8 reads `getScienceVec()[0]`
/// without checking (undefined for an empty vector; `SCIENCE_INVALID` here).
fn populate_rank<H: ControlBarWindowsHost>(h: &mut H, rank: u32, count: usize, set_name: &[u8]) {
    for i in 0..count {
        let win = h.science_purchase_window(rank, i);
        let command_button = h.command_set_button(set_name, i).flatten();
        let Some(cb) = command_button.filter(|cb| !bit_test(cb.options, command_option::SCRIPT_ONLY)) else {
            h.win_hide(win, true);
            continue;
        };
        h.win_hide(win, false);
        h.win_enable(win, false);
        h.set_control_command(win, &cb);
        let st = match (rank, cb.science_vec.first()) {
            (1, None) => continue, // if (!commandButton->getScienceVec().empty()) { .. }
            (_, Some(&s)) => s,
            (_, None) => SCIENCE_INVALID,
        };
        if h.player_is_science_disabled(st) {
            h.win_enable(win, false);
        } else if h.player_is_science_hidden(st) {
            h.win_hide(win, true);
        } else {
            if !h.player_has_science(st) && h.science_player_has_prereqs(st) && h.science_purchase_cost(st) <= h.player_science_purchase_points() {
                h.win_enable(win, true);
            }
            if h.player_has_science(st) {
                h.win_set_status(win, WIN_STATUS_ALWAYS_COLOR);
            } else {
                h.win_clear_status(win, WIN_STATUS_ALWAYS_COLOR);
            }
            if !h.science_player_has_root_prereqs(st) {
                h.win_hide(win, true);
            }
        }
    }
}

/// `ControlBar::populatePurchaseScience(player)` (ControlBar.cpp:167): the rank 1, 3 and 8 science buttons of the
/// purchase screen from the three command sets of the player's template, then the points, experience bar and title.
pub fn populate_purchase_science<H: ControlBarWindowsHost>(h: &mut H, update_context_purchase_science: impl FnOnce(&mut H)) {
    if h.script_engine_is_game_ending() {
        return;
    }
    let names = if h.player_exists() { h.purchase_science_command_set_names() } else { None };
    let Some(names) = names else { return };
    if names[0].is_empty() || names[1].is_empty() || names[2].is_empty() {
        return;
    }
    let set1 = h.command_set_button(&names[0], 0).is_some();
    let set3 = h.command_set_button(&names[1], 0).is_some();
    let set8 = h.command_set_button(&names[2], 0).is_some();
    for i in 0..MAX_PURCHASE_SCIENCE_RANK_1 {
        let w = h.science_purchase_window(1, i);
        h.win_hide(w, true);
    }
    for i in 0..MAX_PURCHASE_SCIENCE_RANK_3 {
        let w = h.science_purchase_window(3, i);
        h.win_hide(w, true);
    }
    for i in 0..MAX_PURCHASE_SCIENCE_RANK_8 {
        let w = h.science_purchase_window(8, i);
        h.win_hide(w, true);
    }
    if !set1 || !set3 || !set8 {
        return;
    }
    populate_rank(h, 1, MAX_PURCHASE_SCIENCE_RANK_1, &names[0]);
    populate_rank(h, 3, MAX_PURCHASE_SCIENCE_RANK_3, &names[1]);
    populate_rank(h, 8, MAX_PURCHASE_SCIENCE_RANK_8, &names[2]);

    let Some(parent) = h.context_parent_purchase_science() else { return };
    if let Some(win) = h.win_get_window_from_name(parent, b"GeneralsExpPoints.wnd:StaticTextRankPointsAvailable") {
        let text: Vec<u16> = h.player_science_purchase_points().to_string().encode_utf16().collect();
        h.static_text_set_text(win, &text);
    }
    // (StaticTextLevel is commented out upstream: "redundant to StaticTextTitle in the Zero Hour context")
    if let Some(win) = h.win_get_window_from_name(parent, b"GeneralsExpPoints.wnd:ProgressBarExperience") {
        let down = h.player_skill_points_level_down();
        let progress = ((h.player_skill_points() - down) * 100) / (h.player_skill_points_level_up() - down);
        h.progress_bar_set_progress(win, progress);
    }
    if let Some(win) = h.win_get_window_from_name(parent, b"GeneralsExpPoints.wnd:StaticTextTitle") {
        let label = format!("SCIENCE:Rank{}", h.player_rank_level());
        let text = h.game_text_fetch(label.as_bytes());
        h.static_text_set_text(win, &text);
    }
    // to avoid a one frame delay where windows may become enabled/disabled, run the update at once
    update_context_purchase_science(h);
}

/// `ControlBar::updateContextPurchaseScience()` (ControlBar.cpp:458): the experience bar of the purchase screen. (The
/// rest of the function is a comment block upstream.)
pub fn update_context_purchase_science<H: ControlBarWindowsHost>(h: &mut H) {
    let Some(parent) = h.context_parent_purchase_science() else { return };
    if let Some(win) = h.win_get_window_from_name(parent, b"GeneralsExpPoints.wnd:ProgressBarExperience") {
        let down = h.player_skill_points_level_down();
        let progress = ((h.player_skill_points() - down) * 100) / (h.player_skill_points_level_up() - down);
        h.progress_bar_set_progress(win, progress);
    }
}

/// `ControlBar::updatePurchaseScience()` (ControlBar.cpp:2931): empty upstream.
pub fn update_purchase_science() {}
/// `ControlBar::updateCommandMarkerImage(image)` (ControlBar.cpp:3090): empty upstream.
pub fn update_command_marker_image() {}
/// `ControlBar::drawTransitionHandler()` (ControlBar.cpp:3183): empty upstream.
pub fn draw_transition_handler() {}

/// `ControlBar::hideCommunicator(b)` (ControlBar.cpp:2921): the communicator button hides or shows, if there is one.
pub fn hide_communicator<H: ControlBarWindowsHost>(h: &mut H, communicator_button: Option<u32>, b: bool) {
    if let Some(w) = communicator_button {
        h.win_hide(w, b);
    }
}

/// `ControlBar::getForegroundMarkerPos(x, y)` (ControlBar.cpp:3172) and `getBackgroundMarkerPos` (:3177): the stored
/// `m_controlBarForegroundMarkerPos` / `m_controlBarBackgroundMarkerPos`.
pub fn get_marker_pos(marker_pos: (i32, i32)) -> (i32, i32) {
    marker_pos
}

/// A command set, as `newCommandSet` and `newCommandSetOverride` build them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandSetRec {
    /// `getName()`.
    pub name: Vec<u8>,
    /// The buttons by name (`m_command[i]`).
    pub commands: Vec<Option<Vec<u8>>>,
    /// `m_isOverride`.
    pub is_override: bool,
    /// `m_nextOverride`, by its place in the control bar's `command_sets`.
    pub next_override: Option<usize>,
}

/// `CommandSet::friend_addToList(listHead)` (ControlBar.cpp:846): to the head of the list (`m_next = *listHead;
/// *listHead = this`). The lists are `Vec`s, head first.
pub fn friend_add_to_list<T>(list_head: &mut Vec<T>, item: T) {
    list_head.insert(0, item);
}

/// `ControlBar::newCommandButton(name)` (ControlBar.cpp:1931): a button with the name, linked at the head of
/// `m_commandButtons`. Its place (0).
pub fn new_command_button(command_buttons: &mut Vec<CommandButtonInfo>, name: &[u8]) -> usize {
    let new_button = CommandButtonInfo { name: name.to_vec(), ..CommandButtonInfo::default() };
    friend_add_to_list(command_buttons, new_button);
    0
}

/// `ControlBar::newCommandSet(name)` (ControlBar.cpp:2058): a set with the name at the head of `m_commandSets`. Its place (0).
pub fn new_command_set(command_sets: &mut Vec<CommandSetRec>, name: &[u8]) -> usize {
    let set = CommandSetRec { name: name.to_vec(), ..CommandSetRec::default() };
    friend_add_to_list(command_sets, set);
    0
}

/// `ControlBar::newCommandSetOverride(setToOverride)` (ControlBar.cpp:2072): a copy of the set, with the same name,
/// marked as an override and made the original's `m_nextOverride`; `None` for no set to override. Returns the copy
/// (not linked to the list: upstream does not link it).
pub fn new_command_set_override(set_to_override: Option<&mut CommandSetRec>, copy_place: usize) -> Option<CommandSetRec> {
    let to = set_to_override?;
    let mut set = CommandSetRec { name: to.name.clone(), ..CommandSetRec::default() };
    // *set = *setToOverride;
    set.commands = to.commands.clone();
    set.next_override = to.next_override;
    set.is_override = to.is_override;
    // set->markAsOverride();
    set.is_override = true;
    // setToOverride->setNextOverride(set);
    to.next_override = Some(copy_place);
    Some(set)
}

/// `ControlBar::processContextSensitiveButtonTransition(button, gadgetMessage)` (ControlBar.cpp:2107): the transition
/// is processed as a command transition; see
/// [`crate::drafts::sweep_gui_logic::control_bar_command_processing::process_command_transition_ui`].
pub use super::control_bar_command_processing::process_command_transition_ui as process_context_sensitive_button_transition;

/// `ControlBar::setControlBarSchemeByName(name)` (ControlBar.cpp:2842): if there is a scheme manager it sets the scheme;
/// the control bar then switches to its default stage (`CONTROL_BAR_STAGE_DEFAULT`).
pub fn set_control_bar_scheme_by_name(
    has_scheme_manager: bool,
    set_scheme: impl FnOnce(),
    switch_default_stage: impl FnOnce(),
) {
    if has_scheme_manager {
        set_scheme();
    }
    switch_default_stage();
}

/// `ControlBar::hasAnyShortcutSelection()` (ControlBar.cpp:3509): a visible shortcut button holding a "select all units
/// of type" command.
pub fn has_any_shortcut_selection<H: ControlBarWindowsHost>(h: &H) -> bool {
    for i in 0..h.currently_used_special_powers_buttons() {
        let Some(win) = h.shortcut_button(i) else { continue };
        if h.win_is_hidden(win) {
            continue;
        }
        let Some(command) = h.button_data(win) else { continue };
        if command.command_type == gui::SELECT_ALL_UNITS_OF_TYPE {
            return true;
        }
    }
    false
}

/// `ControlBar::populateSpecialPowerShortcut(player)` (ControlBar.cpp:3280): the shortcut buttons from the player
/// template's shortcut command set: the buttons are all hidden, then shown for each command that applies (its upgrade
/// complete, its power's science held and a ready power object, or an existing object of the type), with the image and
/// text of the matching purchase science button copied onto a science-bound one.
pub fn populate_special_power_shortcut<H: ControlBarWindowsHost>(h: &mut H, update_special_power_shortcut_fn: impl FnOnce(&mut H)) {
    // if(!player || !player->getPlayerTemplate() || !player->isLocalPlayer() || m_currentlyUsedSpecialPowersButtons == 0
    //    || m_specialPowerShortcutButtons == NULL || m_specialPowerShortcutButtonParents == NULL) return;
    if !h.player_exists() || !h.player_is_local() || h.currently_used_special_powers_buttons() == 0 || !h.shortcut_buttons_exist() {
        return;
    }
    let n = h.currently_used_special_powers_buttons();
    for i in 0..MAX_SPECIAL_POWER_SHORTCUTS {
        if let Some(b) = h.shortcut_button(i) {
            h.win_hide(b, true);
        }
        if let Some(p) = h.shortcut_button_parent(i) {
            h.win_hide(p, true);
        }
    }
    let Some(set_name) = h.special_power_shortcut_command_set_name() else { return };
    if set_name.is_empty() {
        return;
    }
    if h.command_set_button(&set_name, 0).is_none() {
        return;
    }
    let mut current_button = 0usize;
    for i in 0..n {
        let Some(Some(command_button)) = h.command_set_button(&set_name, i) else { continue };
        if bit_test(command_button.options, command_option::NEED_UPGRADE) {
            if let Some(mask) = command_button.upgrade_mask {
                if !h.player_has_upgrade_complete(mask) {
                    continue;
                }
            }
        }
        if bit_test(command_button.options, command_option::NEED_SPECIAL_POWER_SCIENCE) {
            let Some((power_type, required_science)) = command_button.special_power else {
                // DEBUG_CRASH( ("CommandButton %s needs a SpecialPower entry, but it's either incorrect or missing.", ..) );
                continue;
            };
            if h.find_most_ready_shortcut_special_power_of_type(power_type).is_none() {
                continue;
            }
            if required_science != SCIENCE_INVALID {
                if !h.player_has_science(required_science) {
                    continue;
                }
            } else {
                let mut best_index: i32 = -1;
                for (science_index, &science) in command_button.science_vec.iter().enumerate() {
                    if h.player_has_science(science) {
                        best_index = science_index as i32;
                    } else {
                        break;
                    }
                }
                if best_index != -1 {
                    let science = command_button.science_vec[best_index as usize];
                    let Some(names) = h.purchase_science_command_set_names() else { continue };
                    if names[0].is_empty() || names[1].is_empty() || names[2].is_empty() {
                        continue;
                    }
                    if h.command_set_button(&names[0], 0).is_none() || h.command_set_button(&names[1], 0).is_none() || h.command_set_button(&names[2], 0).is_none() {
                        continue;
                    }
                    let mut found = false;
                    for (set, count) in [(&names[0], MAX_PURCHASE_SCIENCE_RANK_1), (&names[1], MAX_PURCHASE_SCIENCE_RANK_3), (&names[2], MAX_PURCHASE_SCIENCE_RANK_8)] {
                        for k in 0..count {
                            if found {
                                break;
                            }
                            if let Some(Some(command)) = h.command_set_button(set, k) {
                                if command.command_type == gui::PURCHASE_SCIENCE {
                                    if command.science_vec.is_empty() {
                                        // DEBUG_CRASH( ("Commandbutton %s is a purchase science button without any science! ..") );
                                    } else if command.science_vec[0] == science {
                                        h.copy_images_and_text_from(&command_button, &command);
                                        found = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if command_button.command_type == gui::SELECT_ALL_UNITS_OF_TYPE {
            let Some(template) = command_button.thing_template else { continue };
            if h.find_any_existing_object_with_thing_template(template).is_none() {
                continue;
            }
        }
        let (Some(b), Some(p)) = (h.shortcut_button(current_button), h.shortcut_button_parent(current_button)) else { break };
        h.win_hide(b, false);
        h.win_hide(p, false);
        h.win_enable(b, true);
        h.win_enable(p, true);
        h.set_control_command(b, &command_button);
        h.button_set_alt_sound(b, b"GUIGenShortcutClick");
        current_button += 1;
    }
    if let (Some(master), Some(parent)) = (h.context_parent_master(), h.shortcut_parent()) {
        if !h.win_is_hidden(master) && h.win_is_hidden(parent) {
            h.show_special_power_shortcut();
            h.animate_special_power_shortcut(true);
        }
    }
    update_special_power_shortcut_fn(h);
}

/// `ControlBar::updateSpecialPowerShortcut()` (ControlBar.cpp:3535): shows or hides the shortcut bar as there are
/// shortcut buttons to show, then sets each visible button's state from its command's availability.
pub fn update_special_power_shortcut<H: ControlBarWindowsHost>(h: &mut H) {
    let Some(parent) = h.shortcut_parent() else { return };
    if !h.shortcut_buttons_exist() || !h.player_exists() {
        return;
    }
    let has_shortcut_selection_buttons = has_any_shortcut_selection(h);
    let has_any_shortcut_special_power = h.has_any_shortcut_special_power();
    let has_valid_shortcut_button = has_shortcut_selection_buttons || has_any_shortcut_special_power;
    let master = h.context_parent_master();
    let master_shown = master.is_some_and(|m| !h.win_is_hidden(m));
    if has_valid_shortcut_button && h.win_is_hidden(parent) && master_shown {
        h.show_special_power_shortcut();
        h.animate_special_power_shortcut(true);
    } else if !has_valid_shortcut_button && !h.win_is_hidden(parent) && h.gen_shortcuts_animation_finished() {
        h.animate_special_power_shortcut(false);
    }
    if h.win_is_hidden(parent) {
        return;
    }
    if !h.player_is_active() {
        h.hide_special_power_shortcut();
        return;
    }
    if master_shown && h.win_is_hidden(parent) {
        h.show_special_power_shortcut();
    }
    for i in 0..h.currently_used_special_powers_buttons() {
        let Some(win) = h.shortcut_button(i) else { continue };
        if h.win_is_hidden(win) {
            continue;
        }
        let Some(command) = h.button_data(win) else { continue };
        h.win_clear_status(win, WIN_STATUS_NOT_READY);
        h.win_clear_status(win, WIN_STATUS_ALWAYS_COLOR);
        let mut availability = CommandAvailability::Restricted;
        if let Some((power_type, _)) = command.special_power {
            let obj = h.find_most_ready_shortcut_special_power_of_type(power_type);
            availability = h.get_command_availability(&command, obj, win, None);
        } else if command.command_type == gui::SELECT_ALL_UNITS_OF_TYPE {
            availability = CommandAvailability::Hidden;
            if let Some(template) = command.thing_template {
                if h.find_any_existing_object_with_thing_template(template).is_some() {
                    availability = CommandAvailability::Available;
                }
                if let Some(obj) = h.find_most_ready_shortcut_special_power_for_thing(template) {
                    let set_name = h.object_command_set_string(obj);
                    if h.command_set_button(&set_name, 0).is_some() {
                        for command_index in 0..MAX_COMMANDS_PER_SET {
                            let eval_button = h.command_set_button(&set_name, command_index).flatten();
                            let eval_button_win = h.command_window(command_index);
                            if let Some(eb) = eval_button {
                                if eb.command_type == gui::SPECIAL_POWER {
                                    availability = h.get_command_availability(&eb, Some(obj), eval_button_win, Some(win));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        match availability {
            CommandAvailability::Hidden => h.win_hide(win, true),
            CommandAvailability::Restricted => h.win_enable(win, false),
            CommandAvailability::NotReady => {
                h.win_enable(win, false);
                h.win_set_status(win, WIN_STATUS_NOT_READY);
            }
            CommandAvailability::CantAfford => {
                h.win_enable(win, false);
                h.win_set_status(win, WIN_STATUS_ALWAYS_COLOR);
            }
            _ => h.win_enable(win, true),
        }
    }
}

/// `ControlBar::drawSpecialPowerShortcutMultiplierText()` (ControlBar.cpp:3663): on each visible shortcut button with a
/// command and a display string, the count of ready powers of its type when there is more than one, else nothing.
pub fn draw_special_power_shortcut_multiplier_text<H: ControlBarWindowsHost>(h: &mut H) {
    for i in 0..h.currently_used_special_powers_buttons() {
        let Some(win) = h.shortcut_button(i) else { continue };
        if h.win_is_hidden(win) {
            continue;
        }
        let Some(command) = h.button_data(win) else { continue };
        // draw superweapon ready multipliers: the inner loop (its `i` shadows the outer) runs once per shortcut slot and
        // sets the same text each time
        for k in 0..MAX_SPECIAL_POWER_SHORTCUTS {
            if !h.shortcut_display_string_exists(k) {
                // m_shortcutDisplayStrings[ i ] = newDisplayString(); ... (commented out upstream)
            }
            let mut num_ready = 0;
            if let Some((power_type, _)) = command.special_power {
                num_ready = h.count_ready_shortcut_special_powers_of_type(power_type);
            }
            // Lorenzen changed... Displaying a "1" is superfluous
            if num_ready > 1 {
                let text: Vec<u16> = num_ready.to_string().encode_utf16().collect();
                h.button_set_text(win, &text);
            } else {
                h.button_set_text(win, &[]);
            }
        }
    }
}
