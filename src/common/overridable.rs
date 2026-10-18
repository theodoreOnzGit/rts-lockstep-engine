// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Overridable.h
//   GeneralsMD/Code/GameEngine/Include/Common/Override.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Electronic Arts Pacific, Do Not Distribute
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Overridable` and `OVERRIDE<T>`: the INI override mechanism of templates.
//! A template (a `LocomotorTemplate`, a `Science`, ...) can be overridden by
//! a later INI file, which creates a new template that hangs off the old one
//! as its **next override**. Every reader of the template goes through
//! `OVERRIDE<T>`, which follows the chain to its end, so all users see the
//! newest definition.
//!
//! # Translation
//!
//! - **The data.** The C++ base class `Overridable` is the chain link, and the
//!   derived template class is the node. Here one generic [`Overridable<T>`]
//!   holds the chain fields and the template's own data in
//!   [`Overridable::data`], so every node of a chain is a `T`.
//! - **Ownership.** Upstream's destructor deletes the next override, so a
//!   link owns the rest of its chain. That is a `Box`: dropping a node drops
//!   its chain. `OVERRIDE<T>` only points at a template, as its `const T*`
//!   does, so it is a borrowed reference ([`Override<'a, T>`]).
//! - **`setNextOverride` returns what it displaces.** Upstream overwrites the
//!   old next override without deleting it, which leaks it ("makes no attempt
//!   to prevent leaks"). Here the displaced chain is handed back to the
//!   caller, which can keep it (as a separate chain) or drop it. Nothing leaks
//!   and nothing is deleted behind the caller's back.
//! - **`deleteOverrides` consumes the node.** Upstream deletes `this` and
//!   returns `NULL` when the node is marked an override. Here it takes the
//!   `Box` by value: it returns `None` after dropping the node and its chain,
//!   or the node itself, with its chain cleaned up, so the caller keeps the
//!   result in place of the old pointer.
//! - **`OVERRIDE`'s null pointer** is `Option::None`. Upstream's `operator->`
//!   and `operator*` return `NULL` for it, which is [`Override::get`] returning
//!   `None`.

/// `class Overridable`, with the template data `T` of its derived class.
#[derive(Clone, Debug, PartialEq)]
pub struct Overridable<T> {
    /// `m_nextOverride`: the newer definition, owned by this link.
    next_override: Option<Box<Overridable<T>>>,
    /// `m_isOverride`: set by the INI parser on the node it created.
    is_override: bool,
    /// The derived class's own fields.
    pub data: T,
}

impl<T> Overridable<T> {
    /// `Overridable()`: no next override, not an override.
    pub fn new(data: T) -> Self {
        Overridable { next_override: None, is_override: false, data }
    }

    /// `getNextOverride()`: the newer definition, or `None` if there is none.
    pub fn get_next_override(&self) -> Option<&Overridable<T>> {
        self.next_override.as_deref()
    }

    /// `getFinalOverride()`: follow the chain to its last node (`self` if
    /// there is no next override).
    pub fn get_final_override(&self) -> &Overridable<T> {
        let mut cur = self;
        while let Some(next) = cur.next_override.as_deref() {
            cur = next;
        }
        cur
    }

    /// `setNextOverride(next)`. Returns the chain it replaced, which upstream
    /// leaks; see the module docs.
    pub fn set_next_override(&mut self, next: Option<Box<Overridable<T>>>) -> Option<Box<Overridable<T>>> {
        std::mem::replace(&mut self.next_override, next)
    }

    /// `friend_getNextOverride()`: the newer definition, mutable, for the
    /// store that cleans up overrides.
    pub fn friend_get_next_override(&mut self) -> Option<&mut Overridable<T>> {
        self.next_override.as_deref_mut()
    }

    /// `friend_getFinalOverride()`, mutable: the last node of the chain.
    pub fn friend_get_final_override(&mut self) -> &mut Overridable<T> {
        let mut cur = self;
        while cur.next_override.is_some() {
            cur = cur.next_override.as_deref_mut().expect("checked above");
        }
        cur
    }

    /// `markAsOverride()`: this node is an INI override, to be removed by
    /// [`Overridable::delete_overrides`].
    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }

    /// Whether [`Overridable::mark_as_override`] has been called on this node.
    pub fn is_override(&self) -> bool {
        self.is_override
    }

    /// `deleteOverrides()`. A marked node is deleted with its whole chain and
    /// `None` is returned. Otherwise the marked nodes further along are
    /// deleted, and this node is returned.
    pub fn delete_overrides(mut self: Box<Self>) -> Option<Box<Overridable<T>>> {
        if self.is_override {
            return None;
        }
        if let Some(next) = self.next_override.take() {
            self.next_override = next.delete_overrides();
        }
        Some(self)
    }
}

