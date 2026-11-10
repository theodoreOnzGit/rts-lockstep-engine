// SPDX-License-Identifier: GPL-3.0-only
//! The player's input, milestone C2d (GitHub #6): pointer events become the translated client's calls, in the order the
//! game's message stream runs its translators (~~placement, then selection, then commands~~ **Corrected 2026-11-06
//! (#54):** placement (30), the pending GUI command (40), selection (50), then commands (70); `GameClient.cpp`'s translator
//! list). Plain Rust, no Bevy: the application supplies what the view knows (the objects under the pointer or in a
//! dragged box, the ground point under a pixel), and its tests drive the same calls without a window.
//!
//! The client still never changes the simulation: everything here reaches the logic as the game's own command messages
//! on the World's command list ([`WorldClient`]); `rts_lockstep_engine::client::world_host`'s test checks that the questions
//! asked here leave the CRCs alone.

use rts_lockstep_engine::client::command_xlat::{CommandEvaluateType, CommandTranslator, CommandXlatHost, GuiButton};
use rts_lockstep_engine::client::control_bar::{process_command_ui, CbCommandStatus, PressedControl};
use rts_lockstep_engine::client::gui_command_xlat::GuiCommandTranslator;
use rts_lockstep_engine::client::in_game_ui::InGameUiSelection;
use rts_lockstep_engine::client::place_event::PlaceEventTranslator;
use rts_lockstep_engine::client::selection_info::SelectionInfoHost;
use rts_lockstep_engine::client::game_pause::ClientMouse;
use rts_lockstep_engine::client::script_input::apply_script_input;
use rts_lockstep_engine::client::selection_xlat::{Disposition, SelectionTranslator};
use rts_lockstep_engine::client::window_manager::WinInputReturnCode;
use rts_lockstep_engine::client::window_xlat::{translate_game_message as translate_window_message, MouseData, RawMessage, WindowXlatHost};
use rts_lockstep_engine::common::message_types::{MSG_RAW_MOUSE_LEFT_BUTTON_DOWN, MSG_RAW_MOUSE_LEFT_BUTTON_UP, MSG_RAW_MOUSE_POSITION};
use rts_lockstep_engine::client::world_host::WorldClient;
use rts_lockstep_engine::client::control_bar_callback::{left_hud_input_click, RadarClick};
use rts_lockstep_engine::client::radar::{RadarFeedback, RadarFrame};
use rts_lockstep_engine::client::radar_world::ClientRadar;
use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::logic::world::game_logic::World;

/// How far, in pixels, the pointer may move between press and release for a click rather than a drag (upstream's
/// `Mouse::isClick` uses the drag tolerance from `Mouse.ini`, 4 by default in the shipped options).
pub const CLICK_TOLERANCE: i32 = 4;

/// The client's state: the World host and InGameUI, with the translators that act on them.
pub struct Controls {
    pub client: WorldClient,
    pub ui: InGameUiSelection,
    pub selection: SelectionTranslator,
    pub commands: CommandTranslator,
    pub place: PlaceEventTranslator,
    /// #54: `GUICommandTranslator`, a pending control bar command carried out by the next click.
    pub gui: GuiCommandTranslator,
    /// The last hint the pointer made (for the cursor).
    pub hint: Option<MessageType>,
    /// #35: `TheRadar` and what it draws (`rts_lockstep_engine::client::radar_world`).
    pub radar: ClientRadar,
    /// #119: `TheTacticalView->isMouseLocked()` and `TheInGameUI->isScrolling()` as the camera
    /// ([`crate::camera::Camera`]) last left them, for the window translator.
    pub view_mouse_locked: bool,
    pub ui_scrolling: bool,
}

/// #116: what `WindowTranslator` asks, in the game under the application's own windows: the windows use nothing (the
/// application keeps the pointer from the translators over them, and over the shell), the shell is not active, nothing
/// plays a movie, and the tactical view's mouse is locked while a drag selection is under way ~~(the port's camera never
/// locks it)~~ **Corrected 2026-11-10 (#119):** or while the camera's `LookAtTranslator` scrolls (it locks it). The
/// InGameUI answers for the input, the placement and (#119) the scroll.
struct GameWindows<'a> {
    ui: &'a InGameUiSelection,
    mouse_locked: bool,
    scrolling: bool,
    pixel: ICoord2D,
}

impl WindowXlatHost for GameWindows<'_> {
    fn mouse_locked(&self) -> bool {
        self.mouse_locked
    }
    fn ui_is_scrolling(&self) -> bool {
        self.scrolling
    }
    fn ui_input_enabled(&self) -> bool {
        self.ui.input_enabled()
    }
    fn ui_placement_anchored(&self) -> bool {
        self.ui.is_placement_anchored()
    }
    fn shell_active(&self) -> bool {
        false
    }
    fn mouse_status_pos(&self) -> ICoord2D {
        self.pixel
    }
    fn win_process_mouse_event(&mut self, _gwm: u32, _pos: ICoord2D, _data: MouseData) -> WinInputReturnCode {
        WinInputReturnCode::NotUsed
    }
    fn win_process_key(&mut self, _key: u8, _state: u8) -> WinInputReturnCode {
        WinInputReturnCode::NotUsed
    }
    fn movie_playing(&self) -> bool {
        false
    }
    fn allow_exit_out_of_movies(&self) -> bool {
        false
    }
    fn stop_movie(&mut self) {}
}

/// A button of the selected unit's command set, with its label (the button's name, until the game's text is loaded).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarButton {
    pub label: String,
    pub button: GuiButton,
}

impl Controls {
    pub fn new(world: World, player: i32) -> Self {
        let mut c = Controls {
            client: WorldClient::new(world, player),
            ui: InGameUiSelection::new(),
            selection: SelectionTranslator::default(),
            commands: CommandTranslator::default(),
            place: PlaceEventTranslator::default(),
            gui: GuiCommandTranslator,
            hint: None,
            radar: ClientRadar::default(),
            view_mouse_locked: false,
            ui_scrolling: false,
        };
        // #35: `TheRadar->newMap(TheTerrainLogic)` (`GameLogic::startNewGame`)
        c.radar.new_map(&mut c.client.world);
        c
    }

    /// #35: a logic frame's events to the radar: its lists, events and flags; the messages it shows
    /// (`TheInGameUI->message`) join [`WorldClient::ui_messages`]. The answer says which stealth feedback events the radar
    /// refused (their sounds and EVA do not play) and what else to play.
    pub fn take_radar_events(&mut self, events: &[rts_lockstep_engine::logic::client_event::ClientEvent]) -> RadarFrame {
        let f = self.radar.take_events(&mut self.client.world, events);
        for x in &f.feedback {
            if let RadarFeedback::Message(label) = x {
                self.client.ui_messages.push(label.to_vec());
            }
        }
        f
    }

    /// #35: `LeftHUDInput`'s `GWM_LEFT_DOWN` / `GWM_RIGHT_DOWN` at `pixel` of the radar window: with the radar on and the
    /// pixel on the map, the click at that ground point (`control_bar_callback::left_hud_input_click`); else nothing.
    pub fn radar_click(&mut self, pixel: ICoord2D, right: bool) -> Option<RadarClick> {
        let world = self.radar.local_pixel_to_world(&mut self.client.world, &pixel)?;
        self.client.sync_modes(&self.ui);
        Some(left_hud_input_click(&world, right, &self.commands, &mut self.ui, &mut self.client))
    }

    /// The World.
    pub fn world(&mut self) -> &mut World {
        &mut self.client.world
    }

    /// Before each logic frame: the command translator sees the group messages sent since (as the message stream passes
    /// them through `CommandTranslator` after `SelectionTranslator`).
    pub fn observe_sent(&mut self) {
        // (#51: with the selection's voice for a group made or selected, as `translateGameMessage`'s case plays it)
        let sent: Vec<_> = self.client.sent.drain(..).collect();
        for t in sent {
            self.commands.observe_message_with_voice(t, &self.ui, &mut self.client);
        }
    }

