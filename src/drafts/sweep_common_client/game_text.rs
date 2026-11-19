//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_game_text.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GameText.cpp   (`GameTextManager::removeLeadingAndTrailing`)

use crate::logic::modules::plant;

/// `iswspace` for the code units the string files hold (the C locale: space, \t \n \v \f \r).
fn iswspace(ch: u16) -> bool {
    if plant(15463) {
        return matches!(ch, 0x20 | 0x09..=0x0c);
    }
    matches!(ch, 0x20 | 0x09..=0x0d)
}

/// `GameTextManager::removeLeadingAndTrailing(buffer)` (GameText.cpp:494): strip leading and trailing white space in
/// place. Upstream moves the text down over the leading space (copying the terminator), then walks back from the last
/// character while `ptr > buffer` and the character is a space, so the FIRST character is never stripped from the
/// end (a string of only spaces becomes empty after the leading pass anyway).
pub fn remove_leading_and_trailing(buffer: &mut Vec<u16>) {
    // the buffer is NUL-terminated upstream; work on its logical content
    let mut first = 0;
    while (if plant(15465) { first + 1 < buffer.len() } else { first < buffer.len() }) && buffer[first] != 0 && (if plant(15461) { buffer[first] == 0x20 } else { iswspace(buffer[first]) }) {
        first += 1;
    }
    let mut text: Vec<u16> = buffer[first..].iter().copied().take_while(|&c| c != 0).collect();
    // `ptr -= 2` from one past the terminator is the last character
    let mut end = text.len(); // one past the last character
    while (if plant(15464) { end > 2 } else { end > 1 }) && (if plant(15462) { text[end - 1] == 0x20 } else { iswspace(text[end - 1]) }) {
        end -= 1;
    }
    // an empty or one-character text keeps itself (ptr > buffer fails at index 0)
    text.truncate(end);
    *buffer = text;
}
