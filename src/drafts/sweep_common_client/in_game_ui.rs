//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp   (the functions the verified
//!     `crate::client::in_game_ui` selection model does not have: messages, hints, floating text, world animations,
//!     idle workers, movies, script-hidden superweapon displays)
//!   GeneralsMD/Code/GameEngine/Include/GameClient/InGameUI.h
//!
//! The members those functions use are [`InGameUiExtras`]; the globals they reach (`TheDisplayStringManager`,
//! `TheGameLogic`, `TheTacticalView`, `ThePartitionManager`, `TheWindowManager`, `TheVideoPlayer`, `TheTerrainVisual`,
//! `TheGameClient`) are [`InGameUiHost`].
//!
//! Excluded with reasons (#213):
//! - `InGameUI::DEBUG_addFloatingText`: `#if defined(_DEBUG) || defined(_INTERNAL)`, debug-only.
//! - the `AI_DEBUG_TOOLTIPS` block after `createGarrisonHint`: same conditional.
//! - `InGameUI::playMovie` / `stopMovie` / `playCameoMovie` / `stopCameoMovie` / `videoBuffer` open a Bink video
//!   stream: the stream and buffer calls are `// upstream: ... — device-only, not translated`; the bookkeeping around
//!   them (the movie name, the window's video buffer being set and cleared) is kept.

use super::anim2d::{Anim2DCollection, Anim2DHost, Anim2DId, ANIM_2D_STATUS_COMPLETE};
use std::collections::BTreeMap;

/// `LOGICFRAMES_PER_SECOND`.
pub const LOGICFRAMES_PER_SECOND: u32 = 30;
/// `MAX_UI_MESSAGES`.
pub const MAX_UI_MESSAGES: usize = 6;
/// `MAX_MOVE_HINTS`.
pub const MAX_MOVE_HINTS: usize = 256;
/// `MAX_PLAYER_COUNT`.
pub const MAX_PLAYER_COUNT: usize = 16;
/// `WORLD_ANIM_FADE_ON_EXPIRE`.
pub const WORLD_ANIM_FADE_ON_EXPIRE: u32 = 0x1;
/// `WORLD_ANIM_PLAY_ONCE_AND_DESTROY`.
pub const WORLD_ANIM_PLAY_ONCE_AND_DESTROY: u32 = 0x2;
/// `FRAMES_BEFORE_EXPIRE_TO_FADE`.
pub const FRAMES_BEFORE_EXPIRE_TO_FADE: u32 = LOGICFRAMES_PER_SECOND;

/// `Color` is an ARGB `u32` (`GameMakeColor`).
pub type Color = u32;

/// `GameMakeColor(r, g, b, a)`.
pub fn game_make_color(r: u32, g: u32, b: u32, a: u32) -> Color {
    (a << 24) | (r << 16) | (g << 8) | b
}

/// `GameGetColorComponents(color, &r, &g, &b, &a)`.
pub fn game_get_color_components(c: Color) -> (u8, u8, u8, u8) {
    (((c >> 16) & 0xff) as u8, ((c >> 8) & 0xff) as u8, (c & 0xff) as u8, ((c >> 24) & 0xff) as u8)
}

/// `struct UIMessage` (`fullText`, `displayString`, `timestamp`, `color`); the display string is the host's handle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiMessage {
    /// `fullText`.
    pub full_text: Vec<u16>,
    /// `displayString` (a host handle; `None` for NULL).
    pub display_string: Option<u32>,
    /// `timestamp`.
    pub timestamp: u32,
    /// `color`.
    pub color: Color,
}

/// `struct MoveHintStruct`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveHint {
    /// `pos`.
    pub pos: (f32, f32, f32),
    /// `frame`.
    pub frame: u32,
    /// `sourceID`.
    pub source_id: u32,
}

/// `HintType`: only `MOVE_HINT` exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintType {
    /// `MOVE_HINT`.
    MoveHint,
}

