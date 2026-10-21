// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpecialPowerModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/SpecialPowerModule.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp (parseSpecialPowerTemplate,
//   parseAudioEventRTS: the token handling)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, July 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpecialPowerModule` and `SpecialPowerModuleData`: the module that holds a
//! special power's recharge state (when it is next ready, whether it is
//! paused) and starts the power; the concrete powers (`OCLSpecialPower`,
//! `CashHackSpecialPower`, ...) derive from it in layer 9.
//!
//! # The recharge state
//!
//! Four numbers: `m_availableOnFrame` (the frame the power is ready on),
//! `m_pausedCount` ("reference count of sources pausing me": scripts, upgrade
//! modules), `m_pausedOnFrame` and `m_pausedPercent` (the fraction charged when
//! the first pause began). While paused the power is never ready, and on the
//! last unpause the time spent paused is added to `m_availableOnFrame`. A
//! power whose template is `SharedSynced` (the Generals powers, shared between
//! command centers) keeps its ready frame in the **player** instead:
//! `Player::getOrStartSpecialPowerReadyFrame(template)`.
//!
//! # Cut points (layer 7/8)
//!
//! Every function here that upstream writes with `TheGameLogic->getFrame()`,
//! `getObject()->getControllingPlayer()`, `getObject()->isDisabled()` and the
//! template's two flags takes them in a [`SpecialPowerContext`] (the current
//! frame, the template's reload time and shared flag, whether the object has
//! a controlling player, whether it is disabled), and a `shared_ready_frame`
//! closure that asks the player's `getOrStartSpecialPowerReadyFrame`, called
//! exactly where upstream calls it (it has a side effect: it starts the
//! shared timer).
//!
//! Left out because they only reach the object, player, UI, audio, script
//! engine and thing factory:
//!
//! - the constructor's work after the four zeros (the initial
//!   `startPowerRecharge`, `pauseCountdown(TRUE)` for `StartsPaused`, and
//!   registering a public timer with `TheInGameUI->addSuperweapon`), the
//!   destructor's `removeSuperweapon`, and `loadPostProcess`'s
//!   `addSuperweapon`: [`SpecialPowerModule::new`] makes the zeroed state and
//!   the caller does the rest with the functions below;
//! - `onSpecialPowerCreation` (a recharge, the player's
//!   `expressSpecialPowerReadyFrame`, `StartsPaused`, the UI);
//! - `initiateIntentToDoSpecialPower` (walks the object's behavior modules
//!   looking for `SpecialPowerUpdateInterface`s, and records academy stats),
//!   `triggerSpecialPower`, `aboutToDoSpecialPower` (script engine, EVA
//!   voices, the initiate sounds), `createViewObject` (thing factory) and
//!   the four `doSpecialPower...` entry points: of those, the gate every one
//!   starts with ([`SpecialPowerModule::do_special_power_is_blocked`]) and
//!   the choice of whether the power triggers at once
//!   ([`SpecialPowerModuleData::triggers_immediately`]) are translated;
//! - `syncReadyFrameToStatusQuo` is declared and never defined upstream.
//!
//! The `_DEBUG` / `_INTERNAL` / `_ALLOW_DEBUG_CHEATS_IN_RELEASE` blocks
//! (`TheGlobalData->m_specialPowerUsesDelay == FALSE`: "this is a cheat ...
//! remove this for release!") are compiled out of the shipped build
//! (`DONT_ALLOW_DEBUG_CHEATS_IN_RELEASE`), and so are they here.
//!
//! # Two overloads of `getReadyFrame`
//!
//! `getReadyFrame() const` (the interface's) adds the time paused or disabled
//! to the ready frame; the non-`const` overload next to it, "used by player
//! to keep recharge timers in sync", returns `m_availableOnFrame` as it is.
//! Which one runs depends on the constness of the pointer at the call site.
//! Both are here: [`SpecialPowerModule::get_ready_frame`] and
//! [`SpecialPowerModule::get_available_on_frame`].
//!
//! # Where upstream is undefined
//!
//! A template that is null (`SpecialPowerTemplate None`, or a name that was
//! not found) with a controlling player: `isReady` asks it `isSharedNSync()`
//! ([`Undefined::NullSpecialPowerTemplate`], raised by
//! [`SpecialPowerContext::new`]). A division by a reload time of 0 in
//! `getPercentReady` gives an infinity (IEEE; undefined in ISO C++).

