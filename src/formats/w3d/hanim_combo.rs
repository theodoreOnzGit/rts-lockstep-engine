// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hanim.cpp, hanim.h
//   (PivotMapClass, NamedPivotMapClass::Add, Update_Pivot_Map;
//   HAnimComboDataClass: its constructors, Copy, Clear, Set_HAnim,
//   Set_Pivot_Map, Set_Frame, Set_Prev_Frame, Set_Weight,
//   Build_Active_Pivot_Map and the getters; HAnimComboClass: both
//   constructors, Clear, Reset, Normalize_Weights, Get_Num_Anims, the
//   Set_/Get_/Peek_ accessors, Append_Anim_Combo_Data,
//   Remove_Anim_Combo_Data, Peek_Anim_Combo_Data)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htree.cpp
//   (HTreeClass::Get_Bone_Index, which Update_Pivot_Map calls)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, hanim by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Animation combinations (`HAnimComboClass`, GitHub #18, #162): several
//! animations, each with a frame, a weight and optionally a per-pivot weight
//! map, which [`super::htree::HTree::combo_update`] (`HTreeClass::
//! Combo_Update`) blends into one pose.
//!
//! # As upstream keeps them
//!
//! - A combo is a `DynamicVectorClass<HAnimComboDataClass *>`: the entries
//!   are pointers, so one entry can be in several combos
//!   ([`HAnimCombo::append_anim_combo_data`]). Here each entry is an
//!   `Arc<RwLock<HAnimComboData>>`, and so is each pivot map (upstream's are
//!   reference counted and shared, and `Normalize_Weights` rewrites them in
//!   place). An entry made by the combo itself (`HAnimComboClass(n)`) is not
//!   `Shared`; `Clear` clears only those and `Reset` drops them all (it
//!   deletes the unshared ones and empties the vector).
//! - An entry's animation (`HAnim`) is held as a handle `M` the caller
//!   resolves to the animation (upstream a reference-counted `HAnimClass *`;
//!   the differential test uses the index of the loaded animation, so that
//!   every entry naming it samples the same object, channel caches and all).
//! - `Set_Frame(f)` moves the old frame into `PrevFrame` first.
//! - A pivot map ([`PivotMap`]) is a `DynamicVectorClass<float>`: a
//!   `NamedPivotMapClass` also keeps (name, weight) pairs, and
//!   `Update_Pivot_Map(tree)` makes the map the tree's size, every weight 1,
//!   then sets each named pivot's weight from the last pair to the first.
//! - `Build_Active_Pivot_Map` replaces the entry's map with a new one, 1
//!   where the animation has motion for the pivot and 0 elsewhere (none
//!   without an animation).
//! - `Normalize_Weights`: with no pivot maps at all, scales the weights of
//!   the entries that have an animation by `1 / total` when the total is not
//!   0 and `|total - 1| > WWMATH_EPSILON`; with a map on every entry, does
//!   the same per pivot (from 1 to the fewest pivots of the animations) to
//!   the maps; otherwise returns `false`.
//!
//! # Upstream quirks that are kept
//!
//! - `HTreeClass::Get_Bone_Index` returns **0**, not -1, for a name it does
//!   not find, so `Update_Pivot_Map`'s `actualPivot != -1` always holds: a
//!   name the tree lacks sets pivot 0's weight.
//! - `Update_Pivot_Map` compares names with `stricmp` (ASCII case-folded)
//!   and the pairs are applied from the last to the first, so of two pairs
//!   naming the same pivot the earlier one wins.
//! - `Normalize_Weights` reads every animation's pivot count before it
//!   checks for a missing animation, so a combo with a missing animation is
//!   undefined there (and in `Combo_Update`, which does the same).
//! - `HAnimComboDataClass::Copy` does not copy `Shared`.
//! - `Normalize_Weights` compares `|total - 1.0|` computed in `double`
//!   (`weight_total - 1.0`) and converted to `float` for `WWMath::Fabs`.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: an entry index outside the vector (`IndexOutOfRange`;
//! upstream asserts and reads past it); a missing animation where the
//! pivot count is read (`NullPointer`); a pivot map read at a pivot at or
//! past its count (`IndexOutOfRange`: past its allocation, or an element
//! `Resize` left uninitialised); `Update_Pivot_Map` on a tree with a negative
//! pivot count (`Allocation`) or whose 16-byte pivot name has no terminator
//! where `stricmp` still needs a byte (`StringOverrun`).
//! `Remove_Anim_Combo_Data` takes the entry to remove by identity
//! (`Arc::ptr_eq`), as upstream compares pointers.

