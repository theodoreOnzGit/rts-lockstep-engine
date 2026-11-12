// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (InGameUI::militarySubtitle and removeMilitarySubtitle,
//     lines 4063-4135; update's military subtitle, lines 1687-1797; postDraw's, lines 3485-3509; disableTooltipsUntil,
//     clearTooltipsDisabled and areTooltipsDisabled, lines 5660-5674; init's language fonts, lines 1105-1155; the
//     constructor's message, caption and popup members, lines 924-949)
//   GeneralsMD/Code/GameEngine/Include/GameClient/InGameUI.h (MilitarySubtitleData, MAX_SUBTITLE_LINES)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #151: the military subtitle, the typed-out briefing caption a script shows (`MILITARY_CAPTION`, the logic's
//! `ClientEvent::InGameUi(MilitarySubtitle)`).
//!
//! # As upstream does it
//!
//! - **`militarySubtitle(label, duration)`** (InGameUI.cpp:4063, [`MilitarySubtitleUi::military_subtitle`]) removes the
//!   current subtitle, gives the label to the briefing history (`UpdateDiplomacyBriefingText(label, FALSE)`), and makes
//!   a new one from `TheGameText->fetch(label)` unless that is empty or `duration <= 0`: it lives until the logic frame
//!   `now + (Int)(30.0f * duration / 1000.0f)` (and the tooltips are off until then), at `MilitaryCaptionPosition`
//!   scaled from 800x600 to the display, its first line in `MilitaryCaptionTitleFont` (its size through
//!   `adjustFontSize`), its first character typed after the language's `MilitaryCaptionDelayMS` (in frames,
//!   truncated), in `MilitaryCaptionColor`.
//! - **The update** (InGameUI.cpp:1687, [`MilitarySubtitleUi::update`], once an engine frame): while the scripts freeze
//!   time (`isTimeFrozenScript`) the three frame counters go back one, so the caption keeps its pace in a frozen
//!   frame. Past its lifetime its alpha falls by `REAL_TO_INT((now - lifetime) * 0.1f)` each update and it is removed
//!   once that would go below 0. Before, the block cursor toggles when 10 frames have passed since it last did, and
//!   when the next character's frame has passed, one character is typed: a `\n` starts a new line (at most
//!   `MAX_SUBTITLE_LINES`, in `MilitaryCaptionFont`, after the delay again), any other is appended, moves the block
//!   cursor to the line's end and plays `MilitarySubtitlesTyping`, the next due after the language's
//!   `MilitaryCaptionSpeed` frames (InGameUI.ini's `MilitaryCaptionSpeed` when there is no language data). After the
//!   last character the next is due after the lifetime: the caption stays until it fades.
//! - **The draw** (`postDraw`, InGameUI.cpp:3485, [`MilitarySubtitleUi::draw`]): each line at the position, one under
//!   the other by its height, in the colour with a black drop shadow of the same alpha; then the cursor, while it is
//!   on, a filled block `0.8 x` the line's font height wide and that high, at the block position, in the colour.
//! - **`removeMilitarySubtitle`** ([`MilitarySubtitleUi::remove_military_subtitle`]) turns the tooltips back on
//!   (`clearTooltipsDisabled`) and frees the lines. `InGameUI::reset` (a new game) and the destructor call it.
//!
//! # Where this differs from upstream
//!
//! - What the caption reaches outside `InGameUI` is the host's ([`MilitarySubtitleHost`]): the logic's frame, the text,
//!   the display's size, the language data, the fonts' measure, the sound, the briefing history.
//! - A display string is its text and font ([`SubtitleString`]); the draw is a list of [`SubtitleDraw`]s for the client.
//! - A fifth line (`currentDisplayString` 4, one past `displayStrings`) is undefined upstream: the update truncates the
//!   text (`DEBUG_CRASH`, no-op in release) but leaves the index at 4, which the draw and the removal then read past
//!   the array. Here [`MilitarySubtitleUi::undefined`] records it; the draw and the removal skip the missing line.
//! - A character typed past the text's end (`getCharAt(index)` with `index` over the length) reads past the string:
//!   undefined, also recorded. It needs the logic frame to pass the lifetime as an unsigned number and not as a signed
//!   one (`(Int)lifetime < (Int)currLogicFrame` decides the fade, `incrementOnFrame < currLogicFrame` the typing), which
//!   a game's frame (from 0, upward) never does. At the length itself it reads the terminator, 0, and `appendChar(0)`
//!   appends nothing.

