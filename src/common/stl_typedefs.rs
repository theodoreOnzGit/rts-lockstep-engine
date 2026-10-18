// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/STLTypedefs.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Created: John McDonald
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The comparison functors of `namespace rts` in `STLTypedefs.h`, for the
//! `AsciiString` keys the logic uses: `less_than_nocase<AsciiString>`
//! (the order of `std::map` keys that compare without case) and
//! `equal_to<AsciiString>`.
//!
//! What is **not** translated, and why:
//!
//! - **The `hash<T>` functors** (`hash<AsciiString>`, `hash<NameKeyType>`,
//!   `hash<ObjectID>`, `hash<DrawableID>`) exist only for `hash_map`. Their
//!   values come from the STL implementation (`std::hash<const char*>` and the
//!   `UnsignedInt` hash of the STLport build), and the order in which a
//!   `hash_map` is iterated depends on them. The translation keeps no hash
//!   order: lookups use Rust's `HashMap` (its `Hash`), and anything that is
//!   iterated uses an ordered map (see the plan's "Determinism hazards"). No
//!   value needs to match.
//! - **`equal_to<const char*>`** (`strcmp`) has no caller once the strings are
//!   `AsciiString`.
//! - **`less_than_nocase<UnicodeString>`** waits for `UnicodeString`, which
//!   is not translated yet (see `docs/phase1-porting-graph.md`, layer 2).
//! - **The typedefs** (`AsciiStringList`, `ObjectPointerList`, `VecCoord3D`,
//!   ...) are spellings of `std::list`/`std::vector`/`std::map`; the Rust
//!   code uses those types directly.
//!
//! # Equality and `AsciiString`
//!
//! Upstream's `operator==` for `AsciiString` compares `str()` with `strcmp`,
//! and `str()` of an empty `AsciiString` without a buffer is `""`. So a string
//! with no buffer equals an empty string with one. [`equal_to`] keeps that
//! rule. The derived `PartialEq`/`Hash` of [`AsciiString`] do **not**: they
//! see `None` and `Some(b"")` as different (the difference is recorded in the
//! differential test and in the report on this unit).
//!
//! Verified against the real `rts::less_than_nocase` and `rts::equal_to`
//! templates by `tests/differential_stl_typedefs.rs`.

use super::ascii_string::AsciiString;
use std::cmp::Ordering;

/// `rts::equal_to<AsciiString>`: `operator==`, that is `strcmp(str(), str()) == 0`.
pub fn equal_to(a: &AsciiString, b: &AsciiString) -> bool {
    a.str() == b.str()
}

/// `rts::less_than_nocase<AsciiString>`: `compareNoCase(b) < 0`.
pub fn less_than_nocase(a: &AsciiString, b: &AsciiString) -> bool {
    a.compare_no_case(b.str()) == Ordering::Less
}

/// A key ordered by `rts::less_than_nocase<AsciiString>`, for use in a
/// `BTreeMap` in place of `std::map<AsciiString, V, less_than_nocase>`.
///
/// Like `std::map`, two keys are equal when neither is less than the other,
/// so `"Abc"` and `"aBC"` are the same key. The `Eq` impl follows that
/// (`cmp == Equal`), not the byte equality of `AsciiString`.
#[derive(Clone, Debug, Default)]
pub struct NoCaseAsciiKey(pub AsciiString);

impl Ord for NoCaseAsciiKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.compare_no_case(other.0.str())
    }
}

impl PartialOrd for NoCaseAsciiKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for NoCaseAsciiKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for NoCaseAsciiKey {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn s(bytes: &[u8]) -> AsciiString {
        AsciiString::try_from_bytes(bytes).expect("short string")
    }

    #[test]
    fn nocase_order_ignores_case_and_equal_keys_collapse() {
        assert!(less_than_nocase(&s(b"apple"), &s(b"BANANA")));
        assert!(!less_than_nocase(&s(b"Abc"), &s(b"aBC")));
        let mut m = BTreeMap::new();
        m.insert(NoCaseAsciiKey(s(b"Abc")), 1);
        m.insert(NoCaseAsciiKey(s(b"aBC")), 2);
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn equal_to_treats_no_buffer_as_empty() {
        assert!(equal_to(&AsciiString::new(), &s(b"")));
        assert!(!equal_to(&s(b"a"), &s(b"A")));
    }
}
