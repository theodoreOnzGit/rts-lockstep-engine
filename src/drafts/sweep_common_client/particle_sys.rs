//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp   (`ParticleSystemManager::findParentTemplate`)
//!
//! Excluded (#213): `ParticleSystemTemplate::parseRandomRGBColor` (ParticleSys.cpp:2737): its whole body is inside
//! `#if 0`, upstream's function is empty (the field is parsed by `parseRGBColorKeyframe` and the `RandomVariable`
//! parsers instead).

use crate::client::particle_sys::ParticleSystemManager;

/// `ParticleSystemManager::findParentTemplate(name, parentNum)` (ParticleSys.cpp:3064): "Should really only be called by
/// TheScriptEngine". The `parentNum`-th template (counting down, in map order) whose `m_slaveSystemName` is `name`.
/// An empty name finds nothing. The port keeps `m_templateMap` as a `BTreeMap` (key order); upstream's hash map has no
/// defined order, so which template is the n-th among several parents is unspecified upstream and name-ordered here.
pub fn find_parent_template(m: &ParticleSystemManager, name: &[u8], parent_num: i32) -> Option<Vec<u8>> {
    if name.is_empty() {
        return None;
    }
    let mut parent_num = parent_num;
    for (key, t) in m.templates.iter() {
        if name == t.info.slave_system_name.as_slice() {
            let this = parent_num;
            parent_num -= 1;
            if this == 0 {
                return Some(key.clone());
            }
        }
    }
    None
}
