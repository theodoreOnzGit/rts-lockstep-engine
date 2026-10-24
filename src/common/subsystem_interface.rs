// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/SubsystemInterface.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/SubsystemInterface.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, October 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The subsystem framework: the `SubsystemInterface` base class (`init`,
//! `postProcessLoad`, `reset`, `update`, `draw`, and a name) and
//! `SubsystemInterfaceList`, the registry that initialises subsystems in
//! the order they are added, and resets and shuts them down in reverse.
//!
//! # Translation
//!
//! - **No global registry.** Upstream's `TheSubsystemList` is a global that
//!   every subsystem constructor registers itself with. Here the list is an
//!   owned value of the logic, and a subsystem is added by
//!   [`SubsystemInterfaceList::init_subsystem`]. Nothing registers itself.
//! - **No trait objects.** Upstream's list holds `SubsystemInterface*` of many
//!   types. The list here is generic over one subsystem type `S`, and the
//!   logic supplies `S` as an enum of its subsystems (dispatch by enum, not
//!   `dyn`). A subsystem is therefore one `S` value, and each call dispatches
//!   through the trait's static methods.
//! - **The INI loads.** `initSubsystem` loads up to three INI sources after
//!   `init()`: `path1`, `path2` and a directory. Those loads belong to the
//!   INI reader, whose block table lives with the logic. The translation
//!   takes them as the closure `load_ini`, run after `init()` and before the
//!   subsystem is added, in the same place. Passing a no-op closure is the
//!   same as upstream passing `NULL` for all three paths.
//! - **Shutdown order.** `shutdownAll` deletes the subsystems last-added
//!   first. Here it pops them off the end, so each `Drop` runs in that order.
//!   The list's destructor shuts down whatever remains.
//! - **Not translated: the `DUMP_PERF_STATS` build.** Its timing members,
//!   `UPDATE`/`DRAW` wrappers, `dumpTimesForAll` and the `m_allSubsystems`
//!   list exist only for performance dumps, and are not part of the lockstep
//!   state. The release configuration of the upstream game has them off.
//!   In the normal build, `UPDATE()`/`DRAW()` are `update()`/`draw()`, which
//!   is what the trait calls directly.
//! - **`draw()` default.** Upstream's default `draw()` calls `DEBUG_CRASH`,
//!   which is a no-op in release builds (`Debug.h`). The default here does
//!   nothing.

use super::ascii_string::AsciiString;

/// `class SubsystemInterface`: the contract every engine subsystem keeps.
///
/// Implementors store their own name (upstream's `m_name`) and return it from
/// [`SubsystemInterface::name`]. The constructor is the implementor's own
/// (upstream's constructor only makes the data valid, with no defaults).
pub trait SubsystemInterface {
    /// `getName()`.
    fn name(&self) -> &AsciiString;

    /// `setName(name)`. Called by [`SubsystemInterfaceList::init_subsystem`]
    /// before [`SubsystemInterface::init`].
    fn set_name(&mut self, name: AsciiString);

    /// `init()`: assign default values and allocate what the subsystem needs
    /// for its lifetime.
    fn init(&mut self);

    /// `postProcessLoad()`: called for all subsystems after all of them are
    /// initialised, for inter-system dependencies. Empty by default, as
    /// upstream.
    fn post_process_load(&mut self) {}

    /// `reset()`: return to an empty state ready for a new game, keeping
    /// the persistent state and the allocations.
    fn reset(&mut self);

    /// `update()`: per-frame work, called once per pass of the game loop.
    fn update(&mut self);

    /// `draw()`: client-side drawing. Empty by default, as upstream's
    /// release `DEBUG_CRASH` (see the module docs).
    fn draw(&mut self) {}
}

/// `class SubsystemInterfaceList`: the registry of the subsystems `S` the
/// logic runs, in the order they were added.
pub struct SubsystemInterfaceList<S: SubsystemInterface> {
    /// `m_subsystems`. Index order is the order of `init_subsystem` calls.
    subsystems: Vec<S>,
}

impl<S: SubsystemInterface> Default for SubsystemInterfaceList<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: SubsystemInterface> SubsystemInterfaceList<S> {
    /// `SubsystemInterfaceList()`: no subsystems.
    pub fn new() -> Self {
        SubsystemInterfaceList { subsystems: Vec::new() }
    }

    /// `initSubsystem(sys, path1, path2, dirpath, pXfer, name)`.
    ///
    /// Sets the name, calls `init()`, runs the INI loads (`load_ini`, see the
    /// module docs), then adds the subsystem to the list. `init` and the
    /// loads return nothing, so the list grows only when they complete.
    pub fn init_subsystem(&mut self, mut sys: S, name: AsciiString, load_ini: impl FnOnce()) {
        sys.set_name(name);
        sys.init();
        load_ini();
        self.subsystems.push(sys);
    }

