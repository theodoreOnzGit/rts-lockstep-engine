// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/LatchRestore.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: John K. McDonald, Jr., 09/19/2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `template <typename T> class LatchRestore`: a scope guard that sets a
//! variable to a new value for the lifetime of the guard and restores the
//! old value when the guard is dropped, so that early returns cannot leave
//! the variable latched.
//!
//! Upstream copies the old value into the guard and writes the new value
//! into the variable in its constructor. The destructor writes the saved
//! value back. This translation does the same with `mem::replace`, so `T`
//! needs no `Clone`: the old value is moved out of the variable and moved
//! back on drop. Restore runs in `Drop`, on every exit from the scope,
//! including a panic unwinding through it.
//!
//! # Access while latched
//!
//! The guard holds the only `&mut` to the variable for as long as it lives,
//! so the variable is read and written through the guard (`Deref` and
//! `DerefMut` to `T`). Upstream code that reads the member directly inside
//! the scope (as C++ can, because the member is reachable from `this`) is
//! translated to reading `*latch` inside the scope.
//!
//! Nested guards on the same variable restore in last-in, first-out order,
//! as upstream's do: each guard saves whatever the variable held when it was
//! created, which is the value the enclosing guard latched.
//!
//! Verified against the unchanged header by `tests/differential_latch_override.rs`.

use std::ops::{Deref, DerefMut};

/// `LatchRestore<T>`: holds `dest` latched to a new value, and restores the
/// value it saved when dropped.
#[derive(Debug)]
#[must_use = "the latch restores the variable when this guard is dropped"]
pub struct LatchRestore<'a, T> {
    /// `whereToRestore`: the variable being latched.
    where_to_restore: &'a mut T,
    /// `valueToRestore`. Always `Some` until `Drop` moves it back.
    value_to_restore: Option<T>,
}

impl<'a, T> LatchRestore<'a, T> {
    /// `LatchRestore(T& dest, const T& src)`: saves `dest` and stores `src`
    /// in it. Upstream copies `src`; here it is moved in.
    pub fn new(dest: &'a mut T, src: T) -> Self {
        let saved = std::mem::replace(dest, src);
        LatchRestore { where_to_restore: dest, value_to_restore: Some(saved) }
    }
}

impl<T> Drop for LatchRestore<'_, T> {
    /// `~LatchRestore()`: `whereToRestore = valueToRestore`.
    fn drop(&mut self) {
        if let Some(saved) = self.value_to_restore.take() {
            *self.where_to_restore = saved;
        }
    }
}

impl<T> Deref for LatchRestore<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.where_to_restore
    }
}

impl<T> DerefMut for LatchRestore<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.where_to_restore
    }
}

#[cfg(test)]
mod tests {
    use super::LatchRestore;

    #[test]
    fn restores_on_drop_and_latches_inside() {
        let mut team = 1;
        {
            let mut latch = LatchRestore::new(&mut team, 7);
            assert_eq!(*latch, 7);
            *latch = 9;
            assert_eq!(*latch, 9);
        }
        assert_eq!(team, 1);
    }

    #[test]
    fn restores_on_early_return() {
        fn func(v: &mut i32, fire: bool) -> bool {
            let mut latch = LatchRestore::new(v, 5);
            if fire {
                return true;
            }
            *latch += 1;
            false
        }
        let mut v = 2;
        assert!(func(&mut v, true));
        assert_eq!(v, 2);
        assert!(!func(&mut v, false));
        assert_eq!(v, 2);
    }

    #[test]
    fn nested_latches_restore_last_in_first_out() {
        let mut v = 0;
        let mut outer = LatchRestore::new(&mut v, 1);
        {
            let mut inner = LatchRestore::new(&mut *outer, 2);
            assert_eq!(*inner, 2);
            *inner = 3;
        }
        assert_eq!(*outer, 1);
        drop(outer);
        assert_eq!(v, 0);
    }

    #[test]
    fn works_for_non_copy_values() {
        let mut name = String::from("old");
        {
            let latch = LatchRestore::new(&mut name, String::from("new"));
            assert_eq!(latch.as_str(), "new");
        }
        assert_eq!(name, "old");
    }
}