/// `class FloatingTextData`.
#[derive(Clone, Debug, PartialEq)]
pub struct FloatingTextData {
    /// `m_color`.
    pub color: Color,
    /// `m_pos3D`.
    pub pos_3d: (f32, f32, f32),
    /// `m_frameTimeOut`.
    pub frame_time_out: u32,
    /// `m_frameCount`.
    pub frame_count: u32,
    /// `m_dString` (host handle).
    pub d_string: Option<u32>,
}

/// `class WorldAnimationData`.
#[derive(Clone, Debug)]
pub struct WorldAnimationData {
    /// `m_anim`.
    pub anim: Anim2DId,
    /// `m_worldPos`.
    pub world_pos: (f32, f32, f32),
    /// `m_expireFrame`.
    pub expire_frame: u32,
    /// `m_options`.
    pub options: u32,
    /// `m_zRisePerSecond`.
    pub z_rise_per_second: f32,
}

/// `class SuperweaponInfo` (the parts the draws and script flags use).
#[derive(Clone, Debug, PartialEq)]
pub struct SuperweaponInfo {
    /// `m_id` (the object's id).
    pub id: u32,
    /// `m_hiddenByScript`.
    pub hidden_by_script: bool,
    /// `m_color`.
    pub color: Color,
    /// `m_nameDisplayString` (host handle).
    pub name_display_string: u32,
    /// `m_timeDisplayString` (host handle).
    pub time_display_string: u32,
}

/// What these functions reach through globals.
pub trait InGameUiHost: Anim2DHost {
    /// `TheDisplayStringManager->newDisplayString()` (a handle).
    fn new_display_string(&mut self) -> u32;
    /// `TheDisplayStringManager->freeDisplayString(handle)`.
    fn free_display_string(&mut self, handle: u32);
    /// `displayString->setFont(TheFontLibrary->getFont(font, TheGlobalLanguageData->adjustFontSize(pointSize), bold))`
    /// with the UI's message font members.
    fn set_message_font(&mut self, handle: u32);
    /// `displayString->setText(text)`.
    fn display_string_set_text(&mut self, handle: u32, text: &[u16]);
    /// `displayString->getWidth()`.
    fn display_string_width(&self, handle: u32) -> i32;
    /// `displayString->getSize(&width, NULL)`.
    fn display_string_size(&self, handle: u32) -> (i32, i32);
    /// `displayString->getFont()->height`.
    fn display_string_font_height(&self, handle: u32) -> f32;
    /// `displayString->draw(x, y, color, dropColor)`.
    fn display_string_draw(&mut self, handle: u32, x: i32, y: i32, color: Color, drop_color: Color);
    /// `TheGameLogic->isGamePaused()`.
    fn game_paused(&self) -> bool;
    /// `TheGameClient->getFrame()`.
    fn client_frame(&self) -> u32;
    /// `TheTacticalView->worldToScreen(pos, &screen)`.
    fn world_to_screen(&self, pos: (f32, f32, f32)) -> Option<(i32, i32)>;
    /// `TheTacticalView->getMaxZoom()`, `getZoom()`.
    fn view_max_zoom_and_zoom(&self) -> (f32, f32);
    /// `ThePartitionManager->getShroudStatusForPlayer(localPlayerIndex, ...)` is `CELLSHROUD_CLEAR` for a position.
    fn pos_clear_for_local_player(&self, pos: (f32, f32, f32)) -> bool;
    /// Same for a partition cell (`worldToCell` then the cell status).
    fn cell_of_pos_clear_for_local_player(&self, x: f32, y: f32) -> bool;
    /// `ThePlayerList->getLocalPlayer()->getPlayerIndex()`.
    fn local_player_index(&self) -> usize;
    /// `TheVideoPlayer->open(name)`: `Some(handle)`; Bink is device-only, so a host may always say `None`.
    fn video_open(&mut self, movie_name: &[u8]) -> Option<u32>;
    /// `TheWindowManager->winGetWindowFromId(NULL, nameToKey(name))`: whether the window exists; then `winGetInstanceData()
    /// ->setVideoBuffer(buffer)` (`None` clears it).
    fn window_set_video_buffer(&mut self, window_name: &str, buffer: Option<u32>);
    /// `TheWindowManager->winCreateFromScript(name)`: the window's handle.
    fn win_create_from_script(&mut self, name: &str) -> Option<u32>;
    /// `TheWindowManager->winGetWindowFromId(NULL, nameToKey("ControlBar.wnd:ButtonIdleWorker"))`.
    fn find_idle_worker_window(&mut self) -> Option<u32>;
    /// `m_idleWorkerWin->winEnable(enable)`.
    fn idle_worker_window_enable(&mut self, win: u32, enable: bool);
    /// `GadgetButtonSetText(win, UnicodeString::TheEmptyString)`.
    fn idle_worker_window_clear_text(&mut self, win: u32);
    /// `TheTerrainVisual->removeFactionBibDrawable(draw)` then `TheGameClient->destroyDrawable(draw)`.
    fn destroy_placement_icon(&mut self, drawable: u32);
    /// `TheTerrainVisual->removeAllBibs()`.
    fn remove_all_bibs(&mut self);
    /// `TheGlobalData->m_maxLineBuildObjects`.
    fn max_line_build_objects(&self) -> usize;
}

