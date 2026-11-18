//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/IMEManager.cpp (IMEManager::getCompositionString,
//!   updateCompositionString, getResultsString, convertToUnicode, openCandidateList, closeCandidateList,
//!   updateCandidateList, updateProperties, getIndexBase, resizeCandidateWindow, getCandidate, getCandidatePageStart,
//!   openStatusWindow, closeStatusWindow)
//!
//! The Windows Input Method Manager calls (`Imm*`, `MultiByteToWideChar`, `GetKeyboardLayout`) are the host's, through
//! [`ImmApi`]: `crate::client::ime_manager` (verified) does not compose (its module docs say why), so these are the
//! composition and candidate list halves of the class that it leaves out.
//!
//! Sweep 2 findings for this file:
//! - `IMEManager::printConversionStatus`, `printSentenceStatus` (IMEManager.cpp:448, 467): excluded, `DEBUG_IME`
//!   (commented out at IMEManager.cpp:83) and they only `DEBUG_LOG`.

use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::client::window_script::WinId;

/// `MAX_COMPSTRINGLEN`.
pub const MAX_COMPSTRINGLEN: usize = 2 * 1024;

/// `IME_CAND_UNKNOWN`, `IME_CAND_CODE` (imm.h).
pub const IME_CAND_UNKNOWN: u32 = 0x0000;
pub const IME_CAND_CODE: u32 = 0x0001;
/// `IME_PROP_UNICODE`, `IME_PROP_CANDLIST_START_FROM_1` (imm.h).
pub const IME_PROP_UNICODE: i32 = 0x0008_0000;
pub const IME_PROP_CANDLIST_START_FROM_1: i32 = 0x0001_0000;

/// A `CANDIDATELIST` as `ImmGetCandidateList` fills it, with the strings already extracted from its offsets.
#[derive(Clone, Debug, Default)]
pub struct CandidateList {
    pub dw_style: u32,
    pub dw_count: u32,
    pub dw_selection: u32,
    pub dw_page_start: u32,
    pub dw_page_size: u32,
    /// `string = clist + dwOffset[i]`, each as the bytes (ANSI) or the UTF-16 units (Unicode) it holds.
    pub strings: Vec<CandidateString>,
}

/// One candidate string of a list, as the IME gave it.
#[derive(Clone, Debug)]
pub enum CandidateString {
    /// `WideChar *`.
    Wide(Vec<u16>),
    /// `Char *` in the ANSI code page: converted by `convertToUnicode`.
    Ansi(Vec<u8>),
}

/// The Win32 IMM / NLS calls the manager makes.
pub trait ImmApi {
    /// `ImmGetCompositionStringW(context, GCS_COMPSTR, buf, MAX_COMPSTRINGLEN)`: the byte count (negative: failure) and
    /// the UTF-16 units.
    fn get_composition_string_w(&self, max_bytes: usize) -> (i32, Vec<u16>);
    /// `ImmGetCompositionStringW(context, GCS_CURSORPOS, NULL, 0)`.
    fn get_composition_cursor_pos_w(&self) -> i32;
    /// `ImmGetCompositionStringA(context, GCS_COMPSTR, buf, max)`: the byte count and bytes.
    fn get_composition_string_a(&self, max_bytes: usize) -> (i32, Vec<u8>);
    /// `ImmGetCompositionString(context, GCS_CURSORPOS, NULL, 0)`.
    fn get_composition_cursor_pos_a(&self) -> i32;
    /// `_mbsnccnt(buf, n)`: the multibyte characters in the first `n` bytes.
    fn mbsnccnt(&self, buf: &[u8], n: i32) -> i32;
    /// `ImmGetCompositionStringW(context, GCS_RESULTSTR, ..)`.
    fn get_result_string_w(&self, max_bytes: usize) -> (i32, Vec<u16>);
    /// `ImmGetCompositionStringA(context, GCS_RESULTSTR, ..)`.
    fn get_result_string_a(&self, max_bytes: usize) -> (i32, Vec<u8>);
    /// `MultiByteToWideChar(CP_ACP, 0, mbcs, len, ..)`: the UTF-16 units; empty on failure.
    fn multi_byte_to_wide_char(&self, mbcs: &[u8]) -> Vec<u16>;
    /// `ImmGetCandidateListCountW(context, &listCount)` / `...A`: the buffer size, <= 0 on failure.
    fn get_candidate_list_count(&self, unicode: bool) -> i32;
    /// `ImmGetCandidateListW(context, i, clist, size)` / `...A`: the bytes copied (0: failure) and the list.
    fn get_candidate_list(&self, list_index: i32, unicode: bool, size: i32) -> (i32, CandidateList);
    /// `ImmGetProperty(GetKeyboardLayout(0), IGP_PROPERTY)`.
    fn get_property(&self) -> i32;
}

