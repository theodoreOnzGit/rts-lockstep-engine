//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 109,238 allocation calls (test: tests/differential_draft_lists.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/predlod.cpp
//!
//! `PredictiveLODOptimizerClass::AllocVisibleObjArrays(num_objects)`. Upstream allocates ONE array of `2 * (num_objects + 1)`
//! `LODHeapNode`s and makes `VisibleObjArray2` point `num_objects + 1` elements into it; here that is one `Vec` and the
//! second array's start offset. `LODHeapNode` is private to predlod.cpp; the node type is a parameter.

use crate::logic::modules::plant;
/// `VisibleObjArray1`, `VisibleObjArray2` and `VisibleObjArraySize` (class statics).
#[derive(Clone, Debug)]
pub struct VisibleObjArrays<N> {
    /// The single allocation (`VisibleObjArray1` is its start).
    pub array: Vec<N>,
    /// `VisibleObjArray2 - VisibleObjArray1`, 0 while unallocated.
    pub array2_offset: usize,
    /// `VisibleObjArraySize`.
    pub size: i32,
}

impl<N> Default for VisibleObjArrays<N> {
    fn default() -> Self {
        VisibleObjArrays { array: Vec::new(), array2_offset: 0, size: 0 }
    }
}

impl<N: Clone + Default> VisibleObjArrays<N> {
    /// `PredictiveLODOptimizerClass::AllocVisibleObjArrays(num_objects)` (predlod.cpp:397): only when the arrays are too
    /// small, free the old one ("Only the first array is actually allocated") and allocate `2 * (num_objects + 1)` nodes;
    /// array 2 starts after the first `num_objects + 1`.
    pub fn alloc_visible_obj_arrays(&mut self, num_objects: i32) {
        if if plant(11670) { self.size <= num_objects } else { self.size < num_objects } {
            self.size = if plant(11674) { num_objects + 1 } else if plant(11673) { self.size } else { num_objects };
            let n = (num_objects + 1) as usize;
            self.array = vec![N::default(); if plant(11672) { 2 * (num_objects as usize) } else { 2 * n }];
            self.array2_offset = if plant(11671) { num_objects as usize } else { n };
        }
    }
}