/// `InGameUI`'s members used by the functions below.
#[derive(Clone, Debug)]
pub struct InGameUiExtras {
    /// `m_superweapons[MAX_PLAYER_COUNT]`: per player, a map from a name to its infos.
    pub superweapons: Vec<BTreeMap<Vec<u8>, Vec<SuperweaponInfo>>>,
    /// `m_superweaponHiddenByScript`.
    pub superweapon_hidden_by_script: bool,
    /// `m_uiMessages[MAX_UI_MESSAGES]`.
    pub ui_messages: Vec<UiMessage>,
    /// `m_messageColor1`, `m_messageColor2`.
    pub message_color_1: Color,
    /// `m_messageColor2`.
    pub message_color_2: Color,
    /// `m_isDragSelecting`.
    pub is_drag_selecting: bool,
    /// `m_dragSelectRegion` as `(lo.x, lo.y, hi.x, hi.y)`.
    pub drag_select_region: (i32, i32, i32, i32),
    /// `m_moveHint[MAX_MOVE_HINTS]`.
    pub move_hint: Vec<MoveHint>,
    /// `m_nextMoveHint`.
    pub next_move_hint: usize,
    /// `m_placeIcon[]` (drawable ids; `None` for NULL).
    pub place_icon: Vec<Option<u32>>,
    /// `m_floatingTextList` (the newest at the front).
    pub floating_text_list: Vec<FloatingTextData>,
    /// `m_floatingTextMoveVanishRate`.
    pub floating_text_move_vanish_rate: f32,
    /// `m_floatingTextMoveUpSpeed`.
    pub floating_text_move_up_speed: f32,
    /// `updateFloatingText`'s function-static `lastLogicFrameUpdate` (first value is the frame of the first call).
    pub last_logic_frame_update: Option<u32>,
    /// `m_worldAnimationList` (the newest at the front).
    pub world_animation_list: Vec<WorldAnimationData>,
    /// `m_idleWorkers[MAX_PLAYER_COUNT]` (object ids).
    pub idle_workers: Vec<Vec<u32>>,
    /// `m_idleWorkerWin`.
    pub idle_worker_win: Option<u32>,
    /// `m_currentIdleWorkerDisplay`.
    pub current_idle_worker_display: i32,
    /// `m_inputEnabled`.
    pub input_enabled: bool,
    /// `m_replayWindow`.
    pub replay_window: Option<u32>,
    /// `m_videoStream`, `m_videoBuffer`, `m_currentlyPlayingMovie`.
    pub video_stream: Option<u32>,
    /// `m_videoBuffer`.
    pub video_buffer: Option<u32>,
    /// `m_currentlyPlayingMovie`.
    pub currently_playing_movie: Vec<u8>,
    /// `m_cameoVideoStream`.
    pub cameo_video_stream: Option<u32>,
    /// `m_cameoVideoBuffer`.
    pub cameo_video_buffer: Option<u32>,
}

