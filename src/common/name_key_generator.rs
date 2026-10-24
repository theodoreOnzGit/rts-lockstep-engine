// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/NameKeyGenerator.h
//   GeneralsMD/Code/GameEngine/Source/Common/NameKeyGenerator.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Created: Michael Booth, May 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `NameKeyGenerator`: turns strings into small integer keys, the first
//! string seen getting key 1, the next 2, and so on.
//!
//! **The key values are lockstep state.** Module tags, `Dict` entries and
//! many lookups store `NameKeyType`s, and the order in which names are first
//! seen (INI load order, map load order) decides their values. So this keeps
//! upstream's exact structure, not just its mapping: 45,007 hash sockets,
//! each a chain searched newest first, because which bucket a lookup finds
//! decides whether a name gets a new key. That matters for the two lookups
//! together: `nameToKey` hashes the name as written and compares with
//! `strcmp`; `nameToLowercaseKey` hashes the lower-cased name and compares
//! with `_stricmp`, and **stores the name as given**. So `nameToKey("Foo")`
//! then `nameToLowercaseKey("foo")` usually give two different keys (the
//! lowercase hash of "foo" lands in another socket), while
//! `nameToLowercaseKey("FOO")` then `nameToKey("FOO")` may or may not share
//! one, depending on whether the two hashes collide. Upstream quirk,
//! reproduced.
//!
//! Other upstream details kept:
//!
//! - The hash adds `Byte` values, and `Byte` is `char`, signed on MSVC x86
//!   (and on x86-64 g++): bytes from 0x80 are added sign-extended.
//! - Before `init()` or `reset()` the next id is `NAMEKEY_INVALID` (0), so a
//!   name looked up on an uninitialised generator gets key 0.
//! - Names are C strings: a name is read up to its first NUL byte.
//! - The debug-build bucket-depth check (`_DEBUG`/`_INTERNAL` only) is not
//!   translated: it changes nothing but a debug crash.
//!
//! # Where upstream is undefined
//!
//! - `nameToLowercaseKey` passes each `Byte` to `tolower`; for a byte from
//!   0x80 that is a negative `int`, which is undefined for `tolower` (C
//!   standard 7.4). [`NameKeyError::NonAsciiLowercase`].
//! - The id counter is an `UnsignedInt` cast to the enum `NameKeyType`,
//!   whose values end at `0x7fffffff`; a key past that is out of the enum's
//!   range (undefined). [`NameKeyError::KeysExhausted`] (it would take 2^31
//!   distinct names).
//!
//! The `try_*` functions return those errors; the plain ones panic.

/// `enum NameKeyType`: a key handed out by [`NameKeyGenerator`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NameKeyType(pub u32);

impl NameKeyType {
    /// `NAMEKEY_INVALID`.
    pub const INVALID: NameKeyType = NameKeyType(0);
}

/// `NAMEKEY_MAX`: "max ordinal value of a NameKey (some code relies on these
/// fitting into 24 bits safely)". Upstream does not enforce it.
pub const NAMEKEY_MAX: u32 = 1 << 23;

/// The largest value of the enum (`FORCE_NAMEKEYTYPE_LONG`).
const NAMEKEY_ENUM_MAX: u32 = 0x7fff_ffff;

/// `SOCKET_COUNT`: "should be prime, and not close to a power of 2".
pub const SOCKET_COUNT: usize = 45007;

/// Where upstream is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKeyError {
    /// `tolower` of a negative `char` (a byte of 0x80 or more).
    NonAsciiLowercase,
    /// The next key would be past `0x7fffffff`.
    KeysExhausted,
}

impl std::fmt::Display for NameKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NameKeyError::NonAsciiLowercase => {
                "nameToLowercaseKey: tolower of a byte >= 0x80 (undefined upstream)"
            }
            NameKeyError::KeysExhausted => {
                "NameKeyGenerator: key past 0x7fffffff (undefined upstream)"
            }
        })
    }
}

/// One `Bucket`: its key and `m_nameString`.
#[derive(Clone, Debug)]
struct Bucket {
    key: NameKeyType,
    name: Vec<u8>,
}

