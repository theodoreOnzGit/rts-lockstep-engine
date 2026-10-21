// SPDX-License-Identifier: GPL-3.0-only
//! The behaviour half of an update module: an enum of the concrete modules.
//!
//! # Why an enum, and what goes in it
//!
//! Upstream's `UpdateModule::update()` is virtual: the scheduler calls
//! `u->update()` and the concrete class (`LifetimeUpdate`, `AIUpdate`, ...)
//! runs. No trait objects here, so the concrete modules are **variants of
//! [`UpdateBody`]**, and [`UpdateBody::update`] is a `match` that forwards to
//! the variant's own `update`. The layer-9 agent who translates a module adds
//! one variant and one arm (in a `// --- p1wN-name ---` block), and nothing
//! else in the world changes.
//!
//! Every variant's `update` has the same shape,
//! `fn update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime`:
//! it is **checked out** of its object while it runs, so `&mut self` and
//! `&mut World` do not alias, and it reaches its own object through `me` and
//! the world (`world.objects.find_mut(me.object)`), any other object through
//! its id, and the client through `world.events`.
//!
//! # [`ProbeUpdate`]
//!
//! The one variant that exists now is a scripted module with no upstream
//! counterpart. It is the executable form of the worked example in
//! `docs/world-design.md` and what the differential test drives: on each call
//! it performs a list of actions (destroy an object, wake another module,
//! create an object, deal damage through the cut point, emit a client event)
//! and returns a scripted sleep time. It is part of the library (not
//! `#[cfg(test)]`) because integration tests use it. Production code never
//! constructs one.

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::logic::client_event::ClientEvent;
use crate::logic::damage::DamageInfo;
use crate::logic::module::update_module::{
    UpdateModule, UpdateModuleData, UpdateSleepTime, SleepyUpdatePhase,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::object::{BehaviorSlot, Object};
use crate::logic::world::sleepy::UpdateHandle;
// --- p1w6-object ---
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::UPDATE_SLEEP_FOREVER;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::module_probe::ProbeModule;
// --- end p1w6-object ---

/// The concrete update modules. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateBody {
    /// The scripted module of the tests and of `docs/world-design.md`.
    Probe(ProbeUpdate),
    // --- p1w6-object ---
    /// A module with every interface and scripted answers (`module_probe.rs`),
    /// for the dispatch of `Object`. It has no update behaviour.
    ModuleProbe(ProbeModule),
    // --- end p1w6-object ---
    // --- p1l9-example ---
    /// `CommandSetUpgrade` (the lead's layer-9 example).
    CommandSetUpgrade(crate::logic::modules::command_set_upgrade::CommandSetUpgrade),
    // --- end p1l9-example ---
// --- p1l9-upgrades ---
    /// `WeaponSetUpgrade`.
    WeaponSetUpgrade(crate::logic::modules::weapon_set_upgrade::WeaponSetUpgrade),
    /// `WeaponBonusUpgrade`.
    WeaponBonusUpgrade(crate::logic::modules::weapon_bonus_upgrade::WeaponBonusUpgrade),
    /// `ArmorUpgrade`.
    ArmorUpgrade(crate::logic::modules::armor_upgrade::ArmorUpgrade),
    /// `ModelConditionUpgrade`.
    ModelConditionUpgrade(crate::logic::modules::model_condition_upgrade::ModelConditionUpgrade),
    /// `ExperienceScalarUpgrade`.
    ExperienceScalarUpgrade(crate::logic::modules::experience_scalar_upgrade::ExperienceScalarUpgrade),
    /// `ActiveShroudUpgrade`.
    ActiveShroudUpgrade(crate::logic::modules::active_shroud_upgrade::ActiveShroudUpgrade),
    /// `MaxHealthUpgrade`.
    MaxHealthUpgrade(crate::logic::modules::max_health_upgrade::MaxHealthUpgrade),
    /// `GrantScienceUpgrade`.
    GrantScienceUpgrade(crate::logic::modules::grant_science_upgrade::GrantScienceUpgrade),
// --- end p1l9-upgrades ---
    // New variants go here, one per translated module.
}