use crate::client::radius_cursor::InGameUiIniSettings;
use crate::client::window_script::GameFont;
use crate::common::base_type::{real_to_int, ICoord2D};
use crate::common::ini::game_make_color;
use crate::logic::modules::plant;

/// `MAX_SUBTITLE_LINES` (InGameUI.h:259).
pub const MAX_SUBTITLE_LINES: u32 = 4;
/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: i32 = 30;

/// One of the subtitle's `DisplayString`s: its text and its font (`setFont`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubtitleString {
    pub text: Vec<u16>,
    pub font: GameFont,
}

/// `struct MilitarySubtitleData` (InGameUI.h:648).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MilitarySubtitleData {
    /// `subtitle`: the whole text, its lines separated by `\n`.
    pub subtitle: Vec<u16>,
    /// `index`: the next character to type.
    pub index: u32,
    /// `position`.
    pub position: ICoord2D,
    /// `displayStrings[MAX_SUBTITLE_LINES]` (`None` for `NULL`).
    pub display_strings: [Option<SubtitleString>; MAX_SUBTITLE_LINES as usize],
    /// `currentDisplayString`.
    pub current_display_string: u32,
    /// `lifetime`, a logic frame.
    pub lifetime: u32,
    /// `blockDrawn`: the typing cursor is on.
    pub block_drawn: bool,
    /// `blockBeginFrame`.
    pub block_begin_frame: u32,
    /// `blockPos`.
    pub block_pos: ICoord2D,
    /// `incrementOnFrame`.
    pub increment_on_frame: u32,
    /// `color`.
    pub color: i32,
}

/// One thing `postDraw` draws for the subtitle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubtitleDraw {
    /// `displayString->draw(x, y, color, dropColor)`.
    Text { x: i32, y: i32, text: Vec<u16>, font: GameFont, color: i32, drop_color: i32 },
    /// `TheDisplay->drawFillRect(x, y, width, height, color)`.
    FillRect { x: i32, y: i32, width: i32, height: i32, color: i32 },
}

/// What the subtitle reaches outside `InGameUI`.
pub trait MilitarySubtitleHost {
    /// `TheGameLogic->getFrame()`.
    fn logic_frame(&self) -> u32;
    /// `TheGameText->fetch(label)`.
    fn fetch(&mut self, label: &[u8]) -> Vec<u16>;
    /// `TheDisplay->getWidth()`, `getHeight()`.
    fn display_size(&self) -> (i32, i32);
    /// `TheGlobalLanguageData->m_militaryCaptionDelayMS`.
    fn military_caption_delay_ms(&self) -> i32;
    /// `TheGlobalLanguageData->m_militaryCaptionSpeed`, `None` when `TheGlobalLanguageData` is `NULL`.
    fn language_military_caption_speed(&self) -> Option<i32>;
    /// `TheGlobalLanguageData->adjustFontSize(size)`.
    fn adjust_font_size(&self, size: i32) -> i32;
    /// `DisplayString::getSize(&width, &height)` of `text` in `font`.
    fn string_size(&self, text: &[u16], font: &GameFont) -> (i32, i32);
    /// `GameFont::height`.
    fn font_height(&self, font: &GameFont) -> i32;
    /// `TheAudio->addAudioEvent(&event)` of the named event.
    fn add_audio_event(&mut self, name: &[u8]);
    /// `TheScriptEngine->isTimeFrozenScript()`.
    fn is_time_frozen_script(&self) -> bool;
    /// `UpdateDiplomacyBriefingText(label, FALSE)` (`crate::client::diplomacy`).
    fn update_diplomacy_briefing_text(&mut self, label: &[u8]);
}

