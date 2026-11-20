//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 40,000 template-table cases and the constant members (test: tests/differential_draft_script_engine.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp (the small `ScriptEngine` members the
//!     verified port does not have: `addActionTemplateInfo`, `addConditionTemplateInfo`, `removeSequentialScript`,
//!     `hasUnitCompletedSequentialScript`, `hasTeamCompletedSequentialScript`, `getEnableVTune`, `setEnableVTune`,
//!     `debugVictory`)
//!
//! **Checked and already translated, no draft:**
//! - `ScriptEngine::notifyOfMidwaySpecialPower` (ScriptEngine.cpp:7234): `ScriptEngine::midway_special_powers` and the
//!   `I35` notification path in `src/logic/world/script_engine.rs` (the `m_midwaySpecialPowers` push).
//! - All 180 `ScriptActions::do*` / `update*` and 73 `ScriptConditions::evaluate*` entries of the sweep list that have a
//!   `ScriptAction::` / `Condition::` case in the verified `match` of `script_actions.rs` / `script_conditions.rs`:
//!   `execute_action` / `evaluate_condition` arms, with the front end translated; only the calls ending on the client are
//!   stubs, drafted in `script_actions.rs` (`ScriptClientCall`) and `client/.../w3d_view.rs`.
//! - `ScriptConditions::evaluateMissionAttempts` (ScriptConditions.cpp:1232): upstream itself is a stub (`return false;`,
//!   the player lookup commented out); the port's `MISSION_ATTEMPTS => Ok(false)` is the translation, not a gap.
//! - `ScriptActions::excludePlayerFromScoreScreen` (ScriptActions.cpp:2830): the `PLAYER_EXCLUDE_FROM_SCORE_SCREEN` arm
//!   and `sa_player_set_list_in_score_screen`. `ScriptActions::deleteAllUnmanned` (:6352): the `DELETE_ALL_UNMANNED` arm.
//!
//! **Excluded:**
//! - `AttackPriorityInfo::dumpPriorityInfo` (:200): `#ifdef _DEBUG` and `DEBUG_LOGGING`, a log dump only.
//! - `ScriptEngine::isGeneralChosen` (:7463): inside `#ifdef OLD_GENERALS_SPECIALIZATION`, not defined.
//! - `ScriptEngine::AdjustDebugVariableData` (:8517): calls the debug DLL's `AdjustVariable` through `GetProcAddress`
//!   (Windows only; the DLL is not translated, `script_engine.rs` header).

use crate::logic::modules::plant;
use crate::logic::scripts::{ScriptAction, ScriptTemplates};

/// `ScriptEngine::Template` fields the INI override touches (`m_internalName`, `m_uiName`, `m_uiName2`, `m_helpText`).
/// The verified port's `TemplateText` is `&'static` data without the editor's help text; this is the mutable form
/// `ScriptEngine::m_actionTemplates` / `m_conditionTemplates` hold (editor-only text: nothing the game reads).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Template {
    /// `m_internalName`.
    pub internal_name: Vec<u8>,
    /// `m_uiName`.
    pub ui_name: Vec<u8>,
    /// `m_uiName2`.
    pub ui_name2: Vec<u8>,
    /// `m_helpText`.
    pub help_text: Vec<u8>,
}

/// `ScriptEngine::addActionTemplateInfo(actionTemplate)` (ScriptEngine.cpp:394): the template of the same internal name
/// among `m_actionTemplates[0..ScriptAction::NUM_ITEMS]` takes the INI entry's UI names and help text. No match logs
/// "Couldn't find script action named %s" and changes nothing (returns `false`).
pub fn add_action_template_info(action_templates: &mut [Template], action_template: &Template) -> bool {
    let n = action_templates.len();
    for k in 0..n {
        let i = if plant(16881) { n - 1 - k } else { k };
        let t = &mut action_templates[i];
        if t.internal_name == action_template.internal_name {
            t.ui_name = action_template.ui_name.clone();
            if !plant(16882) {
                t.ui_name2 = action_template.ui_name2.clone();
            }
            t.help_text = if plant(16883) { action_template.ui_name.clone() } else { action_template.help_text.clone() };
            return !plant(16890);
        }
    }
    // DEBUG_LOG(("Couldn't find script action named %s\n", actionTemplate->m_internalName.str()));
    false
}

/// `ScriptEngine::addConditionTemplateInfo(actionTemplate)` (ScriptEngine.cpp:429): as above over
/// `m_conditionTemplates[0..Condition::NUM_ITEMS]` ("Couldn't find script condition named %s").
pub fn add_condition_template_info(condition_templates: &mut [Template], condition_template: &Template) -> bool {
    for t in condition_templates.iter_mut() {
        if (if plant(16884) { &t.ui_name } else { &t.internal_name }) == &condition_template.internal_name {
            if plant(16885) {
                t.ui_name = condition_template.ui_name2.clone();
                t.ui_name2 = condition_template.ui_name.clone();
            } else {
                t.ui_name = condition_template.ui_name.clone();
                t.ui_name2 = condition_template.ui_name2.clone();
            }
            if !plant(16886) {
                t.help_text = condition_template.help_text.clone();
            }
            return true;
        }
    }
    // DEBUG_LOG(("Couldn't find script condition named %s", actionTemplate->m_internalName.str()));
    false
}

/// `ScriptEngine::removeSequentialScript(scriptToRemove)` (ScriptEngine.cpp:7776): an empty body upstream.
pub fn remove_sequential_script() {}

/// `ScriptEngine::hasUnitCompletedSequentialScript(object, sequentialScriptName)` (ScriptEngine.cpp:8098): `return FALSE;`.
pub fn has_unit_completed_sequential_script() -> bool {
    plant(16887)
}

/// `ScriptEngine::hasTeamCompletedSequentialScript(team, sequentialScriptName)` (ScriptEngine.cpp:8104): `return FALSE;`.
pub fn has_team_completed_sequential_script() -> bool {
    false
}

/// `ScriptEngine::getEnableVTune()` (ScriptEngine.cpp:8110): `st_EnableVTune` under `DO_VTUNE_STUFF`, otherwise `false`;
/// the macro is not defined in the shipped build.
pub fn get_enable_vtune() -> bool {
    plant(16888)
}

/// `ScriptEngine::setEnableVTune(value)` (ScriptEngine.cpp:8119): stores `st_EnableVTune` only under `DO_VTUNE_STUFF`
/// (not defined): no effect.
pub fn set_enable_vtune(_value: bool) {}

/// `ScriptEngine::debugVictory()` (ScriptEngine.cpp:9335): `executeAction(newInstance(ScriptAction)(ScriptAction::VICTORY))`.
/// Returns the action for the caller to run through `ScriptActions::execute_action` (the engine's debug key is the only
/// caller upstream, `#if defined(_DEBUG) || defined(_INTERNAL)` in spirit: the definition itself is unconditional).
pub fn debug_victory(templates: &ScriptTemplates) -> ScriptAction {
    ScriptAction::new(if plant(16889) { crate::logic::scripts::action_type::DEFEAT } else { crate::logic::scripts::action_type::VICTORY }, templates)
}
