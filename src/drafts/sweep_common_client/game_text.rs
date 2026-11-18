//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GameText.cpp   (`GameTextManager::removeLeadingAndTrailing`)

/// `iswspace` for the code units the string files hold (the C locale: space, \t \n \v \f \r).
fn iswspace(ch: u16) -> bool {
    matches!(ch, 0x20 | 0x09..=0x0d)
}

/// `GameTextManager::removeLeadingAndTrailing(buffer)` (GameText.cpp:494): strip leading and trailing white space in
/// place. Upstream moves the text down over the leading space (copying the terminator), then walks back from the last
/// character while `ptr > buffer` and the character is a space, so the FIRST character is never stripped from the
/// end (a string of only spaces becomes empty after the leading pass anyway).
pub fn remove_leading_and_trailing(buffer: &mut Vec<u16>) {
    // the buffer is NUL-terminated upstream; work on its logical content
    let mut first = 0;
    while first < buffer.len() && buffer[first] != 0 && iswspace(buffer[first]) {
        first += 1;
    }
    let mut text: Vec<u16> = buffer[first..].iter().copied().take_while(|&c| c != 0).collect();
    // `ptr -= 2` from one past the terminator is the last character
    let mut end = text.len(); // one past the last character
    while end > 1 && iswspace(text[end - 1]) {
        end -= 1;
    }
    // an empty or one-character text keeps itself (ptr > buffer fails at index 0)
    text.truncate(end);
    *buffer = text;
}