use std::sync::Arc;

use crate::common::audio_event_name::AudioEventName;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_SPECIAL_POWER};
use crate::common::special_power::SpecialPowerStore;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};

/// `class SpecialPowerModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpecialPowerModuleData {
    /// The `BehaviorModuleData` base.
    pub behavior: BehaviorModuleData,
    /// `m_specialPowerTemplate`: "pointer to the special power template",
    /// as the template's id (`SpecialPowerTemplate::getID`, unique); `None`
    /// for a null pointer.
    pub special_power_template: Option<u32>,
    // --- p1w11-sonnet ---
    /// The template field as the name that was written, `None` if the field was never read. A module made by the module factory reads its data
    /// with no store to look the name up in ([`SpecialPowerModuleData::friend_new_module_data_deferred`]); the concrete module finds the template
    /// by this name when it needs it, in `world.statics.special_power_store`.
    pub special_power_template_name: Option<Vec<u8>>,
    // --- end p1w11-sonnet ---
    /// `m_initiateSound`.
    pub initiate_sound: AudioEventName,
    /// `m_updateModuleStartsAttack`: "update module determines when the
    /// special power actually starts! If true, update module is required."
    pub update_module_starts_attack: bool,
    /// `m_startsPaused`: "paused on creation, someone else will have to
    /// unpause (like upgrade module, or script)".
    pub starts_paused: bool,
    /// `m_scriptedSpecialPowerOnly`.
    pub scripted_special_power_only: bool,
}

/// The state `SpecialPowerModuleData`'s field parsers work on: the data, and
/// `TheSpecialPowerStore` (`None` while it is null).
pub struct SpecialPowerModuleDataCtx<'a> {
    /// The data being read.
    pub data: SpecialPowerModuleData,
    store: Option<&'a SpecialPowerStore>,
    // --- p1w11-sonnet ---
    /// Keep the template's name and look nothing up (the module factory's data proc has no store).
    defer: bool,
    // --- end p1w11-sonnet ---
}

impl SpecialPowerModuleDataCtx<'_> {
    /// `INI::parseSpecialPowerTemplate`: the next token names a template in
    /// the store (`None` stores a null pointer; an unknown name does too, a
    /// `DEBUG_CRASH` upstream that is compiled out). `ERROR_BUG` while
    /// `TheSpecialPowerStore` is null.
    fn parse_special_power_template(ini: &mut Ini<'_>, c: &mut SpecialPowerModuleDataCtx<'_>, _: &UserData) -> IniResult<()> {
        let token = ini.get_next_token(None)?;
        // --- p1w11-sonnet --- (the name is kept either way)
        c.data.special_power_template_name = Some(token.clone());
        if c.defer && c.store.is_none() {
            return Ok(());
        }
        // --- end p1w11-sonnet ---
        let Some(store) = c.store else {
            return Err(IniError::new(IniErrorKind::Bug));
        };
        c.data.special_power_template = store.find_special_power_template(&token).map(|t| t.id());
        Ok(())
    }
    fn parse_update_module_starts_attack(ini: &mut Ini<'_>, c: &mut SpecialPowerModuleDataCtx<'_>, _: &UserData) -> IniResult<()> {
        c.data.update_module_starts_attack = ini.parse_bool()?;
        Ok(())
    }
    fn parse_starts_paused(ini: &mut Ini<'_>, c: &mut SpecialPowerModuleDataCtx<'_>, _: &UserData) -> IniResult<()> {
        c.data.starts_paused = ini.parse_bool()?;
        Ok(())
    }
    fn parse_initiate_sound(ini: &mut Ini<'_>, c: &mut SpecialPowerModuleDataCtx<'_>, _: &UserData) -> IniResult<()> {
        c.data.initiate_sound.parse(ini)
    }
    fn parse_scripted_special_power_only(ini: &mut Ini<'_>, c: &mut SpecialPowerModuleDataCtx<'_>, _: &UserData) -> IniResult<()> {
        c.data.scripted_special_power_only = ini.parse_bool()?;
        Ok(())
    }
}

impl SpecialPowerModuleData {
    /// `SpecialPowerModuleData()`: no template, no sound, all false.
    pub fn new() -> Self {
        Self::default()
    }