    /// `postProcessLoadAll()`: in list order.
    pub fn post_process_load_all(&mut self) {
        for sys in &mut self.subsystems {
            sys.post_process_load();
        }
    }

    /// `resetAll()`: in reverse list order.
    pub fn reset_all(&mut self) {
        for sys in self.subsystems.iter_mut().rev() {
            sys.reset();
        }
    }

    /// `shutdownAll()`: drops the subsystems in reverse list order, then
    /// empties the list.
    pub fn shutdown_all(&mut self) {
        while let Some(sys) = self.subsystems.pop() {
            drop(sys);
        }
    }

    /// The subsystems, in list order.
    pub fn subsystems(&self) -> &[S] {
        &self.subsystems
    }

    /// The subsystems, mutable, in list order. The game loop calls
    /// [`SubsystemInterface::update`] through this, in list order.
    pub fn subsystems_mut(&mut self) -> &mut [S] {
        &mut self.subsystems
    }
}

impl<S: SubsystemInterface> Drop for SubsystemInterfaceList<S> {
    /// `~SubsystemInterfaceList()`: `shutdownAll()`. Upstream also asserts
    /// that the list is empty before that, a debug-only check that is not
    /// repeated here, since a panic in `drop` would abort.
    fn drop(&mut self) {
        self.shutdown_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Probe {
        name: AsciiString,
        id: u8,
        log: Rc<RefCell<Vec<String>>>,
    }

    impl SubsystemInterface for Probe {
        fn name(&self) -> &AsciiString {
            &self.name
        }
        fn set_name(&mut self, name: AsciiString) {
            self.name = name;
        }
        fn init(&mut self) {
            let n = String::from_utf8_lossy(self.name.str()).into_owned();
            self.log.borrow_mut().push(format!("init {} {}", self.id, n));
        }
        fn post_process_load(&mut self) {
            self.log.borrow_mut().push(format!("post {}", self.id));
        }
        fn reset(&mut self) {
            self.log.borrow_mut().push(format!("reset {}", self.id));
        }
        fn update(&mut self) {
            self.log.borrow_mut().push(format!("update {}", self.id));
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            self.log.borrow_mut().push(format!("drop {}", self.id));
        }
    }

    fn probe(id: u8, log: &Rc<RefCell<Vec<String>>>) -> Probe {
        Probe { name: AsciiString::new(), id, log: Rc::clone(log) }
    }

    #[test]
    fn init_sets_name_before_init_then_loads_then_adds() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut list = SubsystemInterfaceList::new();
        let l2 = Rc::clone(&log);
        list.init_subsystem(probe(1, &log), AsciiString::try_from_bytes(b"Audio").unwrap(), move || {
            l2.borrow_mut().push("load 1".to_string());
        });
        assert_eq!(*log.borrow(), vec!["init 1 Audio", "load 1"]);
        assert_eq!(list.subsystems().len(), 1);
        assert_eq!(list.subsystems()[0].name().str(), b"Audio");
    }

    #[test]
    fn reset_is_reverse_and_shutdown_drops_in_reverse() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut list = SubsystemInterfaceList::new();
        for id in 1..=3 {
            list.init_subsystem(probe(id, &log), AsciiString::new(), || {});
        }
        log.borrow_mut().clear();
        list.reset_all();
        list.shutdown_all();
        assert_eq!(
            *log.borrow(),
            vec!["reset 3", "reset 2", "reset 1", "drop 3", "drop 2", "drop 1"]
        );
        assert!(list.subsystems().is_empty());
    }

    #[test]
    fn dropping_the_list_shuts_down_what_remains() {
        let log = Rc::new(RefCell::new(Vec::new()));
        {
            let mut list = SubsystemInterfaceList::new();
            list.init_subsystem(probe(1, &log), AsciiString::new(), || {});
            list.init_subsystem(probe(2, &log), AsciiString::new(), || {});
        }
        assert!(log.borrow().ends_with(&["drop 2".to_string(), "drop 1".to_string()]));
    }

    #[test]
    fn post_process_load_is_in_order_and_update_reaches_each() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut list = SubsystemInterfaceList::new();
        list.init_subsystem(probe(1, &log), AsciiString::new(), || {});
        list.init_subsystem(probe(2, &log), AsciiString::new(), || {});
        log.borrow_mut().clear();
        list.post_process_load_all();
        for sys in list.subsystems_mut() {
            sys.update();
            sys.draw();
        }
        assert_eq!(*log.borrow(), vec!["post 1", "post 2", "update 1", "update 2"]);
    }

    #[test]
    fn default_list_is_empty() {
        let list: SubsystemInterfaceList<Probe> = SubsystemInterfaceList::default();
        assert!(list.subsystems().is_empty());
    }
}