/// `GameGetColorComponents(color, &r, &g, &b, &a)`.
fn color_components(c: i32) -> (u8, u8, u8, u8) {
    let c = c as u32;
    ((c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8)
}

/// `TheFontLibrary->getFont(name, size, bold)`.
fn get_font(name: &[u8], size: i32, bold: bool) -> GameFont {
    GameFont { name: name.to_vec(), point_size: size, bold }
}

/// `(Int)(((Real)LOGICFRAMES_PER_SECOND * ms) / 1000.0f)`: milliseconds in logic frames, truncated.
fn ms_to_frames(ms: i32) -> i32 {
    ((LOGICFRAMES_PER_SECOND as f32 * ms as f32) / 1000.0f32) as i32
}

/// `InGameUI`'s military subtitle and the tooltips' switch it turns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MilitarySubtitleUi {
    /// `m_militarySubtitle` (`None` for `NULL`).
    pub military_subtitle: Option<MilitarySubtitleData>,
    /// `m_tooltipsDisabledUntil`.
    pub tooltips_disabled_until: u32,
    /// Upstream's undefined behaviour met (the first): see the module docs.
    pub undefined: Option<&'static str>,
}

impl MilitarySubtitleUi {
    /// `disableTooltipsUntil(frameNum)` (InGameUI.cpp:5660).
    pub fn disable_tooltips_until(&mut self, frame_num: u32) {
        if frame_num > self.tooltips_disabled_until {
            self.tooltips_disabled_until = frame_num;
        }
    }

    /// `clearTooltipsDisabled()` (InGameUI.cpp:5666).
    pub fn clear_tooltips_disabled(&mut self) {
        if !plant(9431) {
            self.tooltips_disabled_until = 0;
        }
    }

    /// `areTooltipsDisabled()` (InGameUI.cpp:5671) at the logic frame `frame`.
    pub fn are_tooltips_disabled(&self, frame: u32) -> bool {
        frame < self.tooltips_disabled_until
    }

    /// `militarySubtitle(label, duration)` (InGameUI.cpp:4063), with InGameUI's members `s`.
    pub fn military_subtitle<H: MilitarySubtitleHost>(&mut self, h: &mut H, s: &InGameUiIniSettings, label: &[u8], duration: i32) {
        self.remove_military_subtitle();
        h.update_diplomacy_briefing_text(label);
        let title = h.fetch(label);
        if title.is_empty() || (duration <= 0 && !plant(9432)) {
            // ("Trying to create a military subtitle but either title is empty or duration is <= 0")
            return;
        }
        let curr_logic_frame = h.logic_frame();
        let message_timeout = curr_logic_frame.wrapping_add(ms_to_frames(duration) as u32) as i32;
        self.disable_tooltips_until(message_timeout as u32);
        let (width, height) = h.display_size();
        let multiplier = (width as f32 / 800.0f32, height as f32 / 600.0f32);
        let position = ICoord2D { x: (s.military_caption_position.x as f32 * multiplier.0) as i32, y: (s.military_caption_position.y as f32 * multiplier.1) as i32 };
        let delay = if plant(9433) { 0 } else { ms_to_frames(h.military_caption_delay_ms()) };
        let first = SubtitleString { text: Vec::new(), font: get_font(&s.military_caption_title_font, h.adjust_font_size(s.military_caption_title_point_size), s.military_caption_title_bold) };
        let c = &s.military_caption_color;
        self.military_subtitle = Some(MilitarySubtitleData {
            subtitle: title,
            index: 0,
            position,
            display_strings: [Some(first), None, None, None],
            current_display_string: 0,
            lifetime: message_timeout as u32,
            block_drawn: true,
            block_begin_frame: curr_logic_frame,
            block_pos: position,
            increment_on_frame: curr_logic_frame.wrapping_add(delay as u32),
            color: game_make_color(c.red as u8, c.green as u8, c.blue as u8, c.alpha as u8),
        });
    }

    /// `removeMilitarySubtitle()` (InGameUI.cpp:4117).
    pub fn remove_military_subtitle(&mut self) {
        if self.military_subtitle.is_none() {
            return;
        }
        self.clear_tooltips_disabled();
        self.military_subtitle = None;
    }