use std::sync::{Arc, RwLock};

use super::assetmgr::HAnim;
use super::htree::HTree;
use super::{W3dResult, W3dUndefined};
use crate::logic::modules::plant;

/// `WWMATH_EPSILON`.
const WWMATH_EPSILON: f32 = 0.0001f32;

/// A shared pivot map (upstream's reference-counted `PivotMapClass *`).
pub type PivotMapRef = Arc<RwLock<PivotMap>>;

/// A shared combo entry (upstream's `HAnimComboDataClass *`).
pub type ComboDataRef<M> = Arc<RwLock<HAnimComboData<M>>>;

/// `NamedPivotMapClass::WeightInfoStruct`: a pivot name and its weight.
#[derive(Clone, Debug, PartialEq)]
pub struct WeightInfo {
    /// `Name` (a C string, without its terminator).
    pub name: Vec<u8>,
    pub weight: f32,
}

/// `PivotMapClass` (a `DynamicVectorClass<float>`), or a
/// `NamedPivotMapClass` when [`PivotMap::weight_info`] is `Some`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PivotMap {
    /// The vector's active elements (`Count()` of them).
    pub weights: Vec<f32>,
    /// `NamedPivotMapClass::WeightInfo`; `None` for a plain `PivotMapClass`
    /// (`As_Named_Pivot_Map()` is null).
    pub weight_info: Option<Vec<WeightInfo>>,
}

impl PivotMap {
    /// `new PivotMapClass`: an empty map.
    pub fn new() -> Self {
        PivotMap::default()
    }

    /// `new NamedPivotMapClass`: an empty named map.
    pub fn new_named() -> Self {
        PivotMap { weights: Vec::new(), weight_info: Some(Vec::new()) }
    }

    /// The map shared (`NEW_REF`).
    pub fn shared(self) -> PivotMapRef {
        Arc::new(RwLock::new(self))
    }

    /// `DynamicVectorClass<float>::Add(w)`.
    pub fn add(&mut self, w: f32) {
        self.weights.push(w);
    }

    /// `Count()`.
    pub fn count(&self) -> usize {
        self.weights.len()
    }

    /// `(*map)[i]` for a read: undefined at or past `Count()`.
    pub fn get(&self, i: i32) -> W3dResult<f32> {
        usize::try_from(i).ok().and_then(|i| self.weights.get(i).copied()).ok_or_else(|| W3dUndefined::IndexOutOfRange.into())
    }

    /// `(*map)[i] = w`: undefined at or past `Count()`.
    pub fn set(&mut self, i: i32, w: f32) -> W3dResult<()> {
        let slot = usize::try_from(i).ok().and_then(|i| self.weights.get_mut(i)).ok_or(W3dUndefined::IndexOutOfRange)?;
        *slot = w;
        Ok(())
    }

    /// `As_Named_Pivot_Map() != NULL`.
    pub fn is_named(&self) -> bool {
        self.weight_info.is_some()
    }

    /// `NamedPivotMapClass::Add(Name, Weight)`: a (name, weight) pair (the
    /// name copied, `nstrdup`). Nothing on a plain map (the method is
    /// `NamedPivotMapClass`'s).
    pub fn add_named(&mut self, name: &[u8], weight: f32) {
        if let Some(info) = &mut self.weight_info {
            info.push(WeightInfo { name: name.to_vec(), weight });
        }
    }

    /// `NamedPivotMapClass::Update_Pivot_Map(Tree)`: the map made the tree's
    /// size, every weight 1, then each pair's pivot (`Get_Bone_Index`) given
    /// its weight, from the last pair to the first. Nothing on a plain map.
    pub fn update_pivot_map(&mut self, tree: &HTree) -> W3dResult<()> {
        let Some(info) = self.weight_info.clone() else { return Ok(()) };
        // first, resize the base pivot map to fit the tree
        let num_pivots = tree.num_pivots();
        if num_pivots < 0 {
            return Err(W3dUndefined::Allocation.into());
        }
        // first, reset all weights to 1 (the default)
        self.weights = vec![1.0f32; num_pivots as usize];
        // for each named pivot, find the correct index and set the weight if
        // the indicated bone is present (Get_Bone_Index gives 0 when it is
        // not, so it always is)
        let order: Vec<&WeightInfo> = if plant(9574) { info.iter().collect() } else { info.iter().rev().collect() };
        for wi in order {
            let actual_pivot = get_bone_index(tree, &wi.name)?;
            if actual_pivot != -1 {
                self.set(actual_pivot, wi.weight)?;
            }
        }
        Ok(())
    }
}