impl InGameUiExtras {
    /// The constructor's initial values for these members.
    pub fn new() -> InGameUiExtras {
        InGameUiExtras {
            superweapons: vec![BTreeMap::new(); MAX_PLAYER_COUNT],
            superweapon_hidden_by_script: false,
            ui_messages: vec![UiMessage::default(); MAX_UI_MESSAGES],
            message_color_1: 0,
            message_color_2: 0,
            is_drag_selecting: false,
            drag_select_region: (0, 0, 0, 0),
            move_hint: vec![MoveHint::default(); MAX_MOVE_HINTS],
            next_move_hint: 0,
            place_icon: Vec::new(),
            floating_text_list: Vec::new(),
            floating_text_move_vanish_rate: 0.0,
            floating_text_move_up_speed: 0.0,
            last_logic_frame_update: None,
            world_animation_list: Vec::new(),
            idle_workers: vec![Vec::new(); MAX_PLAYER_COUNT],
            idle_worker_win: None,
            current_idle_worker_display: -1,
            input_enabled: true,
            replay_window: None,
            video_stream: None,
            video_buffer: None,
            currently_playing_movie: Vec::new(),
            cameo_video_stream: None,
            cameo_video_buffer: None,
        }
    }

    /// `InGameUI::showObjectSuperweaponDisplayByScript(obj)` (InGameUI.cpp:699): clear `m_hiddenByScript` on every info
    /// of every player whose id is the object's.
    pub fn show_object_superweapon_display_by_script(&mut self, obj_id: u32) {
        for player in self.superweapons.iter_mut() {
            for list in player.values_mut() {
                for info in list.iter_mut() {
                    if info.id == obj_id {
                        info.hidden_by_script = false;
                    }
                }
            }
        }
    }

    /// `InGameUI::getSuperweaponDisplayEnabledByScript()` (InGameUI.cpp:726): upstream returns
    /// `m_superweaponHiddenByScript` itself, which is the OPPOSITE of what the name says (`setSuperweaponDisplayEnabledByScript(enable)`
    /// stores `!enable`). Kept as upstream has it.
    pub fn get_superweapon_display_enabled_by_script(&self) -> bool {
        self.superweapon_hidden_by_script
    }

    /// `InGameUI::addMessageText(formattedMessage, rgbColor)` (InGameUI.cpp:2085). `rgb_color` is
    /// `rgbColor->getAsInt()` (0x00RRGGBB) when given.
    pub fn add_message_text<H: InGameUiHost + ?Sized>(&mut self, host: &mut H, formatted_message: &[u16], rgb_color: Option<u32>) {
        let mut color1 = self.message_color_1;
        let mut color2 = self.message_color_2;
        if let Some(rgb) = rgb_color {
            color1 = rgb | game_make_color(0, 0, 0, 255);
            color2 = rgb | game_make_color(0, 0, 0, 255);
        }
        // delete the message stuff at the last index
        let last = MAX_UI_MESSAGES - 1;
        self.ui_messages[last].full_text.clear();
        if let Some(h) = self.ui_messages[last].display_string.take() {
            host.free_display_string(h);
        }
        self.ui_messages[last].timestamp = 0;
        // shift all the messages down one index and remove the last one
        for i in (1..MAX_UI_MESSAGES).rev() {
            self.ui_messages[i] = self.ui_messages[i - 1].clone();
        }
        // set the new message in index 0
        self.ui_messages[0].full_text = formatted_message.to_vec();
        self.ui_messages[0].timestamp = host.logic_frame();
        let ds = host.new_display_string();
        self.ui_messages[0].display_string = Some(ds);
        host.set_message_font(ds);
        host.display_string_set_text(ds, formatted_message);
        // assign a color for this string instance that will stay with it no matter what line it is rendered on
        if self.ui_messages[1].display_string.is_none() || self.ui_messages[1].color == color2 {
            self.ui_messages[0].color = color1;
        } else {
            self.ui_messages[0].color = color2;
        }
    }

    /// `InGameUI::removeMessageAtIndex(i)` (InGameUI.cpp:2134).
    pub fn remove_message_at_index<H: InGameUiHost + ?Sized>(&mut self, host: &mut H, i: usize) {
        self.ui_messages[i].full_text.clear();
        if let Some(h) = self.ui_messages[i].display_string.take() {
            host.free_display_string(h);
        }
        self.ui_messages[i].timestamp = 0;
    }