    /// After the update modules were told, "only trigger the special power
    /// immediately if the updatemodule doesn't start the attack": the
    /// condition `if( !getSpecialPowerModuleData()->m_updateModuleStartsAttack )`
    /// of the four `doSpecialPower...` functions.
    pub fn triggers_immediately(&self) -> bool {
        !self.update_module_starts_attack
    }

    /// `SpecialPowerModuleData::buildFieldParse(p)` is the base's plus
    /// `SpecialPowerTemplate`, `UpdateModuleStartsAttack`, `StartsPaused`,
    /// `InitiateSound`, `ScriptedSpecialPowerOnly`; here the four-step
    /// equivalent is [`SpecialPowerModuleData::friend_new_module_data`], which
    /// builds the list over a [`SpecialPowerModuleDataCtx`].
    ///
    /// `SpecialPowerModule::friend_newModuleData(ini)`: new data, read with
    /// `initFromINIMultiProc(data, buildFieldParse)` if there is an `ini`.
    /// `store` is `TheSpecialPowerStore` (`None` while null).
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>, store: Option<&SpecialPowerStore>) -> IniResult<SpecialPowerModuleData> {
        Self::friend_new_module_data_with(ini, store, false)
    }

    // --- p1w11-sonnet ---
    /// [`friend_new_module_data`](Self::friend_new_module_data) as the module factory's data proc must make it: no store, so the template field keeps
    /// its name ([`SpecialPowerModuleData::special_power_template_name`]) and looks nothing up.
    pub fn friend_new_module_data_deferred(ini: Option<&mut Ini<'_>>) -> IniResult<SpecialPowerModuleData> {
        Self::friend_new_module_data_with(ini, None, true)
    }

    fn friend_new_module_data_with(ini: Option<&mut Ini<'_>>, store: Option<&SpecialPowerStore>, defer: bool) -> IniResult<SpecialPowerModuleData> {
        let mut ctx = SpecialPowerModuleDataCtx { data: SpecialPowerModuleData::new(), store, defer };
        if let Some(ini) = ini {
            let fields = [
                FieldParse::new("SpecialPowerTemplate", SpecialPowerModuleDataCtx::parse_special_power_template, UserData::None),
                FieldParse::new("UpdateModuleStartsAttack", SpecialPowerModuleDataCtx::parse_update_module_starts_attack, UserData::None),
                FieldParse::new("StartsPaused", SpecialPowerModuleDataCtx::parse_starts_paused, UserData::None),
                FieldParse::new("InitiateSound", SpecialPowerModuleDataCtx::parse_initiate_sound, UserData::None),
                FieldParse::new("ScriptedSpecialPowerOnly", SpecialPowerModuleDataCtx::parse_scripted_special_power_only, UserData::None),
                FieldParse::end(),
            ];
            let mut p = MultiIniFieldParse::new();
            BehaviorModuleData::build_field_parse(&mut p)?;
            p.add(&fields)?;
            ini.init_from_ini_multi(&mut ctx, &p)?;
        }
        Ok(ctx.data)
    }
}

/// The two flags and the reload time of a `SpecialPowerTemplate` that the
/// module reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialPowerTiming {
    /// `getReloadTime()`: frames.
    pub reload_time: u32,
    /// `isSharedNSync()`.
    pub is_shared_n_sync: bool,
}

/// What a `SpecialPowerModule` function reads off the world (cut points: see
/// the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialPowerContext {
    /// `TheGameLogic->getFrame()`.
    now: u32,
    /// The data's template, as the two numbers read off it; `None` for a null
    /// template.
    template: Option<SpecialPowerTiming>,
    /// `getObject()->getControllingPlayer()` is not null.
    has_controlling_player: bool,
    /// `getObject()->isDisabled()`.
    is_disabled: bool,
}

impl SpecialPowerContext {
    /// A context. [`Undefined::NullSpecialPowerTemplate`] for a null template
    /// with a controlling player (upstream calls `isSharedNSync()` on it).
    pub fn new(now: u32, template: Option<SpecialPowerTiming>, has_controlling_player: bool, is_disabled: bool) -> IniResult<Self> {
        if template.is_none() && has_controlling_player {
            return Err(IniError::undefined(Undefined::NullSpecialPowerTemplate));
        }
        Ok(SpecialPowerContext { now, template, has_controlling_player, is_disabled })
    }

    /// `TheGameLogic->getFrame()`.
    pub fn now(&self) -> u32 {
        self.now
    }