/// The C-string view of a name: up to its first NUL.
fn c_str(name: &[u8]) -> &[u8] {
    match name.iter().position(|&b| b == 0) {
        Some(n) => &name[..n],
        None => name,
    }
}

/// `calcHashForString`: `result = (result << 5) + result + *pp++`, with
/// `*pp` a signed `Byte`, in `UnsignedInt` (wrapping).
pub fn calc_hash_for_string(name: &[u8]) -> u32 {
    c_str(name).iter().fold(0u32, |r, &b| {
        (r << 5).wrapping_add(r).wrapping_add(b as i8 as i32 as u32)
    })
}

/// `calcHashForLowercaseString`: the same over `tolower(*pp)`; an error for
/// a byte from 0x80 (see the module docs). In the C locale `tolower` maps
/// only `A`-`Z`.
pub fn try_calc_hash_for_lowercase_string(name: &[u8]) -> Result<u32, NameKeyError> {
    let mut r = 0u32;
    for &b in c_str(name) {
        if b >= 0x80 {
            return Err(NameKeyError::NonAsciiLowercase);
        }
        r = (r << 5)
            .wrapping_add(r)
            .wrapping_add(u32::from(b.to_ascii_lowercase()));
    }
    Ok(r)
}

/// `class NameKeyGenerator`.
#[derive(Clone, Debug)]
pub struct NameKeyGenerator {
    /// `m_sockets`: each chain in insertion order; upstream prepends, so it
    /// is searched from the back.
    sockets: Vec<Vec<Bucket>>,
    /// `m_nextID`.
    next_id: u32,
}

impl Default for NameKeyGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl NameKeyGenerator {
    /// The constructor: empty, and `m_nextID = NAMEKEY_INVALID` ("uninitialized
    /// system").
    pub fn new() -> Self {
        NameKeyGenerator {
            sockets: vec![Vec::new(); SOCKET_COUNT],
            next_id: NameKeyType::INVALID.0,
        }
    }

    /// `init()`: free every bucket, keys start at 1 again. (Upstream asserts,
    /// in debug builds only, that it was not initialised before.)
    pub fn init(&mut self) {
        self.free_sockets();
        self.next_id = 1;
    }

    /// `reset()`: free every bucket, keys start at 1 again.
    pub fn reset(&mut self) {
        self.free_sockets();
        self.next_id = 1;
    }

    /// `freeSockets()`.
    fn free_sockets(&mut self) {
        for s in &mut self.sockets {
            s.clear();
        }
    }

    /// The new bucket of `nameToKey`/`nameToLowercaseKey`: key `m_nextID++`,
    /// prepended to its socket.
    fn allocate(&mut self, socket: usize, name: &[u8]) -> Result<NameKeyType, NameKeyError> {
        if self.next_id > NAMEKEY_ENUM_MAX {
            return Err(NameKeyError::KeysExhausted);
        }
        let key = NameKeyType(self.next_id);
        self.next_id += 1;
        self.sockets[socket].push(Bucket {
            key,
            name: name.to_vec(),
        });
        Ok(key)
    }

    /// `nameToKey(name)`: the key of `name` (compared with `strcmp`), a new
    /// one if it was not seen in its socket.
    pub fn try_name_to_key(&mut self, name: &[u8]) -> Result<NameKeyType, NameKeyError> {
        let name = c_str(name);
        let socket = (calc_hash_for_string(name) % SOCKET_COUNT as u32) as usize;
        if let Some(b) = self.sockets[socket].iter().rev().find(|b| b.name == name) {
            return Ok(b.key);
        }
        self.allocate(socket, name)
    }

    /// `nameToKey(name)`; panics where upstream is undefined.
    pub fn name_to_key(&mut self, name: &[u8]) -> NameKeyType {
        self.try_name_to_key(name).unwrap_or_else(|e| panic!("{e}"))
    }

    /// `nameToLowercaseKey(name)`: hashed lower-cased, compared with
    /// `_stricmp` (ASCII case folding), and stored **as given**.
    pub fn try_name_to_lowercase_key(&mut self, name: &[u8]) -> Result<NameKeyType, NameKeyError> {
        let name = c_str(name);
        let socket = (try_calc_hash_for_lowercase_string(name)? % SOCKET_COUNT as u32) as usize;
        if let Some(b) = self.sockets[socket]
            .iter()
            .rev()
            .find(|b| b.name.eq_ignore_ascii_case(name))
        {
            return Ok(b.key);
        }
        self.allocate(socket, name)
    }