/// `HTreeClass::Get_Bone_Index(name)`: the first pivot whose name `stricmp`
/// finds equal, else **0** (upstream's value for "not found").
pub fn get_bone_index(tree: &HTree, name: &[u8]) -> W3dResult<i32> {
    for i in 0..tree.num_pivots().max(0) {
        let p = tree.pivot(i as usize).ok_or(W3dUndefined::NullPointer)?;
        if stricmp_eq(&p.name, name)? {
            return Ok(i);
        }
    }
    Ok(if plant(9575) { -1 } else { 0 })
}

/// `stricmp(pivot_name, name) == 0` for a 16-byte pivot name (which may lack
/// its terminator) and a C string without its terminator.
fn stricmp_eq(pivot_name: &[u8], name: &[u8]) -> W3dResult<bool> {
    let mut i = 0;
    loop {
        let a = *pivot_name.get(i).ok_or(W3dUndefined::StringOverrun)?;
        let b = name.get(i).copied().unwrap_or(0);
        if !a.eq_ignore_ascii_case(&b) {
            return Ok(false);
        }
        if a == 0 {
            return Ok(true);
        }
        i += 1;
    }
}

/// `HAnimComboDataClass`: one animation of a combo.
#[derive(Clone, Debug)]
pub struct HAnimComboData<M> {
    /// `HAnim` (a handle the caller resolves; `None` for null).
    pub hanim: Option<M>,
    pub frame: f32,
    pub prev_frame: f32,
    pub weight: f32,
    /// `PivotMap`.
    pub pivot_map: Option<PivotMapRef>,
    /// `Shared`: not made by the combo (set by the caller).
    pub shared: bool,
}

impl<M: Clone> HAnimComboData<M> {
    /// `HAnimComboDataClass(shared)`: no animation, frames 0, weight 1, no
    /// map.
    pub fn new(shared: bool) -> Self {
        HAnimComboData { hanim: None, frame: 0.0, prev_frame: 0.0, weight: 1.0, pivot_map: None, shared }
    }

    /// The entry shared (`new HAnimComboDataClass`).
    pub fn into_ref(self) -> ComboDataRef<M> {
        Arc::new(RwLock::new(self))
    }

    /// `Copy(src)`: everything but `Shared` from `src`, or the defaults
    /// (`Shared` kept) without one.
    pub fn copy(&mut self, src: Option<&HAnimComboData<M>>) {
        match src {
            Some(s) => {
                self.hanim = s.hanim.clone();
                self.pivot_map = s.pivot_map.clone();
                self.frame = s.frame;
                self.prev_frame = s.prev_frame;
                self.weight = s.weight;
            }
            None => {
                self.hanim = None;
                self.pivot_map = None;
                self.frame = 0.0;
                self.prev_frame = 0.0;
                self.weight = 1.0;
            }
        }
    }

    /// `Clear()`: no animation, no map, frames 0, weight 1.
    pub fn clear(&mut self) {
        self.hanim = None;
        // not sure if the pivot map should be deleted or just have everything
        // set to one. removing it effectively sets it to one
        self.pivot_map = None;
        self.frame = 0.0;
        self.prev_frame = 0.0;
        self.weight = 1.0;
    }

    /// `Set_HAnim(motion)`.
    pub fn set_hanim(&mut self, motion: Option<M>) {
        self.hanim = motion;
    }

    /// `Set_Frame(frame)`: `PrevFrame = Frame; Frame = frame;`.
    pub fn set_frame(&mut self, frame: f32) {
        if !plant(9576) {
            self.prev_frame = self.frame;
        }
        self.frame = frame;
    }

    /// `Set_Prev_Frame(frame)`.
    pub fn set_prev_frame(&mut self, frame: f32) {
        self.prev_frame = frame;
    }

    /// `Set_Weight(weight)`.
    pub fn set_weight(&mut self, weight: f32) {
        self.weight = weight;
    }

    /// `Set_Pivot_Map(map)`.
    pub fn set_pivot_map(&mut self, map: Option<PivotMapRef>) {
        self.pivot_map = map;
    }

