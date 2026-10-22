// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/ThingFactory.cpp   (`findTemplate`, `newObject`)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp     (the module loop of `Object::Object`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TheThingFactory->findTemplate(name)` and `TheThingFactory->newObject(tmplate, team, statusBits)` over the world: the whole of
//! `newObject`, which `object_create.rs` has from the point the template and the made modules are known. A module of the layer-9 batches (or a
//! later layer) that makes an object from a template by name (`ReplaceObjectUpgrade`, and after it the object creation list, the spawn behaviour,
//! the production update) calls these.
//!
//! # What this adds to `World::new_object`
//!
//! - **The template**: `world.statics.thing_factory` (`TheThingFactory`), by name ([`World::thing_factory_find_template`]: the root's id) and by
//!   id; the object is made from the template's **final override** (`Object::Object`'s `tt = tt->getFinalOverride()`).
//! - **The build variation** (`ThingFactory::newObject`, template side: [`ThingFactory::new_object`], one `GameLogicRandomValue` of `world.random`
//!   per object of a template that has variations).
//! - **The modules** of the template's behavior module list, in order, skipping an empty name (`Object::Object`'s loop), each made with
//!   [`new_module`](crate::logic::modules::new_module) for the id the object is about to get. A class that is not translated yet has no
//!   body: it is a stub that has the interfaces the template recorded for it (`ModuleInfo`'s `interfaceMask`, which the module factory
//!   registered), so the object has the interfaces upstream's would (a create module is told `onBuildComplete`, and does nothing), and the
//!   call is listed in `world.cuts.untranslated_modules`.
//!
//! A module factory error (upstream throws, and nothing catches it) is an [`NewObjectError`].

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::IniError;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::thing_template::ModuleInfoData;
use crate::logic::world::game_logic::{World, WorldError};
use crate::logic::world::ids::TeamId;
use crate::logic::world::object_create::{NewModule, NewObject};

/// Why `ThingFactory::newObject` did not make an object.
#[derive(Debug, PartialEq)]
pub enum NewObjectError {
    /// `throw ERROR_BAD_ARG`: no such template (the id names none).
    NoSuchTemplate(u16),
    /// A module's constructor refused its data (an `INI_INVALID_DATA` upstream throws).
    Module(Vec<u8>, IniError),
    /// The world refused the object.
    World(WorldError),
}

impl World {
    /// `TheThingFactory->findTemplate(name)`: the id of the root template of that name (the pointer upstream returns), `None` for no such
    /// template. The name is case sensitive.
    pub fn thing_factory_find_template(&self, name: &[u8]) -> Option<u16> {
        self.statics.thing_factory.find_template(name).map(|t| t.data.get_template_id())
    }

    /// `TheThingFactory->newObject(tmplate, team, statusBits)`: the template by id (see [`World::thing_factory_find_template`]), the team
    /// (`None`: the neutral player's default team), the status bits the object starts with.
    pub fn thing_factory_new_object(&mut self, template_id: u16, team: Option<TeamId>, status: ObjectStatusMaskType) -> Result<ObjectId, NewObjectError> {
        let statics = self.statics.clone();
        let factory = &statics.thing_factory;
        let root = factory.find_by_template_id(template_id).ok_or(NewObjectError::NoSuchTemplate(template_id))?;
        // pick a build variation
        let plan = factory.new_object(Some(&root.data), &mut self.random, status).map_err(|_| NewObjectError::NoSuchTemplate(template_id))?;
        let chosen = factory.find_by_template_id(plan.template_id).ok_or(NewObjectError::NoSuchTemplate(plan.template_id))?;
        // `Object::Object`: "most overridden version of tt, so this is okay"
        let tt = Arc::new(chosen.get_final_override().data.clone());
        let id_to_be = self.next_object_id();
        let mut modules = Vec::new();
        let info = tt.get_behavior_module_info();
        for i in 0..info.get_count() {
            let name = info.get_nth_name(i);
            if name.is_empty() {
                continue;
            }
            let tag = info.get_nth_tag(i);
            let Some(ModuleInfoData::Logic(data)) = info.get_nth_data(i) else { continue };
            match crate::logic::modules::new_module(id_to_be, name, tag, data) {
                Some(Ok(m)) => modules.push(m),
                Some(Err(e)) => return Err(NewObjectError::Module(name.to_vec(), e)),
                None => {
                    let mut stub = NewModule::plain(name, tag);
                    stub.interfaces = info.nuggets()[i as usize].interface_mask;
                    self.cuts.untranslated_modules.push(name.to_vec());
                    modules.push(stub);
                }
            }
        }
        self.new_object(NewObject { template: tt, status, team, modules }).map_err(NewObjectError::World)
    }

    // --- p1w13-sonnet --- (batch R5: RiderChangeContain)
    /// `TheThingFactory->findTemplate(name)->isEquivalentTo(obj->getTemplate())`, for a template the factory found (`template_id`) and an object:
    /// [`crate::common::thing_template::is_equivalent_to`] of the factory's two roots; an object whose template is not the factory's (a template made
    /// outside it) is equivalent only to the same template id.
    pub fn template_is_equivalent_to_object(&self, template_id: u16, obj: ObjectId) -> bool {
        let Some(o) = self.objects.find(obj) else { return false };
        let theirs = o.template().get_template_id();
        let factory = &self.statics.thing_factory;
        match (factory.find_by_template_id(template_id), factory.find_by_template_id(theirs)) {
            (Some(a), Some(b)) => crate::common::thing_template::is_equivalent_to(a, b),
            _ => template_id == theirs,
        }
    }
    // --- end p1w13-sonnet ---
}