    /// `nameToLowercaseKey(name)`; panics where upstream is undefined.
    pub fn name_to_lowercase_key(&mut self, name: &[u8]) -> NameKeyType {
        self.try_name_to_lowercase_key(name)
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// `keyToName(key)`: a linear search, sockets in order, each chain newest
    /// first; the empty string if no bucket has the key.
    pub fn key_to_name(&self, key: NameKeyType) -> &[u8] {
        for s in &self.sockets {
            if let Some(b) = s.iter().rev().find(|b| b.key == key) {
                return &b.name;
            }
        }
        b""
    }

    /// `parseStringAsNameKeyType`: the INI field parser that stores
    /// `nameToKey(ini->getNextToken())`.
    pub fn parse_string_as_name_key_type(
        &mut self,
        token: &[u8],
    ) -> Result<NameKeyType, NameKeyError> {
        self.try_name_to_key(token)
    }
}

/// `class StaticNameKey`: a name whose key is looked up on first use and
/// then cached (`WellKnownKeys.h` is a list of these).
#[derive(Clone, Debug)]
pub struct StaticNameKey {
    key: std::cell::Cell<NameKeyType>,
    name: &'static [u8],
}

impl StaticNameKey {
    /// `StaticNameKey(p)`.
    pub const fn new(name: &'static [u8]) -> Self {
        StaticNameKey {
            key: std::cell::Cell::new(NameKeyType::INVALID),
            name,
        }
    }

    /// `key()`: `nameToKey(m_name)` the first time (while the cached key is
    /// `NAMEKEY_INVALID`), the cached key after that. Upstream's generator
    /// is the global `TheNameKeyGenerator`; here it is passed in.
    ///
    /// **Upstream quirk:** the cache is never cleared, so after the
    /// generator is reset the old key is returned. And a key of 0 (looked up
    /// on an uninitialised generator) is looked up again next time.
    pub fn key(&self, gen: &mut NameKeyGenerator) -> NameKeyType {
        if self.key.get() == NameKeyType::INVALID {
            self.key.set(gen.name_to_key(self.name));
        }
        self.key.get()
    }

    /// The name.
    pub fn name(&self) -> &'static [u8] {
        self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_in_first_seen_order() {
        let mut g = NameKeyGenerator::new();
        assert_eq!(
            g.name_to_key(b"Early"),
            NameKeyType(0),
            "uninitialised: key 0"
        );
        g.init();
        assert_eq!(g.name_to_key(b"Alpha"), NameKeyType(1));
        assert_eq!(g.name_to_key(b"Beta"), NameKeyType(2));
        assert_eq!(g.name_to_key(b"Alpha"), NameKeyType(1));
        assert_eq!(g.name_to_key(b"Alpha\0junk"), NameKeyType(1), "C string");
        assert_eq!(g.key_to_name(NameKeyType(2)), b"Beta");
        assert_eq!(g.key_to_name(NameKeyType(99)), b"");
        assert_eq!(
            g.try_name_to_lowercase_key(b"caf\xe9"),
            Err(NameKeyError::NonAsciiLowercase)
        );
        assert!(
            g.try_name_to_key(b"caf\xe9").is_ok(),
            "nameToKey is defined for high bytes"
        );
        g.reset();
        assert_eq!(g.name_to_key(b"Beta"), NameKeyType(1));
        let k = StaticNameKey::new(b"Beta");
        assert_eq!(k.key(&mut g), NameKeyType(1));
        g.reset();
        g.name_to_key(b"Other");
        assert_eq!(
            k.key(&mut g),
            NameKeyType(1),
            "the cached key survives a reset (upstream quirk)"
        );
    }

    #[test]
    fn signed_byte_hash() {
        // 0xff as a signed char is -1.
        assert_eq!(calc_hash_for_string(b"\xff"), u32::MAX);
        assert_eq!(
            calc_hash_for_string(b"ab"),
            (u32::from(b'a') * 33).wrapping_add(u32::from(b'b'))
        );
    }
}