    /// #116: `WindowTranslator` (priority 10) on a raw mouse message at `pixel`, before the game's translators: whether it
    /// goes on to them. The application has already kept the pointer from them over its own windows and the shell, so
    /// the windows here use nothing; what is left is the translator's input gate (with the input off a mouse message is
    /// taken, but a release while a placement is anchored) and its mouse lock (a drag selection keeps everything).
    pub fn window_keeps(&mut self, t: MessageType, pixel: ICoord2D) -> bool {
        let mut h = GameWindows { ui: &self.ui, mouse_locked: self.selection.drag_selecting || self.view_mouse_locked, scrolling: self.ui_scrolling, pixel };
        translate_window_message(&RawMessage::mouse(t, pixel), &mut h) == Disposition::Keep
    }

    /// #116: `MSG_FRAME_TICK` through the selection translator, once a client frame: with the input off its gate ends a
    /// drag selection under way (the other translators' cases of the tick are not translated).
    pub fn frame_tick(&mut self) {
        self.selection.input_disabled_gate(&mut self.ui);
    }

    /// #116: a logic frame's events on the client's input: the scripts' `DISABLE_INPUT` / `ENABLE_INPUT` and deselections
    /// (`rts_lockstep_engine::client::script_input`), on the InGameUI and `mouse`. The build tooltip ~~and the look-at
    /// translator's modes have nothing to reset here (no tooltip in the game; the camera input keeps no scroll mode)~~
    /// has nothing to delete (no tooltip in the game). **Corrected 2026-11-10 (#119):** the answer is whether a
    /// `LookAtResetModes` came, for the camera's `LookAtTranslator::resetModes` ([`crate::camera::Camera::reset_modes`]).
    pub fn take_script_input(&mut self, events: &[rts_lockstep_engine::logic::client_event::ClientEvent], mouse: &mut ClientMouse) -> bool {
        use rts_lockstep_engine::logic::client_event::ScriptInputCall;
        let mut reset = false;
        for e in events {
            if apply_script_input(e, &mut self.ui, mouse, &mut self.client) == Some(ScriptInputCall::LookAtResetModes) {
                reset = true;
            }
        }
        reset
    }

    /// The left button pressed at `pixel`: a building at the cursor is anchored there; with a GUI command pending the press
    /// is used up (#54). Whether the press was taken. #116: after the window translator; the selection translator keeps
    /// the press for its drag.
    pub fn left_press(&mut self, pixel: ICoord2D, ground: Coord3D) -> bool {
        if !self.window_keeps(MSG_RAW_MOUSE_LEFT_BUTTON_DOWN, pixel) {
            return false;
        }
        self.client.view.terrain = vec![(pixel, ground)];
        if self.place.left_button_down(pixel, &mut self.ui, &mut self.client) {
            return true;
        }
        if self.gui.left_button_down(&mut self.ui) == Disposition::Destroy {
            return true;
        }
        self.selection.left_button_down(pixel, &mut self.ui);
        false
    }

    /// The pointer moved to `pixel`: an anchored building stretches toward it and turns to face it. #116: after the window
    /// translator; with the button down the selection translator starts a drag past the click tolerance.
    pub fn pointer_moved(&mut self, pixel: ICoord2D, ground: Coord3D) {
        if !self.window_keeps(MSG_RAW_MOUSE_POSITION, pixel) {
            return;
        }
        self.selection.mouse_position(pixel, CLICK_TOLERANCE, &mut self.ui);
        if self.place.mouse_position(pixel, &mut self.ui) {
            let (start, _) = self.ui.placement_points();
            let start_ground = self.client.view.terrain.iter().find(|(p, _)| *p == start).map_or(ground, |(_, g)| *g);
            self.client.view.terrain.push((pixel, ground));
            self.ui.turn_placement(start_ground, ground);
        }
    }

    /// The left button released at `release`, pressed at `press`: `in_region` are the objects under the click or in the
    /// dragged box, `picked` the one the view picks under the pointer (for an order), `ground` the terrain under the
    /// release. Returns whether a placement, a pending GUI command, a selection or an order took it.
    ///
    /// #116: after the window translator (with the input off the release is taken, but while a placement is anchored),
    /// the selection translator's raw release ends its drag; every order made then passes the command translator's input
    /// gate (`WorldClient`'s), so with the input off none reaches the logic.
    pub fn left_release(&mut self, press: ICoord2D, release: ICoord2D, in_region: &[ObjectId], picked: Option<ObjectId>, ground: Coord3D) -> bool {
        if !self.window_keeps(MSG_RAW_MOUSE_LEFT_BUTTON_UP, release) {
            return false;
        }
        self.selection.left_button_up(&mut self.ui);
        self.client.sync_modes(&self.ui);
        let frame = self.client.world.frame();
        if self.place.left_click(frame, &mut self.ui, &mut self.client) {
            return true;
        }
        // #54: `GUICommandTranslator` (40) sees every `MSG_MOUSE_LEFT_CLICK`, a click or the end of a drag
        self.client.sync_modes(&self.ui);
        if self.gui.left_click(&ground, picked, &mut self.ui, &mut self.client) == Disposition::Destroy {
            return true;
        }
        let is_point = (release.x - press.x).abs() <= CLICK_TOLERANCE && (release.y - press.y).abs() <= CLICK_TOLERANCE;
        self.client.sync_modes(&self.ui);
        if self.selection.left_click(in_region, is_point, &self.commands, &mut self.ui, &mut self.client) == Disposition::Destroy {
            return true;
        }
        self.client.sync_modes(&self.ui);
        self.commands.click(false, is_point, &ground, picked, &mut self.ui, &mut self.client)
    }

    /// The pointer rests over `picked` (or the ground at `ground`): the hint of what a click would order.
    pub fn hover(&mut self, picked: Option<ObjectId>, ground: Coord3D) -> Option<MessageType> {
        self.client.sync_modes(&self.ui);
        self.client.hints.clear();
        // (#116: the selection translator's gate keeps the position without its mouse-over hint while the input is off)
        if self.selection.input_disabled_gate(&mut self.ui).is_some() {
            self.hint = None;
            return None;
        }
        self.commands.mouse_over(picked, &ground, &mut self.ui, &mut self.client);
        self.hint = self.client.hints.last().map(|(t, _)| *t);
        self.hint
    }

    /// The command set of the first selected object (the control bar's context for one selected unit).
    pub fn bar_buttons(&mut self) -> Vec<BarButton> {
        let Some(o) = self.ui.first_selected_drawable() else { return Vec::new() };
        if !self.client.is_locally_controlled(o) {
            return Vec::new();
        }
        let w = &self.client.world;
        let name = w.object_get_command_set_string(o);
        let Some(set) = w.control_bar_find_command_set(&name) else { return Vec::new() };
        (0..rts_lockstep_engine::logic::command_button::MAX_COMMANDS_PER_SET)
            .filter_map(|i| w.command_set_get_command_button(set, i))
            .filter_map(|b| {
                let label = String::from_utf8_lossy(&w.cuts.command_buttons.get(b)?.name).into_owned();
                Some(BarButton { label, button: rts_lockstep_engine::client::command_world::gui_button(w, b)? })
            })
            .collect()
    }

    /// #12: a bar button's text label (`TextLabel`) through the game's text, without its hot key marks (`&`); `None`
    /// when it has none or the game's text lacks it (the button's name is shown then).
    pub fn bar_label(&self, b: &BarButton) -> Option<String> {
        use rts_lockstep_engine::common::science::GameTextFetch;
        let w = &self.client.world;
        let cb = w.statics.command_buttons.find_command_button(b.label.as_bytes())?;
        if cb.labels[0].is_empty() {
            return None;
        }
        let s = String::from_utf16_lossy(w.statics.game_text.fetch(&cb.labels[0]).str()).replace('&', "");
        (!s.is_empty() && !s.starts_with("MISSING")).then_some(s)
    }

    /// A control bar button pressed. #116: its messages pass the command translator's input gate (`WorldClient`'s).
    pub fn press(&mut self, button: &GuiButton) -> CbCommandStatus {
        self.client.sync_modes(&self.ui);
        let selected = self.ui.first_selected_drawable();
        let multi = self.ui.select_count() > 1;
        process_command_ui(button, PressedControl::Command, selected, multi, selected.is_none(), &mut self.ui, &mut self.client)
    }

