// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Dict.h
//   GeneralsMD/Code/GameEngine/Source/Common/Dict.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Created: Steven Johnson, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class Dict`: a sorted list of `NameKeyType` -> value pairs (bool, int,
//! real, ASCII or Unicode string), the property bag of map objects, teams,
//! players and waypoints.
//!
//! Upstream shares the pair array between copies (reference-counted,
//! copied on write); here a `Dict` owns its pairs and `clone` copies them,
//! which is the same value semantics. What is kept exactly, because it
//! decides results:
//!
//! - **The key packing.** A pair stores `(key << 8) | type` in 32 bits, so
//!   only the low 24 bits of a key survive (`getName` is `>> 8`); upstream
//!   asserts `key < 2^23` in debug builds only. A key from `2^24` is stored
//!   truncated, and since lookups compare the full key with the stored 24
//!   bits, such a pair is never found again: setting it twice adds two
//!   pairs. Quirk, reproduced.
//! - **The order of pairs**, which `getNthKey` and friends expose: new pairs
//!   are appended, then the whole array is re-sorted by upstream's own
//!   Shell sort (gap halving), which is not stable.
//! - **`remove`** renames the pair to key `0x7fffffff` (stored as
//!   `0xffffff`), sorts, and drops the last pair. If a real pair also has
//!   key `0xffffff`, which of the two is dropped is up to the Shell sort.
//! - **The binary search** of `findPairByKey`, `mid = (lo + hi - 1) >> 1`.
//! - **Whether pair storage exists** (`m_data` non-null): an empty `Dict`
//!   that never had pairs (or was `clear`ed) has none, while one whose
//!   pairs were all removed still has it. This matters only for the
//!   undefined case below.
//!
//! # Where upstream is undefined
//!
//! `getNthBool`/`Int`/`Real`/`AsciiString`/`UnicodeString(n)` index the pair
//! array without a range check when storage exists (debug builds assert):
//! for `n` outside `[0, getPairCount())` they read past the pairs in use.
//! The `try_get_nth_*` methods return [`DictError::IndexOutOfRange`] there;
//! with no storage they return the default value, as upstream does.
//! `MAX_LEN` exceeded is upstream's `throw ERROR_OUT_OF_MEMORY`:
//! [`DictError::TooManyPairs`].

use super::name_key_generator::NameKeyType;

/// `Dict::MAX_LEN`: the most pairs a `Dict` may hold.
pub const MAX_LEN: usize = 32767;

/// `enum Dict::DataType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DataType {
    /// `DICT_NONE`: "invalid key/index".
    None = -1,
    Bool = 0,
    Int = 1,
    Real = 2,
    AsciiString = 3,
    UnicodeString = 4,
}

/// Where upstream throws or is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DictError {
    /// `throw ERROR_OUT_OF_MEMORY`: more than [`MAX_LEN`] pairs.
    TooManyPairs,
    /// `getNth*(n)` with `n` outside the pairs in use (undefined upstream).
    IndexOutOfRange,
}

impl std::fmt::Display for DictError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DictError::TooManyPairs => "Dict: more than MAX_LEN pairs (ERROR_OUT_OF_MEMORY upstream)",
            DictError::IndexOutOfRange => "Dict::getNth*: index outside the pairs in use (undefined upstream)",
        })
    }
}

/// A pair's value. `DictPair`'s `void* m_value` slot holds a `Bool`, `Int`,
/// `Real` or a (pointer-sized) string object.
#[derive(Clone, Debug, PartialEq)]
pub enum DictValue {
    Bool(bool),
    Int(i32),
    Real(f32),
    AsciiString(Vec<u8>),
    /// UTF-16 code units (`WideChar` is 16 bits on Win32).
    UnicodeString(Vec<u16>),
}

impl DictValue {
    fn data_type(&self) -> DataType {
        match self {
            DictValue::Bool(_) => DataType::Bool,
            DictValue::Int(_) => DataType::Int,
            DictValue::Real(_) => DataType::Real,
            DictValue::AsciiString(_) => DataType::AsciiString,
            DictValue::UnicodeString(_) => DataType::UnicodeString,
        }
    }
}