    /// `Build_Active_Pivot_Map()`: a new map, 1 for each pivot the animation
    /// has motion for (`Is_Node_Motion_Present`), else 0; none without an
    /// animation.
    pub fn build_active_pivot_map<'w>(&mut self, motion: impl Fn(&M) -> &'w HAnim) -> W3dResult<()> {
        let Some(h) = &self.hanim else {
            self.pivot_map = None;
            return Ok(());
        };
        let anim = motion(h);
        let numpivots = anim.get_num_pivots();
        let mut map = PivotMap::new();
        let mut count = 0;
        while count < numpivots {
            if anim.is_node_motion_present(count)? || plant(9578) {
                map.add(1.0);
            } else {
                map.add(0.0);
            }
            count += 1;
        }
        self.pivot_map = Some(map.shared());
        Ok(())
    }
}

/// `HAnimComboClass`.
#[derive(Clone, Debug, Default)]
pub struct HAnimCombo<M> {
    /// `HAnimComboData`.
    pub data: Vec<ComboDataRef<M>>,
}

impl<M: Clone> HAnimCombo<M> {
    /// `HAnimComboClass()`: no animations.
    pub fn new() -> Self {
        HAnimCombo { data: Vec::new() }
    }

    /// `HAnimComboClass(num_animations)`: that many entries of its own (not
    /// shared).
    pub fn with_anims(num_animations: usize) -> Self {
        HAnimCombo { data: (0..num_animations).map(|_| HAnimComboData::new(false).into_ref()).collect() }
    }

    fn entry(&self, index: i32) -> W3dResult<&ComboDataRef<M>> {
        usize::try_from(index).ok().and_then(|i| self.data.get(i)).ok_or_else(|| W3dUndefined::IndexOutOfRange.into())
    }

    fn read<R>(&self, index: i32, f: impl FnOnce(&HAnimComboData<M>) -> R) -> W3dResult<R> {
        let e = self.entry(index)?;
        let g = e.read().expect("combo entry lock");
        Ok(f(&g))
    }

    fn write<R>(&self, index: i32, f: impl FnOnce(&mut HAnimComboData<M>) -> R) -> W3dResult<R> {
        let e = self.entry(index)?;
        let mut g = e.write().expect("combo entry lock");
        Ok(f(&mut g))
    }

    /// `Clear()`: clears each entry that is not shared.
    pub fn clear(&mut self) {
        for d in self.data.iter().rev() {
            let mut g = d.write().expect("combo entry lock");
            if !g.shared || plant(9577) {
                g.clear();
            }
        }
    }

    /// `Reset()`: empties the vector (the entries that are not shared are
    /// deleted; the shared ones stay with their owners).
    pub fn reset(&mut self) {
        self.data.clear();
    }

    /// `Get_Num_Anims()`.
    pub fn get_num_anims(&self) -> i32 {
        self.data.len() as i32
    }

    /// `Set_Motion(indx, motion)`.
    pub fn set_motion(&self, index: i32, motion: Option<M>) -> W3dResult<()> {
        self.write(index, |d| d.set_hanim(motion))
    }

    /// `Get_Motion(indx)` (and `Peek_Motion`: the handle either way).
    pub fn get_motion(&self, index: i32) -> W3dResult<Option<M>> {
        self.read(index, |d| d.hanim.clone())
    }

    /// `Peek_Motion(indx)`.
    pub fn peek_motion(&self, index: i32) -> W3dResult<Option<M>> {
        self.get_motion(index)
    }

    /// `Set_Frame(indx, frame)`.
    pub fn set_frame(&self, index: i32, frame: f32) -> W3dResult<()> {
        self.write(index, |d| d.set_frame(frame))
    }

    /// `Set_Prev_Frame(indx, frame)`.
    pub fn set_prev_frame(&self, index: i32, frame: f32) -> W3dResult<()> {
        self.write(index, |d| d.set_prev_frame(frame))
    }

    /// `Get_Frame(indx)`.
    pub fn get_frame(&self, index: i32) -> W3dResult<f32> {
        self.read(index, |d| d.frame)
    }

    /// `Get_Prev_Frame(indx)`.
    pub fn get_prev_frame(&self, index: i32) -> W3dResult<f32> {
        self.read(index, |d| d.prev_frame)
    }

    /// `Set_Weight(indx, weight)`.
    pub fn set_weight(&self, index: i32, weight: f32) -> W3dResult<()> {
        self.write(index, |d| d.set_weight(weight))
    }

