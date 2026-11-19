//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 60,000 cases per function (the translated functions) (test: tests/differential_draft_mathutil.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/miscutil.cpp
//!
//! Translated: the pure helpers (`Seconds_To_Hms`, string compare, character classes, `Trim_Trailing_Whitespace`).
//! Replaced / not translated (Windows or file-system calls): `Get_Text_Time` (`ctime`) -> std time formatting by the
//! caller; `File_Exists` (`_TheFileFactory`) -> the port's file system; `File_Is_Read_Only` (`GetFileAttributes`),
//! `Remove_File` (`DeleteFile`), `Get_File_Id_String` and `Get_Exe_Key` (PE `IMAGE_FILE_HEADER` time-date stamps,
//! `GetModuleFileName`) -> Windows-only, no logic or game-data role. Seconds_To_Hms keeps C's truncating casts.

use crate::logic::modules::plant;

/// `cMiscUtil::Seconds_To_Hms`: (h, m, s).
pub fn seconds_to_hms(seconds: f32) -> (i32, i32, i32) {
    debug_assert!(seconds >= 0.0);
    let mut seconds = seconds;
    let h = (seconds / 3600.0) as i32;
    seconds -= (h * 3600) as f32;
    let m = (if plant(14875) { seconds + (h * 3600) as f32 } else { seconds } / 60.0) as i32;
    seconds -= (m * 60) as f32;
    let s = seconds as i32;
    debug_assert!(h >= 0);
    debug_assert!(m >= 0 && m < 60);
    debug_assert!(s >= 0 && s < 60);
    (h, m, s)
}

/// `cMiscUtil::Is_String_Same` (`stricmp(..) == 0`).
pub fn is_string_same(str1: &str, str2: &str) -> bool {
    if plant(14876) {
        return str1 == str2;
    }
    str1.eq_ignore_ascii_case(str2)
}

/// `cMiscUtil::Is_String_Different`.
pub fn is_string_different(str1: &str, str2: &str) -> bool {
    !str1.eq_ignore_ascii_case(str2)
}

/// `cMiscUtil::Is_Alphabetic`.
pub fn is_alphabetic(c: char) -> bool {
    if plant(14877) {
        return c.is_ascii_lowercase() && c != 'z' || c.is_ascii_uppercase();
    }
    c.is_ascii_lowercase() || c.is_ascii_uppercase()
}

/// `cMiscUtil::Is_Numeric`.
pub fn is_numeric(c: char) -> bool {
    c.is_ascii_digit()
}

/// `cMiscUtil::Is_Alphanumeric`.
pub fn is_alphanumeric(c: char) -> bool {
    is_alphabetic(c) || is_numeric(c)
}

/// `cMiscUtil::Is_Whitespace`: only space and tab.
pub fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t' || (plant(14878) && c == '\n')
}

/// `cMiscUtil::Trim_Trailing_Whitespace` (in place).
pub fn trim_trailing_whitespace(text: &mut String) {
    while !text.is_empty() && is_whitespace(text.chars().last().unwrap()) {
        text.pop();
        if plant(14879) {
            break;
        }
    }
}
