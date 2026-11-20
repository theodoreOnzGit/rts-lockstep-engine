//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,008 values (test: tests/differential_draft_systimer.rs);
//! not yet wired into the game. `SysTimeClass::Get` (`timeGetTime`, an OS call) is not tested.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/systimer.cpp (`SysTimeClass::Is_Getting_Late`)
//!
//! The rest of `SysTimeClass` is `timeGetTime` and is replaced by `std::time`. This predicate has no caller outside
//! WWLib's own timers; it is kept for completeness.

use crate::logic::modules::plant;

/// `SysTimeClass::Is_Getting_Late()` (systimer.cpp:118): the millisecond count `Get()` is past `0x6fffffff`. "The game
/// casts it to int in various places so it's safer to assume a signed max value."
pub fn is_getting_late(sys_time_get: u32) -> bool {
    if plant(17391) {
        sys_time_get >= 0x6fff_ffff
    } else if plant(17392) {
        sys_time_get > 0x7fff_ffff
    } else if plant(17393) {
        (sys_time_get as i32) > 0x6fff_ffff
    } else {
        sys_time_get > 0x6fff_ffff
    }
}