impl UpdateBody {
    /// `UpdateModuleInterface::update()`: the concrete module's update.
    pub fn update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        match self {
            UpdateBody::Probe(p) => p.update(world, me),
            // --- p1w6-object ---
            UpdateBody::ModuleProbe(_) => UPDATE_SLEEP_FOREVER,
            // --- end p1w6-object ---
            // --- p1l9-example ---
            // not an update module: the scheduler never holds it
            UpdateBody::CommandSetUpgrade(_) => UPDATE_SLEEP_FOREVER,
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::WeaponBonusUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ArmorUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ModelConditionUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ExperienceScalarUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ActiveShroudUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::MaxHealthUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::GrantScienceUpgrade(_) => UPDATE_SLEEP_FOREVER,
// --- end p1l9-upgrades ---
            // --- end p1l9-example ---
        }
    }

    // --- p1w6-object ---
    /// The interface calls of `Object` (see `module_call.rs`): the body's
    /// answer to `call`. A module answers [`ModuleReply::Unhandled`] to a call
    /// it has no behaviour for (upstream's empty virtual).
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match self {
            UpdateBody::Probe(_) => ModuleReply::Unhandled,
            UpdateBody::ModuleProbe(m) => m.handle(world, me, call),
            // --- p1l9-example ---
            UpdateBody::CommandSetUpgrade(m) => m.module_call(world, me, call),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::WeaponBonusUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ArmorUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ModelConditionUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ExperienceScalarUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ActiveShroudUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::MaxHealthUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::GrantScienceUpgrade(m) => m.module_call(world, me, call),
// --- end p1l9-upgrades ---
            // --- end p1l9-example ---
        }
    }
    // --- end p1w6-object ---
}

// --- p1w6-object ---
// --- p1l9-example ---
impl UpdateBody {
    /// Whether the concrete module saves its whole base chain itself, in
    /// upstream's order (every translated layer-9 module), rather than the
    /// slot saving the bases first (the probes). Upstream's
    /// `X::xfer` writes `X`'s version, then calls `UpdateModule::xfer` (the
    /// scheduling half) or `UpgradeModule::xfer`, then `X`'s fields; a slot
    /// that wrote the scheduling half before the module would put the bytes
    /// in the wrong order.
    pub fn owns_base_chain(&self) -> bool {
        !matches!(self, UpdateBody::Probe(_) | UpdateBody::ModuleProbe(_))
    }

    /// The whole chain's `xfer` for a module that [owns it](UpdateBody::owns_base_chain):
    /// `update` is the scheduling half (`UpdateModule`) for an update module,
    /// `None` for any other.
    pub fn xfer_chain(&mut self, update: Option<&mut UpdateModule>, xfer: &mut dyn Xfer) -> XferResult<()> {
        let _ = &update; // used by the update modules' arms
        match self {
            UpdateBody::CommandSetUpgrade(m) => m.xfer(xfer),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.xfer(xfer),
            UpdateBody::WeaponBonusUpgrade(m) => m.xfer(xfer),
            UpdateBody::ArmorUpgrade(m) => m.xfer(xfer),
            UpdateBody::ModelConditionUpgrade(m) => m.xfer(xfer),
            UpdateBody::ExperienceScalarUpgrade(m) => m.xfer(xfer),
            UpdateBody::ActiveShroudUpgrade(m) => m.xfer(xfer),
            UpdateBody::MaxHealthUpgrade(m) => m.xfer(xfer),
            UpdateBody::GrantScienceUpgrade(m) => m.xfer(xfer),
// --- end p1l9-upgrades ---
            _ => self.xfer(xfer),
        }
    }

    /// The whole chain's `crc` (see [`UpdateBody::xfer_chain`]).
    pub fn crc_chain(&mut self, update: Option<&mut UpdateModule>, xfer: &mut dyn Xfer) -> XferResult<()> {
        let _ = &update;
        match self {
            UpdateBody::CommandSetUpgrade(m) => m.crc(xfer),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.crc(xfer),
            UpdateBody::WeaponBonusUpgrade(m) => m.crc(xfer),
            UpdateBody::ArmorUpgrade(m) => m.crc(xfer),
            UpdateBody::ModelConditionUpgrade(m) => m.crc(xfer),
            UpdateBody::ExperienceScalarUpgrade(m) => m.crc(xfer),
            UpdateBody::ActiveShroudUpgrade(m) => m.crc(xfer),
            UpdateBody::MaxHealthUpgrade(m) => m.crc(xfer),
            UpdateBody::GrantScienceUpgrade(m) => m.crc(xfer),
// --- end p1l9-upgrades ---
            _ => self.crc(xfer),
        }
    }