    /// `update()`'s military subtitle (InGameUI.cpp:1687), once an engine frame, with InGameUI's members `s`.
    pub fn update<H: MilitarySubtitleHost>(&mut self, h: &mut H, s: &InGameUiIniSettings) {
        let curr_logic_frame = h.logic_frame();
        let Some(m) = self.military_subtitle.as_mut() else { return };
        // "if the timeis frozen by a script, then we still want the text to display"
        if h.is_time_frozen_script() && !plant(9434) {
            m.lifetime = m.lifetime.wrapping_sub(1);
            m.block_begin_frame = m.block_begin_frame.wrapping_sub(1);
            m.increment_on_frame = m.increment_on_frame.wrapping_sub(1);
        }
        if (m.lifetime as i32) < curr_logic_frame as i32 {
            // "steal colins fade from above"
            let (r, g, b, a) = color_components(m.color);
            let amount = real_to_int(curr_logic_frame.wrapping_sub(m.lifetime) as f32 * 0.1f32);
            if i32::from(a) - amount < 0 {
                self.remove_military_subtitle();
            } else {
                let a = (i32::from(a) - if plant(9435) { amount * 2 } else { amount }).max(0) as u8;
                m.color = game_make_color(r, g, b, a);
            }
            return;
        }
        // "trigger whether or not we should draw the block"
        let toggle_after = if plant(9436) { 8 } else { 9 };
        if m.block_begin_frame.wrapping_add(toggle_after) < curr_logic_frame {
            m.block_begin_frame = curr_logic_frame;
            m.block_drawn = !m.block_drawn;
        }
        if m.increment_on_frame >= curr_logic_frame {
            return;
        }
        // (`getCharAt(index)`: at the text's length its terminator, 0; past it, memory past the string: undefined, see
        // the module docs)
        if m.index as usize > m.subtitle.len() && self.undefined.is_none() {
            self.undefined = Some("military subtitle: a character read past the text (getCharAt)");
        }
        let temp_w_char = m.subtitle.get(m.index as usize).copied().unwrap_or(0);
        let cur = m.current_display_string as usize;
        if temp_w_char == u16::from(b'\n') {
            // "increment the Block position's Y value to draw it on the next line"
            let height = m.display_strings.get(cur).and_then(|d| d.as_ref()).map_or(0, |d| h.string_size(&d.text, &d.font).1);
            m.block_pos.y = m.block_pos.y.wrapping_add(height);
            m.current_display_string += 1;
            if m.current_display_string < MAX_SUBTITLE_LINES {
                m.block_pos.x = m.position.x;
                let font = get_font(&s.military_caption_font, h.adjust_font_size(s.military_caption_point_size), s.military_caption_bold);
                m.display_strings[m.current_display_string as usize] = Some(SubtitleString { text: Vec::new(), font });
                m.block_drawn = true;
                m.increment_on_frame = curr_logic_frame.wrapping_add(ms_to_frames(h.military_caption_delay_ms()) as u32);
            } else {
                // "if we've exceeded the allocated number of display strings, this will force us to essentially truncate
                // the remaining text" (the index stays past the array: module docs)
                m.index = m.subtitle.len() as u32;
                if self.undefined.is_none() {
                    self.undefined = Some("military subtitle: a fifth line (displayStrings[4])");
                }
            }
        } else {
            let Some(Some(d)) = m.display_strings.get_mut(cur) else { return };
            // `appendChar(c)`: `UnicodeString::concat(c)` concatenates `{ c, 0 }`, so a 0 (the terminator `getCharAt`
            // reads one past the text) appends nothing
            if temp_w_char != 0 {
                d.text.push(temp_w_char);
            }
            let width = h.string_size(&d.text, &d.font).0;
            m.block_pos.x = m.position.x.wrapping_add(width);
            // "lets make a sound"
            h.add_audio_event(b"MilitarySubtitlesTyping");
            let speed = match h.language_military_caption_speed() {
                Some(v) if !plant(9437) => v,
                _ => s.military_caption_speed,
            };
            m.increment_on_frame = curr_logic_frame.wrapping_add(speed as u32);
        }
        m.index += 1;
        if m.index as usize >= m.subtitle.len() {
            // "We're at the end of the subtitle, set everything to persist till the subtitle has expired"
            m.increment_on_frame = m.lifetime.wrapping_add(1);
        }
    }

