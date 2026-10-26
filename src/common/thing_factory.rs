// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/ThingFactory.h
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/ThingFactory.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIObject.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, April 2001 (ThingFactory.cpp)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ThingFactory` (`TheThingFactory`): the store of every [`ThingTemplate`]
//! read from the `Object` and `ObjectReskin` INI blocks, with lookup by name
//! and by id, the template overrides of map INI files, `reset`, the
//! post-load name resolution, and the template side of `newObject` and
//! `newDrawable`.
//!
//! # Translation choices
//!
//! - **The template list and the hash map.** Upstream links the templates
//!   into a singly linked list with **new ones at the head**
//!   (`m_firstTemplate`; `findByTemplateID`, `postProcessLoad` and `reset`
//!   walk it in that order) and finds them by name in a `hash_map` keyed on
//!   the case-sensitive `AsciiString`. Here a `Vec` in creation order, read
//!   backwards ([`ThingFactory::templates`] is newest first, as the list),
//!   and an **ordered map** from name to the template's position. Nothing
//!   iterates the hash map, so its order is not observable.
//! - **Override chains.** A template is an [`Overridable`]: the first one
//!   made for a name is the **root** (what `findTemplate` returns, in the
//!   list and the map), and an `Object` block read in an
//!   `INI_LOAD_CREATE_OVERRIDES` load makes a copy of the **final override**
//!   of the root, hung off it (`newOverride`) and not in the list, and reads
//!   into that. Objects read the final override. See [`Overridable`].
//! - **Copies copy the `Overridable` base.** `ThingTemplate` has no
//!   assignment operator of its own, so `*newTemplate = *defaultT` (a new
//!   template starts as a copy of `DefaultThingTemplate`), `*newTemplate =
//!   *child` (`newOverride`) and `copyFrom` (`ObjectReskin`) also copy
//!   `m_nextOverride` and `m_isOverride`. The flag is copied: a template
//!   made in an override load (so marked) from an unmarked default is
//!   unmarked, and one made from a marked default is marked, with the
//!   consequence for `reset`. A copied `m_nextOverride` makes two templates
//!   share one chain, which each destructor frees: a source that has an
//!   override chain is [`Undefined::AliasedOverrideChain`] (as in
//!   `common::special_power`). The chain that a `copyFrom` overwrites
//!   (an existing template read again by a non-override load) is dropped,
//!   as upstream leaks it.
//! - **`reset()` can leave a dangling link.** It walks the list, deletes the
//!   override chains and the templates that were made by an override load
//!   (their root is marked), and unlinks a deleted template **only if it is
//!   the head** of the list: a deleted template further down leaves the
//!   previous one's `m_nextThingTemplate` pointing at freed memory. The
//!   Rust unlinks properly, and reports [`Undefined::DanglingTemplateLink`]
//!   after it has (the same for a name in the map that is left pointing at a
//!   deleted template). Map-only templates are normally at the head, since a
//!   map's INI is loaded after the base INI.
//! - **Name keys.** Nothing in this file calls `NAMEKEY`; the field parsers
//!   of [`ThingTemplate`] do.
//! - **`DefaultThingTemplate`** is whatever template has that name; it is not
//!   special to the Rust either.
//!
//! # Template ids and node ids
//!
//! `m_nextTemplateID` starts at 1 and counts up as an `UnsignedShort`; the
//! 65 536th template wraps it to 0 and a later one repeats an id
//! (`DEBUG_ASSERTCRASH` is compiled out). Kept (`wrapping_add`). A template's
//! id is shared by its root and every override copy. The **node id** of
//! [`ThingTemplate::node_id`] is this port's stand-in for the object's
//! address, unique to every template object the factory makes.
//!
//! # `newObject` and `newDrawable`
//!
//! `newObject` is split at the cut with the object layer (layer 8):
//! [`ThingFactory::new_object`] is the template side, which picks a **build
//! variation** (`GameLogicRandomValue(0, n - 1)` over
//! `getBuildVariations()`, and `findTemplate` of the name it picks, kept if
//! there is such a template) and returns the template to make the object
//! from. Left to layer 8, listed here so that nothing is lost:
//! `TheGameLogic->friend_createObject( tmplate, statusBits, team )` (the
//! `Object` constructor), the loop that calls `onCreate` on every behavior
//! module that is a `CreateModuleInterface`,
//! `ThePartitionManager->registerObject( obj )` and `obj->initObject()`.
//! `newDrawable` is `TheGameClient->friend_createDrawable( tmplate,
//! statusBits )`: the client's, so it is the client event
//! [`ClientEvent::ObjectCreated`] ([`ThingFactory::new_drawable`]).
//! ~~(`GameLogic::registerObject` calls it for every object, `GameLogic.cpp`
//! line 4139.)~~ **CORRECTED 2026-10-18** (p1w6-object): `GameLogic::sendObjectCreated`
//! (`GameLogic.cpp` 4137-4145) makes the drawable, and its only caller is
//! `Object::initObject` (`Object.cpp` 535), which `newObject` calls last, not
//! `registerObject` (checked: `grep sendObjectCreated` over `GameEngine` finds
//! the declaration, that one call and the definition).
//!
//! # Where upstream is undefined
//!
//! - A template copied from one that has an override chain, and a `reset()`
//!   that deletes a template from the middle of the list (above).
//! - Everything undefined in [`ThingTemplate`]'s field parsers.
//!
//! # Verified
//!
//! Against upstream `ThingFactory.cpp`, `ThingTemplate.cpp` and
//! `INIObject.cpp`, compiled unchanged, by `tests/differential_thing_template.rs`.

use std::collections::BTreeMap;

use crate::common::ini::{Ini, IniError, IniErrorKind, IniLoadType, IniResult, Undefined};
use crate::common::kind_of::KINDOF_COMMANDCENTER;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::overridable::Overridable;
use crate::common::random_value::RandomValues;
use crate::common::thing_template::{object_fields, reskin_fields, ObjectFill, ThingParseEnv, ThingTemplate};
use crate::logic::client_event::ClientEvent;

/// What can go wrong in the factory outside the INI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThingFactoryError {
    /// `throw ERROR_BAD_ARG`: `newObject` or `newDrawable` without a template.
    BadArg,
}

impl std::fmt::Display for ThingFactoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThingFactoryError::BadArg => f.write_str("ERROR_BAD_ARG: no thing template"),
        }
    }
}

impl std::error::Error for ThingFactoryError {}

/// What `newObject`'s template side decides: the template to make the object
/// from (after the build variation) and the status bits to make it with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewObjectPlan {
    /// The id of the template to make the object from.
    pub template_id: u16,
    /// The `statusBits` argument, which must be set before `onCreate`.
    pub status_bits: ObjectStatusMaskType,
}

/// Where a template object is: the root's position in the factory and its
/// depth in the root's override chain (0 is the root).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Node {
    root: usize,
    depth: usize,
}

/// `class ThingFactory`.
#[derive(Clone, Debug)]
pub struct ThingFactory {
    /// The list `m_firstTemplate` heads, oldest first (the head is last).
    templates: Vec<Overridable<ThingTemplate>>,
    /// `m_templateHashMap`: a name to the position of the root in `templates`.
    template_map: BTreeMap<Vec<u8>, usize>,
    /// `m_nextTemplateID`.
    next_template_id: u16,
    /// The next node id (see the module docs).
    next_node_id: u64,
}

impl Default for ThingFactory {
    fn default() -> Self {
        ThingFactory::new()
    }
}

impl ThingFactory {
    /// `ThingFactory()`: no templates, the next id is 1 ("not zero!").
    pub fn new() -> Self {
        ThingFactory { templates: Vec::new(), template_map: BTreeMap::new(), next_template_id: 1, next_node_id: 1 }
    }

    /// `init()`: nothing.
    pub fn init(&mut self) {}

    /// `update()`: nothing.
    pub fn update(&mut self) {}

    /// `freeDatabase()` (the destructor): forget every template. (The id
    /// counter is not reset: it is a member of the object that is gone, so a
    /// new `ThingFactory` starts again at 1.)
    pub fn free_database(&mut self) {
        self.templates.clear();
        self.template_map.clear();
    }

    fn fresh_node_id(&mut self) -> u64 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    /// `m_nextTemplateID`: the id the next template gets.
    pub fn next_template_id(&self) -> u16 {
        self.next_template_id
    }

    /// `m_templateHashMap`, in name order: each name and the position in the list (0 is the
    /// head, as [`ThingFactory::templates`] counts) of the root it finds.
    pub fn template_map_entries(&self) -> impl Iterator<Item = (&[u8], usize)> {
        let n = self.templates.len();
        self.template_map.iter().map(move |(name, &index)| (name.as_slice(), n - 1 - index))
    }

    /// The number of templates (roots) in the list.
    pub fn template_count(&self) -> usize {
        self.templates.len()
    }