/// `struct DictPair`: the packed key (`(key << 8) | type`, 32 bits) and the
/// value.
#[derive(Clone, Debug, PartialEq)]
struct DictPair {
    packed: u32,
    value: DictValue,
}

impl DictPair {
    /// `getName()`: `(NameKeyType)(m_key >> 8)`.
    fn name(&self) -> u32 {
        self.packed >> 8
    }
}

/// `createKey(keyVal, nt)`: `((UnsignedInt)keyVal << 8) | (UnsignedInt)nt`.
fn create_key(key: NameKeyType, t: DataType) -> u32 {
    (key.0 << 8) | (t as i32 as u32 & 0xff)
}

/// `class Dict`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dict {
    /// `m_data`: `None` for a null pointer; else the pairs in use.
    data: Option<Vec<DictPair>>,
}

impl Dict {
    /// `Dict(0)`: empty, no storage.
    pub fn new() -> Self {
        Dict { data: None }
    }

    /// `Dict(numPairsToPreAllocate)`: with `n > 0`, empty storage exists.
    pub fn with_preallocated(n: i32) -> Result<Self, DictError> {
        let mut d = Dict::new();
        if n != 0 {
            if n > MAX_LEN as i32 {
                return Err(DictError::TooManyPairs);
            }
            if n > 0 {
                d.data = Some(Vec::new());
            }
        }
        Ok(d)
    }

    /// Whether pair storage exists (upstream's `m_data != NULL`).
    pub fn has_storage(&self) -> bool {
        self.data.is_some()
    }

    fn pairs(&self) -> &[DictPair] {
        self.data.as_deref().unwrap_or(&[])
    }

    /// `clear()`: remove all pairs, and the storage.
    pub fn clear(&mut self) {
        self.data = None;
    }

    /// `getPairCount()`.
    pub fn get_pair_count(&self) -> i32 {
        self.pairs().len() as i32
    }

    /// `getNthKey(n)`: `NAMEKEY_INVALID` out of range. The key is the stored
    /// 24 bits.
    pub fn get_nth_key(&self, n: i32) -> NameKeyType {
        match self.nth(n) {
            Some(p) => NameKeyType(p.name()),
            None => NameKeyType::INVALID,
        }
    }

    /// `getNthType(n)`: `DICT_NONE` out of range.
    pub fn get_nth_type(&self, n: i32) -> DataType {
        self.nth(n).map_or(DataType::None, |p| p.value.data_type())
    }

    fn nth(&self, n: i32) -> Option<&DictPair> {
        if n < 0 {
            return None;
        }
        self.pairs().get(n as usize)
    }

    /// `findPairByKey(key)`: binary search on the stored names, comparing
    /// the full key.
    fn find(&self, key: NameKeyType) -> Option<usize> {
        let base = self.pairs();
        let mut min: i32 = 0;
        let mut max: i32 = base.len() as i32;
        // NameKeyType is an int-sized enum: compared as Int.
        let key = key.0 as i32;
        while min < max {
            let mid = ((min + max) - 1) >> 1;
            let mid_key = base[mid as usize].name() as i32;
            if key > mid_key {
                min = mid + 1;
            } else if key < mid_key {
                max = mid;
            } else {
                return Some(mid as usize);
            }
        }
        None
    }

    /// `getType(key)`: `DICT_NONE` if there is no such pair.
    pub fn get_type(&self, key: NameKeyType) -> DataType {
        self.find(key).map_or(DataType::None, |i| self.pairs()[i].value.data_type())
    }

    /// `known(key, d)`.
    pub fn known(&self, key: NameKeyType, d: DataType) -> bool {
        self.get_type(key) == d
    }

    /// The value for `key`, whatever its type (`None` if missing).
    pub fn get(&self, key: NameKeyType) -> Option<&DictValue> {
        self.find(key).map(|i| &self.pairs()[i].value)
    }

    /// `getBool(key, &exists)`: `(value, exists)`, `false` if missing or of
    /// another type.
    pub fn get_bool(&self, key: NameKeyType) -> (bool, bool) {
        match self.get(key) {
            Some(DictValue::Bool(v)) => (*v, true),
            _ => (false, false),
        }
    }

