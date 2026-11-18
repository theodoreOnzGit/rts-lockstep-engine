//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DVolumetricShadow.cpp
//!
//! Sweep 2 (#213): `W3DShadowGeometryManagerIterator::Get_Current_Geom` (W3DVolumetricShadow.cpp:4096), the typed accessor
//! over the hash-table iterator's current value (`(W3DShadowGeometry *)Get_Current()`).

/// `W3DShadowGeometryManagerIterator::Get_Current_Geom`: the current geometry of the iteration over the manager's table;
/// `None` once the iteration is done (`Get_Current()` is then NULL). `geoms` is the table in iteration order and `index` the
/// iterator position.
pub fn get_current_geom<T>(geoms: &[T], index: usize) -> Option<&T> {
    geoms.get(index)
}