    /// `postDraw()`'s military subtitle (InGameUI.cpp:3485): what it draws, in order.
    pub fn draw<H: MilitarySubtitleHost>(&self, h: &H) -> Vec<SubtitleDraw> {
        let mut out = Vec::new();
        let Some(m) = &self.military_subtitle else { return out };
        let mut pos = m.position;
        let (_, _, _, a) = color_components(m.color);
        let drop_color = game_make_color(0, 0, 0, a);
        for d in m.display_strings.iter().take(m.current_display_string as usize + 1).flatten() {
            out.push(SubtitleDraw::Text { x: pos.x, y: pos.y, text: d.text.clone(), font: d.font.clone(), color: m.color, drop_color });
            let height = h.string_size(&d.text, &d.font).1;
            pos.y = pos.y.wrapping_add(height);
        }
        if m.block_drawn {
            if let Some(Some(d)) = m.display_strings.get(m.current_display_string as usize) {
                let size_y = h.font_height(&d.font);
                let size_x = (size_y as f32 * if plant(9438) { 0.5f32 } else { 0.8f32 }) as i32;
                out.push(SubtitleDraw::FillRect { x: m.block_pos.x, y: m.block_pos.y, width: size_x, height: size_y, color: m.color });
            }
        }
        out
    }
}

impl InGameUiIniSettings {
    /// The constructor's `InGameUI()` values of the message, military caption and popup members (InGameUI.cpp:924-949).
    pub fn set_constructor_caption_values(&mut self) {
        self.message_color1 = game_make_color(255, 255, 255, 255);
        self.message_color2 = game_make_color(180, 180, 180, 255);
        self.message_position = ICoord2D { x: 10, y: 10 };
        self.message_font = b"Arial".to_vec();
        self.message_point_size = 10;
        self.message_bold = false;
        self.message_delay_ms = 5000;
        self.military_caption_color = crate::common::base_type::RGBAColorInt { red: 200, green: 200, blue: 30, alpha: 255 };
        self.military_caption_position = ICoord2D { x: 10, y: 380 };
        self.military_caption_title_font = b"Courier".to_vec();
        self.military_caption_title_point_size = 12;
        self.military_caption_title_bold = true;
        self.military_caption_font = b"Courier".to_vec();
        self.military_caption_point_size = 12;
        self.military_caption_bold = false;
        self.military_caption_randomize_typing = false;
        self.military_caption_speed = 1;
        self.popup_message_color = game_make_color(255, 255, 255, 255);
    }

    /// `InGameUI::init`'s "override INI values with language localized values" (InGameUI.cpp:1105): each of the
    /// language's fonts whose name is not empty replaces the member's name, size and bold.
    pub fn apply_language_fonts(&mut self, lang: &crate::client::global_language::GlobalLanguage) {
        fn take(f: &crate::client::global_language::FontDesc, name: &mut Vec<u8>, size: &mut i32, bold: &mut bool) {
            if !f.name.is_empty() {
                *name = f.name.clone();
                *size = f.size;
                *bold = f.bold;
            }
        }
        take(&lang.drawable_caption_font, &mut self.drawable_caption_font, &mut self.drawable_caption_point_size, &mut self.drawable_caption_bold);
        take(&lang.message_font, &mut self.message_font, &mut self.message_point_size, &mut self.message_bold);
        take(&lang.military_caption_title_font, &mut self.military_caption_title_font, &mut self.military_caption_title_point_size, &mut self.military_caption_title_bold);
        take(&lang.military_caption_font, &mut self.military_caption_font, &mut self.military_caption_point_size, &mut self.military_caption_bold);
        take(&lang.superweapon_countdown_normal_font, &mut self.superweapon_normal_font, &mut self.superweapon_normal_point_size, &mut self.superweapon_normal_bold);
        take(&lang.superweapon_countdown_ready_font, &mut self.superweapon_ready_font, &mut self.superweapon_ready_point_size, &mut self.superweapon_ready_bold);
        take(&lang.named_timer_countdown_normal_font, &mut self.named_timer_normal_font, &mut self.named_timer_normal_point_size, &mut self.named_timer_normal_bold);
        take(&lang.named_timer_countdown_ready_font, &mut self.named_timer_ready_font, &mut self.named_timer_ready_point_size, &mut self.named_timer_ready_bold);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host: the frame and freeze set by the test, text = the label's bytes, 800x600, characters 6 wide and the
    /// font's size high, the language's delay 750 and speed 2.
    #[derive(Default)]
    struct T {
        frame: u32,
        frozen: bool,
        sounds: u32,
        briefings: Vec<Vec<u8>>,
    }
    impl MilitarySubtitleHost for T {
        fn logic_frame(&self) -> u32 {
            self.frame
        }
        fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
            label.iter().map(|&b| u16::from(b)).collect()
        }
        fn display_size(&self) -> (i32, i32) {
            (800, 600)
        }
        fn military_caption_delay_ms(&self) -> i32 {
            750
        }
        fn language_military_caption_speed(&self) -> Option<i32> {
            Some(2)
        }
        fn adjust_font_size(&self, size: i32) -> i32 {
            size
        }
        fn string_size(&self, text: &[u16], font: &GameFont) -> (i32, i32) {
            (6 * text.len() as i32, font.point_size)
        }
        fn font_height(&self, font: &GameFont) -> i32 {
            font.point_size
        }
        fn add_audio_event(&mut self, _name: &[u8]) {
            self.sounds += 1;
        }
        fn is_time_frozen_script(&self) -> bool {
            self.frozen
        }
        fn update_diplomacy_briefing_text(&mut self, label: &[u8]) {
            self.briefings.push(label.to_vec());
        }
    }

