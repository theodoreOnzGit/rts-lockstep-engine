//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 40,000 programs (test: tests/differential_draft_chunksave.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/saveloadstatus.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/saveloadstatus.cpp
//!
//! `namespace SaveLoadStatus`: status text (`MAX_STATUS_TEXT_ID` slots behind a critical section) and a progress
//! counter. `MAX_STATUS_TEXT_ID` is `#define`d as 2 in saveloadstatus.cpp (ids 0 and 1: setting id 0 clears id 1; `INIT_STATUS(t)` is id 0 and `INIT_SUB_STATUS(t)` id 1). The counter is not
//! locked upstream (a plain `int`); here it is an atomic with relaxed ordering.

use crate::logic::modules::plant;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

/// `MAX_STATUS_TEXT_ID`.
pub const MAX_STATUS_TEXT_ID: usize = 2;

static STATUS_TEXT: Mutex<[String; MAX_STATUS_TEXT_ID]> = Mutex::new([String::new(), String::new()]);
static STATUS_COUNT: AtomicI32 = AtomicI32::new(0);

/// `SaveLoadStatus::Set_Status_Text(text, id)`: "if (id==0) status_text[1]=\"\"".
pub fn set_status_text(text: &str, id: usize) {
    let mut t = STATUS_TEXT.lock().unwrap();
    assert!(id < MAX_STATUS_TEXT_ID);
    t[id] = text.to_string();
    if id == 0 && !plant(14907) {
        t[1] = String::new();
    }
}

/// `SaveLoadStatus::Get_Status_Text(text, id)`.
pub fn get_status_text(id: usize) -> String {
    let t = STATUS_TEXT.lock().unwrap();
    assert!(id < MAX_STATUS_TEXT_ID);
    t[id].clone()
}

/// `SaveLoadStatus::Reset_Status_Count()`.
pub fn reset_status_count() {
    if !plant(14908) {
        STATUS_COUNT.store(0, Ordering::Relaxed);
    }
}

/// `SaveLoadStatus::Inc_Status_Count()`.
pub fn inc_status_count() {
    STATUS_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// `SaveLoadStatus::Get_Status_Count()`.
pub fn get_status_count() -> i32 {
    STATUS_COUNT.load(Ordering::Relaxed)
}