/// `IMEManager`'s composition, candidate and status state.
#[derive(Clone, Debug, Default)]
pub struct ImeCandidateState {
    /// `m_context` is non-NULL.
    pub has_context: bool,
    /// `m_window`: the text entry attached, if any.
    pub window: Option<WinId>,
    pub candidate_window: Option<WinId>,
    pub candidate_text_area: Option<WinId>,
    pub candidate_up_arrow: Option<WinId>,
    pub candidate_down_arrow: Option<WinId>,
    pub status_window: Option<WinId>,
    /// `m_compositionString`, `m_compositionStringLength`, `m_compositionCursorPos`.
    pub composition_string: Vec<u16>,
    pub composition_string_length: i32,
    pub composition_cursor_pos: i32,
    /// `m_resultsString`.
    pub results_string: Vec<u16>,
    /// `m_candidateString` (NULL: empty), `m_candidateCount`, `m_pageSize`, `m_pageStart`, `m_selectedIndex`.
    pub candidate_string: Vec<Vec<u16>>,
    pub candidate_count: i32,
    pub page_size: i32,
    pub page_start: i32,
    pub selected_index: i32,
    /// `m_indexBase`, `m_unicodeIME`.
    pub index_base: i32,
    pub unicode_ime: bool,
}

impl ImeCandidateState {
    /// `IMEManager::getCompositionString(string)` (IMEManager.cpp:1083): `string.set(m_compositionString)`.
    pub fn get_composition_string(&self) -> Vec<u16> {
        self.composition_string.clone()
    }

    /// `IMEManager::updateCompositionString()` (:1101): the composition string and cursor from the IME, the wide call
    /// first, the ANSI call if that failed (converted, the cursor counted in characters and clamped to the string).
    pub fn update_composition_string<I: ImmApi>(&mut self, imm: &I) {
        self.composition_cursor_pos = 0;
        self.composition_string.clear();
        self.composition_string_length = 0;
        if self.has_context {
            let (result, wide) = imm.get_composition_string_w(MAX_COMPSTRINGLEN);
            if result >= 0 {
                self.composition_string_length = result / 2;
                self.composition_string = wide;
                self.composition_cursor_pos = imm.get_composition_cursor_pos_w() & 0xffff;
            } else {
                let (result, temp_buf) = imm.get_composition_string_a(MAX_COMPSTRINGLEN * 2);
                if result > 0 {
                    let temp_buf = &temp_buf[..(result as usize).min(temp_buf.len())];
                    let converted = imm.multi_byte_to_wide_char(temp_buf);
                    let mut conv_res = converted.len() as i32;
                    self.composition_string = converted;
                    if conv_res < 0 {
                        conv_res = 0;
                    } else {
                        self.composition_cursor_pos = imm.get_composition_cursor_pos_a() & 0xffff;
                        conv_res = self.composition_string.len() as i32; // GameStrlen
                    }
                    self.composition_cursor_pos = imm.mbsnccnt(temp_buf, self.composition_cursor_pos);
                    self.composition_string.truncate(conv_res as usize);
                    self.composition_string_length = conv_res;
                    if self.composition_cursor_pos > conv_res {
                        self.composition_cursor_pos = conv_res;
                    } else if self.composition_cursor_pos < 0 {
                        self.composition_cursor_pos = 0;
                    }
                }
            }
        }
        // DEBUG_ASSERTCRASH( m_compositionStringLength < MAX_COMPSTRINGLEN, ("composition string too large"));
        self.composition_string.truncate(self.composition_string_length.max(0) as usize);
    }

