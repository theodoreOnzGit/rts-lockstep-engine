//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 10,000 random programs of WWSaveLoad operations (test: tests/differential_draft_saveload.rs);
//! not yet wired into the game. Not covered: `Request_Ref_Counted_Pointer_Remap` (pointer_remap.rs is its own unit), `Force_Link_WWSaveLoad`, the order `On_Post_Load` runs in,
//! registering a factory or sub-system twice and unregistering one that is not linked (upstream asserts / runs off the list). Corrected 2026-11-20 (#215): `Unregister_Persist_Factory` by instance.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/saveload.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/saveload.cpp
//!
//! `SaveLoadSystemClass`: the static registry of sub-systems and persist factories, the pointer remapper and the
//! post-load list. Its statics (`SubSystemListHead`, `FactoryListHead`, `PointerRemapper`, `PostLoadList`) are one
//! process-wide `Mutex`-guarded [`SaveLoadState`]. `Link_*` push on the FRONT of the intrusive lists, so
//! `Find_Sub_System` / `Find_Persist_Factory` return the most recently registered match; the `Vec`s here keep that
//! order (index 0 is the head). The `WWLOG_*`/`WWRELEASE_SAY` timing macros are debug logging only. The
//! `UPDATE_NETWORK` macro of `Post_Load_Processing` is written out. `TIMEGETTIME()` (milliseconds since boot) is a
//! caller-supplied clock.

use super::chunk_save::ChunkSaveClass;
use super::persistfactory::{PersistFactoryClass, PersistObject};
use super::pointer_remap::PointerRemapClass;
use super::saveloadstatus;
use super::saveloadsubsystem::SaveLoadSubSystemClass;
use crate::logic::modules::plant;
use crate::ww::chunkio::ChunkLoad;
use std::sync::{LazyLock, Mutex};

/// The static state of `SaveLoadSystemClass`.
pub struct SaveLoadState {
    /// `SubSystemListHead` .. `NextSubSystem`: index 0 is the head.
    pub sub_systems: Vec<SaveLoadSubSystemClass>,
    /// `FactoryListHead` .. `NextFactory`: index 0 is the head.
    pub factories: Vec<PersistFactoryClass>,
    /// `PointerRemapper`.
    pub pointer_remapper: PointerRemapClass,
    /// `PostLoadList` (an `SList`: `Add_Head` and `Remove_Head`, so index 0 is the head).
    pub post_load_list: Vec<PersistObject>,
}

static STATE: LazyLock<Mutex<SaveLoadState>> =
    LazyLock::new(|| Mutex::new(SaveLoadState { sub_systems: Vec::new(), factories: Vec::new(), pointer_remapper: PointerRemapClass::new(), post_load_list: Vec::new() }));

/// `class SaveLoadSystemClass` (all members are static upstream).
pub struct SaveLoadSystemClass;

impl SaveLoadSystemClass {
    /// `Save(csave, subsystem)`: the sub-system in a chunk of its own id, if it has data.
    pub fn save(csave: &mut ChunkSaveClass, subsystem: &SaveLoadSubSystemClass) -> bool {
        let mut ok = true;
        if subsystem.contains_data() {
            csave.begin_chunk(subsystem.chunk_id());
            if plant(18278) {
                csave.end_chunk();
                ok &= subsystem.save(csave);
            } else {
                ok &= subsystem.save(csave);
                csave.end_chunk();
            }
        }
        ok
    }

    /// `Load(cload, auto_post_load)` (saveload.cpp:74): each chunk is given to the sub-system registered for its id
    /// (unknown ids are skipped); then pointers are remapped and, if `auto_post_load`, the post-load list is run.
    /// The remap results are returned for the caller to apply (upstream writes through the registered pointers).
    pub fn load(cload: &mut ChunkLoad<'_>, auto_post_load: bool) -> (bool, Vec<(usize, Option<usize>)>) {
        STATE.lock().unwrap().pointer_remapper.reset();

        let mut ok = true;

        // Load each chunk we encounter and link the manager into the PostLoad list
        while cload.open_chunk().unwrap_or(false) {
            saveloadstatus::inc_status_count(); // Count the sub systems loaded

            let sys = Self::find_sub_system(cload.cur_chunk_id().unwrap_or(0));
            if let Some(sys) = sys {
                // INIT_SUB_STATUS(sys->Name())
                saveloadstatus::set_status_text(sys.name(), 1);
                ok &= sys.load(cload);
            }
            let _ = cload.close_chunk();
        }

        // Process all of the pointer remap requests
        let results = {
            let mut st = STATE.lock().unwrap();
            let (plain, counted) = st.pointer_remapper.process();
            st.pointer_remapper.reset();
            let mut all = plain;
            all.extend(counted);
            all
        };

        // Call PostLoad on each PersistClass that wanted post-load
        if auto_post_load && !plant(18279) {
            Self::post_load_processing(None, &mut || 0);
        }

        (ok, results)
    }