    /// The templates in list order: newest first, as `firstTemplate()` and
    /// `friend_getNextTemplate()` walk them. Each is the root of its chain.
    pub fn templates(&self) -> impl Iterator<Item = &Overridable<ThingTemplate>> {
        self.templates.iter().rev()
    }

    /// `firstTemplate()`: the newest template.
    pub fn first_template(&self) -> Option<&Overridable<ThingTemplate>> {
        self.templates.last()
    }

    fn node(&self, n: Node) -> &Overridable<ThingTemplate> {
        let mut cur = &self.templates[n.root];
        for _ in 0..n.depth {
            cur = cur.get_next_override().expect("a node of the chain");
        }
        cur
    }

    fn node_mut(&mut self, n: Node) -> &mut Overridable<ThingTemplate> {
        let mut cur = &mut self.templates[n.root];
        for _ in 0..n.depth {
            cur = cur.friend_get_next_override().expect("a node of the chain");
        }
        cur
    }

    /// The root of the template of this name, if any (what
    /// `findTemplateInternal(name, FALSE)` returns).
    fn root_index(&self, name: &[u8]) -> Option<usize> {
        self.template_map.get(name).copied()
    }

    /// `findTemplate(name, check)`: the template with this name (case
    /// sensitive), as its root; `None` if there is none. (`check` only
    /// decides whether a debug build complains.)
    pub fn find_template(&self, name: &[u8]) -> Option<&Overridable<ThingTemplate>> {
        self.root_index(name).map(|i| &self.templates[i])
    }

    /// batch R41: the template with this name (its root) to change (a test seam: the differential tests' templates are not read from INI).
    pub fn friend_find_template_mut(&mut self, name: &[u8]) -> Option<&mut Overridable<ThingTemplate>> {
        self.root_index(name).map(|i| &mut self.templates[i])
    }

    /// `findByTemplateID(id)`: the first template in list order with this id
    /// ("not particularly fast (does a linear search)").
    pub fn find_by_template_id(&self, id: u16) -> Option<&Overridable<ThingTemplate>> {
        self.templates().find(|t| t.data.get_template_id() == id)
    }

    /// The template (root) with this node id, if it is in the list.
    fn root_by_template_id(&self, id: u16) -> Option<usize> {
        (0..self.templates.len()).rev().find(|&i| self.templates[i].data.get_template_id() == id)
    }

    /// `addTemplate(tmplate)`: link at the head and map the name (replacing
    /// a template of the same name in the map, as `operator[]` does).
    fn add_template(&mut self, tmplate: Overridable<ThingTemplate>) -> usize {
        let index = self.templates.len();
        self.template_map.insert(tmplate.data.get_name().to_vec(), index);
        self.templates.push(tmplate);
        index
    }

    /// `*dst = *src` for two templates: the data, and the `Overridable` base
    /// (`m_isOverride` kept, `m_nextOverride` shared, which is undefined: see
    /// the module docs).
    fn copy_of(src: &Overridable<ThingTemplate>) -> IniResult<Overridable<ThingTemplate>> {
        if src.get_next_override().is_some() {
            return Err(IniError::undefined(Undefined::AliasedOverrideChain));
        }
        let mut n = Overridable::new(src.data.clone());
        if src.is_override() {
            n.mark_as_override();
        }
        Ok(n)
    }

    /// `newTemplate(name)`: "create a new template with name 'name' and add
    /// to template list"; a copy of `DefaultThingTemplate` if there is one.
    /// `default_occlusion_delay` is `TheGlobalData->m_defaultOcclusionDelay`,
    /// which the constructor reads (a copy of the default has the default's).
    /// Returns the position of the new root.
    pub fn new_template(&mut self, name: &[u8], default_occlusion_delay: u32) -> IniResult<usize> {
        // if the default template is present, get it and copy over any data to the new template
        let mut new_template = match self.find_template(b"DefaultThingTemplate") {
            Some(default_t) => {
                // copy over static data
                let mut copy = Self::copy_of(default_t)?;
                copy.data.set_copied_from_default();
                copy
            }
            None => Overridable::new(ThingTemplate::new(default_occlusion_delay)),
        };
        new_template.data.friend_set_node_id(self.fresh_node_id());
        // give template a unique identifier
        new_template.data.friend_set_template_id(self.next_template_id);
        self.next_template_id = self.next_template_id.wrapping_add(1);
        // assign name
        new_template.data.friend_set_template_name(name);
        // add to list
        Ok(self.add_template(new_template))
    }

    /// `newOverride(thingTemplate)`: a copy of the final override of the root
    /// at `root`, marked as an override, hung off it. "NOTE that newTemplate
    /// is *NOT* added to master template list". Returns where the copy is.
    fn new_override(&mut self, root: usize) -> IniResult<Node> {
        let node_id = self.fresh_node_id();
        let child = self.templates[root].get_final_override();
        // copy data from final override to 'newTemplate' as a set of initial default values
        let mut new_template = Self::copy_of(child)?;
        new_template.data.set_copied_from_default();
        new_template.data.friend_set_node_id(node_id);
        new_template.mark_as_override();
        let displaced = self.templates[root].friend_get_final_override().set_next_override(Some(Box::new(new_template)));
        debug_assert!(displaced.is_none());
        let mut depth = 0;
        let mut cur = &self.templates[root];
        while let Some(next) = cur.get_next_override() {
            cur = next;
            depth += 1;
        }
        Ok(Node { root, depth })
    }

    /// `reset()`: "go through all templates and delete any overrides". A
    /// template made by an override load goes, with its chain; the others
    /// lose their override copies. See the module docs for the dangling link
    /// that this leaves upstream when a deleted template is not at the head
    /// of the list: the Rust has unlinked it properly and says so with
    /// [`Undefined::DanglingTemplateLink`].
    pub fn reset(&mut self) -> IniResult<()> {
        let n = self.templates.len();
        // list order: newest first
        let old = std::mem::take(&mut self.templates);
        let mut list: Vec<Option<Overridable<ThingTemplate>>> = old.into_iter().rev().map(Some).collect();
        let mut kept: Vec<Option<Overridable<ThingTemplate>>> = Vec::with_capacity(n);
        let mut head = 0usize;
        let mut dangling = false;
        let mut old_to_new: Vec<Option<usize>> = vec![None; n];
        let mut erased_names: Vec<Vec<u8>> = Vec::new();
        for (pos, slot) in list.iter_mut().enumerate() {
            let t = slot.take().expect("each template once");
            let possible_adjustment = pos == head;
            let name = t.data.get_name().to_vec();
            // storage position of this template before the reset
            let old_index = n - 1 - pos;
            match Box::new(t).delete_overrides() {
                None => {
                    if possible_adjustment {
                        head = pos + 1;
                    } else {
                        dangling = true;
                    }
                    // Also needs to be removed from the Hash map.
                    erased_names.push(name);
                }
                Some(still_valid) => {
                    old_to_new[old_index] = Some(kept.len());
                    kept.push(Some(*still_valid));
                }
            }
        }
        // `kept` is in list order (newest first); storage is oldest first
        let mut storage: Vec<Overridable<ThingTemplate>> = kept.into_iter().map(|t| t.expect("kept")).collect();
        storage.reverse();
        let kept_count = storage.len();
        // the new position of a kept template: reversed
        for entry in old_to_new.iter_mut().flatten() {
            *entry = kept_count - 1 - *entry;
        }
        for name in &erased_names {
            self.template_map.remove(name);
        }
        let mut remapped = BTreeMap::new();
        for (name, index) in std::mem::take(&mut self.template_map) {
            match old_to_new[index] {
                Some(i) => {
                    remapped.insert(name, i);
                }
                // a name that was left in the map pointing at a deleted template
                None => dangling = true,
            }
        }
        self.templates = storage;
        self.template_map = remapped;
        if dangling {
            return Err(IniError::undefined(Undefined::DanglingTemplateLink));
        }
        Ok(())
    }

    /// `resolveNames()` of the template at `n`: resolve its prerequisites'
    /// names to templates, mark the templates that are a prerequisite's first
    /// alternatives as build facilities, and command centers too. (The
    /// images are looked up by the client, see the module docs of
    /// [`ThingTemplate`].) The flag goes on the **root** of the named
    /// template (what `findTemplate` returns), whatever override it has.
    fn resolve_names_of(&mut self, n: Node) {
        // keep a copy of the prerequisites to resolve, since the lookups read the factory
        let mut prereqs = self.node(n).data.prereq_info().to_vec();
        for p in &mut prereqs {
            p.resolve_names(|name| self.find_template(name).map(|t| t.data.get_template_id()));
        }
        const MAX_BF: usize = 32;
        let mut facilities: Vec<u16> = Vec::new();
        for p in &prereqs {
            for id in p.all_possible_build_facility_templates(MAX_BF).into_iter().flatten() {
                facilities.push(id);
            }
        }
        *self.node_mut(n).data.prereq_info_mut() = prereqs;
        for id in facilities {
            // casting const away is a little evil, but justified in this case
            if let Some(root) = self.root_by_template_id(id) {
                self.templates[root].data.set_is_build_facility(true);
            }
        }
        if self.node(n).data.is_kind_of(KINDOF_COMMANDCENTER) {
            // Command centers are considered factories. jba.
            self.node_mut(n).data.set_is_build_facility(true);
        }
    }