    /// `Get_Weight(indx)`.
    pub fn get_weight(&self, index: i32) -> W3dResult<f32> {
        self.read(index, |d| d.weight)
    }

    /// `Set_Pivot_Weight_Map(indx, map)`.
    pub fn set_pivot_weight_map(&self, index: i32, map: Option<PivotMapRef>) -> W3dResult<()> {
        self.write(index, |d| d.set_pivot_map(map))
    }

    /// `Get_Pivot_Weight_Map(indx)` (and `Peek_`).
    pub fn get_pivot_weight_map(&self, index: i32) -> W3dResult<Option<PivotMapRef>> {
        self.read(index, |d| d.pivot_map.clone())
    }

    /// `Peek_Pivot_Weight_Map(indx)`.
    pub fn peek_pivot_weight_map(&self, index: i32) -> W3dResult<Option<PivotMapRef>> {
        self.get_pivot_weight_map(index)
    }

    /// `Append_Anim_Combo_Data(data)`.
    pub fn append_anim_combo_data(&mut self, data: ComboDataRef<M>) {
        self.data.push(data);
    }

    /// `Remove_Anim_Combo_Data(data)`: the first entry that is `data`
    /// (`DynamicVectorClass::Delete` by pointer); nothing when none is.
    pub fn remove_anim_combo_data(&mut self, data: &ComboDataRef<M>) {
        if let Some(i) = self.data.iter().position(|d| Arc::ptr_eq(d, data)) {
            self.data.remove(i);
        }
    }

    /// `Peek_Anim_Combo_Data(index)`.
    pub fn peek_anim_combo_data(&self, index: i32) -> W3dResult<ComboDataRef<M>> {
        self.entry(index).cloned()
    }

