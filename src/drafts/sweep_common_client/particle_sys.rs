//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_find_parent.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp   (`ParticleSystemManager::findParentTemplate`)
//!
//! Excluded (#213): `ParticleSystemTemplate::parseRandomRGBColor` (ParticleSys.cpp:2737): its whole body is inside
//! `#if 0`, upstream's function is empty (the field is parsed by `parseRGBColorKeyframe` and the `RandomVariable`
//! parsers instead).

use crate::client::particle_sys::ParticleSystemManager;
use crate::logic::modules::plant;

/// `ParticleSystemManager::findParentTemplate(name, parentNum)` (ParticleSys.cpp:3064): "Should really only be called by
/// TheScriptEngine". The `parentNum`-th template (counting down, in map order) whose `m_slaveSystemName` is `name`.
/// An empty name finds nothing. The port keeps `m_templateMap` as a `BTreeMap` (key order); upstream's hash map has no
/// defined order, so which template is the n-th among several parents is unspecified upstream and name-ordered here.
pub fn find_parent_template(m: &ParticleSystemManager, name: &[u8], parent_num: i32) -> Option<Vec<u8>> {
    if name.is_empty() && !plant(15482) {
        return None;
    }
    let mut parent_num = parent_num;
    let order: Vec<(&Vec<u8>, &crate::client::particle_sys::ParticleSystemTemplate)> =
        if plant(15483) { m.templates.iter().rev().collect() } else { m.templates.iter().collect() };
    for (key, t) in order {
        let same = if plant(15484) { name.eq_ignore_ascii_case(t.info.slave_system_name.as_slice()) } else { name == t.info.slave_system_name.as_slice() };
        if plant(15485) {
            // the count goes down for every template, not only the matches
            let this = parent_num;
            parent_num -= 1;
            if same && this == 0 {
                return Some(key.clone());
            }
            continue;
        }
        if same {
            let this = parent_num;
            parent_num -= 1;
            if plant(15486) {
                if parent_num == 0 {
                    return Some(key.clone());
                }
                continue;
            }
            if this == 0 {
                return Some(key.clone());
            }
        }
    }
    None
}