    /// Whether a click on `picked` would order the selection rather than select it (`EVALUATE_ONLY`).
    pub fn would_order(&mut self, picked: ObjectId) -> bool {
        let at = self.client.object_position(picked);
        self.client.sync_modes(&self.ui);
        self.commands.evaluate_context_command(Some(picked), &at, CommandEvaluateType::EvaluateOnly, &mut self.ui, &mut self.client)
            != rts_lockstep_engine::common::message_types::MSG_INVALID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo_world;
    use rts_lockstep_engine::common::message_types::{MSG_DO_ATTACK_OBJECT, MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT, MSG_DO_MOVETO, MSG_DO_MOVETO_HINT};

    fn pos(c: &mut Controls, o: ObjectId) -> Coord3D {
        c.client.object_position(o)
    }

    /// C2d: the input glue in the demo game, as the application drives it. Predicted, from the translators' own tests:
    /// - a click on the first Runner (in the region and picked) selects it; after the next frame player 1's selection in
    ///   the logic is `[runner]`;
    /// - the hover over the ground at (200, 60) hints a move; a click there sends `MSG_DO_MOVETO`, and after 90 frames the
    ///   Runner is closer to it;
    /// - a drag box over both Soldiers selects them (both newly in the selection, the runner out);
    /// - the hover over an enemy hints an attack after moving; a click on it is an order, not a selection (the enemy is not
    ///   selected) and sends `MSG_DO_ATTACK_OBJECT`;
    /// - the dozer's control bar shows its one button, `Command_BuildDepot`; pressing it starts the Depot's placement, and
    ///   a press then a release on the ground at (150, 200) sends the construct order; a Depot of player 1's stands there a
    ///   frame later.
    #[test]
    fn the_input_glue_follows_the_translators() {
        let (w, o) = demo_world();
        let mut c = Controls::new(w, 1);
        let frame = |c: &mut Controls| {
            c.observe_sent();
            c.world().logic_update().expect("a frame");
        };
        let at = |x: i32, y: i32| ICoord2D { x, y };
        let ground = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
        let selection = |c: &Controls| c.client.world.players.get_nth(1).and_then(|p| p.current_selection.as_ref()).map(|s| s.object_ids.clone()).unwrap_or_default();

        let r = o.runners[0];
        let rp = pos(&mut c, r);
        assert!(c.left_release(at(10, 10), at(10, 10), &[r], Some(r), rp));
        frame(&mut c);
        assert_eq!(selection(&c), vec![r]);
        assert_eq!(c.hover(None, ground(200.0, 60.0)), Some(MSG_DO_MOVETO_HINT));
        assert!(c.left_release(at(50, 10), at(50, 10), &[], None, ground(200.0, 60.0)));
        assert_eq!(c.client.sent, vec![MSG_DO_MOVETO]);
        let d = |c: &mut Controls| {
            let p = pos(c, r);
            ((p.x - 200.0).powi(2) + (p.y - 60.0).powi(2)).sqrt()
        };
        let before = d(&mut c);
        for _ in 0..90 {
            frame(&mut c);
        }
        let after = d(&mut c);
        eprintln!("C2d glue: the Runner {before} -> {after} from the point");
        assert!(after < before);

        assert!(c.left_release(at(0, 0), at(60, 60), &o.soldiers, None, ground(0.0, 0.0)));
        frame(&mut c);
        let mut sel = selection(&c);
        sel.sort();
        let mut want = o.soldiers.clone();
        want.sort();
        assert_eq!(sel, want);

        let e = o.enemies[0];
        let ep = pos(&mut c, e);
        assert_eq!(c.hover(Some(e), ep), Some(MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT));
        assert!(c.would_order(e));
        assert!(c.left_release(at(90, 90), at(90, 90), &[e], Some(e), ep));
        assert_eq!(c.client.sent, vec![MSG_DO_ATTACK_OBJECT]);
        assert!(!c.ui.is_selected(e));
        frame(&mut c);

        let dz = o.dozer;
        let dp = pos(&mut c, dz);
        assert!(c.left_release(at(20, 20), at(20, 20), &[dz], Some(dz), dp));
        frame(&mut c);
        let bar = c.bar_buttons();
        assert_eq!(bar.iter().map(|b| b.label.as_str()).collect::<Vec<_>>(), vec!["Command_BuildDepot"]);
        assert_eq!(c.press(&bar[0].button), CbCommandStatus::Used);
        assert!(c.ui.pending_place_type().is_some());
        assert!(c.left_press(at(150, 200), ground(150.0, 200.0)));
        assert!(c.left_release(at(150, 200), at(150, 200), &[], None, ground(150.0, 200.0)));
        frame(&mut c);
        let depots: Vec<_> = c.client.world.objects.ids_in_order().into_iter().filter(|&x| c.client.world.find_object(x).is_some_and(|y| y.template().get_name() == b"Depot")).collect();
        assert_eq!(depots.len(), 1, "sent {:?}, messages {:?}", c.client.sent, c.client.ui_messages);
        assert_eq!(c.client.world.get_controlling_player(depots[0]), Some(1));
        assert!(c.client.world.cuts.team_live_errors.is_empty(), "{:?}", c.client.world.cuts.team_live_errors);
    }

    /// GitHub #51: a game of voiced soldiers, two of player 1's and one enemy, from the gate's Soldier with `VoiceSelect`,
    /// `VoiceMove`, `VoiceAttack` and a looping `SoundAmbient`. The player clicks one soldier (frame 2), drags a box over
    /// both (frame 5), orders a move (frame 10) and an attack on the enemy (frame 40), over 90 frames, as the application
    /// drives it. With `drawn_client`, the translators' unit voices and the logic's events go to the translated audio
    /// manager and the device, and every frame the voice responses of every message type are also asked of the selection
    /// (their reads); without, the voices are dropped. Returns the CRCs, the sent message types, the events the manager
    /// was given through `ClientAudio` (the voices), the kept file index of the select voice, the files the device read and
    /// each drawable's ambient sound with whether it is still playing.
    #[allow(clippy::type_complexity)]
    fn play_voiced_soldiers(drawn_client: bool) -> (Vec<u32>, Vec<MessageType>, Vec<Vec<u8>>, Option<i32>, Vec<Vec<u8>>, Vec<(Vec<u8>, bool)>) {
        use rts_lockstep_engine::client::unit_voice::{pick_and_play_unit_voice_response, PickAndPlayInfo, VoiceSource};
        use rts_lockstep_engine::common::message_types::*;
        use rts_lockstep_engine::common::thing_template::ThingTemplateAudioType;
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        use rts_lockstep_engine::logic::world::gate_scenario::{gate_map_world_with, make, INI};
        use rts_lockstep_engine::logic::world::team_ops::factory_find_team;
        let start = INI.find("Object Soldier\n").expect("the template");
        let end = start + INI[start..].find("\nEnd\n").expect("its end") + 5;
        let block = INI[start..end].replacen("Object Soldier", "Object VoiceSoldier", 1).replacen(
            "KindOf = VEHICLE",
            "KindOf = VEHICLE SELECTABLE\n VisionRange = 250\n VoiceSelect = SoldierSelect\n VoiceMove = SoldierMove\n VoiceAttack = SoldierAttack\n SoundAmbient = SoldierHum",
            1,
        );
        let mut w = gate_map_world_with(&block);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy_team = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let at = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
        let a = make(&mut w, b"VoiceSoldier", team, at(80.0, 60.0));
        let b = make(&mut w, b"VoiceSoldier", team, at(80.0, 90.0));
        let e = make(&mut w, b"VoiceSoldier", enemy_team, at(360.0, 360.0));
        w.pathfinder_op(rts_lockstep_engine::logic::world::pathfinder_host_world::PfOp::NewMap);
        w.cuts.game_loop.game_mode = rts_lockstep_engine::common::recorder::game_mode::SKIRMISH;
        w.players.set_local_player(Some(1));
        let mut c = Controls::new(w, 1);
        let mut tables = rts_lockstep_engine::client::audio_info::AudioTables::new();
        tables
            .load_text(b"a.ini", b"AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n SoundsExtension = wav\n SampleCount2D = 8\n SampleCount3D = 8\n MinSampleVolume = 1%\n DefaultSoundVolume = 100%\n Default3DSoundVolume = 100%\nEnd\n\
AudioEvent SoldierSelect\n Sounds = sel1 sel2 sel3\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent SoldierMove\n Sounds = move\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent SoldierAttack\n Sounds = attack\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent SoldierHum\n Sounds = hum\n Type = world everyone\n Control = loop\n MinRange = 100\n MaxRange = 2000\n Volume = 100%\nEnd\n")
            .expect("the audio INI");
        let mut audio = rts_lockstep_engine::client::client_audio::ClientAudio::new(rts_lockstep_engine::client::game_audio::AudioManager::new(tables));
        let mut device = crate::audio::ClientDevice::new(crate::audio::NullOutput::default());
        let wav = {
            let mut v = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x22\x56\0\0\x44\xac\0\0\x02\0\x10\0data".to_vec();
            v.extend_from_slice(&(2 * 4410u32).to_le_bytes());
            v.extend(std::iter::repeat_n(0u8, 2 * 4410));
            v
        };
        let files = |p: &[u8]| p.starts_with(b"Data\\Audio\\Sounds\\");
        let view = crate::audio::listener_view([200.0, 200.0], 0.0, 0.95, 300.0);
        let pixel = |x: i32, y: i32| ICoord2D { x, y };
        let all_msgs = [
            MSG_DOCK, MSG_CREATE_SELECTED_GROUP, MSG_EVACUATE, MSG_ENTER, MSG_DO_MOVETO, MSG_DO_SALVAGE, MSG_DO_FORCE_ATTACK_GROUND,
            MSG_SWITCH_WEAPONS, MSG_DO_ATTACK_OBJECT, MSG_DO_WEAPON_AT_OBJECT, MSG_DO_WEAPON_AT_LOCATION, MSG_DO_GUARD_OBJECT,
            MSG_DO_SPECIAL_POWER, MSG_INTERNET_HACK,
        ];
        let (mut crcs, mut sent, mut fetched) = (Vec::new(), Vec::new(), Vec::new());
        for f in 0..90u32 {
            match f {
                2 => assert!(c.left_release(pixel(10, 10), pixel(10, 10), &[a], Some(a), at(80.0, 60.0))),
                5 => assert!(c.left_release(pixel(0, 0), pixel(60, 60), &[a, b], None, at(0.0, 0.0))),
                10 => assert!(c.left_release(pixel(50, 10), pixel(50, 10), &[], None, at(200.0, 60.0))),
                40 => {
                    let ep = c.client.world.find_object(e).map(|o| *o.thing.position()).expect("the enemy");
                    assert!(c.left_release(pixel(90, 90), pixel(90, 90), &[e], Some(e), ep));
                }
                _ => {}
            }
            sent.extend(c.client.sent.iter().copied());
            c.observe_sent();
            if drawn_client {
                // the reads of every voice response, asked of the selection (their voices dropped)
                let list = c.ui.all_selected_drawables().to_vec();
                let info = PickAndPlayInfo { air: true, draw_target: Some(e), weapon_slot: Some(0), special_power_type: 3 };
                let before = c.client.voices.len();
                for m in all_msgs {
                    pick_and_play_unit_voice_response(&list, m, Some(&info), &c.ui, &mut c.client);
                }
                c.client.voices.truncate(before);
            }
            c.world().logic_update().expect("a frame");
            let ev = c.world().take_client_events();
            let voices: Vec<_> = c.client.voices.drain(..).collect();
            if drawn_client {
                let w = &c.client.world;
                let host = rts_lockstep_engine::client::client_audio::WorldAudioHost { world: w, local_player_index: None, files: &files, language: b"English" };
                for v in &voices {
                    audio.play_unit_voice(&host, v);
                }
                for x in &ev {
                    audio.apply_event(&host, &mut device, x);
                }
                audio.update_ambient(&host, &mut device);
                audio.manager.update(&host, &mut device, &view);
                device.pump(|p| {
                    if !fetched.iter().any(|x: &Vec<u8>| x == p) {
                        fetched.push(p.to_vec());
                    }
                    if files(p) { crate::audio::Fetch::Bytes(wav.clone()) } else { crate::audio::Fetch::Missing }
                });
                crate::audio::AudioOutput::advance(&mut device.out, 1.0 / 30.0);
            }
            crcs.push(c.client.world.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        let log: Vec<Vec<u8>> = audio.log.iter().map(|(n, _)| n.clone()).collect();
        let index = audio.voice_indices.get(&(b"VoiceSoldier".to_vec(), VoiceSource::Template(ThingTemplateAudioType::VoiceSelect))).copied();
        // each drawable's ambient sound, and whether its handle is still playing (or requested)
        let ambient = audio
            .ambient
            .drawables
            .values()
            .filter_map(|d| d.sound.as_ref())
            .map(|e| (e.event_name.clone(), audio.manager.is_currently_playing(e.playing_handle)))
            .collect();
        (crcs, sent, log, index, fetched, ambient)
    }

    /// GitHub #51: selecting and ordering voiced units with the audio attached leaves the logic's CRCs as without it.
    ///
    /// *Predictions, written before the first run:* the CRCs equal frame for frame over 90 frames, and the same messages
    /// sent in both runs (a click: `MSG_DESTROY_SELECTED_GROUP`? and `MSG_CREATE_SELECTED_GROUP`; the drag the same; then
    /// `MSG_DO_MOVETO`, `MSG_DO_ATTACK_OBJECT`). With the client, the manager is given, in order: `SoldierHum` three
    /// times (made outside a map load; within the 2000 range), `SoldierSelect` (frame 3, the group made by the click),
    /// `SoldierSelect` again (frame 6), `SoldierMove` (frame 10), `SoldierAttack` (frame 40); the looping hums stay
    /// requested or playing, so `update` restarts none. The select voice's kept index is 1 (the first select played
    /// `sel1`, index 0; the second `sel2`, as `SoldierSelect` is not `RANDOM`: `++m_playingAudioIndex`), so the device
    /// reads `sel1.wav` and `sel2.wav`.
    ///
    /// *Results (2026-11-06), first run:* the CRCs equal over 90 frames; the same six messages sent in both runs
    /// (`MSG_DESTROY_SELECTED_GROUP` and `MSG_CREATE_SELECTED_GROUP` for the click and again for the drag, then
    /// `MSG_DO_MOVETO`, `MSG_DO_ATTACK_OBJECT`); the voices `SoldierSelect`, `SoldierSelect`, `SoldierMove`,
    /// `SoldierAttack`; the select index 1; `sel1.wav`, `sel2.wav`, `move.wav`, `attack.wav` and `hum.wav` read (as
    /// `Sounds\English\\..`: the localized path is tried first and the test's file system has every file). **Miss (in the
    /// test):** the three `SoldierHum` were not in `ClientAudio`'s log, because the ambient sounds are added to the manager
    /// by `drawable_ambient` directly, not through the log; they were played (`hum.wav` read). The assertion now reads
    /// each drawable's ambient sound instead: three `SoldierHum`, still playing (or requested) at the end, so `update`
    /// restarted none.
    #[test]
    fn voiced_units_selected_and_ordered_leave_the_crcs_alone() {
        let (plain, plain_sent, ..) = play_voiced_soldiers(false);
        let (crcs, sent, log, index, started, ambient) = play_voiced_soldiers(true);
        eprintln!("#51 voices: sent {sent:?}; manager given {:?}; started {:?}; select index {index:?}", log.iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect::<Vec<_>>(), started.iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect::<Vec<_>>());
        assert_eq!(crcs, plain);
        assert_eq!(sent, plain_sent);
        let names: Vec<&[u8]> = log.iter().map(|n| n.as_slice()).collect();
        assert_eq!(names, [&b"SoldierSelect"[..], b"SoldierSelect", b"SoldierMove", b"SoldierAttack"]);
        let hum = (b"SoldierHum".to_vec(), true);
        assert_eq!(ambient, [hum.clone(), hum.clone(), hum], "three hums, still playing");
        assert!(started.iter().any(|s| s.ends_with(b"hum.wav")));
        assert_eq!(index, Some(1));
        for f in [&b"sel1.wav"[..], b"sel2.wav"] {
            assert!(started.iter().any(|s| s.ends_with(f)), "{}", String::from_utf8_lossy(f));
        }
    }

    /// One frame's command list as the recorder reads it (`TheCommandList`, `Recorder::writeToFile`): each message's type,
    /// player and arguments.
    type RecordedFrame = Vec<(MessageType, i32, Vec<rts_lockstep_engine::common::message_stream::GameMessageArgument>)>;

    /// The command list the logic is about to run (`Recorder::updateRecord` reads the same list).
    fn record_command_list(w: &World) -> RecordedFrame {
        use rts_lockstep_engine::common::message_stream::Messages;
        let m = &w.cuts.game_loop.messages;
        m.list_messages(Messages::COMMAND_LIST)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| m.get(id))
            .map(|g| (g.get_type(), g.get_player_index(), g.arguments().to_vec()))
            .collect()
    }

    /// `getAIStateType()` of `obj`'s AI.
    fn ai_state(w: &mut World, obj: ObjectId) -> Option<i32> {
        use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleReply};
        let ai = w.find_object(obj)?.ai?;
        match w.module_call(obj, ai, ModuleCall::AiGetAiStateType) {
            ModuleReply::Int(i) => Some(i),
            _ => None,
        }
    }

    /// The Hut's rally point (`getRallyPoint()` of its exit).
    fn rally_point(w: &mut World, obj: ObjectId) -> Option<Coord3D> {
        use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleReply};
        let exit = w.get_object_exit_interface(obj)?;
        match w.module_call(obj, exit, ModuleCall::GetRallyPoint) {
            ModuleReply::Coord(c) => Some(c),
            _ => None,
        }
    }