    /// The whole chain's `loadPostProcess` (see [`UpdateBody::xfer_chain`]).
    pub fn load_post_process_chain(&mut self, update: Option<&mut UpdateModule>) {
        let _ = &update;
        match self {
            UpdateBody::CommandSetUpgrade(m) => m.load_post_process(),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.load_post_process(),
            UpdateBody::WeaponBonusUpgrade(m) => m.load_post_process(),
            UpdateBody::ArmorUpgrade(m) => m.load_post_process(),
            UpdateBody::ModelConditionUpgrade(m) => m.load_post_process(),
            UpdateBody::ExperienceScalarUpgrade(m) => m.load_post_process(),
            UpdateBody::ActiveShroudUpgrade(m) => m.load_post_process(),
            UpdateBody::MaxHealthUpgrade(m) => m.load_post_process(),
            UpdateBody::GrantScienceUpgrade(m) => m.load_post_process(),
// --- end p1l9-upgrades ---
            _ => self.load_post_process(),
        }
    }
}
// --- end p1l9-example ---

impl Snapshot for UpdateBody {
    /// The `crc` of the concrete module (its own state; the bases are the slot's).
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            UpdateBody::Probe(p) => p.crc(xfer),
            UpdateBody::ModuleProbe(m) => m.crc(xfer),
            // --- p1l9-example ---
            UpdateBody::CommandSetUpgrade(m) => m.crc(xfer),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.crc(xfer),
            UpdateBody::WeaponBonusUpgrade(m) => m.crc(xfer),
            UpdateBody::ArmorUpgrade(m) => m.crc(xfer),
            UpdateBody::ModelConditionUpgrade(m) => m.crc(xfer),
            UpdateBody::ExperienceScalarUpgrade(m) => m.crc(xfer),
            UpdateBody::ActiveShroudUpgrade(m) => m.crc(xfer),
            UpdateBody::MaxHealthUpgrade(m) => m.crc(xfer),
            UpdateBody::GrantScienceUpgrade(m) => m.crc(xfer),
// --- end p1l9-upgrades ---
            // --- end p1l9-example ---
        }
    }
    /// The `xfer` of the concrete module.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            UpdateBody::Probe(p) => p.xfer(xfer),
            UpdateBody::ModuleProbe(m) => m.xfer(xfer),
            // --- p1l9-example ---
            UpdateBody::CommandSetUpgrade(m) => m.xfer(xfer),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.xfer(xfer),
            UpdateBody::WeaponBonusUpgrade(m) => m.xfer(xfer),
            UpdateBody::ArmorUpgrade(m) => m.xfer(xfer),
            UpdateBody::ModelConditionUpgrade(m) => m.xfer(xfer),
            UpdateBody::ExperienceScalarUpgrade(m) => m.xfer(xfer),
            UpdateBody::ActiveShroudUpgrade(m) => m.xfer(xfer),
            UpdateBody::MaxHealthUpgrade(m) => m.xfer(xfer),
            UpdateBody::GrantScienceUpgrade(m) => m.xfer(xfer),
// --- end p1l9-upgrades ---
            // --- end p1l9-example ---
        }
    }
    /// The `loadPostProcess` of the concrete module.
    fn load_post_process(&mut self) {
        match self {
            UpdateBody::Probe(p) => p.load_post_process(),
            UpdateBody::ModuleProbe(m) => m.load_post_process(),
            // --- p1l9-example ---
            UpdateBody::CommandSetUpgrade(m) => m.load_post_process(),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.load_post_process(),
            UpdateBody::WeaponBonusUpgrade(m) => m.load_post_process(),
            UpdateBody::ArmorUpgrade(m) => m.load_post_process(),
            UpdateBody::ModelConditionUpgrade(m) => m.load_post_process(),
            UpdateBody::ExperienceScalarUpgrade(m) => m.load_post_process(),
            UpdateBody::ActiveShroudUpgrade(m) => m.load_post_process(),
            UpdateBody::MaxHealthUpgrade(m) => m.load_post_process(),
            UpdateBody::GrantScienceUpgrade(m) => m.load_post_process(),
// --- end p1l9-upgrades ---
            // --- end p1l9-example ---
        }
    }
}

impl Snapshot for ProbeUpdate {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// Version 1: the number of calls so far.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_unsigned_int(&mut self.calls)
    }
    fn load_post_process(&mut self) {}
}
// --- end p1w6-object ---