    /// `getInt(key, &exists)`.
    pub fn get_int(&self, key: NameKeyType) -> (i32, bool) {
        match self.get(key) {
            Some(DictValue::Int(v)) => (*v, true),
            _ => (0, false),
        }
    }

    /// `getReal(key, &exists)`.
    pub fn get_real(&self, key: NameKeyType) -> (f32, bool) {
        match self.get(key) {
            Some(DictValue::Real(v)) => (*v, true),
            _ => (0.0, false),
        }
    }

    /// `getAsciiString(key, &exists)`.
    pub fn get_ascii_string(&self, key: NameKeyType) -> (Vec<u8>, bool) {
        match self.get(key) {
            Some(DictValue::AsciiString(v)) => (v.clone(), true),
            _ => (Vec::new(), false),
        }
    }

    /// `getUnicodeString(key, &exists)`.
    pub fn get_unicode_string(&self, key: NameKeyType) -> (Vec<u16>, bool) {
        match self.get(key) {
            Some(DictValue::UnicodeString(v)) => (v.clone(), true),
            _ => (Vec::new(), false),
        }
    }

    /// `getNth*(n)`'s common part: the pair, `None` with no storage, an error
    /// out of range with storage (undefined upstream).
    fn try_nth(&self, n: i32) -> Result<Option<&DictValue>, DictError> {
        match &self.data {
            None => Ok(None),
            Some(p) => {
                if n < 0 || n as usize >= p.len() {
                    Err(DictError::IndexOutOfRange)
                } else {
                    Ok(Some(&p[n as usize].value))
                }
            }
        }
    }

    /// `getNthBool(n)`: `false` if the pair is of another type.
    pub fn try_get_nth_bool(&self, n: i32) -> Result<bool, DictError> {
        Ok(matches!(self.try_nth(n)?, Some(DictValue::Bool(true))))
    }

    /// `getNthInt(n)`.
    pub fn try_get_nth_int(&self, n: i32) -> Result<i32, DictError> {
        Ok(match self.try_nth(n)? {
            Some(DictValue::Int(v)) => *v,
            _ => 0,
        })
    }

    /// `getNthReal(n)`.
    pub fn try_get_nth_real(&self, n: i32) -> Result<f32, DictError> {
        Ok(match self.try_nth(n)? {
            Some(DictValue::Real(v)) => *v,
            _ => 0.0,
        })
    }

    /// `getNthAsciiString(n)`.
    pub fn try_get_nth_ascii_string(&self, n: i32) -> Result<Vec<u8>, DictError> {
        Ok(match self.try_nth(n)? {
            Some(DictValue::AsciiString(v)) => v.clone(),
            _ => Vec::new(),
        })
    }

    /// `getNthUnicodeString(n)`.
    pub fn try_get_nth_unicode_string(&self, n: i32) -> Result<Vec<u16>, DictError> {
        Ok(match self.try_nth(n)? {
            Some(DictValue::UnicodeString(v)) => v.clone(),
            _ => Vec::new(),
        })
    }

    /// `setPrep(key, type)` + the store + `sortPairs()`: the pair with `key`
    /// gets the value (its type may change), or a new pair is appended; then
    /// the array is re-sorted.
    fn set_value(&mut self, key: NameKeyType, value: DictValue) -> Result<(), DictError> {
        let found = self.find(key);
        let needed = self.pairs().len() + usize::from(found.is_none());
        if needed > MAX_LEN {
            return Err(DictError::TooManyPairs);
        }
        let packed = create_key(key, value.data_type());
        let pairs = self.data.get_or_insert_with(Vec::new);
        match found {
            Some(i) => pairs[i] = DictPair { packed, value },
            None => pairs.push(DictPair { packed, value }),
        }
        self.sort_pairs();
        Ok(())
    }

    /// `setBool(key, value)`.
    pub fn set_bool(&mut self, key: NameKeyType, value: bool) -> Result<(), DictError> {
        self.set_value(key, DictValue::Bool(value))
    }

    /// `setInt(key, value)`.
    pub fn set_int(&mut self, key: NameKeyType, value: i32) -> Result<(), DictError> {
        self.set_value(key, DictValue::Int(value))
    }