    /// `Post_Load_Processing(network_callback)` (saveload.cpp:117): `On_Post_Load` on each registered object, newest
    /// first (`Remove_Head`). `UPDATE_NETWORK`: every time more than 20 ms have passed, the callback is called.
    /// `now_ms` is `TIMEGETTIME()`.
    pub fn post_load_processing(mut network_callback: Option<&mut dyn FnMut()>, now_ms: &mut dyn FnMut() -> u32) -> bool {
        let mut time = now_ms();

        loop {
            let obj = {
                let mut st = STATE.lock().unwrap();
                if st.post_load_list.is_empty() {
                    None
                } else {
                    Some(st.post_load_list.remove(0))
                }
            };
            let Some(obj) = obj else { break };

            // UPDATE_NETWORK
            if let Some(cb) = network_callback.as_mut() {
                let time2 = now_ms();
                let elapsed = if plant(18276) { time2.saturating_sub(time) } else { time2.wrapping_sub(time) };
                if if plant(18274) { elapsed >= 20 } else { elapsed > 20 } {
                    cb();
                    if !plant(18275) {
                        time = time2;
                    }
                }
            }

            obj.on_post_load();
            obj.set_post_load_registered(false);
        }
        true
    }

    /// `Register_Sub_System(sys)` -> `Link_Sub_System`: "sys should never be registered twice!".
    pub fn register_sub_system(sys: SaveLoadSubSystemClass) {
        let mut st = STATE.lock().unwrap();
        assert!(!st.sub_systems.contains(&sys));
        st.sub_systems.insert(0, sys);
    }

    /// `Unregister_Sub_System(sys)` -> `Unlink_Sub_System` (upstream loops forever / reads NULL if `sys` is not linked;
    /// here nothing happens).
    pub fn unregister_sub_system(sys: SaveLoadSubSystemClass) {
        let mut st = STATE.lock().unwrap();
        if let Some(i) = st.sub_systems.iter().position(|s| *s == sys) {
            st.sub_systems.remove(i);
        }
    }

    /// `Find_Sub_System(chunk_id)`: "TODO: need a d-s that gives fast searching based on chunk_id!!"
    pub fn find_sub_system(chunk_id: u32) -> Option<SaveLoadSubSystemClass> {
        STATE.lock().unwrap().sub_systems.iter().find(|s| s.chunk_id() == chunk_id).copied()
    }

    /// `Register_Persist_Factory(factory)` -> `Link_Factory`.
    pub fn register_persist_factory(factory: PersistFactoryClass) {
        let mut st = STATE.lock().unwrap();
        // (upstream asserts the factory is not already linked; two factories may share a chunk id)
        if plant(18273) {
            st.factories.push(factory);
        } else {
            st.factories.insert(0, factory);
        }
    }

    /// `Unregister_Persist_Factory(factory)` -> `Unlink_Factory`: removes the factory itself (its instance id; upstream walks the list
    /// comparing addresses), `chunk_id` being its chunk id. Nothing happens if it is not linked (upstream would run off the list).
    pub fn unregister_persist_factory(instance: u32, chunk_id: u32) {
        let mut st = STATE.lock().unwrap();
        let found = if plant(18271) { st.factories.iter().position(|f| f.chunk_id == chunk_id) } else { st.factories.iter().position(|f| f.instance() == instance) };
        if let Some(i) = found {
            st.factories.remove(i);
        }
    }

    /// `Find_Persist_Factory(chunk_id)`.
    pub fn find_persist_factory(chunk_id: u32) -> Option<PersistFactoryClass> {
        let st = STATE.lock().unwrap();
        if plant(18272) {
            st.factories.iter().rev().find(|f| f.chunk_id == chunk_id).copied()
        } else {
            st.factories.iter().find(|f| f.chunk_id == chunk_id).copied()
        }
    }

    /// `Is_Post_Load_Callback_Registered(obj)` (obsolete upstream).
    pub fn is_post_load_callback_registered(obj: &PersistObject) -> bool {
        STATE.lock().unwrap().post_load_list.iter().any(|o| o.same_object(obj))
    }

    /// `Register_Post_Load_Callback(obj)`: once per object, at the head of the list.
    pub fn register_post_load_callback(obj: &PersistObject) {
        if !obj.is_post_load_registered() || plant(18277) {
            obj.set_post_load_registered(true);
            STATE.lock().unwrap().post_load_list.insert(0, obj.clone());
        }
    }

    /// `Register_Pointer(old_pointer, new_pointer)`.
    pub fn register_pointer(old_pointer: usize, new_pointer: usize) {
        STATE.lock().unwrap().pointer_remapper.register_pointer(old_pointer, new_pointer);
    }

    /// `Request_Pointer_Remap(pointer_to_convert)` (`slot` is the caller's token for the pointer's location).
    pub fn request_pointer_remap(slot: usize, current_value: usize, file: &'static str, line: i32) {
        STATE.lock().unwrap().pointer_remapper.request_pointer_remap(slot, current_value, file, line);
    }

    /// `Request_Ref_Counted_Pointer_Remap(pointer_to_convert)`.
    pub fn request_ref_counted_pointer_remap(slot: usize, current_value: usize, file: &'static str, line: i32) {
        STATE.lock().unwrap().pointer_remapper.request_ref_counted_pointer_remap(slot, current_value, file, line);
    }
}

/// `Force_Link_WWSaveLoad()` (saveload.cpp): `FORCE_LINK(Twiddler)`, a linker trick to keep the Twiddler's static
/// factories; in Rust the registrations are explicit (see `twiddler::register_twiddler_factories`).
pub fn force_link_wwsaveload() {
    super::twiddler::register_twiddler_factories();
}