    /// `InGameUI::beginAreaSelectHint(msg)` (InGameUI.cpp:2148): `msg->getArgument(0)->pixelRegion`.
    pub fn begin_area_select_hint(&mut self, pixel_region: (i32, i32, i32, i32)) {
        self.is_drag_selecting = true;
        self.drag_select_region = pixel_region;
    }

    /// `InGameUI::expireHint(type, hintIndex)` (InGameUI.cpp:3836).
    pub fn expire_hint(&mut self, t: HintType, hint_index: usize) {
        match t {
            HintType::MoveHint => {
                if hint_index >= MAX_MOVE_HINTS {
                    return;
                }
                self.move_hint[hint_index].source_id = 0;
                self.move_hint[hint_index].frame = 0;
            }
        }
    }

    /// `InGameUI::createMoveHint(msg)` (InGameUI.cpp:2165). `source_id` / `location` are the message's argument 0
    /// (`objectID` and `location`; the same union slot upstream reads both ways). `lone_selected_is_immobile` is
    /// `getSelectCount() == 1` and the first selected object `isKindOf(KINDOF_IMMOBILE)`.
    pub fn create_move_hint<H: InGameUiHost + ?Sized>(&mut self, host: &H, source_id: u32, location: (f32, f32, f32), lone_selected_is_immobile: bool) {
        // first, remove any existing move hint for this source if present
        for i in 0..MAX_MOVE_HINTS {
            if self.move_hint[i].source_id == source_id && self.move_hint[i].frame != 0 {
                self.expire_hint(HintType::MoveHint, i);
            }
        }
        if lone_selected_is_immobile {
            // "Don't allow move hints to be created if our selected object can't move!"
            return;
        }
        let n = self.next_move_hint;
        self.move_hint[n].frame = host.client_frame();
        self.move_hint[n].pos = location;
        self.next_move_hint += 1;
        // wrap around
        if self.next_move_hint == MAX_MOVE_HINTS {
            self.next_move_hint = 0;
        }
    }

    /// `InGameUI::createAttackHint(msg)` (InGameUI.cpp:2200): empty upstream.
    pub fn create_attack_hint(&mut self) {}

    /// `InGameUI::createForceAttackHint(msg)` (InGameUI.cpp:2208): empty upstream.
    pub fn create_force_attack_hint(&mut self) {}

    /// `InGameUI::createGarrisonHint(msg)` (InGameUI.cpp:2216): `draw->onSelected()` on the found drawable; the caller
    /// runs [`crate::drafts::sweep_common_client::drawable::DrawableUi::on_selected`] for it.
    pub fn create_garrison_hint(&mut self) {}