    /// `setReal(key, value)`.
    pub fn set_real(&mut self, key: NameKeyType, value: f32) -> Result<(), DictError> {
        self.set_value(key, DictValue::Real(value))
    }

    /// `setAsciiString(key, value)`.
    pub fn set_ascii_string(&mut self, key: NameKeyType, value: &[u8]) -> Result<(), DictError> {
        self.set_value(key, DictValue::AsciiString(value.to_vec()))
    }

    /// `setUnicodeString(key, value)`.
    pub fn set_unicode_string(&mut self, key: NameKeyType, value: &[u16]) -> Result<(), DictError> {
        self.set_value(key, DictValue::UnicodeString(value.to_vec()))
    }

    /// `sortPairs()`: upstream's Shell sort, by stored name (not stable).
    fn sort_pairs(&mut self) {
        let Some(p) = self.data.as_mut() else { return };
        let n = p.len() as i32;
        let mut gap = n >> 1;
        while gap > 0 {
            for i in gap..n {
                let mut j = i - gap;
                while j >= 0 {
                    if p[j as usize].name() > p[(j + gap) as usize].name() {
                        p.swap(j as usize, (j + gap) as usize);
                    } else {
                        break;
                    }
                    j -= gap;
                }
            }
            gap >>= 1;
        }
    }

    /// `remove(key)`: false if there is no such pair. The pair is renamed
    /// `0x7fffffff` (stored `0xffffff`) as a `DICT_BOOL`, the array is sorted
    /// and the last pair dropped (see the module docs). `setNameAndType`
    /// clears the value only when the type changes, so a removed `Bool` pair
    /// keeps its value and any other becomes `false`; that is visible only if
    /// the sort leaves it in place of a real `0xffffff` pair.
    pub fn remove(&mut self, key: NameKeyType) -> bool {
        let Some(i) = self.find(key) else { return false };
        let pairs = self.data.as_mut().expect("found a pair, so storage exists");
        let kept = matches!(pairs[i].value, DictValue::Bool(true));
        pairs[i] = DictPair { packed: create_key(NameKeyType(0x7fff_ffff), DataType::Bool), value: DictValue::Bool(kept) };
        self.sort_pairs();
        self.data.as_mut().expect("storage").pop();
        true
    }

    /// `copyPairFrom(that, key)`: copy `that`'s pair with `key` (replacing
    /// ours), or remove ours if `that` has none.
    pub fn copy_pair_from(&mut self, that: &Dict, key: NameKeyType) -> Result<(), DictError> {
        match that.find(key) {
            Some(i) => {
                let p = &that.pairs()[i];
                // setPrep(key, type) then copyFrom: the stored key is that
                // pair's packed key (the same value).
                self.set_value(key, p.value.clone())
            }
            None => {
                if self.find(key).is_some() {
                    self.remove(key);
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_and_removal_quirks() {
        let mut d = Dict::new();
        assert!(!d.has_storage());
        assert_eq!(d.try_get_nth_int(0), Ok(0), "no storage: the default");
        d.set_int(NameKeyType(7), 1).unwrap();
        d.set_int(NameKeyType(3), 2).unwrap();
        assert_eq!(d.get_nth_key(0), NameKeyType(3));
        assert_eq!(d.get_int(NameKeyType(7)), (1, true));
        assert_eq!(d.get_real(NameKeyType(7)), (0.0, false), "wrong type");
        // a key from 2^24 is stored truncated and never found again
        d.set_int(NameKeyType((1 << 24) + 5), 9).unwrap();
        d.set_int(NameKeyType((1 << 24) + 5), 9).unwrap();
        assert_eq!(d.get_pair_count(), 4);
        assert_eq!(d.get_nth_key(1), NameKeyType(5));
        assert!(d.remove(NameKeyType(3)));
        assert!(!d.remove(NameKeyType(3)));
        assert!(d.remove(NameKeyType(7)));
        assert_eq!(d.get_pair_count(), 2);
        assert_eq!(d.try_get_nth_int(2), Err(DictError::IndexOutOfRange));
    }
}
