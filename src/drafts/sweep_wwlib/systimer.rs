//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/systimer.cpp (`SysTimeClass::Is_Getting_Late`)
//!
//! The rest of `SysTimeClass` is `timeGetTime` and is replaced by `std::time`. This predicate has no caller outside
//! WWLib's own timers; it is kept for completeness.

/// `SysTimeClass::Is_Getting_Late()` (systimer.cpp:118): the millisecond count `Get()` is past `0x6fffffff`. "The game
/// casts it to int in various places so it's safer to assume a signed max value."
pub fn is_getting_late(sys_time_get: u32) -> bool {
    sys_time_get > 0x6fff_ffff
}