    /// `IMEManager::getResultsString()` (:1170): the finished text from the IME, wide first, else ANSI converted.
    pub fn get_results_string<I: ImmApi>(&mut self, imm: &I) {
        let mut string_len: i32 = 0;
        self.results_string.clear();
        if self.has_context {
            let (result, wide) = imm.get_result_string_w(MAX_COMPSTRINGLEN);
            if result >= 0 {
                string_len = result / 2;
                self.results_string = wide;
            } else {
                let (result, temp_buf) = imm.get_result_string_a(MAX_COMPSTRINGLEN * 2);
                if result > 0 {
                    let temp_buf = &temp_buf[..(result as usize).min(temp_buf.len())];
                    let converted = imm.multi_byte_to_wide_char(temp_buf);
                    string_len = converted.len() as i32;
                    self.results_string = converted;
                }
            }
        }
        // DEBUG_ASSERTCRASH( stringLen < MAX_COMPSTRINGLEN, ("results string too large"));
        self.results_string.truncate(string_len.max(0) as usize);
    }

    /// `IMEManager::convertToUnicode(mbcs, unicode)` (:1215): `MultiByteToWideChar` into a new string; empty on failure.
    pub fn convert_to_unicode<I: ImmApi>(imm: &I, mbcs: &[u8]) -> Vec<u16> {
        imm.multi_byte_to_wide_char(mbcs)
    }

    /// `IMEManager::updateProperties()` (:1473): the keyboard layout's index base and Unicode flag.
    pub fn update_properties<I: ImmApi>(&mut self, imm: &I) {
        let prop = imm.get_property();
        self.index_base = if prop & IME_PROP_CANDLIST_START_FROM_1 != 0 { 1 } else { 0 };
        self.unicode_ime = prop & IME_PROP_UNICODE != 0;
    }

    /// `IMEManager::getIndexBase()` (:1485).
    pub fn get_index_base(&self) -> i32 {
        self.index_base
    }

    /// `IMEManager::getCandidate(index)` (:1543): the candidate, or the empty string for an index out of range.
    pub fn get_candidate(&self, index: i32) -> Vec<u16> {
        if !self.candidate_string.is_empty() && index >= 0 && index < self.candidate_count {
            return self.candidate_string[index as usize].clone();
        }
        Vec::new() // UnicodeString::TheEmptyString
    }

    /// `IMEManager::getCandidatePageStart()` (:1575).
    pub fn get_candidate_page_start(&self) -> i32 {
        self.page_start
    }

    /// `IMEManager::updateCandidateList(candidateFlags)` (:1341): reads the first candidate list the flags name; the
    /// page size, count, start and selection are the list's (its page start is first moved onto the page of the
    /// selection); the arrows show when there are more candidates before or after the page.
    pub fn update_candidate_list<I: ImmApi, H: GuiHost>(&mut self, imm: &I, wm: &mut GameWindowManager, host: &mut H, candidate_flags: i32) {
        self.candidate_string.clear();
        self.page_size = 10;
        self.candidate_count = 0;
        self.page_start = 0;
        self.selected_index = 0;
        if self.candidate_window.is_none() || !self.has_context || candidate_flags == 0 {
            return;
        }
        let _ = host;
        let mut candidate: i32 = 1;
        for i in 0..32 {
            if candidate_flags & candidate != 0 {
                let mut unicode = true;
                let mut size = imm.get_candidate_list_count(true);
                if size <= 0 {
                    unicode = false;
                    size = imm.get_candidate_list_count(false);
                    if size <= 0 {
                        return;
                    }
                }
                let (bytes_copied, mut clist) = imm.get_candidate_list(i, unicode, size);
                let mut ok = true;
                if bytes_copied == 0 || bytes_copied > size {
                    // DEBUG_ASSERTCRASH(bytesCopied < size,("IME candidate buffer overrun"));
                    ok = false;
                }
                if ok && clist.dw_style != IME_CAND_UNKNOWN && clist.dw_style != IME_CAND_CODE {
                    if clist.dw_page_start > clist.dw_selection || clist.dw_selection >= clist.dw_page_start + clist.dw_page_size {
                        clist.dw_page_start = (clist.dw_selection / clist.dw_page_size.max(1)) * clist.dw_page_size;
                    }
                    self.page_size = clist.dw_page_size as i32;
                    self.candidate_count = clist.dw_count as i32;
                    self.page_start = clist.dw_page_start as i32;
                    self.selected_index = clist.dw_selection as i32;
                    if let Some(up) = self.candidate_up_arrow {
                        wm.win_hide(up, self.page_start == 0);
                    }
                    if let Some(down) = self.candidate_down_arrow {
                        wm.win_hide(down, self.candidate_count - self.page_start <= self.page_size);
                    }
                }
                if self.candidate_count > 0 {
                    self.candidate_string = vec![Vec::new(); self.candidate_count as usize];
                }
                if !self.candidate_string.is_empty() {
                    for k in 0..(self.candidate_count.max(0) as usize) {
                        match clist.strings.get(k) {
                            Some(CandidateString::Wide(w)) => self.candidate_string[k] = w.clone(),
                            Some(CandidateString::Ansi(a)) => self.candidate_string[k] = imm.multi_byte_to_wide_char(a),
                            None => {}
                        }
                    }
                }
                return;
            }
            candidate <<= 1;
        }
    }