/// `template <class T> class OVERRIDE`: a non-owning reference to a template
/// that always reads the final override of its chain.
#[derive(Debug, PartialEq)]
pub struct Override<'a, T> {
    /// `m_overridable`: the template this object points at, which may be
    /// any node of the chain. Reads go to the final override.
    overridable: Option<&'a Overridable<T>>,
}

impl<T> Clone for Override<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Override<'_, T> {}

impl<'a, T> Override<'a, T> {
    /// `OVERRIDE(const T *overridable = NULL)`.
    pub fn new(overridable: Option<&'a Overridable<T>>) -> Self {
        Override { overridable }
    }

    /// `operator=(const T *overridable)`.
    pub fn set(&mut self, overridable: Option<&'a Overridable<T>>) {
        self.overridable = overridable;
    }

    /// `operator->`, `operator*` and the `const T*` conversion: the data of
    /// the final override, or `None` for a null `OVERRIDE`.
    pub fn get(&self) -> Option<&'a T> {
        self.overridable.map(|o| &o.get_final_override().data)
    }

    /// `getNonOverloadedPointer()`: the node this object points at, which
    /// need not be the final override.
    pub fn get_non_overloaded_pointer(&self) -> Option<&'a Overridable<T>> {
        self.overridable
    }
}

#[cfg(test)]
mod tests {
    use super::{Override, Overridable};

    fn chain(values: &[i32]) -> Box<Overridable<i32>> {
        let mut head: Option<Box<Overridable<i32>>> = None;
        for &v in values.iter().rev() {
            let mut node = Box::new(Overridable::new(v));
            node.set_next_override(head.take());
            head = Some(node);
        }
        head.expect("non-empty chain")
    }

    #[test]
    fn final_override_is_the_last_node() {
        let c = chain(&[1, 2, 3]);
        assert_eq!(c.get_final_override().data, 3);
        assert_eq!(c.get_next_override().map(|n| n.data), Some(2));
        assert!(c.get_final_override().get_next_override().is_none());
    }

    #[test]
    fn override_reads_the_final_definition() {
        let c = chain(&[10, 20]);
        let head = Override::new(Some(&*c));
        assert_eq!(head.get(), Some(&20));
        assert_eq!(head.get_non_overloaded_pointer().map(|n| n.data), Some(10));
        let null: Override<'_, i32> = Override::new(None);
        assert_eq!(null.get(), None);
    }

    #[test]
    fn set_next_override_returns_the_displaced_chain() {
        let mut a = Overridable::new(1);
        let old = a.set_next_override(Some(chain(&[2, 3])));
        assert!(old.is_none());
        let displaced = a.set_next_override(None);
        assert_eq!(displaced.map(|d| d.get_final_override().data), Some(3));
        assert!(a.get_next_override().is_none());
    }

    #[test]
    fn delete_overrides_removes_marked_nodes() {
        // 1 -> 2(override) -> 3: the override and everything after it go.
        let mut c = chain(&[1, 2, 3]);
        c.friend_get_next_override().expect("node 2").mark_as_override();
        let kept = c.delete_overrides().expect("root kept");
        assert_eq!(kept.get_final_override().data, 1);
        assert!(kept.get_next_override().is_none());
    }

    #[test]
    fn delete_overrides_of_a_marked_root_returns_none() {
        let mut c = chain(&[1, 2]);
        assert!(!c.is_override());
        c.mark_as_override();
        assert!(c.is_override());
        assert!(c.delete_overrides().is_none());
    }

    #[test]
    fn override_set_retargets_the_reference() {
        let a = chain(&[1]);
        let b = chain(&[5, 6]);
        let mut r = Override::new(None);
        r.set(Some(&*a));
        assert_eq!(r.get(), Some(&1));
        r.set(Some(&*b));
        assert_eq!(r.get(), Some(&6));
    }

    #[test]
    fn friend_final_override_is_mutable() {
        let mut c = chain(&[1, 2, 3]);
        c.friend_get_final_override().data = 30;
        assert_eq!(c.get_final_override().data, 30);
    }
}