    /// What [`play_gui_commands`] saw.
    struct GuiRun {
        crcs: Vec<u32>,
        recorded: Vec<RecordedFrame>,
        /// The orders sent at each step: the guard, the attack move, the rally point.
        sent: Vec<Vec<MessageType>>,
        /// The Soldier's AI state 10 frames after the guard order.
        guard_state: Option<i32>,
        /// The Soldier's distance from the attack move's point when it was ordered and 90 frames later.
        attack_move: (f32, f32),
        rally: Option<Coord3D>,
        voices: Vec<Vec<u8>>,
        fetched: Vec<Vec<u8>>,
    }

    /// #54: the demo game, as the application drives it: the player clicks a Soldier (frame 2), presses its Guard button
    /// and clicks the ground at (140, 100) (frame 5), presses Attack move and clicks (150, 170) (frame 40), then clicks the
    /// Hut (frame 140), presses its rally point button and clicks (90, 230) (frame 145), over 160 frames. The unit voices
    /// go to the translated audio manager and the device, with a test audio INI for the Soldier's two voices. Each frame's
    /// command list is recorded before the logic runs it.
    fn play_gui_commands() -> GuiRun {
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        let (w, o) = demo_world();
        let mut c = Controls::new(w, 1);
        let mut tables = rts_lockstep_engine::client::audio_info::AudioTables::new();
        tables
            .load_text(b"a.ini", b"AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n SoundsExtension = wav\n SampleCount2D = 8\n SampleCount3D = 8\n MinSampleVolume = 1%\n DefaultSoundVolume = 100%\n Default3DSoundVolume = 100%\nEnd\n\
AudioEvent DemoSoldierGuard\n Sounds = guard\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent DemoSoldierMove\n Sounds = move\n Type = ui everyone\n Volume = 100%\nEnd\n")
            .expect("the audio INI");
        let mut audio = rts_lockstep_engine::client::client_audio::ClientAudio::new(rts_lockstep_engine::client::game_audio::AudioManager::new(tables));
        let mut device = crate::audio::ClientDevice::new(crate::audio::NullOutput::default());
        let wav = {
            let mut v = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x22\x56\0\0\x44\xac\0\0\x02\0\x10\0data".to_vec();
            v.extend_from_slice(&(2 * 4410u32).to_le_bytes());
            v.extend(std::iter::repeat_n(0u8, 2 * 4410));
            v
        };
        let files = |p: &[u8]| p.starts_with(b"Data\\Audio\\Sounds\\");
        let view = crate::audio::listener_view([100.0, 100.0], 0.0, 0.95, 300.0);
        let pixel = |x: i32, y: i32| ICoord2D { x, y };
        let ground = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
        let s = o.soldiers[0];
        let press = |c: &mut Controls, name: &str| {
            let bar = c.bar_buttons();
            let b = bar.iter().find(|b| b.label == name).unwrap_or_else(|| panic!("{name} in {bar:?}")).button.clone();
            assert_eq!(c.press(&b), CbCommandStatus::Used);
            assert!(c.ui.gui_command().is_some(), "{name} waits for its target");
        };
        // a click on the ground: the press and the release, both taken by the pending command
        let order = |c: &mut Controls, at: Coord3D| {
            c.client.sent.clear();
            assert!(c.left_press(pixel(300, 300), at), "the press is used up");
            assert!(c.left_release(pixel(300, 300), pixel(300, 300), &[], None, at), "the click is used up");
            assert!(c.ui.gui_command().is_none(), "the command ended");
            c.client.sent.clone()
        };
        let mut run = GuiRun { crcs: Vec::new(), recorded: Vec::new(), sent: Vec::new(), guard_state: None, attack_move: (0.0, 0.0), rally: None, voices: Vec::new(), fetched: Vec::new() };
        let attack_at = ground(150.0, 170.0);
        let dist = |c: &mut Controls| {
            let p = c.client.object_position(s);
            let (dx, dy) = (p.x - attack_at.x, p.y - attack_at.y);
            rts_lockstep_engine::ww::wwmath::sqrt(dx * dx + dy * dy)
        };
        for f in 0..160u32 {
            match f {
                2 => assert!(c.left_release(pixel(10, 10), pixel(10, 10), &[s], Some(s), c.client.object_position(s))),
                5 => {
                    press(&mut c, "Command_DemoGuard");
                    run.sent.push(order(&mut c, ground(140.0, 100.0)));
                }
                40 => {
                    press(&mut c, "Command_DemoAttackMove");
                    run.sent.push(order(&mut c, attack_at));
                    run.attack_move.0 = dist(&mut c);
                }
                130 => run.attack_move.1 = dist(&mut c),
                140 => {
                    let hp = c.client.object_position(o.hut);
                    assert!(c.left_release(pixel(20, 20), pixel(20, 20), &[o.hut], Some(o.hut), hp));
                }
                145 => {
                    press(&mut c, "Command_DemoRally");
                    run.sent.push(order(&mut c, ground(90.0, 230.0)));
                }
                _ => {}
            }
            c.observe_sent();
            run.recorded.push(record_command_list(&c.client.world));
            c.world().logic_update().expect("a frame");
            if f == 15 {
                run.guard_state = ai_state(c.world(), s);
            }
            if f == 146 {
                run.rally = rally_point(c.world(), o.hut);
            }
            let ev = c.world().take_client_events();
            let voices: Vec<_> = c.client.voices.drain(..).collect();
            let w = &c.client.world;
            let host = rts_lockstep_engine::client::client_audio::WorldAudioHost { world: w, local_player_index: None, files: &files, language: b"English" };
            for v in &voices {
                run.voices.push(v.name.clone());
                audio.play_unit_voice(&host, v);
            }
            for x in &ev {
                audio.apply_event(&host, &mut device, x);
            }
            audio.manager.update(&host, &mut device, &view);
            device.pump(|p| {
                if !run.fetched.iter().any(|x: &Vec<u8>| x == p) {
                    run.fetched.push(p.to_vec());
                }
                if files(p) { crate::audio::Fetch::Bytes(wav.clone()) } else { crate::audio::Fetch::Missing }
            });
            crate::audio::AudioOutput::advance(&mut device.out, 1.0 / 30.0);
            run.crcs.push(c.client.world.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        assert!(c.client.world.cuts.team_live_errors.is_empty(), "{:?}", c.client.world.cuts.team_live_errors);
        run
    }

    /// The recorded command lists played into a fresh demo game with no client (as a replay's playback appends them),
    /// without the messages of type `drop`; the CRCs, with the same AI state and rally point reads as the recorded run.
    fn play_back(recorded: &[RecordedFrame], drop: Option<MessageType>) -> Vec<u32> {
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        let (mut w, o) = demo_world();
        let mut crcs = Vec::new();
        for (f, frame) in recorded.iter().enumerate() {
            for (t, player, args) in frame.iter().filter(|m| Some(m.0) != drop) {
                w.push_command(*t, *player, |m| {
                    for a in args {
                        m.append_argument(*a);
                    }
                });
            }
            w.logic_update().expect("a played frame");
            if f == 15 {
                ai_state(&mut w, o.soldiers[0]);
            }
            if f == 146 {
                rally_point(&mut w, o.hut);
            }
            crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        crcs
    }

    /// GitHub #54: a pending control bar command is carried out by the next click (`GUICommandTranslator`), in the demo
    /// game as the application drives it, and the input path adds nothing to the logic but the orders.
    ///
    /// *Predictions, written before the first run* (`issue54_predictions.md`): the guard click sends exactly
    /// `[MSG_DO_GUARD_POSITION]` (the ground has no object, so not `MSG_DO_GUARD_OBJECT`) and 10 frames later the Soldier's
    /// AI is in `AI_GUARD` (16); the attack move click sends `[MSG_DO_ATTACKMOVETO]` and 90 frames later the Soldier is
    /// closer to its point; the rally click sends `[MSG_SET_RALLY_POINT]` and a frame later the Hut's exit has its rally
    /// point at (90, 230). The voices `DemoSoldierGuard` and `DemoSoldierMove` are played (the device reads `guard.wav` and
    /// `move.wav`; the Hut has no voice). The recorded command lists played back into a fresh demo game with no client give
    /// the same CRC every frame; played back without the guard order they differ from frame 6 on.
    ///
    /// *Results (2026-11-06):* the orders as predicted (`MSG_DO_GUARD_POSITION`, `MSG_DO_ATTACKMOVETO`,
    /// `MSG_SET_RALLY_POINT`, one each); the guard state 16; the attack move took the Soldier from 91.1 to 6.3 from its
    /// point; the voices `DemoSoldierGuard` and `DemoSoldierMove` played (`guard.wav`, `move.wav` read; the two selections
    /// asked for the empty select voice); the playback's CRCs equal over 160 frames. **Misses:** (1) on the first run the
    /// rally point was not set: the demo had no `BasicHumanLocomotor`, the one locomotor `doSetRallyPoint` asks for a path
    /// with, so the logic refused it, as upstream does; the demo now has one. (2) Without the guard order the CRCs differ
    /// from index 5, not 6: index 5 is the CRC after frame 5's logic update, which ran the order (my off-by-one).
    #[test]
    fn a_pending_gui_command_is_ordered_by_the_click_and_adds_nothing_else() {
        use rts_lockstep_engine::common::message_types::{MSG_DO_ATTACKMOVETO, MSG_DO_GUARD_POSITION, MSG_SET_RALLY_POINT};
        use rts_lockstep_engine::logic::ai::ai_update_types::AI_GUARD;
        let run = play_gui_commands();
        let played = play_back(&run.recorded, None);
        let unguarded = play_back(&run.recorded, Some(MSG_DO_GUARD_POSITION));
        let first_diff = unguarded.iter().zip(&run.crcs).position(|(a, b)| a != b);
        eprintln!(
            "#54: sent {:?}; guard state {:?}; attack move {:?}; rally {:?}; voices {:?}; fetched {:?}; without the guard the CRCs first differ at frame {first_diff:?}",
            run.sent,
            run.guard_state,
            run.attack_move,
            run.rally,
            run.voices.iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect::<Vec<_>>(),
            run.fetched.iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect::<Vec<_>>()
        );
        assert_eq!(run.sent, vec![vec![MSG_DO_GUARD_POSITION], vec![MSG_DO_ATTACKMOVETO], vec![MSG_SET_RALLY_POINT]]);
        assert_eq!(run.guard_state, Some(AI_GUARD as i32));
        assert!(run.attack_move.1 < run.attack_move.0, "{:?}", run.attack_move);
        let r = run.rally.expect("a rally point");
        assert_eq!((r.x, r.y), (90.0, 230.0));
        assert!(run.voices.iter().any(|v| v == b"DemoSoldierGuard"));
        assert!(run.voices.iter().any(|v| v == b"DemoSoldierMove"));
        for f in [&b"guard.wav"[..], b"move.wav"] {
            assert!(run.fetched.iter().any(|s| s.ends_with(f)), "{}", String::from_utf8_lossy(f));
        }
        assert_eq!(played, run.crcs, "the playback of the recorded orders gives the same CRCs");
        assert!(first_diff.is_some(), "without the guard order the game differs");
    }

    /// The demo's Hut selected, its rally point button pressed and the ground at `at` clicked, then one logic frame: the
    /// frame's events and the client's logic messages after it.
    fn rally_from_the_demo(c: &mut Controls, hut: ObjectId, at: Coord3D) -> Vec<rts_lockstep_engine::logic::client_event::ClientEvent> {
        let px = ICoord2D { x: 20, y: 20 };
        c.observe_sent();
        c.world().logic_update().expect("a frame");
        c.world().take_client_events();
        let hp = c.client.object_position(hut);
        assert!(c.left_release(px, px, &[hut], Some(hut), hp));
        c.observe_sent();
        c.world().logic_update().expect("a frame");
        c.world().take_client_events();
        let bar = c.bar_buttons();
        let b = bar.iter().find(|b| b.label == "Command_DemoRally").expect("the rally button").button.clone();
        assert_eq!(c.press(&b), CbCommandStatus::Used);
        assert!(c.left_press(ICoord2D { x: 300, y: 300 }, at));
        assert!(c.left_release(ICoord2D { x: 300, y: 300 }, ICoord2D { x: 300, y: 300 }, &[], None, at));
        c.observe_sent();
        c.world().logic_update().expect("a frame");
        let ev = c.world().take_client_events();
        c.client.take_logic_messages(&ev);
        ev
    }

    /// GitHub #56: the demo's game start sets the logic's local player (`PlayerList::newGame`'s `setLocalPlayer` of the
    /// first human side), so the logic's feedback to the local player reaches the client.
    ///
    /// *Predictions, written before the first run:* after `demo_world` the World's local player is 1 (it was `None`:
    /// `PlayerArena::new` leaves it unset and the demo never set it). A rally point set on the Hut at (90, 230) gives, in that
    /// frame's events, the dispatcher's `UiMessage { "GUI:RallyPointSet" }` and `Audio { "RallyPointSet", player 1 }`
    /// (`doSetRallyPoint`, `isLocallyControlled`), and the client's message list gets `GUI:RallyPointSet`. The control: the
    /// same game with the neutral player local (`setLocalPlayer(NULL)`) gets the rally point but neither message. The local
    /// player is not in the CRC (`PlayerList::crc` and `Player::crc` do not write it): both games' CRCs are equal after the
    /// frame.
    #[test]
    fn the_demo_sets_the_local_player_and_a_rally_point_gives_the_local_message() {
        use rts_lockstep_engine::logic::client_event::ClientEvent;
        use rts_lockstep_engine::logic::world::game_logic_dispatch::ClientCall;
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        use rts_lockstep_engine::logic::world::player_list::player_list_set_local_player;
        let at = Coord3D { x: 90.0, y: 230.0, z: 0.0 };
        let (w, o) = demo_world();
        assert_eq!(w.players.local_index(), Some(crate::demo::DEMO_LOCAL_PLAYER));
        let mut c = Controls::new(w, crate::demo::DEMO_LOCAL_PLAYER);
        let ev = rally_from_the_demo(&mut c, o.hut, at);
        let dispatched: Vec<_> = ev.iter().filter_map(|e| if let ClientEvent::Dispatch(d) = e { Some(d.clone()) } else { None }).collect();
        eprintln!("#56: dispatched {dispatched:?}; messages {:?}", c.client.ui_messages.iter().map(|m| String::from_utf8_lossy(m).into_owned()).collect::<Vec<_>>());
        assert!(dispatched.contains(&ClientCall::UiMessage { label: "GUI:RallyPointSet" }), "{dispatched:?}");
        assert!(dispatched.iter().any(|d| matches!(d, ClientCall::Audio { name: "RallyPointSet", player: Some(1), .. })), "{dispatched:?}");
        assert!(c.client.ui_messages.iter().any(|m| m == b"GUI:RallyPointSet"));
        let crc = c.world().logic_crc(CRC_RECALC, None).expect("the CRC");

        // the control: the neutral player local
        let (mut w, o) = demo_world();
        player_list_set_local_player(&mut w, None).expect("the neutral player");
        let mut n = Controls::new(w, crate::demo::DEMO_LOCAL_PLAYER);
        let ev = rally_from_the_demo(&mut n, o.hut, at);
        assert!(!ev.iter().any(|e| matches!(e, ClientEvent::Dispatch(ClientCall::UiMessage { .. }))), "{ev:?}");
        assert!(n.client.ui_messages.is_empty(), "{:?}", n.client.ui_messages);
        assert_eq!(n.world().logic_crc(CRC_RECALC, None).expect("the CRC"), crc, "the local player is not in the CRC");
    }

    /// #35: the radar in the demo game. Predicted before the run: after the first frame the radar lists the local player's
    /// seven objects (two Runners, two Soldiers, the dozer, the Caster, the Hut) and the enemy's two Soldiers; the demo
    /// player has no radar building, so the radar is off and a click on it does nothing; forced on (a script's
    /// `RADAR_FORCE_ENABLE`), a left click in the middle of a 200 x 150 window with nothing selected looks at the map's
    /// middle (no message sent); with a Runner selected it sends `MSG_DO_MOVETO` to that point; a right click looks; the
    /// radar draws a 200 x 150 image; and the logic's CRC after 30 frames equals that of a game without the radar's
    /// questions but with the same move order.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn the_radar_shows_the_demo_and_its_clicks_look_and_move() {
        use rts_lockstep_engine::common::message_types::MSG_DO_MOVETO;
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        let (w, o) = demo_world();
        let mut c = Controls::new(w, crate::demo::DEMO_LOCAL_PLAYER);
        c.radar.radar.set_radar_window(Some((ICoord2D { x: 0, y: 0 }, ICoord2D { x: 200, y: 150 })));
        c.world().logic_update().expect("a frame");
        let ev = c.world().take_client_events();
        c.take_radar_events(&ev);
        assert_eq!((c.radar.radar.local_object_list.len(), c.radar.radar.object_list.len()), (7, 2));
        let middle = ICoord2D { x: 100, y: 75 };
        assert_eq!(c.radar_click(middle, false), None);
        c.radar.radar.force_on(true);
        let Some(RadarClick::LookAt(at)) = c.radar_click(middle, false) else { panic!("a look") };
        let ext = c.client.world.terrain.extent().expect("the map");
        assert!((at.x - ext.width() / 2.0).abs() < 8.0 && (at.y - ext.height() / 2.0).abs() < 8.0, "{at:?}");
        assert!(c.client.sent.is_empty());
        c.ui.select_drawable(o.runners[0], &mut c.client);
        assert!(matches!(c.radar_click(middle, true), Some(RadarClick::LookAt(_))));
        assert_eq!(c.radar_click(middle, false), Some(RadarClick::Ordered));
        assert_eq!(c.client.sent, vec![MSG_DO_MOVETO]);
        let img = c.radar.draw(&mut c.client.world, None, ICoord2D { x: 200, y: 150 }, None).expect("forced on");
        assert_eq!((img.width, img.height), (200, 150));
        for _ in 0..30 {
            c.world().logic_update().expect("a frame");
            let ev = c.world().take_client_events();
            c.take_radar_events(&ev);
            let _ = c.radar.draw(&mut c.client.world, None, ICoord2D { x: 200, y: 150 }, None);
        }
        let crc = c.world().logic_crc(CRC_RECALC, None).expect("the CRC");
        // the control: the same order without the radar's questions
        let (w, o) = demo_world();
        let mut p = Controls::new(w, crate::demo::DEMO_LOCAL_PLAYER);
        p.world().logic_update().expect("a frame");
        p.ui.select_drawable(o.runners[0], &mut p.client);
        rts_lockstep_engine::client::command_xlat::CommandXlatHost::append_command(&mut p.client, MSG_DO_MOVETO, &[rts_lockstep_engine::client::command_xlat::CmdArg::Location(at)]);
        for _ in 0..30 {
            p.world().logic_update().expect("a frame");
        }
        assert_eq!(p.world().logic_crc(CRC_RECALC, None).expect("the CRC"), crc);
    }

    /// #116: a script of the demo world's side 0 (added when the map has none) that runs `action` once, at the script engine's next evaluation
    /// (`CONDITION_TRUE`, one-shot).
    fn add_script(w: &mut World, name: &[u8], action: i32) {
        use rts_lockstep_engine::logic::scripts::{condition_type, Condition, OrCondition, Script, ScriptAction, ScriptList};
        let templates = rts_lockstep_engine::logic::script_templates::script_templates(&mut w.names);
        let mut s = Script::new();
        s.script_name = name.to_vec();
        s.conditions = vec![OrCondition { conditions: vec![Condition::new(condition_type::CONDITION_TRUE, &templates)] }];
        s.actions = vec![ScriptAction::new(action, &templates)];
        // (the demo map has no sides: one is added, with no properties, to hold the script)
        if w.sides.get_num_sides() == 0 {
            w.sides.add_side(&rts_lockstep_engine::common::dict::Dict::new());
        }
        w.sides.get_side_info_mut(0).expect("side 0").scripts.get_or_insert_with(ScriptList::default).scripts.push(s);
    }

    /// #116: one client frame as the application runs it: the sent messages observed, the logic frame, its script
    /// input events on the InGameUI and the mouse, then the selection translator's tick. Returns the events.
    fn client_frame(c: &mut Controls, mouse: &mut ClientMouse) -> Vec<rts_lockstep_engine::logic::client_event::ClientEvent> {
        c.observe_sent();
        c.world().logic_update().expect("a frame");
        let events = c.world().take_client_events();
        c.take_script_input(&events, mouse);
        c.frame_tick();
        events
    }

    /// #116: a script's `DISABLE_INPUT` in the World blocks the player's click and ends a drag; `ENABLE_INPUT` gives the
    /// input back.
    ///
    /// *Predictions, written before the first run:*
    /// - a click on the first Runner selects it (the logic's selection `[runner]` a frame later);
    /// - a drag started (press at (0, 0), moved to (60, 60)) is under way: `m_dragSelecting` and `isSelecting`;
    /// - the frame that runs `DISABLE_INPUT` sends the client, in order, `SetInputEnabled(false)`, `MouseSetVisibility(false)`,
    ///   `DeselectAllDrawables`, `ClearAttackMoveToMode`, `SetWaypointMode(false)`, `DeleteBuildTooltipLayout`,
    ///   `LookAtResetModes`; after it the input is off, the mouse hidden, nothing selected in the client, the drag ended,
    ///   and the only message sent is `MSG_DESTROY_SELECTED_GROUP` (a system message); a frame later the logic's
    ///   selection is empty;
    /// - the drag's release over the Soldiers is taken by the window translator (`false`, nothing sent), and so is a
    ///   click on a Soldier; a hover makes no hint; over 30 frames the logic's selection stays empty;
    /// - with the input off, a message put on the stream directly passes only if it is `MSG_META_OPTIONS` or
    ///   `MSG_CLEAR_GAME_DATA` (not `MSG_DO_MOVETO` or `MSG_CREATE_SELECTED_GROUP`);
    /// - after `ENABLE_INPUT` (its two calls) the input is on and the mouse shown, and a click on a Soldier selects it.
    ///
    /// *Results (2026-11-10):* all held (after the test's own fix: the demo map had no side for the script).
    #[test]
    fn a_scripts_disable_input_blocks_the_players_click() {
        use rts_lockstep_engine::client::in_game_ui::SelectionUiHost;
        use rts_lockstep_engine::common::message_types::*;
        use rts_lockstep_engine::logic::client_event::{ClientEvent, ScriptInputCall as C};
        use rts_lockstep_engine::logic::scripts::action_type::{DISABLE_INPUT, ENABLE_INPUT};
        let (w, o) = demo_world();
        let mut c = Controls::new(w, 1);
        let mut mouse = ClientMouse::default();
        let at = |x: i32, y: i32| ICoord2D { x, y };
        let selection = |c: &Controls| c.client.world.players.get_nth(1).and_then(|p| p.current_selection.as_ref()).map(|s| s.object_ids.clone()).unwrap_or_default();
        let inputs = |ev: &[ClientEvent]| ev.iter().filter_map(|e| if let ClientEvent::ScriptInput(i) = e { Some(*i) } else { None }).collect::<Vec<_>>();

        let r = o.runners[0];
        let rp = pos(&mut c, r);
        assert!(c.left_release(at(10, 10), at(10, 10), &[r], Some(r), rp));
        client_frame(&mut c, &mut mouse);
        assert_eq!(selection(&c), vec![r]);

        assert!(!c.left_press(at(0, 0), Coord3D::default()));
        c.pointer_moved(at(60, 60), Coord3D::default());
        assert!(c.selection.drag_selecting && c.ui.is_selecting());

        add_script(c.world(), b"NoInput", DISABLE_INPUT);
        let ev = client_frame(&mut c, &mut mouse);
        assert_eq!(
            inputs(&ev),
            [C::SetInputEnabled(false), C::MouseSetVisibility(false), C::DeselectAllDrawables, C::ClearAttackMoveToMode, C::SetWaypointMode(false), C::DeleteBuildTooltipLayout, C::LookAtResetModes]
        );
        assert!(!c.ui.input_enabled() && !mouse.visible && c.ui.select_count() == 0);
        // (#119) the LookAtResetModes call is handed on for the camera's translator
        assert!(c.take_script_input(&[ClientEvent::ScriptInput(C::LookAtResetModes)], &mut mouse));
        assert!(!c.take_script_input(&[], &mut mouse));
        assert!(!c.selection.drag_selecting && !c.ui.is_selecting());
        assert_eq!(c.client.sent, vec![MSG_DESTROY_SELECTED_GROUP]);
        client_frame(&mut c, &mut mouse);
        assert_eq!(selection(&c), Vec::<ObjectId>::new());

        assert!(!c.left_release(at(0, 0), at(60, 60), &o.soldiers, None, Coord3D::default()));
        let s0 = o.soldiers[0];
        let sp = pos(&mut c, s0);
        assert!(!c.left_press(at(30, 30), sp));
        assert!(!c.left_release(at(30, 30), at(30, 30), &[s0], Some(s0), sp));
        assert_eq!(c.hover(Some(s0), sp), None);
        assert!(c.client.sent.is_empty());
        for _ in 0..30 {
            client_frame(&mut c, &mut mouse);
        }
        assert_eq!(selection(&c), Vec::<ObjectId>::new());
        assert!(c.client.world.cuts.team_live_errors.is_empty(), "{:?}", c.client.world.cuts.team_live_errors);

        add_script(c.world(), b"Input", ENABLE_INPUT);
        let ev = client_frame(&mut c, &mut mouse);
        assert_eq!(inputs(&ev), [C::SetInputEnabled(true), C::MouseSetVisibility(true)]);
        assert!(c.ui.input_enabled() && mouse.visible);
        assert!(c.left_release(at(30, 30), at(30, 30), &[s0], Some(s0), sp));
        client_frame(&mut c, &mut mouse);
        assert_eq!(selection(&c), vec![s0]);

        // (last: a `MSG_CLEAR_GAME_DATA` on the command list would end the game at the next frame)
        c.ui.set_input_enabled(false);
        c.client.sync_modes(&c.ui);
        c.client.sent.clear();
        for t in [MSG_DO_MOVETO, MSG_META_OPTIONS, MSG_CREATE_SELECTED_GROUP, MSG_CLEAR_GAME_DATA] {
            c.client.append_message_bool(t, true);
        }
        assert_eq!(c.client.sent, vec![MSG_META_OPTIONS, MSG_CLEAR_GAME_DATA]);
    }

    /// #116: the client attached to a game whose script turns the input off leaves the logic's CRCs as without it.
    ///
    /// *Predictions, written before the first run:* over 300 frames of the demo world with `DISABLE_INPUT` added at frame
    /// 20 and `ENABLE_INPUT` at frame 280, the World alone and the World with the client (its script input events taken,
    /// its tick, and between frames 30 and 270 a click on a Runner, a drag over the Soldiers, a hover and a press of the
    /// dozer's bar button every 10 frames) have equal CRCs frame for frame. The client sends exactly one message, the
    /// deselection's `MSG_DESTROY_SELECTED_GROUP` (nothing was selected, so the logic changes nothing it CRCs).
    ///
    /// *Results (2026-11-10):* held: 300 equal CRCs (the last `0862d2e3` in both), the one message sent
    /// `MSG_DESTROY_SELECTED_GROUP` (1003). The first run of this test and of the one above failed in the test's own
    /// setup, not the gates: the demo map has no sides, so the script had nowhere to go (`add_script` now adds one).
    #[test]
    fn the_crcs_are_unchanged_with_the_input_gates_attached() {
        use rts_lockstep_engine::logic::scripts::action_type::{DISABLE_INPUT, ENABLE_INPUT};
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        let run = |attached: bool| -> (Vec<u32>, Vec<MessageType>) {
            let (w, o) = demo_world();
            let mut c = Controls::new(w, 1);
            let mut mouse = ClientMouse::default();
            let at = |x: i32, y: i32| ICoord2D { x, y };
            let mut crcs = Vec::new();
            let mut sent = Vec::new();
            // (the dozer's construct button, pressed with nothing selected)
            let button = {
                let w = &c.client.world;
                let name = w.object_get_command_set_string(o.dozer);
                w.control_bar_find_command_set(&name).and_then(|set| {
                    (0..rts_lockstep_engine::logic::command_button::MAX_COMMANDS_PER_SET)
                        .find_map(|i| w.command_set_get_command_button(set, i))
                        .and_then(|b| rts_lockstep_engine::client::command_world::gui_button(w, b))
                })
            };
            assert!(button.is_some(), "the dozer's button");
            for f in 0..300u32 {
                match f {
                    20 => add_script(c.world(), b"NoInput", DISABLE_INPUT),
                    280 => add_script(c.world(), b"Input", ENABLE_INPUT),
                    _ => {}
                }
                if attached && (30..270).contains(&f) && f % 10 == 0 {
                    let r = o.runners[0];
                    let rp = pos(&mut c, r);
                    c.left_press(at(10, 10), rp);
                    c.left_release(at(10, 10), at(10, 10), &[r], Some(r), rp);
                    c.left_press(at(0, 0), Coord3D::default());
                    c.pointer_moved(at(60, 60), Coord3D::default());
                    c.left_release(at(0, 0), at(60, 60), &o.soldiers, None, Coord3D::default());
                    c.hover(Some(r), rp);
                    if let Some(b) = &button {
                        c.press(b);
                    }
                }
                sent.extend(c.client.sent.iter().copied());
                if attached {
                    client_frame(&mut c, &mut mouse);
                } else {
                    c.world().logic_update().expect("a frame");
                    c.world().take_client_events();
                }
                crcs.push(c.client.world.logic_crc(CRC_RECALC, None).expect("the CRC"));
            }
            (crcs, sent)
        };
        let (alone, none) = run(false);
        let (with_client, sent) = run(true);
        eprintln!("#116 CRC: last {:08x} / {:08x}; sent with the client {sent:?}", alone[299], with_client[299]);
        assert!(none.is_empty());
        assert_eq!(sent, vec![rts_lockstep_engine::common::message_types::MSG_DESTROY_SELECTED_GROUP]);
        assert_eq!(alone, with_client);
    }
}