    /// Whether the ready frame is the player's: a controlling player and a
    /// shared template.
    pub fn uses_player_ready_frame(&self) -> bool {
        self.has_controlling_player && matches!(self.template, Some(t) if t.is_shared_n_sync)
    }
}

/// What `startPowerRecharge` leaves to the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RechargeAction {
    /// Nothing was done: no template (`DEBUG_CRASH("special power not
    /// found")`), no object, or no controlling player.
    Nothing,
    /// The template is shared: the caller calls the player's
    /// `resetOrStartSpecialPowerReadyFrame(template)`; the module's own frame
    /// is unchanged.
    ResetPlayerReadyFrame,
    /// `m_availableOnFrame` was set to now plus the reload time.
    SetOwnReadyFrame,
}

/// `class SpecialPowerModule : public BehaviorModule, public SpecialPowerModuleInterface`.
#[derive(Clone, Debug)]
pub struct SpecialPowerModule {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// The typed module data (`getSpecialPowerModuleData()`).
    data: Arc<SpecialPowerModuleData>,
    /// `m_availableOnFrame`: "on this frame, this special power is available".
    available_on_frame: u32,
    /// `m_pausedCount`: "reference count of sources pausing me".
    paused_count: i32,
    /// `m_pausedOnFrame`.
    paused_on_frame: u32,
    /// `m_pausedPercent`.
    paused_percent: f32,
}