/// What a [`ProbeUpdate`] spec describes for a module of a new object.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeModuleSpec {
    /// `getUpdatePhase()` of the module.
    pub phase: SleepyUpdatePhase,
    /// `m_nextCallFrameAndPhase`'s frame before registration (0 = unset,
    /// which `registerObject` turns into the current frame).
    pub initial_call_frame: u32,
    /// The module's script.
    pub script: Vec<ProbeStep>,
}

/// One action of a [`ProbeStep`].
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeAction {
    /// `TheGameLogic->destroyObject(findObjectByID(id))`.
    Destroy(ObjectId),
    /// `target->setWakeFrame(obj, delay)` as `UpdateModule` does it: the
    /// frame is `now + delay` ([`UpdateModule::set_wake_frame`]), then
    /// `TheGameLogic->friend_awakenUpdateModule(obj, u, frame)`. A delay of 0
    /// is a wake in the past once the module has run, and two modules that
    /// wake each other that way never let the frame end (as upstream).
    Awaken { target: UpdateHandle, delay: u32 },
    /// `newObject` with these modules, then `registerObject`.
    Spawn(Vec<ProbeModuleSpec>),
    /// `target->attemptDamage(&info)` with a damage of `amount`.
    Damage { target: ObjectId, amount: f32 },
    /// `TheAudio->addAudioEvent(...)` for this module's object.
    Sound(Vec<u8>),
}

/// One call of a [`ProbeUpdate`]: the actions, then the sleep it returns.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeStep {
    /// What the call does, in order.
    pub actions: Vec<ProbeAction>,
    /// The `UpdateSleepTime` the call returns (any number, as upstream's enum
    /// can hold: 0 and above `i32::MAX` included).
    pub sleep: u32,
}

/// A scripted update module. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeUpdate {
    /// The script; call `k` runs `script[k % len]` (an empty script sleeps
    /// one frame and does nothing).
    pub script: Vec<ProbeStep>,
    /// How many times `update` has been called.
    pub calls: u32,
}

impl ProbeUpdate {
    /// A module that runs `script` cyclically.
    pub fn new(script: Vec<ProbeStep>) -> ProbeUpdate {
        ProbeUpdate { script, calls: 0 }
    }

    /// The module's `update()`.
    pub fn update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        let step = if self.script.is_empty() {
            ProbeStep { actions: Vec::new(), sleep: 1 }
        } else {
            self.script[self.calls as usize % self.script.len()].clone()
        };
        self.calls = self.calls.wrapping_add(1);
        for action in &step.actions {
            match action {
                ProbeAction::Destroy(id) => world.destroy_object(*id),
                ProbeAction::Awaken { target, delay } => {
                    let when = UpdateModule::set_wake_frame(world.frame(), UpdateSleepTime(*delay));
                    // A failed wake is upstream's RELEASE_CRASH; the probe has no way to react.
                    let _ = world.awaken_update_module(target.object, *target, when);
                }
                ProbeAction::Spawn(specs) => {
                    let id = world.allocate_object_id();
                    // A refused registration is a bad id; the allocator cannot make one.
                    let _ = world.register_object(probe_object(id, specs));
                }
                ProbeAction::Damage { target, amount } => {
                    let mut info = DamageInfo::default();
                    info.input.source_id = me.object;
                    info.input.amount = *amount;
                    world.attempt_damage(*target, &mut info);
                }
                ProbeAction::Sound(name) => {
                    world.events.push(ClientEvent::Audio { event_name: name.clone(), object_id: me.object, player_index: -1 });
                }
            }
        }
        UpdateSleepTime(step.sleep)
    }
}

/// An object (not yet registered) with one [`ProbeUpdate`] per spec.
///
/// # Panics
///
/// Never in practice: `UpdateModule::new` refuses only absent module data.
pub fn probe_object(id: ObjectId, specs: &[ProbeModuleSpec]) -> Object {
    let mut obj = Object::new(id);
    for spec in specs {
        let data = Arc::new(UpdateModuleData::new());
        let mut update = UpdateModule::new(id, Some(data), spec.phase).expect("module data is present");
        if spec.initial_call_frame != 0 {
            update.friend_set_next_call_frame(spec.initial_call_frame);
        }
        obj.behaviors.push(BehaviorSlot { update, body: Some(UpdateBody::Probe(ProbeUpdate::new(spec.script.clone()))) });
    }
    obj
}