    /// `postProcessLoad()`: "post process phase after loading the database
    /// files": resolve the names of every template, in list order.
    pub fn post_process_load(&mut self) {
        for root in (0..self.templates.len()).rev() {
            self.resolve_names_of(Node { root, depth: 0 });
        }
    }

    /// `newObject(tmplate, team, statusBits)`, template side: pick a build
    /// variation if the template has any and make the object from that
    /// template. `template_id` is the id of `tmplate`; `ERROR_BAD_ARG` for
    /// `None`. See the module docs for what is left to layer 8.
    pub fn new_object(
        &self,
        tmplate: Option<&ThingTemplate>,
        rng: &mut RandomValues,
        status_bits: ObjectStatusMaskType,
    ) -> Result<NewObjectPlan, ThingFactoryError> {
        let Some(tmplate) = tmplate else {
            return Err(ThingFactoryError::BadArg);
        };
        let mut template_id = tmplate.get_template_id();
        let asv = tmplate.get_build_variations();
        if !asv.is_empty() {
            let which = rng.game_logic_random_value(0, asv.len() as i32 - 1);
            // the index is in range: (0, n - 1)
            if let Some(tmp) = self.find_template(&asv[which as usize]) {
                template_id = tmp.data.get_template_id();
            }
        }
        // (DEBUG_ASSERTCRASH: you may not create Objects with a DRAWABLE_ONLY template, compiled out)
        Ok(NewObjectPlan { template_id, status_bits })
    }

    /// `newDrawable(tmplate, statusBits)`: the client makes the drawable
    /// (`TheGameClient->friend_createDrawable`), which is the event.
    /// `object_id` is the object `GameLogic::sendObjectCreated` binds the
    /// drawable to straight after (`bindObjectAndDrawable`), which upstream
    /// does through a pointer and the event carries as an id (0 for a drawable
    /// that belongs to no object). `ERROR_BAD_ARG` for `None`.
    pub fn new_drawable(
        &self,
        tmplate: Option<&ThingTemplate>,
        status_bits: u32,
        object_id: i32,
        events: &mut Vec<ClientEvent>,
    ) -> Result<(), ThingFactoryError> {
        let Some(tmplate) = tmplate else {
            return Err(ThingFactoryError::BadArg);
        };
        events.push(ClientEvent::ObjectCreated { template_id: tmplate.get_template_id(), drawable_status_bits: status_bits, object_id });
        Ok(())
    }

    /// `ThingFactory::parseObjectDefinition(ini, name, reskinFrom)`: the
    /// `Object` block (`reskin_from` empty) or the `ObjectReskin` block.
    ///
    /// Finds the template of this name or makes it (an override load marks a
    /// new one as made for this map only); a name that exists is read into
    /// again by a non-override load (upstream only complains in a debug
    /// build) and gets an override copy from an override load. A reskin
    /// copies the template it names (which must exist already) and reads only
    /// the fields that change how it looks. Then `validate()`, and in an
    /// override load `resolveNames()`. A block that throws leaves the
    /// template as far as it was filled.
    pub fn parse_object_definition(
        &mut self,
        ini: &mut Ini<'_>,
        env: &mut ThingParseEnv<'_>,
        default_occlusion_delay: u32,
        name: &[u8],
        reskin_from: &[u8],
    ) -> IniResult<()> {
        let load_type = ini.get_load_type();
        // find existing item if present
        let node = match self.root_index(name) {
            None => {
                // no item is present, create a new one
                let root = self.new_template(name, default_occlusion_delay)?;
                if load_type == IniLoadType::CreateOverrides {
                    // This ThingTemplate is actually an override, so we will mark it as such so that it
                    // properly gets deleted on ::reset().
                    self.templates[root].mark_as_override();
                }
                Node { root, depth: 0 }
            }
            // upstream complains about a duplicate in a debug build, and goes on to read into it
            Some(root) if load_type != IniLoadType::CreateOverrides => Node { root, depth: 0 },
            Some(root) => self.new_override(root)?,
        };

        if !reskin_from.is_empty() {
            let Some(reskin_root) = self.root_index(reskin_from) else {
                // ObjectReskin must come after the original Object
                return Err(IniError::new(IniErrorKind::InvalidData));
            };
            let source = self.templates[reskin_root].clone();
            let reskin_node_id = source.data.node_id();
            let same_object = node.depth == 0 && node.root == reskin_root;
            // thingTemplate->copyFrom(reskinTmpl), the Overridable base included; a template
            // reskinned from itself assigns itself to itself, which changes nothing
            if !same_object {
                if source.get_next_override().is_some() {
                    return Err(IniError::undefined(Undefined::AliasedOverrideChain));
                }
                let this = self.node_mut(node);
                let mut data = this.data.clone();
                data.copy_from(Some(&source.data));
                let mut replaced = Overridable::new(data);
                if source.is_override() {
                    replaced.mark_as_override();
                }
                *this = replaced;
            }
            let this = self.node_mut(node);
            this.data.set_copied_from_default();
            this.data.set_reskinned_from(reskin_node_id);
            let mut fill = ObjectFill { tt: &mut this.data, env: env.reborrow() };
            ini.init_from_ini(&mut fill, &reskin_fields())?;
        } else {
            let this = self.node_mut(node);
            let mut fill = ObjectFill { tt: &mut this.data, env: env.reborrow() };
            ini.init_from_ini(&mut fill, &object_fields())?;
        }

        self.node_mut(node).data.validate();

        if load_type == IniLoadType::CreateOverrides {
            self.resolve_names_of(node);
        }
        Ok(())
    }