    /// #151, predicted before the run: "AB\nC" for 1000 ms at frame 100 lives to frame 130 with the tooltips off until
    /// then, at (10, 380); its first character is due after frame 122 (750 ms is 22 frames), so frame 123 types `A` (block
    /// at 16), 126 types `B` (speed 2: due after 125), 129 starts the second line (block at y 392, the delay again: due
    /// after 151, past the lifetime), and the line is never typed; at 131 the fade starts: alpha 255 less 0, then frame
    /// 140 less 1 each update... removed once the amount passes the alpha; the tooltips come back on removal. The block
    /// toggles at frame 110 (100 + 9 < 110), and is off at 130 (on at 120 and by the new line, off at 130): the draw is
    /// the two lines. An empty label or a 0 duration makes none, but still reaches the history.
    #[test]
    fn the_caption_types_out_and_fades() {
        let mut s = InGameUiIniSettings::default();
        s.set_constructor_caption_values();
        let mut u = MilitarySubtitleUi::default();
        let mut h = T { frame: 100, ..Default::default() };
        u.military_subtitle(&mut h, &s, b"AB\nC", 1000);
        assert_eq!(h.briefings, vec![b"AB\nC".to_vec()]);
        let m = u.military_subtitle.clone().expect("made");
        assert_eq!((m.lifetime, m.position, m.increment_on_frame, m.color as u32), (130, ICoord2D { x: 10, y: 380 }, 122, 0xFFC8_C81E));
        assert!(u.are_tooltips_disabled(129) && !u.are_tooltips_disabled(130));
        assert_eq!(m.display_strings[0].as_ref().map(|d| d.font.clone()), Some(GameFont { name: b"Courier".to_vec(), point_size: 12, bold: true }));
        let mut typed = Vec::new();
        for f in 101..=130 {
            h.frame = f;
            u.update(&mut h, &s);
            let m = u.military_subtitle.as_ref().expect("alive");
            typed.push((f, m.index, m.block_pos.x, m.block_pos.y, m.block_drawn));
        }
        let at = |f: u32| typed.iter().find(|t| t.0 == f).copied().expect("frame");
        assert_eq!(at(110).4, false, "the block toggles at 110");
        assert_eq!(at(123), (123, 1, 16, 380, at(123).4));
        assert_eq!(at(126).1, 2);
        assert_eq!((at(129).1, at(129).2, at(129).3), (3, 10, 392));
        assert_eq!(h.sounds, 2);
        let m = u.military_subtitle.as_ref().expect("alive");
        assert_eq!((m.current_display_string, m.increment_on_frame), (1, 151));
        // (the second line, empty, is drawn too; the block toggled off at 130)
        let draw = u.draw(&h);
        assert_eq!(draw.len(), 2, "{draw:?}");
        assert!(matches!(&draw[0], SubtitleDraw::Text { x: 10, y: 380, text, .. } if text == &[65, 66]));
        let mut f = 130;
        while u.military_subtitle.is_some() {
            f += 1;
            h.frame = f;
            u.update(&mut h, &s);
            assert!(f < 400);
        }
        assert!(!u.are_tooltips_disabled(0));
        u.military_subtitle(&mut h, &s, b"", 1000);
        u.military_subtitle(&mut h, &s, b"X", 0);
        assert!(u.military_subtitle.is_none());
        assert_eq!(h.briefings.len(), 3);
    }
}