impl SpecialPowerModule {
    /// `SpecialPowerModule(thing, moduleData)`, as far as it is the module's
    /// own state (see the module docs): ready on frame 0, not paused.
    /// `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<SpecialPowerModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let behavior = BehaviorModule::new(object, Some(ModuleDataKind::SpecialPower(data.clone())))?;
        Ok(SpecialPowerModule { behavior, data, available_on_frame: 0, paused_count: 0, paused_on_frame: 0, paused_percent: 0.0 })
    }

    /// `SpecialPowerModule::getInterfaceMask()`: `MODULEINTERFACE_SPECIAL_POWER`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_SPECIAL_POWER
    }

    /// The `BehaviorModule` base.
    pub fn behavior(&self) -> &BehaviorModule {
        &self.behavior
    }

    /// `getSpecialPowerModuleData()`.
    pub fn special_power_module_data(&self) -> &Arc<SpecialPowerModuleData> {
        &self.data
    }

    /// `m_pausedCount`.
    pub fn paused_count(&self) -> i32 {
        self.paused_count
    }

    /// `m_pausedOnFrame`.
    pub fn paused_on_frame(&self) -> u32 {
        self.paused_on_frame
    }

    /// `m_pausedPercent`.
    pub fn paused_percent(&self) -> f32 {
        self.paused_percent
    }

    /// `isModuleForPower(specialPowerTemplate)`: whether the data's template
    /// is that one (compared by id; `None` is a null pointer, which matches a
    /// null argument).
    pub fn is_module_for_power(&self, special_power_template: Option<u32>) -> bool {
        self.data.special_power_template == special_power_template
    }

    /// `isScriptOnly()`: "a special power module that is only supposed to be
    /// fired via scripts" (the cargo planes).
    pub fn is_script_only(&self) -> bool {
        self.data.scripted_special_power_only
    }

    /// `isReady()`: with a shared template and a player, the player's ready
    /// frame has come (`shared_ready_frame` is that player call); else not
    /// paused and the module's frame has come.
    pub fn is_ready(&self, ctx: &SpecialPowerContext, mut shared_ready_frame: impl FnMut() -> u32) -> bool {
        if ctx.uses_player_ready_frame() {
            return ctx.now >= shared_ready_frame();
        }
        self.paused_count == 0 && ctx.now >= self.available_on_frame
    }

    /// `getPercentReady()`: "1.0 = ready now, 0.5 = half charged up etc."
    /// (0.99999 while paused with a full charge, the percent at the pause
    /// while paused, 0 with no template).
    pub fn get_percent_ready(&self, ctx: &SpecialPowerContext, mut shared_ready_frame: impl FnMut() -> u32) -> f32 {
        if self.paused_count > 0 && self.paused_percent == 1.0 {
            // "Don't consider it ready if paused."
            return 0.99999;
        }

        // easy case ... is ready
        if self.is_ready(ctx, &mut shared_ready_frame) {
            return 1.0;
        }

        if self.paused_count > 0 {
            return self.paused_percent;
        }

        // sanity
        let Some(template) = ctx.template else {
            return 0.0;
        };

        let mut ready_frame = self.available_on_frame;

        // unless
        if ctx.uses_player_ready_frame() {
            ready_frame = shared_ready_frame();
        }

        // calculate the percent
        1.0 - (ready_frame.wrapping_sub(ctx.now) as f32 / template.reload_time as f32)
    }

    /// `getReadyFrame() const`: the player's ready frame for a shared
    /// template; else, while paused or disabled, the module's frame plus the
    /// time since the pause began (or since `setReadyFrame`); else the
    /// module's frame.
    pub fn get_ready_frame(&self, ctx: &SpecialPowerContext, mut shared_ready_frame: impl FnMut() -> u32) -> u32 {
        if ctx.uses_player_ready_frame() {
            return shared_ready_frame();
        }
        if self.paused_count > 0 || ctx.is_disabled {
            let paused_frames = ctx.now.wrapping_sub(self.paused_on_frame) as i32;
            self.available_on_frame.wrapping_add(paused_frames as u32)
        } else {
            self.available_on_frame
        }
    }

    /// The non-`const` overload `getReadyFrame()`: `m_availableOnFrame`
    /// ("used by player to keep recharge timers in sync"; see the module
    /// docs).
    pub fn get_available_on_frame(&self) -> u32 {
        self.available_on_frame
    }

    // --- p1w11-sonnet ---
    /// `m_availableOnFrame = frame` alone (`onSpecialPowerCreation` copies the player's shared ready frame into it).
    pub fn set_available_on_frame(&mut self, frame: u32) {
        self.available_on_frame = frame;
    }
    // --- end p1w11-sonnet ---

    /// `setReadyFrame(frame)` ("for use by the scripting engine only"): the
    /// frame, and "if a script should change the ready frame, we need to
    /// update the paused frame" to now.
    pub fn set_ready_frame(&mut self, frame: u32, now: u32) {
        self.available_on_frame = frame;
        self.paused_on_frame = now;
    }

    /// `pauseCountdown(pause)`: the first pause records the frame and the
    /// percent charged; each pause counts; an unpause of a paused module
    /// counts down, and the last one adds the time spent paused to the ready
    /// frame ("so multiple unpauses don't break").
    pub fn pause_countdown(&mut self, pause: bool, ctx: &SpecialPowerContext, shared_ready_frame: impl FnMut() -> u32) {
        if pause {
            if self.paused_count == 0 {
                // "Only record this with the first pausing, otherwise upon final unpausing you
                // would get credited the time between pauses as time served."
                self.paused_on_frame = ctx.now;
                self.paused_percent = self.get_percent_ready(ctx, shared_ready_frame);
            }
            // (an `Int` that upstream overflows only after 2^31 pauses)
            self.paused_count = self.paused_count.wrapping_add(1);
        } else if self.paused_count > 0 {
            self.paused_count -= 1;

            // "And only update the ready time if we are fully unpaused now."
            if self.paused_count == 0 {
                self.available_on_frame = self.available_on_frame.wrapping_add(ctx.now.wrapping_sub(self.paused_on_frame));
            }
        }
    }

    /// `startPowerRecharge()`: "a special power has been used ... start the
    /// recharge process by computing the frame we will become fully available
    /// on in the future again". With no template, or no controlling player,
    /// nothing; a shared template leaves the player to reset its frame;
    /// otherwise `m_availableOnFrame = now + reload time`.
    pub fn start_power_recharge(&mut self, ctx: &SpecialPowerContext) -> RechargeAction {
        let Some(template) = ctx.template else {
            return RechargeAction::Nothing;
        };
        if !ctx.has_controlling_player {
            return RechargeAction::Nothing;
        }
        // "Here, we make sure that general specials work as one between command centers"
        if template.is_shared_n_sync {
            RechargeAction::ResetPlayerReadyFrame
        } else {
            // "set the frame we will be 100% available on now"
            self.available_on_frame = ctx.now.wrapping_add(template.reload_time);
            RechargeAction::SetOwnReadyFrame
        }
    }

    /// The gate each of the four `doSpecialPower...` functions starts with:
    /// `if (m_pausedCount > 0 || getObject()->isDisabled()) return;`.
    pub fn do_special_power_is_blocked(&self, ctx: &SpecialPowerContext) -> bool {
        self.paused_count > 0 || ctx.is_disabled
    }
}