    /// `InGameUI::destroyPlacementIcons()` (InGameUI.cpp:2957).
    pub fn destroy_placement_icons<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        for i in 0..host.max_line_build_objects() {
            if i < self.place_icon.len() {
                if let Some(d) = self.place_icon[i] {
                    host.destroy_placement_icon(d);
                }
                self.place_icon[i] = None;
            }
        }
        host.remove_all_bibs();
    }

    /// `InGameUI::createReplayControl()` (InGameUI.cpp:3881): `winCreateFromScript("ReplayControl.wnd")`.
    pub fn create_replay_control<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        self.replay_window = host.win_create_from_script("ReplayControl.wnd");
    }

    /// `InGameUI::playCameoMovie(movieName)` (InGameUI.cpp:3953).
    pub fn play_cameo_movie<H: InGameUiHost + ?Sized>(&mut self, host: &mut H, movie_name: &[u8]) {
        self.stop_cameo_movie(host);
        self.cameo_video_stream = host.video_open(movie_name);
        if self.cameo_video_stream.is_none() {
            return;
        }
        // upstream: TheDisplay->createVideoBuffer() / allocate(width, height) — device-only, not translated
        // (a failed buffer allocation calls stopCameoMovie() and returns)
        host.window_set_video_buffer("ControlBar.wnd:RightHUD", self.cameo_video_buffer);
    }

    /// `InGameUI::stopCameoMovie()` (InGameUI.cpp:3983).
    pub fn stop_cameo_movie<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        host.window_set_video_buffer("ControlBar.wnd:RightHUD", None);
        // upstream: delete m_cameoVideoBuffer — device-only, not translated
        self.cameo_video_buffer = None;
        if self.cameo_video_stream.is_some() {
            // upstream: m_cameoVideoStream->close() — device-only, not translated
            self.cameo_video_stream = None;
        }
    }

    /// `InGameUI::updateFloatingText()` (InGameUI.cpp:5054).
    pub fn update_floating_text<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        let curr_logic_frame = host.logic_frame();
        // static UnsignedInt lastLogicFrameUpdate = currLogicFrame;
        let last = *self.last_logic_frame_update.get_or_insert(curr_logic_frame);
        // only update the position if we're incrementing frames
        if last == curr_logic_frame {
            return;
        }
        self.last_logic_frame_update = Some(curr_logic_frame);
        let mut i = 0;
        while i < self.floating_text_list.len() {
            // move it up
            self.floating_text_list[i].frame_count += 1;
            // fade the text
            if curr_logic_frame > self.floating_text_list[i].frame_time_out {
                let (r, g, b, a) = game_get_color_components(self.floating_text_list[i].color);
                let amount = ((curr_logic_frame - self.floating_text_list[i].frame_time_out) as f32 * self.floating_text_move_vanish_rate) as i32;
                let a = if (a as i32) - amount < 0 { 0 } else { (a as i32 - amount) as u32 };
                self.floating_text_list[i].color = game_make_color(r as u32, g as u32, b as u32, a);
                // if we have 0 alpha delete it
                if a == 0 {
                    let ftd = self.floating_text_list.remove(i);
                    if let Some(h) = ftd.d_string {
                        host.free_display_string(h);
                    }
                    continue; // don't do the increment below
                }
            }
            i += 1;
        }
    }

    /// `InGameUI::drawFloatingText()` (InGameUI.cpp:5106).
    pub fn draw_floating_text<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        for ftd in self.floating_text_list.clone() {
            let clear = host.cell_of_pos_clear_for_local_player(ftd.pos_3d.0, ftd.pos_3d.1);
            if let (Some(mut pos), Some(ds)) = (host.world_to_screen(ftd.pos_3d), ftd.d_string) {
                if clear {
                    pos.1 -= (ftd.frame_count as f32 * self.floating_text_move_up_speed) as i32;
                    // make drop color black, but use the alpha setting of the fill color specified (for fading)
                    let (_r, _g, _b, a) = game_get_color_components(ftd.color);
                    let drop_color = game_make_color(0, 0, 0, a as u32);
                    let (width, _h) = host.display_string_size(ds);
                    host.display_string_draw(ds, pos.0 - (width / 2), pos.1, ftd.color, drop_color);
                }
            }
        }
    }

    /// `InGameUI::clearFloatingText()` (InGameUI.cpp:5145).
    pub fn clear_floating_text<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        for ftd in self.floating_text_list.drain(..) {
            if let Some(h) = ftd.d_string {
                host.free_display_string(h);
            }
        }
    }

    /// `InGameUI::clearWorldAnimations()` (InGameUI.cpp:5316).
    pub fn clear_world_animations(&mut self, anims: &mut Anim2DCollection) {
        for wad in self.world_animation_list.drain(..) {
            anims.delete_anim(wad.anim);
        }
    }

    /// `InGameUI::updateAndDrawWorldAnimations()` (InGameUI.cpp:5347).
    pub fn update_and_draw_world_animations<H: InGameUiHost + ?Sized>(&mut self, anims: &mut Anim2DCollection, host: &mut H) {
        let mut i = 0;
        while i < self.world_animation_list.len() {
            // update portion ... only when the game is in motion
            if !host.game_paused() {
                let wad = &self.world_animation_list[i];
                let complete = anims.instances[wad.anim].as_ref().map_or(false, |a| a.status & ANIM_2D_STATUS_COMPLETE != 0);
                if host.logic_frame() >= wad.expire_frame || (wad.options & WORLD_ANIM_PLAY_ONCE_AND_DESTROY != 0 && complete) {
                    let wad = self.world_animation_list.remove(i);
                    anims.delete_anim(wad.anim);
                    continue;
                }
                // update the Z value
                let wad = &mut self.world_animation_list[i];
                if wad.z_rise_per_second != 0.0 {
                    wad.world_pos.2 += wad.z_rise_per_second / LOGICFRAMES_PER_SECOND as f32;
                }
            }
            // don't bother going forward with the draw process if this location is shrouded for the local player
            let wad_pos = self.world_animation_list[i].world_pos;
            if !host.pos_clear_for_local_player(wad_pos) {
                i += 1;
                continue;
            }
            // update translucency value
            let (anim, options, expire_frame) = {
                let w = &self.world_animation_list[i];
                (w.anim, w.options, w.expire_frame)
            };
            if options & WORLD_ANIM_FADE_ON_EXPIRE != 0 {
                let frames_till_expire = expire_frame.wrapping_sub(host.logic_frame());
                if frames_till_expire < FRAMES_BEFORE_EXPIRE_TO_FADE {
                    // compute alpha level so that we're totally gone by the expire frame
                    let alpha = frames_till_expire as f32 / FRAMES_BEFORE_EXPIRE_TO_FADE as f32;
                    if let Some(Some(a)) = anims.instances.get_mut(anim) {
                        a.alpha = alpha;
                    }
                }
            }
            // project the point to screen space
            if let Some(mut screen) = host.world_to_screen(wad_pos) {
                let mut width = anims.get_current_frame_width(anim);
                let mut height = anims.get_current_frame_height(anim);
                // scale the width and height given the camera zoom level
                let (max_zoom, zoom) = host.view_max_zoom_and_zoom();
                let zoom_scale = max_zoom / zoom;
                width = (width as f32 * zoom_scale) as u32;
                height = (height as f32 * zoom_scale) as u32;
                // adjust the screen position to draw so the image is centered at the location
                screen.0 -= (width / 2) as i32;
                screen.1 -= (height / 2) as i32;
                anims.draw_sized(anim, screen.0, screen.1, width as i32, height as i32, host);
            }
            i += 1;
        }
    }

    /// `InGameUI::findIdleWorker(obj)` (InGameUI.cpp:5445): the object if it is in its controlling player's list.
    pub fn find_idle_worker(&self, obj: Option<(u32, usize)>) -> Option<u32> {
        let (obj_id, index) = obj?;
        if self.idle_workers[index].is_empty() {
            return None;
        }
        self.idle_workers[index].iter().copied().find(|&o| o == obj_id)
    }

    /// `InGameUI::addIdleWorker(obj)`.
    pub fn add_idle_worker(&mut self, obj: Option<(u32, usize)>) {
        let (obj_id, index) = match obj {
            Some(o) => o,
            None => return,
        };
        if self.find_idle_worker(obj).is_some() {
            return;
        }
        self.idle_workers[index].push(obj_id);
    }

    /// `InGameUI::removeIdleWorker(obj, playerNumber)`: out-of-range player numbers (leaving the game) are ignored.
    pub fn remove_idle_worker(&mut self, obj_id: Option<u32>, player_number: i32) {
        let obj_id = match obj_id {
            Some(o) => o,
            None => return,
        };
        if player_number < 0 || player_number as usize >= MAX_PLAYER_COUNT {
            return;
        }
        let list = &mut self.idle_workers[player_number as usize];
        if let Some(pos) = list.iter().position(|&o| o == obj_id) {
            list.remove(pos);
        }
    }

    /// `InGameUI::getIdleWorkerCount()` (InGameUI.cpp:5578): the local player's list size.
    pub fn get_idle_worker_count<H: InGameUiHost + ?Sized>(&self, host: &H) -> i32 {
        self.idle_workers[host.local_player_index()].len() as i32
    }

    /// `InGameUI::showIdleWorkerLayout()` (InGameUI.cpp:5584).
    pub fn show_idle_worker_layout<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        if self.idle_worker_win.is_none() {
            self.idle_worker_win = host.find_idle_worker_window();
            return;
        }
        host.idle_worker_window_enable(self.idle_worker_win.unwrap(), true);
        self.current_idle_worker_display = self.get_idle_worker_count(host);
    }

    /// `InGameUI::hideIdleWorkerLayout()` (InGameUI.cpp:5606).
    pub fn hide_idle_worker_layout<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        let win = match self.idle_worker_win {
            Some(w) => w,
            None => return,
        };
        host.idle_worker_window_clear_text(win);
        host.idle_worker_window_enable(win, false);
        self.current_idle_worker_display = -1;
    }

    /// `InGameUI::updateIdleWorker()` (InGameUI.cpp:5615).
    pub fn update_idle_worker<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        let idle_count = self.get_idle_worker_count(host);
        if idle_count > 0 && self.current_idle_worker_display != idle_count && self.input_enabled {
            self.show_idle_worker_layout(host);
        }
        if (idle_count <= 0 && self.idle_worker_win.is_some()) || !self.input_enabled {
            self.hide_idle_worker_layout(host);
        }
    }

    /// `InGameUI::resetIdleWorker()` (InGameUI.cpp:5626).
    pub fn reset_idle_worker<H: InGameUiHost + ?Sized>(&mut self, host: &mut H) {
        if let Some(w) = self.idle_worker_win {
            host.idle_worker_window_clear_text(w);
        }
        self.current_idle_worker_display = -1;
        for i in 0..MAX_PLAYER_COUNT {
            self.idle_workers[i].clear();
        }
    }
}