    /// `IMEManager::resizeCandidateWindow(pageSize)` (:1494): the text area grows to `pageSize` lines of its font plus
    /// `IMECandidateWindowLineSpacing`, the window by the same difference, and the down arrow moves down by it.
    /// `font_height` is the text area's font height (`None`: no font, nothing done); `line_spacing` the global.
    pub fn resize_candidate_window<H: GuiHost>(&mut self, wm: &mut GameWindowManager, host: &mut H, page_size: i32, font_height: Option<i32>, line_spacing: i32) {
        let (Some(window), Some(text_area)) = (self.candidate_window, self.candidate_text_area) else { return };
        let Some(font_height) = font_height else { return };
        let newh = page_size * (font_height + line_spacing);
        let (w, h) = wm.win(text_area).size;
        let dif = newh - h;
        wm.win_set_size(host, text_area, w, newh);
        let (w, mut h) = wm.win(window).size;
        h += dif;
        wm.win_set_size(host, window, w, h);
        if let Some(down) = self.candidate_down_arrow {
            let (x, mut y) = (wm.win(down).region.0, wm.win(down).region.1);
            y += dif;
            wm.win_set_position(down, x, y);
        }
    }

    /// `IMEManager::openCandidateList(candidateFlags)` (:1250): the list is read, the window sized, shown on top and
    /// modal, then placed at the right edge of the screen at the top. Upstream also gathers the attached window's
    /// position, size, cursor and font height for a placement it then does not use (`cx = displayWidth - width`,
    /// `cy = 0`).
    pub fn open_candidate_list<I: ImmApi, H: GuiHost>(&mut self, imm: &I, wm: &mut GameWindowManager, host: &mut H, candidate_flags: i32, font_height: Option<i32>, line_spacing: i32) {
        let Some(window) = self.candidate_window else { return };
        self.update_candidate_list(imm, wm, host, candidate_flags);
        let page_size = self.page_size;
        self.resize_candidate_window(wm, host, page_size, font_height, line_spacing);
        wm.win_hide(window, false);
        wm.win_bring_to_top(window);
        wm.win_set_modal(Some(window));
        let (cwidth, _cheight) = wm.win(window).size;
        let cx = wm.display.0 - cwidth;
        let cy = 0;
        self.update_properties(imm);
        wm.win_set_position(window, cx, cy);
    }

    /// `IMEManager::closeCandidateList(candidateFlags)` (:1319): the window hides and stops being modal; the strings go.
    pub fn close_candidate_list(&mut self, wm: &mut GameWindowManager, _candidate_flags: i32) {
        if let Some(window) = self.candidate_window {
            wm.win_hide(window, true);
            wm.win_unset_modal(Some(window));
        }
        self.candidate_string.clear();
        self.candidate_count = 0;
    }

    /// `IMEManager::openStatusWindow()` (:1584): shows the status window.
    pub fn open_status_window(&self, wm: &mut GameWindowManager) {
        if let Some(w) = self.status_window {
            wm.win_hide(w, false);
        }
    }

    /// `IMEManager::closeStatusWindow()` (:1597): hides it.
    pub fn close_status_window(&self, wm: &mut GameWindowManager) {
        if let Some(w) = self.status_window {
            wm.win_hide(w, true);
        }
    }
}
