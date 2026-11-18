//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/pointerremap.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/pointerremap.cpp
//!
//! Adaptation: upstream remaps by writing through a `void **` the caller registered. Rust has no such raw location, so a
//! "pointer" is a `usize` handle, a request carries the handle's old value plus a caller-chosen `slot` token, and
//! `process` returns, per request in sorted order, `(slot, Some(new))` or `(slot, None)` (upstream's `NULL`) for the
//! caller to write back. For the ref-counted table the caller performs the `Add_Ref()` for each `Some`. `File`/`Line`
//! (`WWDEBUG` only) are kept as the optional debug fields.

/// `PointerRemapClass::PtrPairStruct`.
#[derive(Clone, Copy, PartialEq)]
struct PtrPairStruct {
    old_pointer: usize,
    new_pointer: usize,
}

/// `PointerRemapClass::PtrRemapStruct` (`PointerToRemap` becomes `old_value` + `slot`).
#[derive(Clone, PartialEq)]
struct PtrRemapStruct {
    slot: usize,
    old_value: usize,
    file: &'static str,
    line: i32,
}

/// Result of one request: the `slot` token and the new pointer (`None` = failed to re-map, set to NULL).
pub type RemapResult = (usize, Option<usize>);

/// `class PointerRemapClass`.
#[derive(Default)]
pub struct PointerRemapClass {
    pointer_pair_table: Vec<PtrPairStruct>,
    pointer_request_table: Vec<PtrRemapStruct>,
    ref_count_request_table: Vec<PtrRemapStruct>,
}

impl PointerRemapClass {
    /// `PointerRemapClass::PointerRemapClass` (growth steps are `Vec`'s business).
    pub fn new() -> Self {
        Self::default()
    }

    /// `PointerRemapClass::Reset`.
    pub fn reset(&mut self) {
        self.pointer_pair_table.clear();
        self.pointer_request_table.clear();
        self.ref_count_request_table.clear();
    }

    /// `PointerRemapClass::Process` (pointerremap.cpp:79): (plain results, ref-counted results).
    pub fn process(&mut self) -> (Vec<RemapResult>, Vec<RemapResult>) {
        let mut plain = Vec::new();
        let mut counted = Vec::new();

        if !self.pointer_pair_table.is_empty() {
            self.pointer_pair_table.sort_by(|a, b| a.old_pointer.cmp(&b.old_pointer)); // ptr_pair_compare_function
        }

        if !self.pointer_request_table.is_empty() {
            debug_assert!(!self.pointer_pair_table.is_empty());
            self.pointer_request_table.sort_by(|a, b| a.old_value.cmp(&b.old_value)); // ptr_request_compare_function
            plain = self.process_request_table(&self.pointer_request_table);
        }

        // remap the ref-counted pointers
        if !self.ref_count_request_table.is_empty() {
            debug_assert!(!self.pointer_pair_table.is_empty());
            self.ref_count_request_table.sort_by(|a, b| a.old_value.cmp(&b.old_value));
            counted = self.process_request_table(&self.ref_count_request_table);
        }
        (plain, counted)
    }

    /// `PointerRemapClass::Process_Request_Table` (pointerremap.cpp:111).
    fn process_request_table(&self, request_table: &[PtrRemapStruct]) -> Vec<RemapResult> {
        let mut out = Vec::with_capacity(request_table.len());
        let table = &self.pointer_pair_table;
        let mut pair_index = 0usize;

        for req in request_table {
            let pointer_to_remap = req.old_value;
            let pre_search_index = pair_index;

            // Find the pair which contains the pointer we are looking for as its "old" pointer
            while pair_index < table.len() && table[pair_index].old_pointer < pointer_to_remap {
                pair_index += 1;
            }

            if pair_index < table.len() && table[pair_index].old_pointer == pointer_to_remap {
                // we found the match, plug in the new pointer (the caller adds a ref if needed).
                out.push((req.slot, Some(table[pair_index].new_pointer)));
            } else {
                // Failed to re-map the pointer: warn, set to NULL, reset index to the pre_search_index.
                // WWDEBUG_SAY("Warning! Failed to re-map pointer! old_ptr = 0x%X  file = %s  line = %d")
                let _ = (req.file, req.line);
                pair_index = pre_search_index;
                out.push((req.slot, None));
            }
        }
        out
    }

    /// `PointerRemapClass::Register_Pointer`.
    pub fn register_pointer(&mut self, old_pointer: usize, new_pointer: usize) {
        self.pointer_pair_table.push(PtrPairStruct { old_pointer, new_pointer });
    }

    /// `PointerRemapClass::Request_Pointer_Remap` (the `WWDEBUG` form with `file` and `line`).
    pub fn request_pointer_remap(&mut self, slot: usize, current_value: usize, file: &'static str, line: i32) {
        self.pointer_request_table.push(PtrRemapStruct { slot, old_value: current_value, file, line });
    }

    /// `PointerRemapClass::Request_Ref_Counted_Pointer_Remap`.
    pub fn request_ref_counted_pointer_remap(&mut self, slot: usize, current_value: usize, file: &'static str, line: i32) {
        self.ref_count_request_table.push(PtrRemapStruct { slot, old_value: current_value, file, line });
    }
}
