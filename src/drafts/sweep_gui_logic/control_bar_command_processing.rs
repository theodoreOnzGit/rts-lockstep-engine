//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarCommandProcessing.cpp
//!   (ControlBar::processCommandTransitionUI)

/// `CBCommandStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CbCommandStatus {
    /// `CBC_COMMAND_NOT_USED`.
    NotUsed,
    /// `CBC_COMMAND_USED`.
    Used,
}

/// `CB_CONTEXT_NONE`, `CB_CONTEXT_OBSERVER_INFO`, `CB_CONTEXT_OBSERVER_LIST`, `CB_CONTEXT_MULTI_SELECT` as the function reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlBarContextKind {
    None,
    ObserverInfo,
    ObserverList,
    MultiSelect,
    /// Any other context.
    Other,
}

/// `ControlBar::processCommandTransitionUI(control, gadgetMessage)` (ControlBarCommandProcessing.cpp:105): a button
/// transition (the pointer entering or leaving) is used by the control bar when something is selected. With nothing
/// selected (no drawable, or a drawable with no object) outside a multi-select, the bar goes to `CB_CONTEXT_NONE` unless
/// it is on the none or observer contexts, and the message is not used. `switch_to_none` is `switchToContext(
/// CB_CONTEXT_NONE, NULL)`.
pub fn process_command_transition_ui(
    curr_context: ControlBarContextKind,
    selected_drawable_has_object: bool,
    switch_to_none: impl FnOnce(),
) -> CbCommandStatus {
    if curr_context != ControlBarContextKind::MultiSelect && !selected_drawable_has_object {
        if curr_context != ControlBarContextKind::None
            && curr_context != ControlBarContextKind::ObserverInfo
            && curr_context != ControlBarContextKind::ObserverList
        {
            switch_to_none();
        }
        return CbCommandStatus::NotUsed;
    }
    CbCommandStatus::Used
}
