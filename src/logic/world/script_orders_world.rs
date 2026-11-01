// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AI.h (the `ai*` command builders) and Source/GameLogic/ScriptEngine/
//     ScriptActions.cpp (its orders to units, groups and players, as the World carries them)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! Integration batch I8c: the script actions' orders, live. A unit's order is the `AICommandParms` its `ai*` builder makes
//! (`AI.h`), given to its AI module (`ModuleCall::AiDoCommandParms`); a group's is its `AIGroup` member, the group checked out
//! of the World's list; a player's goes to its `m_ai` through the player's own method (which asks whether it has one). An
//! object without an AI is not ordered (`getAI()` is checked by every script action that orders one).
//!
//! `p1w13-sonnet`, integration batch I8c, 2026-11-01.

use crate::common::game_type::ObjectId;
use crate::logic::ai::ai_commands::AiCommandParms;
use crate::logic::ai::ai_update_types::CommandSourceType;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleCall;

impl World {
    /// `getAI()->aiDoCommand(&parms)` for the parms the builder of `cmd` makes (`fill` sets its fields).
    pub(crate) fn script_order(&mut self, o: ObjectId, cmd: i32, src: CommandSourceType, fill: impl FnOnce(&mut AiCommandParms)) {
        let mut p = AiCommandParms::new(cmd, src);
        fill(&mut p);
        self.ai_ask(o, ModuleCall::AiDoCommandParms(p));
    }

    /// The group `g`'s `f`, the group checked out of the list (nothing for a group the list has not).
    pub(crate) fn script_group(&mut self, g: i32, f: impl FnOnce(&mut crate::logic::ai::ai_group::AiGroup, &mut World)) {
        if let Ok(g) = u32::try_from(g) {
            self.with_group(g, f);
        }
    }

    /// A player's method of its `m_ai`; its error recorded.
    pub(crate) fn script_player<T, E: std::fmt::Debug>(&mut self, call: &str, r: Result<T, E>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.cuts.team_live_errors.push(format!("{call}: {e:?}"));
                None
            }
        }
    }
}
