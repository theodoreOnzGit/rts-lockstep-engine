//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_ww3d5_misc.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3dexclusionlist.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3dexclusionlist.h

use std::collections::HashMap;

/// `class W3DExclusionListClass` (w3dexclusionlist.h): the names the asset manager keeps when it releases assets.
/// Upstream keeps a reference to the caller's name vector; here the names are owned. `NameHash` maps a name to its
/// index (`StringClass` hashing is case-sensitive).
#[derive(Clone, Debug)]
pub struct W3DExclusionListClass {
    names: Vec<String>,
    name_hash: HashMap<String, i32>,
}

impl W3DExclusionListClass {
    /// `W3DExclusionListClass::W3DExclusionListClass` (w3dexclusionlist.cpp:36): inserts every name with its
    /// index (`HashTemplateClass::Insert` keeps the first of two equal keys is not specified; a repeated name
    /// is simply present either way).
    pub fn new(names: Vec<String>) -> Self {
        let mut name_hash = HashMap::new();
        for (i, n) in names.iter().enumerate() {
            name_hash.insert(n.clone(), i as i32);
        }
        W3DExclusionListClass { names, name_hash }
    }

    /// The names, `Names`.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// `W3DExclusionListClass::Is_Excluded(PrototypeClass *)` (w3dexclusionlist.cpp:48), on the prototype's
    /// name. A munged name (containing '#') is never excluded; the sub-object name after the first '.' is cut off.
    pub fn is_excluded_prototype(&self, proto_name: &str) -> bool {
        // don't preserve munged prototypes
        if proto_name.contains('#') && !crate::logic::modules::plant(18681) {
            return false;
        }
        // chop off the sub-object name if present
        let root_name = match (if crate::logic::modules::plant(18682) { proto_name.rfind('.') } else { proto_name.find('.') }) {
            Some(p) => &proto_name[..p],
            None => proto_name,
        };
        self.is_excluded(root_name)
    }

    /// `W3DExclusionListClass::Is_Excluded(HTreeClass *)` (w3dexclusionlist.cpp:68): the plain name.
    pub fn is_excluded_htree(&self, htree_name: &str) -> bool {
        self.is_excluded(htree_name)
    }

    /// `W3DExclusionListClass::Is_Excluded(HAnimClass *)` (w3dexclusionlist.cpp:75): the name to check is the one
    /// trailing the '.'; no '.' is "should never happen" and not excluded.
    pub fn is_excluded_hanim(&self, hanim_name: &str) -> bool {
        match hanim_name.find('.') {
            Some(p) => if crate::logic::modules::plant(18683) { self.is_excluded(&hanim_name[..p]) } else { self.is_excluded(&hanim_name[p + 1..]) },
            None => false,
        }
    }

    /// `W3DExclusionListClass::Is_Excluded(const char *)` (w3dexclusionlist.cpp:91).
    pub fn is_excluded(&self, root_name: &str) -> bool {
        if crate::logic::modules::plant(18684) {
            self.name_hash.keys().any(|k| k.to_ascii_lowercase() == root_name.to_ascii_lowercase())
        } else {
            self.name_hash.contains_key(root_name)
        }
    }
}