    /// The fewest pivots of the animations (`MIN` from 100000; 0 when there
    /// are none), as `Combo_Update` and `Normalize_Weights` compute it: a
    /// missing animation is dereferenced (undefined).
    pub fn min_anim_pivots<'w>(&self, motion: &impl Fn(&M) -> &'w HAnim) -> W3dResult<i32> {
        let mut num_anim_pivots = 100000i32;
        for anim_idx in 0..self.get_num_anims() {
            let m = self.peek_motion(anim_idx)?.ok_or(W3dUndefined::NullPointer)?;
            num_anim_pivots = num_anim_pivots.min(motion(&m).get_num_pivots());
        }
        if num_anim_pivots == 100000 {
            num_anim_pivots = 0;
        }
        Ok(num_anim_pivots)
    }

    /// `Normalize_Weights()` (see the module docs); `true` when it
    /// succeeded.
    pub fn normalize_weights<'w>(&self, motion: impl Fn(&M) -> &'w HAnim) -> W3dResult<bool> {
        let anim_count = self.get_num_anims();
        if anim_count == 0 {
            return Ok(true); // Trivially succeeded
        }
        // Loop over all anims. Check if all or none have pivot maps, and also
        // calculate the minimum number of pivots.
        let mut all_pivot_maps = true;
        let mut none_pivot_maps = true;
        let mut num_anim_pivots = 100000i32;
        for anim_idx in 0..anim_count {
            let m = self.peek_motion(anim_idx)?.ok_or(W3dUndefined::NullPointer)?;
            num_anim_pivots = num_anim_pivots.min(motion(&m).get_num_pivots());
            let has_pivot_map = self.peek_pivot_weight_map(anim_idx)?.is_some();
            all_pivot_maps &= has_pivot_map;
            none_pivot_maps &= !has_pivot_map;
        }
        if num_anim_pivots == 100000 {
            num_anim_pivots = 0;
        }
        if none_pivot_maps {
            // Calculate total weight of all active anims, ensure it is very
            // close to 1.
            let mut weight_total = 0.0f32;
            for anim_idx in 0..anim_count {
                if self.peek_motion(anim_idx)?.is_some() {
                    weight_total += self.get_weight(anim_idx)?;
                }
            }
            // weight_total should be very close to 1. If not, normalize this
            // pivot's weights
            if needs_normalizing(weight_total) {
                let oo_total = 1.0f32 / weight_total;
                for anim_idx in 0..anim_count {
                    if self.peek_motion(anim_idx)?.is_some() {
                        let w = self.get_weight(anim_idx)? * oo_total;
                        self.set_weight(anim_idx, w)?;
                    }
                }
            }
        } else if all_pivot_maps {
            // For each pivot, calculate total weight of all active anims,
            // ensure close to 1.
            for piv_idx in 1..num_anim_pivots {
                let mut weight_total = 0.0f32;
                for anim_idx in 0..anim_count {
                    if self.peek_motion(anim_idx)?.is_some() {
                        let map = self.peek_pivot_weight_map(anim_idx)?.ok_or(W3dUndefined::NullPointer)?;
                        let weight = self.get_weight(anim_idx)? * map.read().expect("pivot map lock").get(piv_idx)?;
                        weight_total += weight;
                    }
                }
                if needs_normalizing(weight_total) {
                    let oo_total = 1.0f32 / weight_total;
                    for anim_idx in 0..anim_count {
                        if self.peek_motion(anim_idx)?.is_some() {
                            let map = self.get_pivot_weight_map(anim_idx)?.ok_or(W3dUndefined::NullPointer)?;
                            let mut g = map.write().expect("pivot map lock");
                            let new_weight = if plant(9593) { g.get(piv_idx)? } else { g.get(piv_idx)? * oo_total };
                            g.set(piv_idx, new_weight)?;
                        }
                    }
                }
            }
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}

/// `weight_total != 0.0 && WWMath::Fabs(weight_total - 1.0) >
/// WWMATH_EPSILON`: the difference in `double`, converted to `float` for
/// `Fabs`.
fn needs_normalizing(weight_total: f32) -> bool {
    let diff = (f64::from(weight_total) - 1.0f64) as f32;
    f64::from(weight_total) != 0.0 && crate::ww::wwmath::fabs(diff) > WWMATH_EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::htree::Pivot;
    use crate::ww::matrix3d::Matrix3D;

    fn name16(s: &[u8]) -> [u8; 16] {
        let mut n = [0u8; 16];
        n[..s.len()].copy_from_slice(s);
        n
    }

    fn tree(names: &[&[u8]]) -> HTree {
        HTree {
            name: name16(b"T"),
            num_pivots: names.len() as i32,
            pivots: names.iter().enumerate().map(|(i, n)| Pivot::new(name16(n), if i == 0 { None } else { Some(0) }, Matrix3D::IDENTITY, i as i32)).collect(),
            scale_factor: 1.0,
        }
    }

    #[test]
    fn update_pivot_map_quirks() {
        let t = tree(&[b"ROOT", b"Arm", b"LEG"]);
        let mut m = PivotMap::new_named();
        m.add_named(b"arm", 0.25);
        m.add_named(b"ARM", 0.5);
        m.add_named(b"nothing", 0.75);
        m.update_pivot_map(&t).unwrap();
        // the earlier of two pairs wins; an unknown name sets pivot 0
        assert_eq!(m.weights, vec![0.75, 0.25, 1.0]);
        // a 16-character name with no terminator: undefined only where
        // stricmp still needs a byte
        let mut long = tree(&[b"ROOT"]);
        long.pivots[0].name = *b"ABCDEFGHIJKLMNOP";
        let mut m = PivotMap::new_named();
        m.add_named(b"abcdefghijklmnop", 2.0);
        assert!(m.update_pivot_map(&long).is_err());
        let mut m = PivotMap::new_named();
        m.add_named(b"abx", 2.0);
        m.update_pivot_map(&long).unwrap();
        assert_eq!(m.weights, vec![2.0]);
    }

    #[test]
    fn set_frame_keeps_prev_and_clear_skips_shared() {
        let mut c: HAnimCombo<usize> = HAnimCombo::with_anims(2);
        c.set_frame(0, 3.0).unwrap();
        c.set_frame(0, 4.0).unwrap();
        assert_eq!((c.get_frame(0).unwrap(), c.get_prev_frame(0).unwrap()), (4.0, 3.0));
        let shared = HAnimComboData::new(true).into_ref();
        shared.write().unwrap().set_weight(0.5);
        c.append_anim_combo_data(shared.clone());
        c.clear();
        assert_eq!(c.get_frame(0).unwrap(), 0.0);
        assert_eq!(c.get_weight(2).unwrap(), 0.5);
        c.remove_anim_combo_data(&shared);
        assert_eq!(c.get_num_anims(), 2);
        assert!(c.get_weight(2).is_err());
        assert!(needs_normalizing(2.0) && !needs_normalizing(1.00001) && !needs_normalizing(0.0));
    }
}