impl Snapshot for SpecialPowerModule {
    /// `SpecialPowerModule::crc`: `BehaviorModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)
    }
    /// `SpecialPowerModule::xfer` (version 1): the version,
    /// `BehaviorModule::xfer`, `m_availableOnFrame`, `m_pausedCount`,
    /// `m_pausedOnFrame`, `m_pausedPercent`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)?;
        xfer.xfer_unsigned_int(&mut self.available_on_frame)?;
        xfer.xfer_int(&mut self.paused_count)?;
        xfer.xfer_unsigned_int(&mut self.paused_on_frame)?;
        xfer.xfer_real(&mut self.paused_percent)
    }
    /// `SpecialPowerModule::loadPostProcess`: `BehaviorModule::loadPostProcess`
    /// (the UI registration after it is a cut point, see the module docs).
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::common::name_key_generator::NameKeyGenerator;
    use crate::logic::module::test_support::{load_bytes, save_bytes};

    fn module(text: &str) -> SpecialPowerModule {
        SpecialPowerModule::new(1, Some(Arc::new(data_of(text, None).unwrap_or_default()))).unwrap()
    }

    fn data_of(text: &str, store: Option<&SpecialPowerStore>) -> IniResult<SpecialPowerModuleData> {
        let text = format!("D\n{text}\nEnd\n");
        let mut ini_out: Option<IniResult<SpecialPowerModuleData>> = None;
        struct C<'a> {
            store: Option<&'a SpecialPowerStore>,
            out: &'a mut Option<IniResult<SpecialPowerModuleData>>,
        }
        fn block(ini: &mut Ini<'_>, c: &mut C<'_>) -> IniResult<()> {
            *c.out = Some(SpecialPowerModuleData::friend_new_module_data(Some(ini), c.store));
            Ok(())
        }
        // the block table is built per call: its handler's context borrows the store
        fn run<'a>(text: &str, store: Option<&'a SpecialPowerStore>, out: &'a mut Option<IniResult<SpecialPowerModuleData>>) {
            let blocks: [BlockParse<C<'a>>; 1] = [BlockParse { token: "D", parse: block }];
            let mut c = C { store, out };
            let _ = Ini::load(b"s.ini", text.as_bytes(), IniLoadType::Overwrite, None, &blocks, &mut c);
        }
        run(&text, store, &mut ini_out);
        ini_out.expect("block handler ran")
    }

    fn clock(now: u32, reload: u32, shared: bool, player: bool, disabled: bool) -> SpecialPowerContext {
        SpecialPowerContext::new(now, Some(SpecialPowerTiming { reload_time: reload, is_shared_n_sync: shared }), player, disabled).unwrap()
    }

    #[test]
    fn the_four_fields_without_a_template_line() {
        let d = data_of("StartsPaused = Yes\n UpdateModuleStartsAttack = Yes\n ScriptedSpecialPowerOnly = No\n InitiateSound = Snd_Go", None).unwrap();
        assert!(d.starts_paused && d.update_module_starts_attack && !d.scripted_special_power_only);
        assert_eq!(d.initiate_sound.event_name, b"Snd_Go");
        assert!(d.special_power_template.is_none());
        assert!(!d.triggers_immediately());
        assert!(SpecialPowerModuleData::new().triggers_immediately());
        let d = data_of("InitiateSound = NoSound", None).unwrap();
        assert!(d.initiate_sound.event_name.is_empty());
        assert!(data_of("StartsPaused = maybe", None).is_err());
    }

    #[test]
    fn the_template_field_needs_the_store() {
        // no store: ERROR_BUG, even for None
        let e = data_of("SpecialPowerTemplate = None", None).unwrap_err();
        assert_eq!(e.kind, IniErrorKind::Bug);
        let store = SpecialPowerStore::new();
        let d = data_of("SpecialPowerTemplate = None\n SpecialPowerTemplate = Missing", Some(&store)).unwrap();
        assert_eq!(d.special_power_template, None, "an unknown name is a null pointer");
        // a template in the store is found by its id
        let mut store = SpecialPowerStore::new();
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut ini_ok = false;
        fn def(ini: &mut Ini<'_>, c: &mut (SpecialPowerStore, NameKeyGenerator)) -> IniResult<()> {
            c.0.parse_special_power_definition(ini, &mut c.1, &|_| true)
        }
        let blocks = [BlockParse { token: "SpecialPower", parse: def }];
        let mut c = (std::mem::take(&mut store), names);
        Ini::load(b"p.ini", b"SpecialPower SuperA\nEnd\nSpecialPower SuperB\nEnd\n", IniLoadType::Overwrite, None, &blocks, &mut c).unwrap();
        ini_ok |= c.0.len() == 2;
        assert!(ini_ok);
        let id_b = c.0.find_special_power_template(b"SuperB").unwrap().id();
        let d = data_of("SpecialPowerTemplate = SuperB", Some(&c.0)).unwrap();
        assert_eq!(d.special_power_template, Some(id_b));
        let m = SpecialPowerModule::new(1, Some(Arc::new(d))).unwrap();
        assert!(m.is_module_for_power(Some(id_b)) && !m.is_module_for_power(Some(id_b + 1)) && !m.is_module_for_power(None));
    }

    #[test]
    fn ready_when_not_paused_and_the_frame_has_come() {
        let mut m = module("");
        let c = clock(100, 300, false, true, false);
        assert!(m.is_ready(&c, || panic!("not shared")), "ready on frame 0");
        assert_eq!(m.start_power_recharge(&c), RechargeAction::SetOwnReadyFrame);
        assert_eq!(m.get_available_on_frame(), 400);
        assert!(!m.is_ready(&c, || unreachable!()));
        assert!(m.is_ready(&clock(400, 300, false, true, false), || unreachable!()));
        assert_eq!(m.get_percent_ready(&c, || unreachable!()), 1.0 - 300.0 / 300.0);
        assert_eq!(m.get_percent_ready(&clock(250, 300, false, true, false), || unreachable!()), 1.0 - 150.0 / 300.0);
        assert_eq!(m.get_percent_ready(&clock(400, 300, false, true, false), || unreachable!()), 1.0);
        assert_eq!(m.get_ready_frame(&c, || unreachable!()), 400);
        // disabled: the time since the pause began (0 here) is added
        assert_eq!(m.get_ready_frame(&clock(130, 300, false, true, true), || unreachable!()), 400 + 130);
    }

    #[test]
    fn the_recharge_has_nothing_to_do_without_a_template_or_a_player() {
        let mut m = module("");
        let none = SpecialPowerContext::new(10, None, false, false).unwrap();
        assert_eq!(m.start_power_recharge(&none), RechargeAction::Nothing);
        assert_eq!(m.start_power_recharge(&clock(10, 5, false, false, false)), RechargeAction::Nothing);
        assert_eq!(m.get_available_on_frame(), 0);
        assert_eq!(m.get_percent_ready(&none, || unreachable!()), 1.0, "ready at frame 0, and 1.0 is the first answer");
        // a null template with a player is the undefined case
        let e = SpecialPowerContext::new(10, None, true, false).unwrap_err();
        assert_eq!(e.kind, IniErrorKind::Undefined(Undefined::NullSpecialPowerTemplate));
    }

    #[test]
    fn a_shared_template_uses_the_players_frame() {
        let mut m = module("");
        let c = clock(100, 300, true, true, false);
        assert!(c.uses_player_ready_frame());
        assert_eq!(m.start_power_recharge(&c), RechargeAction::ResetPlayerReadyFrame);
        assert_eq!(m.get_available_on_frame(), 0, "the module's own frame is not touched");
        let mut asked = 0;
        assert!(!m.is_ready(&c, || {
            asked += 1;
            150
        }));
        assert!(m.is_ready(&c, || {
            asked += 1;
            100
        }));
        assert_eq!(asked, 2);
        assert_eq!(m.get_ready_frame(&c, || 777), 777);
        // percent: not ready (asks twice: isReady, then the frame)
        let mut asked = 0;
        let p = m.get_percent_ready(&clock(100, 400, true, true, false), || {
            asked += 1;
            300
        });
        assert_eq!(asked, 2);
        assert_eq!(p, 1.0 - (200.0f32 / 400.0));
        // without a player the template's flag does not matter
        let nobody = clock(100, 300, true, false, false);
        assert!(!nobody.uses_player_ready_frame());
        assert!(m.is_ready(&nobody, || unreachable!()));
    }

    #[test]
    fn pausing_stops_the_clock_and_unpausing_adds_the_time_back() {
        let mut m = module("");
        let c0 = clock(0, 300, false, true, false);
        m.start_power_recharge(&c0);
        // pause at frame 100 (1/3 charged)
        let c = clock(100, 300, false, true, false);
        m.pause_countdown(true, &c, || unreachable!());
        assert_eq!((m.paused_count(), m.paused_on_frame()), (1, 100));
        assert_eq!(m.paused_percent(), 1.0 - 200.0 / 300.0);
        // a second pause does not move the record
        m.pause_countdown(true, &clock(150, 300, false, true, false), || unreachable!());
        assert_eq!((m.paused_count(), m.paused_on_frame()), (2, 100));
        let paused = clock(500, 300, false, true, false);
        assert!(!m.is_ready(&paused, || unreachable!()), "never ready while paused");
        assert_eq!(m.get_percent_ready(&paused, || unreachable!()), m.paused_percent());
        assert_eq!(m.get_ready_frame(&paused, || unreachable!()), 300 + 400);
        // unpause once: still paused; twice: the 400 frames are added
        m.pause_countdown(false, &paused, || unreachable!());
        assert_eq!((m.paused_count(), m.get_available_on_frame()), (1, 300));
        m.pause_countdown(false, &paused, || unreachable!());
        assert_eq!((m.paused_count(), m.get_available_on_frame()), (0, 700));
        // unpausing an unpaused module does nothing
        m.pause_countdown(false, &paused, || unreachable!());
        assert_eq!((m.paused_count(), m.get_available_on_frame()), (0, 700));
    }

    #[test]
    fn a_fully_charged_power_that_is_paused_is_not_quite_ready() {
        let mut m = module("");
        // ready on frame 0, so the charge at frame 50 is 1.0 ("isReady")
        m.pause_countdown(true, &clock(50, 300, false, true, false), || unreachable!());
        assert_eq!(m.paused_percent(), 1.0);
        assert_eq!(m.get_percent_ready(&clock(60, 300, false, true, false), || unreachable!()), 0.99999);
    }

    #[test]
    fn set_ready_frame_records_the_frame_it_was_set() {
        let mut m = module("");
        m.set_ready_frame(900, 123);
        assert_eq!((m.get_available_on_frame(), m.paused_on_frame()), (900, 123));
        // the const getReadyFrame adds frames since then while disabled
        assert_eq!(m.get_ready_frame(&clock(200, 300, false, true, true), || unreachable!()), 900 + 77);
    }

    #[test]
    fn the_do_special_power_gate() {
        let mut m = module("UpdateModuleStartsAttack = Yes");
        assert!(!m.do_special_power_is_blocked(&clock(1, 1, false, true, false)));
        assert!(m.do_special_power_is_blocked(&clock(1, 1, false, true, true)));
        m.pause_countdown(true, &clock(1, 1, false, true, false), || unreachable!());
        assert!(m.do_special_power_is_blocked(&clock(1, 1, false, true, false)));
        assert!(!m.special_power_module_data().triggers_immediately());
        assert!(!module("ScriptedSpecialPowerOnly = No").is_script_only() && module("ScriptedSpecialPowerOnly = Yes").is_script_only());
    }

    #[test]
    fn xfer_saves_the_four_numbers() {
        let mut m = module("");
        m.set_ready_frame(0x0102_0304, 0x0a0b_0c0d);
        m.pause_countdown(true, &clock(0x0a0b_0c0d, 300, false, true, false), || unreachable!());
        let bytes = save_bytes(&mut m).unwrap();
        let mut want = vec![1, 1, 1, 1];
        want.extend_from_slice(&0x0102_0304u32.to_le_bytes());
        want.extend_from_slice(&1i32.to_le_bytes());
        want.extend_from_slice(&0x0a0b_0c0du32.to_le_bytes());
        want.extend_from_slice(&m.paused_percent().to_le_bytes());
        assert_eq!(bytes, want);
        let mut other = module("");
        load_bytes(&mut other, bytes).unwrap();
        assert_eq!((other.get_available_on_frame(), other.paused_count(), other.paused_on_frame()), (0x0102_0304, 1, 0x0a0b_0c0d));
        assert_eq!(SpecialPowerModule::get_interface_mask(), 0x100);
        assert!(SpecialPowerModule::new(1, None).is_err());
    }
}