impl SuperweaponInfo {
    /// `SuperweaponInfo::drawName(x, y, color, dropColor)` (InGameUI.cpp:326): colour 0 means `m_color`; right-aligned
    /// at `x` (drawn at `x - width`).
    pub fn draw_name<H: InGameUiHost + ?Sized>(&self, host: &mut H, x: i32, y: i32, color: Color, drop_color: Color) {
        let color = if color == 0 { self.color } else { color };
        let w = host.display_string_width(self.name_display_string);
        host.display_string_draw(self.name_display_string, x - w, y, color, drop_color);
    }

    /// `SuperweaponInfo::drawTime(x, y, color, dropColor)` (InGameUI.cpp:334).
    pub fn draw_time<H: InGameUiHost + ?Sized>(&self, host: &mut H, x: i32, y: i32, color: Color, drop_color: Color) {
        let color = if color == 0 { self.color } else { color };
        host.display_string_draw(self.time_display_string, x, y, color, drop_color);
    }
}

/// `InGameUI::isDrawableSelected(idToCheck)` (InGameUI.cpp:3340) over the selected list's ids.
pub fn is_drawable_selected(selected: &[u32], id_to_check: u32) -> bool {
    selected.iter().any(|&d| d == id_to_check)
}

/// `InGameUI::isAllSelectedKindOf(kindOf)` (InGameUI.cpp:3381): TRUE when every (non-NULL) selected drawable has the
/// kind; vacuously TRUE for an empty selection. `is_kind_of` is `draw->isKindOf(kindOf)`.
pub fn is_all_selected_kind_of(selected: &[u32], is_kind_of: &dyn Fn(u32) -> bool) -> bool {
    for &d in selected {
        if !is_kind_of(d) {
            return false;
        }
    }
    true
}

/// `InGameUI::disregardDrawable(draw)` (InGameUI.cpp:3439): "make sure drawable is no longer selected" — the caller
/// runs the verified `InGameUiSelection::deselect_drawable` (`crate::client::in_game_ui`) for it.
pub fn disregard_drawable() {}

/// `InGameUI::canSelectedObjectsNonAttackInteractWithObject(objectToInteractWith, rule)` (InGameUI.cpp:4175): any action
/// type from 1 up, except `ACTIONTYPE_ATTACK_OBJECT`, that `canSelectedObjectsDoAction` allows. `attack_object` is
/// `ACTIONTYPE_ATTACK_OBJECT`'s value, `num_actiontypes` is `NUM_ACTIONTYPES`.
pub fn can_selected_objects_non_attack_interact_with_object(num_actiontypes: i32, attack_object: i32, can_do_action: &mut dyn FnMut(i32) -> bool) -> bool {
    for i in 1..num_actiontypes {
        if i != attack_object && can_do_action(i) {
            return true;
        }
    }
    false
}