    /// `INI::parseObjectDefinition(ini)` (INIObject.cpp): the `Object` block,
    /// its name and then the fields.
    pub fn parse_object_block(&mut self, ini: &mut Ini<'_>, env: &mut ThingParseEnv<'_>, default_occlusion_delay: u32) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        self.parse_object_definition(ini, env, default_occlusion_delay, &name, b"")
    }

    /// `INI::parseObjectReskinDefinition(ini)` (INIObject.cpp): the
    /// `ObjectReskin` block, its name, the name it is reskinned from and
    /// then the fields.
    pub fn parse_object_reskin_block(&mut self, ini: &mut Ini<'_>, env: &mut ThingParseEnv<'_>, default_occlusion_delay: u32) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        let reskin_from = ini.get_next_token(None)?;
        self.parse_object_definition(ini, env, default_occlusion_delay, &name, &reskin_from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::damage_fx::DamageFxStore;
    use crate::common::game_common::VeterancyLevel;
    use crate::common::geometry::GeometryType;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::common::name_key_generator::NameKeyType;
    use crate::common::module::{
        Module, ModuleData, ModuleDataKind, ModuleType, MODULEINTERFACE_BODY, MODULEINTERFACE_DIE, MODULEINTERFACE_UPDATE,
    };
    use crate::common::module_factory::{ModuleFactory, ModuleTemplate};
    use crate::common::name_key_generator::NameKeyGenerator;
    use crate::common::science::{GameTextFetch, ScienceStore};
    use crate::common::thing_template::{
        is_equivalent_to, ModuleInfoData, ModuleParseMode, ThingTemplateAudioType, BC_PLACED_BY_PLAYER, BSTATUS_NO, OBJECT_FIELD_NAMES,
        RESKIN_FIELD_NAMES,
    };
    use crate::common::unicode_string::UnicodeString;
    use crate::logic::armor::ArmorStore;
    use crate::logic::weapon_set::WeaponTemplateTable;
    use std::sync::Arc;

    struct Text;
    impl GameTextFetch for Text {
        fn fetch(&self, label: &[u8]) -> UnicodeString {
            if label.len() >= 6 && label[..6].eq_ignore_ascii_case(b"NOTEXT") {
                return UnicodeString::new();
            }
            let mut units: Vec<u16> = vec![u16::from(b'T'), u16::from(b':')];
            units.extend(label.iter().map(|&b| u16::from(b)));
            UnicodeString::try_from_chars(&units).unwrap()
        }
    }

    fn never(_: crate::common::module::ObjectId, _: Option<ModuleDataKind>) -> Result<Module, IniError> {
        Err(IniError::new(IniErrorKind::Bug))
    }
    fn base_data(ini: Option<&mut Ini<'_>>) -> Result<ModuleDataKind, IniError> {
        Ok(ModuleDataKind::Module(Arc::new(ModuleData::friend_new_module_data(ini)?)))
    }

    struct Ctx {
        factory: ThingFactory,
        names: NameKeyGenerator,
        modules: ModuleFactory,
        armors: ArmorStore,
        dfx: DamageFxStore,
        weapons: WeaponTemplateTable,
        sciences: ScienceStore,
        occlusion: u32,
    }

    impl Ctx {
        fn new() -> Self {
            let mut names = NameKeyGenerator::new();
            names.reset();
            let mut modules = ModuleFactory::new();
            let t = |mask| ModuleTemplate { create_proc: Some(never), create_data_proc: Some(base_data), which_interfaces: Some(mask) };
            for (name, mask) in [
                ("TestUpdate", MODULEINTERFACE_UPDATE),
                ("TestUpdate2", MODULEINTERFACE_UPDATE),
                ("TestDie", MODULEINTERFACE_DIE),
                ("TestDie2", MODULEINTERFACE_DIE),
                ("TestBody", MODULEINTERFACE_BODY),
            ] {
                modules.add_module_internal(&mut names, t(mask), ModuleType::Behavior, name).unwrap();
            }
            let mut weapons = WeaponTemplateTable::new();
            let k = names.name_to_key(b"Rifle");
            weapons.add(k);
            Ctx {
                factory: ThingFactory::new(),
                names,
                modules,
                armors: ArmorStore::new(),
                dfx: DamageFxStore::new(),
                weapons,
                sciences: ScienceStore::new(),
                occlusion: 90,
            }
        }
    }

    fn object_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        let mut env = ThingParseEnv {
            names: &mut c.names,
            modules: &mut c.modules,
            armors: &c.armors,
            damage_fx: &c.dfx,
            weapons: &c.weapons,
            sciences: &c.sciences,
            text: &Text,
        };
        c.factory.parse_object_block(ini, &mut env, c.occlusion)
    }
    fn reskin_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        let mut env = ThingParseEnv {
            names: &mut c.names,
            modules: &mut c.modules,
            armors: &c.armors,
            damage_fx: &c.dfx,
            weapons: &c.weapons,
            sciences: &c.sciences,
            text: &Text,
        };
        c.factory.parse_object_reskin_block(ini, &mut env, c.occlusion)
    }
    fn armor_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.armors.parse_armor_definition(ini, &mut c.names)
    }
    fn dfx_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.dfx.parse_damage_fx_definition(ini, &mut c.names)
    }
    static BLOCKS: [BlockParse<Ctx>; 4] = [
        BlockParse { token: "Object", parse: object_block },
        BlockParse { token: "ObjectReskin", parse: reskin_block },
        BlockParse { token: "Armor", parse: armor_block },
        BlockParse { token: "DamageFX", parse: dfx_block },
    ];

    fn load_as(c: &mut Ctx, text: &str, lt: IniLoadType) -> IniResult<()> {
        Ini::load(b"t.ini", text.as_bytes(), lt, None, &BLOCKS, c)
    }
    fn load(c: &mut Ctx, text: &str) -> IniResult<()> {
        load_as(c, text, IniLoadType::Overwrite)
    }
    fn root<'a>(c: &'a Ctx, name: &str) -> &'a ThingTemplate {
        &c.factory.find_template(name.as_bytes()).unwrap().data
    }
    fn final_of<'a>(c: &'a Ctx, name: &str) -> &'a ThingTemplate {
        &c.factory.find_template(name.as_bytes()).unwrap().get_final_override().data
    }
    fn tags(mi: &crate::common::thing_template::ModuleInfo) -> Vec<String> {
        (0..mi.get_count()).map(|i| String::from_utf8_lossy(mi.get_nth_tag(i)).into_owned()).collect()
    }

    #[test]
    fn the_tables_have_the_upstream_counts_and_names() {
        assert_eq!(OBJECT_FIELD_NAMES.len(), 110);
        assert_eq!(RESKIN_FIELD_NAMES.len(), 10);
        // no name twice in a table
        let mut seen = std::collections::BTreeSet::new();
        assert!(OBJECT_FIELD_NAMES.iter().all(|n| seen.insert(*n)));
        assert_eq!(crate::common::thing_template::object_fields().len(), 111);
    }

    #[test]
    fn a_new_template_has_the_constructors_values() {
        let mut c = Ctx::new();
        load(&mut c, "Object Plain\nEnd\n").unwrap();
        let t = root(&c, "Plain");
        assert_eq!(t.get_name(), b"Plain");
        assert_eq!(t.get_template_id(), 1);
        assert_eq!(t.get_occlusion_delay(), 90);
        assert_eq!(t.get_default_radar_priority(), 0);
        assert_eq!(t.friend_calc_vision_range(), 0.0);
        assert_eq!(t.friend_calc_shroud_clearing_range(), -1.0);
        assert_eq!(t.get_shroud_reveal_to_all_range(), -1.0);
        assert_eq!(t.get_build_time(), 1.0);
        assert_eq!(t.get_build_completion(), 1);
        assert_eq!(t.get_crusher_level(), 0);
        assert_eq!(t.get_crushable_level(), 255);
        assert_eq!(t.get_asset_scale(), 0.0);
        assert_eq!(t.get_max_simultaneous_link_key(), NameKeyType::INVALID);
        assert_eq!(t.get_skill_point_value(VeterancyLevel::Elite), 0);
        assert_eq!(t.raw_skill_point_value(VeterancyLevel::Elite), -999);
        let g = t.get_template_geometry_info();
        assert_eq!((g.get_geom_type(), g.get_major_radius(), g.get_height()), (GeometryType::Sphere, 1.0, 1.0));
        // validate() picks the shadow texture from the geometry
        assert_eq!(t.get_shadow_texture_name(), b"shadow");
        assert!(!t.get_audio(ThingTemplateAudioType::VoiceSelect).event_name.len() > 0 || t.get_audio(ThingTemplateAudioType::VoiceSelect).event_name.is_empty());
        assert_eq!(c.factory.template_count(), 1);
        assert!(c.factory.find_template(b"plain").is_none(), "names are case sensitive");
    }

    #[test]
    fn a_block_of_fields_fills_the_template() {
        let mut c = Ctx::new();
        load(
            &mut c,
            "Armor Hard\n Armor = Default 100%\nEnd\nDamageFX Fx\nEnd\n\
Object Tank\n DisplayName = OBJECT:Tank\n RadarPriority = UNIT\n TransportSlotCount = 3\n FenceWidth = 2.5\n IsBridge = Yes\n \
VisionRange = 150\n ShroudClearingRange = 200\n PlacementViewAngle = 180\n SkillPointValue = 1 2 3 4\n ExperienceValue = 5 6 7 8\n \
ExperienceRequired = 0 10 20 30\n IsTrainable = Yes\n Side = America\n Buildable = No\n BuildCost = 900\n BuildTime = 10.5\n \
RefundValue = 400\n BuildCompletion = PLACED_BY_PLAYER\n EnergyProduction = -4\n IsForbidden = Yes\n DisplayColor = R:1 G:2 B:3\n \
EditorSorting = VEHICLE\n KindOf = VEHICLE SELECTABLE\n CommandSet = TankSet\n BuildVariations = A B C\n \
SelectPortrait = Portrait\n ButtonImage = Button\n UpgradeCameo2 = Up2\n \
VoiceSelect = SelectSound\n SoundAmbient = Hum\n SoundFallingFromPlane = Whoosh\n VoiceMove = NoSound\n \
UnitSpecificSounds\n  TurretMove = Move\n  turretmove = Other\n  TurretStop = NoSound\n End\n UnitSpecificFX\n  Boom = BoomFX\n  Quiet = None\n End\n \
Scale = 1.25\n Geometry = BOX\n GeometryMajorRadius = 12\n GeometryMinorRadius = 6\n GeometryHeight = 9\n GeometryIsSmall = Yes\n \
Shadow = SHADOW_VOLUME SHADOW_DECAL\n ShadowSizeX = 3\n ShadowTexture = MyShadow\n OcclusionDelay = 1000\n \
InstanceScaleFuzziness = 0.5\n StructureRubbleHeight = 7\n ThreatValue = 40\n MaxSimultaneousOfType = 3\n MaxSimultaneousLinkKey = LinkA\n \
CrusherLevel = 2\n CrushableLevel = 1\n \
ArmorSet\n  Conditions = VETERAN\n  Armor = Hard\n  DamageFX = Fx\n End\n \
WeaponSet\n  Conditions = None\n  Weapon = PRIMARY Rifle\n End\nEnd\n",
        )
        .unwrap();
        let t = root(&c, "Tank");
        assert_eq!(t.get_display_name().str().len(), "T:OBJECT:Tank".len());
        assert_eq!(t.get_default_radar_priority(), 3);
        assert_eq!(t.get_raw_transport_slot_count(), 3);
        assert_eq!(t.get_fence_width(), 2.5);
        assert!(t.is_bridge());
        assert_eq!(t.friend_calc_vision_range(), 150.0);
        assert_eq!(t.get_placement_view_angle(), 180.0 * (std::f32::consts::PI / 180.0));
        assert_eq!(t.get_skill_point_value(VeterancyLevel::Heroic), 4);
        assert_eq!(t.get_experience_value(VeterancyLevel::Veteran), 6);
        assert_eq!(t.get_experience_required(VeterancyLevel::Elite), 20);
        assert!(t.is_trainable());
        assert_eq!(t.get_default_owning_side(), b"America");
        assert_eq!(t.buildable(), BSTATUS_NO);
        assert_eq!(t.friend_get_build_cost(), 900);
        assert!(t.is_buildable_item());
        assert_eq!(t.get_build_time(), 10.5);
        assert_eq!(t.get_refund_value(), 400);
        assert_eq!(t.get_build_completion(), BC_PLACED_BY_PLAYER);
        assert_eq!(t.get_energy_production(), -4);
        assert!(t.is_forbidden());
        assert_eq!(t.get_editor_sorting(), 3);
        assert!(t.is_kind_of(crate::common::kind_of::KINDOF_VEHICLE) && t.is_kind_of(crate::common::kind_of::KINDOF_SELECTABLE));
        assert!(!t.is_kind_of(crate::common::kind_of::KINDOF_STRUCTURE));
        assert_eq!(t.friend_get_command_set_string(), b"TankSet");
        assert_eq!(t.get_build_variations(), [b"A".to_vec(), b"B".to_vec(), b"C".to_vec()]);
        assert_eq!(t.selected_portrait_image_name(), b"Portrait");
        assert_eq!(t.button_image_name(), b"Button");
        assert_eq!(t.get_upgrade_cameo_name(1), b"Up2");
        assert_eq!(t.get_upgrade_cameo_name(0), b"");
        assert_eq!(t.get_audio(ThingTemplateAudioType::VoiceSelect).event_name, b"SelectSound");
        assert!(t.has_audio(ThingTemplateAudioType::SoundAmbient));
        assert_eq!(t.get_audio(ThingTemplateAudioType::SoundFalling).event_name, b"Whoosh");
        assert!(!t.has_audio(ThingTemplateAudioType::VoiceMove), "NoSound deletes");
        // the first value of a key stays (std::map::insert), keys are case sensitive
        assert_eq!(t.get_per_unit_sound(b"TurretMove").event_name, b"Move");
        assert_eq!(t.get_per_unit_sound(b"turretmove").event_name, b"Other");
        assert_eq!(t.get_per_unit_sound(b"TurretStop").event_name, b"NoSound");
        assert_eq!(t.get_per_unit_sound(b"Missing").event_name, b"");
        assert_eq!(t.get_per_unit_fx(b"Boom"), Some(&b"BoomFX"[..]));
        assert_eq!(t.get_per_unit_fx(b"Quiet"), None);
        assert_eq!(t.per_unit_fx().len(), 2);
        assert_eq!(t.get_asset_scale(), 1.25);
        let g = t.get_template_geometry_info();
        assert_eq!((g.get_geom_type(), g.get_major_radius(), g.get_minor_radius(), g.get_height(), g.get_is_small()), (GeometryType::Box, 12.0, 6.0, 9.0, true));
        assert_eq!(t.get_shadow_type(), 3);
        assert_eq!(t.get_shadow_size_x(), 3.0);
        assert_eq!(t.get_shadow_texture_name(), b"MyShadow");
        assert_eq!(t.get_occlusion_delay(), 30);
        assert_eq!(t.get_instance_scale_fuzziness(), 0.5);
        assert_eq!(t.get_structure_rubble_height(), 7.0);
        assert_eq!(t.get_threat_value(), 40);
        assert_eq!(t.get_max_simultaneous_of_type(None), 3);
        assert_ne!(t.get_max_simultaneous_link_key(), NameKeyType::INVALID);
        assert_eq!((t.get_crusher_level(), t.get_crushable_level()), (2, 1));
        assert_eq!(t.armor_template_sets().len(), 1);
        assert!(t.armor_template_sets()[0].get_armor_template().is_some() && t.armor_template_sets()[0].get_damage_fx().is_some());
        assert_eq!(t.get_weapon_template_sets().len(), 1);
        assert!(t.can_possibly_have_any_weapon());
        assert_eq!(t.get_weapon_template_sets()[0].thing_template(), Some(1));
    }

    #[test]
    fn max_simultaneous_takes_the_superweapon_keyword() {
        let mut c = Ctx::new();
        load(&mut c, "Object S\n MaxSimultaneousOfType = DeterminedBySuperweaponRESTRICTION\nEnd\n").unwrap();
        assert!(root(&c, "S").max_simultaneous_determined_by_superweapon_restriction());
        assert_eq!(root(&c, "S").get_max_simultaneous_of_type(Some(7)), 7);
        assert_eq!(root(&c, "S").get_max_simultaneous_of_type(None), 0);
        let e = load(&mut c, "Object S\n MaxSimultaneousOfType = 70000\nEnd\n").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::Bug);
        load(&mut c, "Object S\n MaxSimultaneousOfType = 4\nEnd\n").unwrap();
        assert!(!root(&c, "S").max_simultaneous_determined_by_superweapon_restriction());
        assert_eq!(root(&c, "S").get_max_simultaneous_of_type(Some(7)), 4);
    }

    #[test]
    fn errors_leave_the_template_filled_as_far_as_it_got() {
        let mut c = Ctx::new();
        let e = load(&mut c, "Object Bad\n VisionRange = 9\n SkillPointValue = 1 2 x 4\n VisionRange = 10\nEnd\n").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        let t = root(&c, "Bad");
        assert_eq!(t.friend_calc_vision_range(), 9.0);
        assert_eq!(t.raw_skill_point_value(VeterancyLevel::Regular), 1);
        assert_eq!(t.raw_skill_point_value(VeterancyLevel::Elite), -999);
        // not validated: the shadow texture was not picked
        assert_eq!(t.get_shadow_texture_name(), b"");
        assert_eq!(load(&mut c, "Object B2\n Nonsense = 1\nEnd\n").unwrap_err().kind, IniErrorKind::UnknownToken);
        assert!(load(&mut c, "Object B3\n Shadow = +SHADOW_DECAL\nEnd\n").unwrap_err().is_undefined());
        // a label that translates to nothing
        assert_eq!(load(&mut c, "Object B4\n DisplayName = NOTEXT\nEnd\n").unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(c.factory.template_count(), 4);
        // Locomotor without an AIUpdate
        assert_eq!(load(&mut c, "Object B5\n Locomotor = SET_NORMAL Foo\nEnd\n").unwrap_err().kind, IniErrorKind::InvalidData);
    }

    #[test]
    fn byte_members_wrap_as_the_signed_byte_does() {
        let mut c = Ctx::new();
        load(&mut c, "Object W\n StructureRubbleHeight = 133\n TransportSlotCount = 200\nEnd\n").unwrap();
        assert_eq!(root(&c, "W").get_structure_rubble_height(), -123.0);
        assert_eq!(root(&c, "W").get_raw_transport_slot_count(), -56);
    }

    #[test]
    fn a_module_that_is_not_translated_is_an_error_never_skipped() {
        let mut c = Ctx::new();
        c.modules.init(&mut c.names).unwrap();
        let e = load(&mut c, "Object M\n Behavior = SpecialAbilityUpdate ModuleTag_X\n  HealingAmount = 3\n End\nEnd\n").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::ModuleNotTranslated(b"SpecialAbilityUpdate".to_vec()));
        assert_eq!(root(&c, "M").get_behavior_module_info().get_count(), 0);
        let e = load(&mut c, "Object M2\n Body = FlightDeckBehavior ModuleTag_B\n End\nEnd\n").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::ModuleNotTranslated(b"FlightDeckBehavior".to_vec()));
        let e = load(&mut c, "Object M3\n Behavior = NoSuchThing ModuleTag_B\n End\nEnd\n").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::ModuleUnknown(b"NoSuchThing".to_vec()));
        let e = load(&mut c, "Object M4\n Behavior = PrisonBehavior ModuleTag_B\n End\nEnd\n").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::ModuleNotInShippedBuild(b"PrisonBehavior".to_vec()));
        assert!(e.thrown().is_none(), "not something upstream throws");
    }

    #[test]
    fn behavior_and_body_lines_make_module_lists_and_check_the_body_rule() {
        let mut c = Ctx::new();
        load(
            &mut c,
            "Object M\n Behavior = TestUpdate ModuleTag_U\n End\n Behavior = TestDie ModuleTag_D\n End\n Body = TestBody ModuleTag_B\n End\nEnd\n",
        )
        .unwrap();
        let mi = root(&c, "M").get_behavior_module_info();
        assert_eq!(tags(mi), ["ModuleTag_U", "ModuleTag_D", "ModuleTag_B"]);
        assert_eq!(mi.get_nth_name(2), b"TestBody");
        assert_eq!(mi.nuggets()[2].interface_mask, MODULEINTERFACE_BODY);
        assert!(matches!(mi.get_nth_data(0), Some(ModuleInfoData::Logic(_))));
        assert_eq!(mi.get_nth_name(9), b"");
        assert!(mi.get_nth_data(-1).is_none());
        // a Body line with a non-body module, and a Behavior line with a body
        assert_eq!(load(&mut c, "Object N1\n Body = TestUpdate ModuleTag_U\n End\nEnd\n").unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(load(&mut c, "Object N2\n Behavior = TestBody ModuleTag_U\n End\nEnd\n").unwrap_err().kind, IniErrorKind::InvalidData);
        // the tag is required
        assert_eq!(load(&mut c, "Object N3\n Behavior = TestUpdate\nEnd\n").unwrap_err().kind, IniErrorKind::InvalidData);
        // a module block needs its End and takes no fields of its own (null parse table)
        assert!(load(&mut c, "Object N4\n Behavior = TestUpdate ModuleTag_U\n  Foo = 1\n End\nEnd\n").unwrap_err().is_undefined());
    }

    #[test]
    fn a_template_copied_from_the_default_drops_default_modules_the_new_one_replaces() {
        let mut c = Ctx::new();
        load(
            &mut c,
            "Object DefaultThingTemplate\n Behavior = TestUpdate ModuleTag_DefaultUpdate\n End\n Behavior = TestDie ModuleTag_DefaultDie\n End\n \
ArmorSet\n Conditions = None\n End\n Side = Nobody\n Scale = 2\nEnd\n\
Object Child\n Behavior = TestUpdate2 ModuleTag_Own\n End\nEnd\n",
        )
        .unwrap();
        let d = root(&c, "DefaultThingTemplate");
        assert_eq!(tags(d.get_behavior_module_info()), ["ModuleTag_DefaultUpdate", "ModuleTag_DefaultDie"]);
        assert!(!d.get_behavior_module_info().nuggets()[0].copied_from_default);
        let child = root(&c, "Child");
        // the update module of the default is gone (same interface); the die module stays
        assert_eq!(tags(child.get_behavior_module_info()), ["ModuleTag_DefaultDie", "ModuleTag_Own"]);
        assert!(child.get_behavior_module_info().nuggets()[0].copied_from_default);
        assert_eq!(child.get_default_owning_side(), b"Nobody");
        assert_eq!(child.get_asset_scale(), 2.0);
        assert_eq!(child.get_template_id(), 2);
        assert_eq!(child.armor_template_sets().len(), 1, "the default's armor set is kept until the child says its own");
        assert!(child.armor_copied_from_default());
        // a template that says its own armor set replaces the default's
        load(&mut c, "Object Other\n ArmorSet\n  Conditions = ELITE\n End\nEnd\n").unwrap();
        assert_eq!(root(&c, "Other").armor_template_sets().len(), 1);
        assert!(!root(&c, "Other").armor_copied_from_default());
        assert!(root(&c, "Other").armor_template_sets()[0].conditions().test(crate::logic::armor_set::ARMORSET_ELITE));
        // an armor set block that fails still dropped the default's
        let _ = load(&mut c, "Object Other2\n ArmorSet\n  Bogus = 1\n End\nEnd\n").unwrap_err();
        assert!(root(&c, "Other2").armor_template_sets().is_empty());
    }

    #[test]
    fn inheritable_and_overrideable_modules_survive_or_go_by_the_rules() {
        let mut c = Ctx::new();
        load(
            &mut c,
            "Object DefaultThingTemplate\n InheritableModule\n  Behavior = TestUpdate ModuleTag_Inherit\n  End\n End\n \
OverrideableByLikeKind\n  Behavior = TestDie ModuleTag_Like\n  End\n End\nEnd\n\
Object Plain\n Behavior = TestUpdate2 ModuleTag_Own\n End\n Behavior = TestDie ModuleTag_OwnDie\n End\nEnd\n\
Object Vehicle\n KindOf = VEHICLE\n Behavior = TestUpdate2 ModuleTag_Own\n End\nEnd\n\
Object VehicleDie2\n KindOf = VEHICLE\n Behavior = TestDie2 ModuleTag_Own\n End\nEnd\n\
Object VehicleDie\n KindOf = VEHICLE\n Behavior = TestDie ModuleTag_Own\n End\nEnd\n\
Object StructureDie2\n KindOf = STRUCTURE VEHICLE\n Behavior = TestDie2 ModuleTag_Own\n End\nEnd\n\
Object NotACandidate\n KindOf = SELECTABLE\n Behavior = TestDie2 ModuleTag_Own\n End\nEnd\n",
        )
        .unwrap();
        let d = root(&c, "DefaultThingTemplate").get_behavior_module_info();
        assert_eq!(tags(d), ["ModuleTag_Inherit", "ModuleTag_Like"]);
        assert!(d.nuggets()[0].inheritable && d.nuggets()[1].overrideable_by_like_kind && !d.nuggets()[0].overrideable_by_like_kind);
        assert_eq!(root(&c, "DefaultThingTemplate").module_parsing_mode(), ModuleParseMode::Normal);
        // the inheritable one is always kept; the like-kind die module goes when the new die module is the same
        // class, the template is immune to the GPS scrambler, or it is not one of the candidates
        assert_eq!(tags(root(&c, "Plain").get_behavior_module_info()), ["ModuleTag_Inherit", "ModuleTag_Own", "ModuleTag_OwnDie"]);
        assert_eq!(tags(root(&c, "Vehicle").get_behavior_module_info()), ["ModuleTag_Inherit", "ModuleTag_Like", "ModuleTag_Own"]);
        assert_eq!(tags(root(&c, "VehicleDie2").get_behavior_module_info()), ["ModuleTag_Inherit", "ModuleTag_Like", "ModuleTag_Own"]);
        assert_eq!(tags(root(&c, "VehicleDie").get_behavior_module_info()), ["ModuleTag_Inherit", "ModuleTag_Own"]);
        assert_eq!(tags(root(&c, "StructureDie2").get_behavior_module_info()), ["ModuleTag_Inherit", "ModuleTag_Own"]);
        assert_eq!(tags(root(&c, "NotACandidate").get_behavior_module_info()), ["ModuleTag_Inherit", "ModuleTag_Own"]);
    }

    #[test]
    fn auto_heal_default_is_dropped_by_untrainable_templates() {
        let mut c = Ctx::new();
        load(
            &mut c,
            "Object DefaultThingTemplate\n InheritableModule\n  Behavior = TestUpdate ModuleTag_DefaultAutoHealBehavior\n  End\n End\n \
InheritableModule\n  Behavior = TestDie ModuleTag_KeepMe\n  End\n End\nEnd\n\
Object Untrained\n Behavior = TestUpdate2 ModuleTag_A\n End\nEnd\n\
Object Trained\n IsTrainable = Yes\n Behavior = TestUpdate2 ModuleTag_A\n End\nEnd\n",
        )
        .unwrap();
        assert_eq!(tags(root(&c, "Untrained").get_behavior_module_info()), ["ModuleTag_KeepMe", "ModuleTag_A"]);
        assert_eq!(tags(root(&c, "Trained").get_behavior_module_info()), ["ModuleTag_DefaultAutoHealBehavior", "ModuleTag_KeepMe", "ModuleTag_A"]);
    }

    #[test]
    fn add_remove_and_replace_module_work_in_override_loads() {
        let mut c = Ctx::new();
        load(&mut c, "Object Unit\n Behavior = TestUpdate ModuleTag_U\n End\n Behavior = TestDie ModuleTag_D\n End\nEnd\n").unwrap();
        // in an override load a module line outside AddModule is an error
        assert_eq!(
            load_as(&mut c, "Object Unit\n Behavior = TestUpdate2 ModuleTag_V\n End\nEnd\n", IniLoadType::CreateOverrides).unwrap_err().kind,
            IniErrorKind::InvalidData
        );
        // the override copy is the final override, the root is untouched
        load_as(
            &mut c,
            "Object Unit\n RemoveModule = ModuleTag_D\n AddModule\n  Behavior = TestDie ModuleTag_D2\n  End\n End\n \
ReplaceModule ModuleTag_U\n  Behavior = TestUpdate ModuleTag_U2\n  End\n End\n VisionRange = 77\nEnd\n",
            IniLoadType::CreateOverrides,
        )
        .unwrap();
        // (the first, failed, override left its copy: two copies)
        assert_eq!(tags(root(&c, "Unit").get_behavior_module_info()), ["ModuleTag_U", "ModuleTag_D"]);
        assert_eq!(tags(final_of(&c, "Unit").get_behavior_module_info()), ["ModuleTag_D2", "ModuleTag_U2"]);
        assert_eq!(final_of(&c, "Unit").friend_calc_vision_range(), 77.0);
        assert_eq!(root(&c, "Unit").friend_calc_vision_range(), 0.0);
        assert_eq!(final_of(&c, "Unit").module_parsing_mode(), ModuleParseMode::Normal);
        // removing a tag that is not there throws, and the mode stays (the next retry is refused)
        let e = load_as(&mut c, "Object Unit\n RemoveModule = ModuleTag_Nope\nEnd\n", IniLoadType::CreateOverrides).unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        assert_eq!(final_of(&c, "Unit").module_parsing_mode(), ModuleParseMode::AddRemoveReplace);
        // replacing with a different class, or with the same tag, throws
        let mut c2 = Ctx::new();
        load(&mut c2, "Object Unit\n Behavior = TestUpdate ModuleTag_U\n End\nEnd\n").unwrap();
        let e = load_as(
            &mut c2,
            "Object Unit\n ReplaceModule ModuleTag_U\n  Behavior = TestUpdate2 ModuleTag_V\n  End\n End\nEnd\n",
            IniLoadType::CreateOverrides,
        )
        .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        assert_eq!(final_of(&c2, "Unit").module_being_replaced_name(), b"TestUpdate");
        assert_eq!(final_of(&c2, "Unit").module_being_replaced_tag(), b"ModuleTag_U");
        let mut c3 = Ctx::new();
        load(&mut c3, "Object Unit\n Behavior = TestUpdate ModuleTag_U\n End\nEnd\n").unwrap();
        let e = load_as(
            &mut c3,
            "Object Unit\n ReplaceModule ModuleTag_U\n  Behavior = TestUpdate ModuleTag_U\n  End\n End\nEnd\n",
            IniLoadType::CreateOverrides,
        )
        .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        // AddModule inside AddModule
        let e = load_as(
            &mut c3,
            "Object Fresh\n AddModule\n  AddModule\n  End\n End\nEnd\n",
            IniLoadType::CreateOverrides,
        )
        .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
    }

    #[test]
    fn draw_and_client_update_blocks_are_kept_as_data() {
        let mut c = Ctx::new();
        load(
            &mut c,
            "Object Drawn\n Draw = W3DModelDraw ModuleTag_Draw\n  OkToChangeModelColor = Yes\n  DefaultConditionState\n   Model = Foo\n   Animation = A B\n  End\n  \
ConditionState = DAMAGED\n   Model = Bar\n  End\n  TransitionState = A B\n  End\n  End\n  ClientUpdate = SwayClientUpdate ModuleTag_Sway\n End\nEnd\n",
        )
        .unwrap();
        let t = root(&c, "Drawn");
        let mi = t.get_draw_module_info();
        assert_eq!(tags(mi), ["ModuleTag_Draw"]);
        assert_eq!(mi.get_nth_name(0), b"W3DModelDraw");
        assert_eq!(mi.nuggets()[0].interface_mask, crate::common::module::MODULEINTERFACE_DRAW);
        let Some(ModuleInfoData::Client(block)) = mi.get_nth_data(0) else { panic!("client data") };
        let lines: Vec<(u32, String)> = block.lines.iter().map(|l| (l.depth, String::from_utf8_lossy(&l.field).into_owned())).collect();
        assert_eq!(
            lines,
            [
                (0, "OkToChangeModelColor".into()),
                (0, "DefaultConditionState".into()),
                (1, "Model".into()),
                (1, "Animation".into()),
                (0, "ConditionState".into()),
                (1, "Model".into()),
                (0, "TransitionState".into())
            ]
        );
        assert_eq!(block.lines[3].args, [b"A".to_vec(), b"B".to_vec()]);
        assert_eq!(block.lines[4].args, [b"DAMAGED".to_vec()]);
        assert_eq!(tags(t.get_client_update_module_info()), ["ModuleTag_Sway"]);
        assert_eq!(t.get_behavior_module_info().get_count(), 0);
        // a draw module needs its tag, and its End
        assert_eq!(load(&mut c, "Object D2\n Draw = W3DModelDraw\nEnd\n").unwrap_err().kind, IniErrorKind::InvalidData);
    }

    #[test]
    fn a_reskin_copies_a_template_and_reads_only_the_look() {
        let mut c = Ctx::new();
        load(
            &mut c,
            "Object Base\n Side = America\n VisionRange = 100\n Geometry = BOX\n GeometryMajorRadius = 5\n Draw = W3DModelDraw ModuleTag_Old\n End\n \
Behavior = TestUpdate ModuleTag_U\n End\nEnd\n\
ObjectReskin Skin Base\n Geometry = CYLINDER\n Draw = W3DModelDraw ModuleTag_New\n End\n FenceWidth = 3\nEnd\n",
        )
        .unwrap();
        let (base, skin) = (c.factory.find_template(b"Base").unwrap(), c.factory.find_template(b"Skin").unwrap());
        assert_eq!(skin.data.get_default_owning_side(), b"America");
        assert_eq!(skin.data.friend_calc_vision_range(), 100.0);
        assert_eq!(skin.data.get_template_geometry_info().get_geom_type(), GeometryType::Cylinder);
        assert_eq!(base.data.get_template_geometry_info().get_geom_type(), GeometryType::Box);
        assert_eq!(skin.data.get_fence_width(), 3.0);
        assert_eq!(skin.data.get_name(), b"Skin");
        assert_eq!(skin.data.get_template_id(), 2);
        assert_eq!(skin.data.reskinned_from(), Some(base.data.node_id()));
        // the draw module of the copy came from the original and was dropped for the new one
        assert_eq!(tags(skin.data.get_draw_module_info()), ["ModuleTag_New"]);
        assert_eq!(tags(skin.data.get_behavior_module_info()), ["ModuleTag_U"]);
        assert!(is_equivalent_to(base, skin) && is_equivalent_to(skin, base));
        // a field that is not a reskin field is an unknown token; a reskin of nothing is an error
        assert_eq!(load(&mut c, "ObjectReskin S2 Base\n VisionRange = 1\nEnd\n").unwrap_err().kind, IniErrorKind::UnknownToken);
        assert_eq!(load(&mut c, "ObjectReskin S3 Missing\nEnd\n").unwrap_err().kind, IniErrorKind::InvalidData);
        // two reskins of one template are equivalent to each other
        load(&mut c, "ObjectReskin Skin2 Base\nEnd\n").unwrap();
        assert!(is_equivalent_to(c.factory.find_template(b"Skin").unwrap(), c.factory.find_template(b"Skin2").unwrap()));
        load(&mut c, "Object Unrelated\nEnd\n").unwrap();
        assert!(!is_equivalent_to(c.factory.find_template(b"Skin").unwrap(), c.factory.find_template(b"Unrelated").unwrap()));
    }

    #[test]
    fn build_variations_make_two_templates_equivalent_and_new_object_picks_one() {
        let mut c = Ctx::new();
        load(&mut c, "Object Boss\n BuildVariations = VarA varb\nEnd\nObject VarA\nEnd\nObject VarB\nEnd\nObject Lonely\n BuildVariations = Nobody\nEnd\n")
            .unwrap();
        let f = &c.factory;
        assert!(is_equivalent_to(f.find_template(b"Boss").unwrap(), f.find_template(b"VarA").unwrap()));
        assert!(is_equivalent_to(f.find_template(b"VarB").unwrap(), f.find_template(b"Boss").unwrap()), "without regard to case");
        assert!(!is_equivalent_to(f.find_template(b"VarA").unwrap(), f.find_template(b"VarB").unwrap()));
        let mut rng = RandomValues::new(1);
        rng.init_game_logic_random(5);
        let mut probe = RandomValues::new(1);
        probe.init_game_logic_random(5);
        let mask = ObjectStatusMaskType::new();
        let plan = f.new_object(Some(&f.find_template(b"Boss").unwrap().data), &mut rng, mask).unwrap();
        let which = probe.game_logic_random_value(0, 1);
        // "varb" is not a template (case sensitive): the original is kept
        let expect = if which == 0 { f.find_template(b"VarA").unwrap().data.get_template_id() } else { f.find_template(b"Boss").unwrap().data.get_template_id() };
        assert_eq!(plan.template_id, expect);
        assert_eq!(rng.get_game_logic_random_seed_crc(), probe.get_game_logic_random_seed_crc());
        // a template without variations makes no random call
        let before = rng.get_game_logic_random_seed_crc();
        let plan = f.new_object(Some(&f.find_template(b"VarA").unwrap().data), &mut rng, mask).unwrap();
        assert_eq!(plan.template_id, f.find_template(b"VarA").unwrap().data.get_template_id());
        assert_eq!(rng.get_game_logic_random_seed_crc(), before);
        assert_eq!(f.new_object(None, &mut rng, mask).unwrap_err(), ThingFactoryError::BadArg);
        let mut events = Vec::new();
        f.new_drawable(Some(&f.find_template(b"VarB").unwrap().data), 5, 42, &mut events).unwrap();
        assert_eq!(events, [ClientEvent::ObjectCreated { template_id: 3, drawable_status_bits: 5, object_id: 42 }]);
        assert_eq!(f.new_drawable(None, 0, 0, &mut events).unwrap_err(), ThingFactoryError::BadArg);
    }

    #[test]
    fn prerequisites_resolve_and_mark_build_facilities() {
        let mut c = Ctx::new();
        load(
            &mut c,
            "Object Barracks\nEnd\nObject Factory\n KindOf = COMMANDCENTER\nEnd\n\
Object Tank\n Prerequisites\n  Object = Factory Barracks\n  Object = Missing\n End\nEnd\n",
        )
        .unwrap();
        // not resolved until the database is post-processed
        assert!(!root(&c, "Barracks").is_build_facility() && !root(&c, "Factory").is_build_facility());
        assert_eq!(root(&c, "Tank").get_prereq_count(), 2);
        assert_eq!(root(&c, "Tank").get_nth_prereq(0).unwrap().prereq_units()[0].name, b"Factory");
        c.factory.post_process_load();
        assert!(root(&c, "Barracks").is_build_facility() && root(&c, "Factory").is_build_facility());
        assert!(!root(&c, "Tank").is_build_facility());
        let p = root(&c, "Tank").get_nth_prereq(0).unwrap();
        assert_eq!(p.prereq_units()[0].unit, Some(2));
        assert_eq!(p.prereq_units()[1].unit, Some(1));
        assert!(p.prereq_units()[0].name.is_empty());
        let p1 = root(&c, "Tank").get_nth_prereq(1).unwrap();
        assert_eq!(p1.prereq_units()[0].unit, None);
        // a second prerequisite block adds an entry (an override load starts afresh)
        load(&mut c, "Object Tank\n Prerequisites\n  Object = Barracks\n End\nEnd\n").unwrap();
        assert_eq!(root(&c, "Tank").get_prereq_count(), 3);
        // an unknown science is an error (the store has none)
        assert_eq!(load(&mut c, "Object T2\n Prerequisites\n  Science = SCIENCE_Nope\n End\nEnd\n").unwrap_err().kind, IniErrorKind::InvalidData);
        // no token on an Object line
        assert_eq!(load(&mut c, "Object T3\n Prerequisites\n  Object =\n End\nEnd\n").unwrap_err().kind, IniErrorKind::InvalidData);
    }

    #[test]
    fn overrides_make_copies_that_reset_removes() {
        let mut c = Ctx::new();
        load(&mut c, "Object A\n VisionRange = 1\n Side = X\nEnd\n").unwrap();
        // a non-override load of an existing name reads into the template (no copy)
        load(&mut c, "Object A\n VisionRange = 2\nEnd\n").unwrap();
        assert_eq!(c.factory.template_count(), 1);
        assert_eq!(root(&c, "A").friend_calc_vision_range(), 2.0);
        assert!(c.factory.find_template(b"A").unwrap().get_next_override().is_none());
        // an override load copies the final override, twice
        load_as(&mut c, "Object A\n VisionRange = 3\nEnd\n", IniLoadType::CreateOverrides).unwrap();
        load_as(&mut c, "Object A\n Side = Y\nEnd\n", IniLoadType::CreateOverrides).unwrap();
        let chain = c.factory.find_template(b"A").unwrap();
        assert_eq!(chain.data.friend_calc_vision_range(), 2.0);
        let f = chain.get_final_override();
        assert_eq!((f.data.friend_calc_vision_range(), f.data.get_default_owning_side()), (3.0, &b"Y"[..]));
        assert_eq!(f.data.get_template_id(), chain.data.get_template_id());
        assert_ne!(f.data.node_id(), chain.data.node_id());
        assert!(f.is_override() && !chain.is_override());
        assert!(is_equivalent_to(chain, f));
        // a name nobody had, in an override load, is made for this map only
        load_as(&mut c, "Object MapOnly\nEnd\n", IniLoadType::CreateOverrides).unwrap();
        assert!(c.factory.find_template(b"MapOnly").unwrap().is_override());
        load(&mut c, "Object Later\nEnd\n").unwrap();
        assert_eq!(c.factory.template_count(), 3);
        // the list is newest first, ids count up
        let names: Vec<Vec<u8>> = c.factory.templates().map(|t| t.data.get_name().to_vec()).collect();
        assert_eq!(names, [b"Later".to_vec(), b"MapOnly".to_vec(), b"A".to_vec()]);
        assert_eq!(c.factory.find_by_template_id(2).unwrap().data.get_name(), b"MapOnly");
        assert!(c.factory.find_by_template_id(9).is_none());
        // MapOnly is in the middle of the list: reset cannot unlink it upstream
        assert_eq!(c.factory.reset().unwrap_err().kind, IniErrorKind::Undefined(Undefined::DanglingTemplateLink));
        assert_eq!(c.factory.template_count(), 2);
        assert!(c.factory.find_template(b"MapOnly").is_none());
        assert!(c.factory.find_template(b"A").unwrap().get_next_override().is_none());
        assert_eq!(root(&c, "A").friend_calc_vision_range(), 2.0);
    }

    #[test]
    fn reset_removes_map_only_templates_at_the_head() {
        let mut c = Ctx::new();
        load(&mut c, "Object Keep1\nEnd\nObject Keep2\nEnd\n").unwrap();
        load_as(&mut c, "Object Gone1\nEnd\nObject Keep1\n Side = Z\nEnd\nObject Gone2\nEnd\n", IniLoadType::CreateOverrides).unwrap();
        assert_eq!(c.factory.template_count(), 4);
        c.factory.reset().unwrap();
        assert_eq!(c.factory.template_count(), 2);
        assert!(c.factory.find_template(b"Gone1").is_none() && c.factory.find_template(b"Gone2").is_none());
        assert_eq!(root(&c, "Keep1").get_default_owning_side(), b"");
        assert_eq!(c.factory.first_template().unwrap().data.get_name(), b"Keep2");
        // ids are not reused
        load(&mut c, "Object New\nEnd\n").unwrap();
        assert_eq!(root(&c, "New").get_template_id(), 5);
    }

    #[test]
    fn a_default_that_has_an_override_chain_cannot_be_copied() {
        let mut c = Ctx::new();
        load(&mut c, "Object DefaultThingTemplate\nEnd\n").unwrap();
        load_as(&mut c, "Object DefaultThingTemplate\n Side = X\nEnd\n", IniLoadType::CreateOverrides).unwrap();
        let e = load(&mut c, "Object Child\nEnd\n").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::Undefined(Undefined::AliasedOverrideChain));
        // and a template made in an override load from a map-only default is marked
        let mut c = Ctx::new();
        load_as(&mut c, "Object DefaultThingTemplate\n Side = D\nEnd\n", IniLoadType::CreateOverrides).unwrap();
        assert!(c.factory.find_template(b"DefaultThingTemplate").unwrap().is_override());
        load(&mut c, "Object Child\nEnd\n").unwrap();
        assert!(c.factory.find_template(b"Child").unwrap().is_override(), "m_isOverride is copied with the default");
        assert_eq!(root(&c, "Child").get_default_owning_side(), b"D");
    }

    #[test]
    fn find_armor_and_weapon_sets_picks_the_best_match() {
        let mut c = Ctx::new();
        load(
            &mut c,
            "Object U\n ArmorSet\n  Conditions = None\n End\n ArmorSet\n  Conditions = VETERAN\n End\n \
WeaponSet\n  Conditions = None\n End\n WeaponSet\n  Conditions = ELITE\n  Weapon = PRIMARY Rifle\n End\nEnd\n",
        )
        .unwrap();
        let mut t = root(&c, "U").clone();
        let mut f = crate::logic::armor_set::ArmorSetFlags::new();
        assert!(!t.find_armor_template_set(&f).unwrap().conditions().any());
        f.set(crate::logic::armor_set::ARMORSET_VETERAN, 1);
        assert!(t.find_armor_template_set(&f).unwrap().conditions().test(crate::logic::armor_set::ARMORSET_VETERAN));
        let mut wf = crate::logic::weapon_set_flags::WeaponSetFlags::new();
        assert!(!t.find_weapon_template_set(&wf).unwrap().has_any_weapons());
        wf.set(crate::logic::weapon_types::WEAPONSET_ELITE, 1);
        assert!(t.find_weapon_template_set(&wf).unwrap().has_any_weapons());
    }

    #[test]
    fn copy_from_keeps_name_id_and_identity() {
        let mut a = ThingTemplate::new(5);
        a.friend_set_template_name(b"A");
        a.friend_set_template_id(7);
        a.friend_set_node_id(100);
        let mut b = ThingTemplate::new(9);
        b.friend_set_template_name(b"B");
        b.friend_set_template_id(8);
        b.friend_set_node_id(200);
        a.copy_from(Some(&b));
        assert_eq!((a.get_name(), a.get_template_id(), a.node_id(), a.get_occlusion_delay()), (&b"A"[..], 7, 100, 9));
        let before = a.get_occlusion_delay();
        a.copy_from(None);
        assert_eq!(a.get_occlusion_delay(), before);
    }
}
