//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplayString.cpp
//!
//! Sweep 2 (#213): `W3DDisplayString::notifyTextChanged` and `setUseHotkey`.

/// The members of `W3DDisplayString` these two touch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct W3DDisplayStringState {
    pub use_hot_key: bool,
    pub hot_key_color: u32,
    /// `m_textChanged`.
    pub text_changed: bool,
    /// `m_textRenderer` / `m_textRendererHotKey` state.
    pub text_renderer_hard_word_wrap: bool,
    pub text_renderer_hot_key_hard_word_wrap: bool,
    pub text_renderer_hot_key_parse: bool,
}

impl W3DDisplayStringState {
    /// `W3DDisplayString::notifyTextChanged` (W3DDisplayString.cpp:118): after `DisplayString::notifyTextChanged`, both text
    /// renderers follow `TheGlobalLanguageData->m_useHardWrap` (when the language data exists), then the extents are
    /// recomputed (`computeExtents()`, the caller's), the sentence flagged for rebuilding, and both renderers reset (their
    /// built sentences dropped: the caller's `Reset()`).
    pub fn notify_text_changed(&mut self, use_hard_wrap: Option<bool>) {
        if let Some(hard) = use_hard_wrap {
            self.text_renderer_hard_word_wrap = hard;
            self.text_renderer_hot_key_hard_word_wrap = hard;
        }
        // computeExtents();
        // set a flag so that if it comes that we need to render this string we know we must first build the sentence
        self.text_changed = true;
        // m_textRenderer.Reset(); m_textRendererHotKey.Reset();
    }

    /// `W3DDisplayString::setUseHotkey` (W3DDisplayString.cpp:385).
    pub fn set_use_hotkey(&mut self, use_hotkey: bool, hot_key_color: u32, use_hard_wrap: Option<bool>) {
        self.use_hot_key = use_hotkey;
        self.hot_key_color = hot_key_color;
        self.text_renderer_hot_key_parse = use_hotkey; // m_textRenderer.Set_Hot_Key_Parse(useHotkey)
        self.notify_text_changed(use_hard_wrap);
    }
}
