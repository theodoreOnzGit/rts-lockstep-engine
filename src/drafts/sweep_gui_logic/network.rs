//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/Network.cpp (class Network: the constructor, deinit, reset, init,
//!   setSawCRCMismatch, parseUserList, startGame, setLocalAddress, initTransport, attachTransport, isTransferCommand,
//!   GetCommandsFromCommandList, getExecutionFrame, processCommand, AllCommandsReady, RelayCommandsToCommandList,
//!   processFrameSynchronizedNetCommand, processRunAheadCommand, processDestroyPlayerCommand, update, liteupdate,
//!   endOfGameCheck, timeForNewFrame, isFrameDataReady, the statistics getters, sendChat, sendDisconnectChat, sendFile,
//!   sendFileAnnounce, getFileTransferProgress, areAllQueuesEmpty, quitGame, selfDestructPlayer, isPacketRouter,
//!   voteForPlayerDisconnect, updateLoadProgress, loadProgressComplete, sendTimeOutGameStart, getLocalPlayerID,
//!   getPlayerName, getNumPlayers, getSlotAverageFPS, toggleNetworkOn, notifyOthersOfCurrentFrame,
//!   notifyOthersOfNewFrame, getPingFrame, getPingsSent, getPingsRecieved, isPlayerConnected)
//!
//! `Network` owns the `ConnectionManager` (`crate::drafts::network::connection_manager`, first-pass draft) and reaches
//! the game through [`NetworkHost`]: `TheGameLogic`, `TheCommandList`, `TheMessageStream`, `TheWindowManager`, the
//! script engine, the recorder, `TheGlobalData` and the clock. The debug-only `m_networkOn` and `toggleNetworkOn`
//! (`_DEBUG`/`_INTERNAL`) are drafted, behind the `network_on` field, because `update` and `liteupdate` read it; a
//! release build has no such field and always runs (`network_on` stays true).
//!
//! `parseUserList(GameInfo*)` is drafted as taking the slot list the connection manager's draft `parse_user_list`
//! takes, since `GameInfo` is the host's.
//!
//! Not translated: `QueryPerformanceCounter` / `QueryPerformanceFrequency` (a Windows timer; the host gives ticks and
//! the frequency), `DEBUG_CRC` dumps (compiled out).

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::message_stream::{GameMessageArgument, MessageType};
use crate::common::message_types::{
    MSG_BEGIN_NETWORK_MESSAGES, MSG_CLEAR_GAME_DATA, MSG_END_NETWORK_MESSAGES, MSG_SELF_DESTRUCT,
};
use crate::common::unicode_string::UnicodeString;
use crate::drafts::network::connection_manager::{ConnectionManager, ConnectionManagerHost, ParseSlot};
use crate::drafts::network::net_command_msg::{
    NetCommandBody, NetCommandMsgPtr, NetCommandType, NetLocalStatus, NetworkFrameLimits, PlayerLeaveCode, MAX_SLOTS,
};

/// `NET_CRC_INTERVAL` (Network.cpp:62), without `DEBUG_CRC`.
pub const NET_CRC_INTERVAL: i32 = 100;
/// `RESEND_INTERVAL`.
pub const RESEND_INTERVAL: i32 = 1;

/// What the network asks of the game.
pub trait NetworkHost {
    /// A message of `TheCommandList`.
    type Msg: Copy;
    /// `TheGameLogic->getFrame()`.
    fn game_logic_frame(&self) -> u32;
    /// `TheCommandList->getFirstMessage()`.
    fn command_list_first(&self) -> Option<Self::Msg>;
    /// `msg->next()`.
    fn command_list_next(&self, msg: Self::Msg) -> Option<Self::Msg>;
    /// `msg->getType()`'s arguments, in order (what `sendLocalGameMessage` wraps).
    fn message_arguments(&self, msg: Self::Msg) -> Vec<GameMessageArgument>;
    /// `msg->getType()`.
    fn message_type(&self, msg: Self::Msg) -> MessageType;
    /// `TheCommandList->removeMessage(msg); msg->deleteInstance();`.
    fn command_list_remove(&mut self, msg: Self::Msg);
    /// `TheCommandList->appendMessage(((NetGameCommandMsg *)msg->getCommand())->constructGameMessage())`.
    fn command_list_append_game_command(&mut self, command: &NetCommandMsgPtr);
    /// `TheCommandList->appendMessage(MSG_SELF_DESTRUCT)` for a player index, with a `FALSE` argument and
    /// `friend_setPlayerIndex(playerIndex)` (processDestroyPlayerCommand).
    fn command_list_append_self_destruct(&mut self, player_index: i32);
    /// `TheMessageStream->appendMessage(type)`; returns whether the type was `MSG_SELF_DESTRUCT` is up to the caller.
    fn message_stream_append(&mut self, ty: MessageType);
    /// `TheMessageStream->appendMessage(MSG_SELF_DESTRUCT)->appendBooleanArgument(arg)`.
    fn message_stream_append_self_destruct(&mut self, arg: bool);
    /// `ThePlayerList->findPlayerWithNameKey(NAMEKEY("player%d"))`: whether player `playerN` exists, and its index.
    fn find_player_index_by_slot_name(&self, slot: u32) -> Option<i32>;
    /// `TheScriptActions->closeWindows(TRUE)`.
    fn script_actions_close_windows(&mut self, immediate: bool);
    /// `TheWindowManager->winCreateFromScript(name)`: the window's handle.
    fn create_window_from_script(&mut self, name: &[u8]) -> Option<u32>;
    /// `TheWindowManager->winDestroy(window)`.
    fn destroy_window(&mut self, window: u32);
    /// `TheScriptEngine->startEndGameTimer()`.
    fn script_engine_start_end_game_timer(&mut self);
    /// `TheRecorder->logCRCMismatch()`.
    fn recorder_log_crc_mismatch(&mut self);
    /// `TheGlobalData->m_networkRunAheadSlack`.
    fn network_run_ahead_slack(&self) -> u32;
    /// `QueryPerformanceCounter`.
    fn performance_counter(&self) -> i64;
    /// `QueryPerformanceFrequency`.
    fn performance_frequency(&self) -> i64;
}

/// `class Network : public NetworkInterface`.
pub struct Network<H: NetworkHost, C: ConnectionManagerHost> {
    pub host: H,
    /// The connection manager's host, kept to build `m_conMgr` in `init`.
    make_conn_host: Option<C>,
    limits: NetworkFrameLimits,
    /// `m_conMgr`.
    pub con_mgr: Option<ConnectionManager<C>>,
    /// `m_lastFrame`.
    pub last_frame: u32,
    /// `m_localStatus`.
    pub local_status: NetLocalStatus,
    /// `m_runAhead`.
    pub run_ahead: i32,
    /// `m_frameRate`.
    pub frame_rate: i32,
    /// `m_lastExecutionFrame`.
    pub last_execution_frame: i32,
    /// `m_lastFrameCompleted`.
    pub last_frame_completed: i32,
    /// `m_didSelfSlug`.
    pub did_self_slug: bool,
    /// `m_perfCountFreq`.
    pub perf_count_freq: i64,
    /// `m_nextFrameTime`.
    pub next_frame_time: i64,
    /// `m_frameDataReady`.
    pub frame_data_ready: bool,
    /// `m_checkCRCsThisFrame`.
    pub check_crcs_this_frame: bool,
    /// `m_sawCRCMismatch`.
    pub saw_crc_mismatch: bool,
    /// `m_playersToDisconnect`.
    pub players_to_disconnect: Vec<i32>,
    /// `m_messageWindow`.
    pub message_window: Option<u32>,
    /// `m_networkOn` (`_DEBUG`/`_INTERNAL`).
    pub network_on: bool,
}

impl<H: NetworkHost, C: ConnectionManagerHost> Network<H, C> {
    /// `Network::Network()` (Network.cpp:277): the flags cleared, no connection manager, no message window,
    /// `m_networkOn = TRUE`. `conn_host` is what `init` builds the connection manager from.
    pub fn new(host: H, conn_host: C, limits: NetworkFrameLimits) -> Self {
        Network {
            host,
            make_conn_host: Some(conn_host),
            limits,
            con_mgr: None,
            last_frame: 0,
            local_status: NetLocalStatus::PreGame,
            run_ahead: 0,
            frame_rate: 0,
            last_execution_frame: 0,
            last_frame_completed: 0,
            did_self_slug: false,
            perf_count_freq: 0,
            next_frame_time: 0,
            frame_data_ready: false,
            check_crcs_this_frame: false,
            saw_crc_mismatch: false,
            players_to_disconnect: Vec::new(),
            message_window: None,
            network_on: true,
        }
    }

    /// `Network::getPingFrame()` (Network.cpp:227).
    pub fn get_ping_frame(&self) -> u32 {
        self.con_mgr.as_ref().map_or(0, |c| c.get_ping_frame())
    }
    /// `Network::getPingsSent()` (:232).
    pub fn get_pings_sent(&self) -> i32 {
        self.con_mgr.as_ref().map_or(0, |c| c.get_pings_sent())
    }
    /// `Network::getPingsRecieved()` (:237).
    pub fn get_pings_recieved(&self) -> i32 {
        self.con_mgr.as_ref().map_or(0, |c| c.get_pings_recieved())
    }

    /// `Network::isPlayerConnected(playerID)` (:242): the local player is connected while in the game or leaving it.
    /// Upstream dereferences `m_conMgr` for the others unchecked (here: not connected without one).
    pub fn is_player_connected(&self, player_id: i32) -> bool {
        if player_id as u32 == self.get_local_player_id() {
            return self.local_status == NetLocalStatus::InGame || self.local_status == NetLocalStatus::Leaving;
        }
        self.con_mgr.as_ref().is_some_and(|c| c.is_player_connected(player_id))
    }

    /// `Network::deinit()` (:298): destroys the connection manager's game messages and the manager, and the message
    /// window. Always true.
    pub fn deinit(&mut self) -> bool {
        if let Some(c) = self.con_mgr.as_mut() {
            c.destroy_game_messages();
        }
        self.con_mgr = None;
        if let Some(w) = self.message_window.take() {
            self.host.destroy_window(w);
        }
        true
    }

    /// `Network::reset()` (:317): `init()`.
    pub fn reset(&mut self) {
        self.init();
    }

    /// `Network::init()` (:324): a new connection manager, the run ahead `min(max(30, MIN_RUNAHEAD), MAX_FRAMES_AHEAD/2)`,
    /// the frame rate 30, the frame counters at `m_runAhead - 1`, pre-game status. The `DEBUG_LOG`s of the timing values
    /// are not translated.
    pub fn init(&mut self) {
        if !self.deinit() {
            return;
        }
        if let Some(conn_host) = self.make_conn_host.take() {
            let mut c = ConnectionManager::new(conn_host, self.limits);
            c.init();
            self.con_mgr = Some(c);
        } else if let Some(c) = self.con_mgr.as_mut() {
            c.init();
        }
        self.last_frame = 0;
        self.run_ahead = std::cmp::min(std::cmp::max(30, self.limits.min_runahead), self.limits.max_frames_ahead / 2);
        self.frame_rate = 30;
        self.last_execution_frame = self.run_ahead - 1; // subtract 1 since we're starting on frame 0
        self.last_frame_completed = self.run_ahead - 1; // subtract 1 since we're starting on frame 0
        self.frame_data_ready = false;
        self.did_self_slug = false;
        self.local_status = NetLocalStatus::PreGame;
        self.perf_count_freq = self.host.performance_frequency();
        self.next_frame_time = 0;
        self.saw_crc_mismatch = false;
        self.check_crcs_this_frame = false;
        self.network_on = true;
    }

    /// `Network::setSawCRCMismatch()` (:372): the mismatch message window, the end game timer and the recorder's log.
    pub fn set_saw_crc_mismatch(&mut self) {
        self.saw_crc_mismatch = true;
        self.host.script_actions_close_windows(true);
        self.message_window = self.host.create_window_from_script(b"Menus/CRCMismatch.wnd");
        self.host.script_engine_start_end_game_timer();
        self.host.recorder_log_crc_mismatch();
        // DEBUG_LOG of the frame and the random seed CRC, and the DEBUG_CRC dumps: not translated.
    }

    /// `Network::sawCRCMismatch()`.
    pub fn saw_crc_mismatch(&self) -> bool {
        self.saw_crc_mismatch
    }

    /// `Network::parseUserList(game)` (:406): the connection manager takes the user list, drops the game messages, and
    /// zeroes `m_runAhead - 1` frames from frame 1 ("the game actually starts at frame 1").
    pub fn parse_user_list(&mut self, local_slot_num: i32, slots: Option<&[ParseSlot]>) {
        let Some(slots) = slots else {
            // DEBUG_LOG(("FAILED parseUserList with a NULL game\n"));
            return;
        };
        let run_ahead = self.run_ahead;
        if let Some(c) = self.con_mgr.as_mut() {
            c.parse_user_list(local_slot_num, slots);
            c.destroy_game_messages();
            c.zero_frames(1, (run_ahead - 1) as u32);
        }
    }

    /// `Network::startGame()` (:424): empty upstream.
    pub fn start_game(&mut self) {}

    /// `Network::setLocalAddress(ip, port)` (:431).
    pub fn set_local_address(&mut self, ip: u32, port: u32) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.set_local_address(ip, port);
        }
    }

    /// `Network::initTransport()` (:441).
    pub fn init_transport(&mut self) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.init_transport();
        }
    }

    /// `Network::attachTransport(transport)` (:448).
    pub fn attach_transport(&mut self, transport: crate::drafts::network::transport::Transport) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.attach_transport(transport);
        }
    }

    /// `Network::isTransferCommand(msg)` (:458): a message strictly between `MSG_BEGIN_NETWORK_MESSAGES` and
    /// `MSG_END_NETWORK_MESSAGES`. `ty` is `None` for upstream's `NULL`.
    pub fn is_transfer_command(ty: Option<MessageType>) -> bool {
        matches!(ty, Some(t) if t.0 > MSG_BEGIN_NETWORK_MESSAGES.0 && t.0 < MSG_END_NETWORK_MESSAGES.0)
    }

    /// `Network::GetCommandsFromCommandList()` (:468): each message of `TheCommandList` that is a transfer command goes
    /// to the other players (while in the game), then leaves the list; any other is processed, and leaves the list if
    /// `processCommand` ate it.
    pub fn get_commands_from_command_list(&mut self) {
        let mut msg = self.host.command_list_first();
        while let Some(m) = msg {
            let next = self.host.command_list_next(m);
            let ty = self.host.message_type(m);
            if Self::is_transfer_command(Some(ty)) {
                if self.local_status == NetLocalStatus::InGame {
                    let frame = self.get_execution_frame() as u32;
                    self.send_local_game_message(m, frame);
                }
                self.host.command_list_remove(m);
            } else if self.process_command(m) {
                self.host.command_list_remove(m);
            }
            msg = next;
        }
    }

    /// `m_conMgr->sendLocalGameMessage(msg, frame)`: the manager wraps the message's type and arguments.
    fn send_local_game_message(&mut self, msg: H::Msg, frame: u32) {
        let ty = self.host.message_type(msg);
        let args = self.host.message_arguments(msg);
        if let Some(c) = self.con_mgr.as_mut() {
            c.send_local_game_message(ty, &args, frame);
        }
    }

    /// `Network::getExecutionFrame()` (:489): "the next valid frame for simultaneous command execution", never going back.
    pub fn get_execution_frame(&mut self) -> i32 {
        let logic_frame = self.host.game_logic_frame() as i32 + self.run_ahead;
        if logic_frame > self.last_execution_frame {
            self.last_execution_frame = logic_frame;
            return logic_frame;
        }
        self.last_execution_frame
    }

    /// `Network::processCommand(msg)` (:504): on a new logic frame (or pre-game) the frame ticks are processed; leaving
    /// the game (`MSG_CLEAR_GAME_DATA`) tells the manager and sets `NETLOCALSTATUS_LEAVING`. Returns true if the message
    /// is "eaten" by the network.
    pub fn process_command(&mut self, msg: H::Msg) -> bool {
        let ty = self.host.message_type(msg);
        if self.last_frame != self.host.game_logic_frame() || self.local_status == NetLocalStatus::PreGame {
            if self.local_status == NetLocalStatus::PreGame {
                if self.host.game_logic_frame() == 1 {
                    self.local_status = NetLocalStatus::InGame;
                    // NetCommandList *netcmdlist = m_conMgr->getFrameCommandList(0); // clear out frame 0 since we skipped it
                    if let Some(c) = self.con_mgr.as_mut() {
                        let _netcmdlist = c.get_frame_command_list(0);
                    }
                } else {
                    return false;
                }
            }
            if self.local_status == NetLocalStatus::InGame {
                let execution_frame = self.get_execution_frame();
                let mut i = self.last_frame_completed + 1;
                while i < execution_frame {
                    if let Some(c) = self.con_mgr.as_mut() {
                        c.process_frame_tick(i as u32);
                    }
                    self.last_frame_completed = i;
                    i += 1;
                }
            }
            self.last_frame = self.host.game_logic_frame();
        }
        if ty == MSG_CLEAR_GAME_DATA && self.local_status == NetLocalStatus::InGame {
            let execution_frame = self.get_execution_frame();
            if let Some(c) = self.con_mgr.as_mut() {
                c.handle_local_player_leaving((execution_frame + 1) as u32);
                // This is the last command we will execute, so send the command count.
                c.process_frame_tick(execution_frame as u32);
                // since we send it for executionFrame+1, we need to process both ticks
                c.process_frame_tick((execution_frame + 1) as u32);
            }
            self.last_frame_completed = execution_frame;
            self.local_status = NetLocalStatus::Leaving;
            return true;
        }
        false
    }

    /// `Network::AllCommandsReady(frame)` (:565): true without a manager, pre-game and post-game; else the manager's.
    pub fn all_commands_ready(&mut self, frame: u32) -> bool {
        if self.con_mgr.is_none() {
            return true;
        }
        if self.local_status == NetLocalStatus::PreGame {
            return true;
        }
        if self.local_status == NetLocalStatus::PostGame {
            return true;
        }
        self.con_mgr.as_mut().map_or(true, |c| c.all_commands_ready(frame, false))
    }

    /// `Network::RelayCommandsToCommandList(frame)` (:585): the frame's commands onto `TheCommandList` in order, a game
    /// command as a game message, the others handled as frame synchronized; then the players queued by
    /// `selfDestructPlayer` are cleared (their `MSG_SELF_DESTRUCT` is commented out upstream).
    pub fn relay_commands_to_command_list(&mut self, frame: u32) {
        if self.con_mgr.is_none() || self.local_status == NetLocalStatus::PreGame {
            return;
        }
        self.check_crcs_this_frame = false;
        let Some(c) = self.con_mgr.as_mut() else { return };
        let netcmdlist = c.get_frame_command_list(frame);
        let mut msg = netcmdlist.get_first_message();
        while let Some(id) = msg {
            let (command, next) = match netcmdlist.get(id) {
                Some(r) => (r.get_command().clone(), r.get_next()),
                None => break,
            };
            let ty = command.read().get_net_command_type();
            if ty == NetCommandType::GameCommand {
                self.host.command_list_append_game_command(&command);
            } else {
                self.process_frame_synchronized_net_command(&command);
            }
            msg = next;
        }
        // for (selfDestructIt ...) { /* all commented out upstream */ }
        self.players_to_disconnect.clear();
        // netcmdlist->deleteInstance();
    }

    /// `Network::processFrameSynchronizedNetCommand(msg)` (:619): a player leaving, a new run ahead, a player destroyed.
    pub fn process_frame_synchronized_net_command(&mut self, cmd: &NetCommandMsgPtr) {
        let ty = cmd.read().get_net_command_type();
        if ty == NetCommandType::PlayerLeave {
            let retval = self.con_mgr.as_mut().map_or(PlayerLeaveCode::Unknown, |c| c.process_player_leave(cmd));
            if retval == PlayerLeaveCode::Local {
                // DEBUG_LOG: Local player left the game on frame ...
                self.local_status = NetLocalStatus::Left;
            } else if retval == PlayerLeaveCode::PacketRouter {
                // DEBUG_LOG: Packet router left the game on frame ...
            } else {
                // DEBUG_LOG: Client left the game on frame ...
            }
        } else if ty == NetCommandType::RunAhead {
            let (run_ahead, frame_rate) = match &cmd.read().body {
                NetCommandBody::RunAhead { run_ahead, frame_rate } => (*run_ahead, *frame_rate),
                _ => (0, 0),
            };
            self.process_run_ahead_command(run_ahead as i32, frame_rate as i32);
        } else if ty == NetCommandType::DestroyPlayer {
            let player_index = match &cmd.read().body {
                NetCommandBody::DestroyPlayer { player_index } => *player_index,
                _ => 0,
            };
            self.process_destroy_player_command(player_index);
        }
    }

    /// `Network::processRunAheadCommand(msg)` (:644): the new run ahead and frame rate, and the connection manager's
    /// frame grouping `(1000 * runAhead / frameRate) / 2`, clamped to `1..=500` ms ("the latency for one way").
    pub fn process_run_ahead_command(&mut self, run_ahead: i32, frame_rate: i32) {
        self.run_ahead = run_ahead;
        self.frame_rate = frame_rate;
        // time_t frameGrouping = (1000 * m_runAhead) / m_frameRate; (an Int division: m_frameRate is not 0 upstream)
        let mut frame_grouping: i64 = if self.frame_rate != 0 { (1000 * self.run_ahead as i64) / self.frame_rate as i64 } else { 0 };
        // since we only want the latency for one way to be a factor.
        frame_grouping /= 2;
        if frame_grouping < 1 {
            frame_grouping = 1; // Having a value less than 1 doesn't make sense.
        }
        if frame_grouping > 500 {
            frame_grouping = 500; // Max of a half a second.
        }
        if let Some(c) = self.con_mgr.as_mut() {
            c.set_frame_grouping(frame_grouping);
        }
    }

    /// `Network::processDestroyPlayerCommand(msg)` (:659): an out-of-range player index is refused; an existing
    /// `player<N>` gets a `MSG_SELF_DESTRUCT` (argument `FALSE`) on `TheCommandList`.
    pub fn process_destroy_player_command(&mut self, player_index: u32) {
        if player_index >= MAX_SLOTS as u32 {
            // DEBUG_ASSERTCRASH(playerIndex < MAX_SLOTS, ("Bad player index"));
            return;
        }
        if let Some(index) = self.host.find_player_index_by_slot_name(player_index) {
            self.host.command_list_append_self_destruct(index);
        }
    }

    /// `Network::update()` (:684): the per-frame network step.
    pub fn update(&mut self) {
        self.frame_data_ready = false;
        if !self.network_on {
            return;
        }
        // Remove commands from TheCommandList and send them to the connection manager.
        self.get_commands_from_command_list();
        if self.con_mgr.is_some() && self.local_status == NetLocalStatus::InGame {
            let execution_frame = self.get_execution_frame();
            let (run_ahead, frame_rate, slug) = (self.run_ahead, self.frame_rate, self.did_self_slug);
            if let Some(c) = self.con_mgr.as_mut() {
                c.update_run_ahead(run_ahead, frame_rate, slug, execution_frame);
            }
            self.did_self_slug = false;
        }
        self.liteupdate();
        if self.local_status == NetLocalStatus::Left {
            self.end_of_game_check();
        }
        let frame = self.host.game_logic_frame();
        // If all the commands are ready for the next frame...
        if self.all_commands_ready(frame) {
            if let Some(c) = self.con_mgr.as_mut() {
                c.handle_all_commands_ready();
            }
            // This needs to come after any other pre-frame execution checks as this changes the timing variables.
            if self.time_for_new_frame() {
                // Put the commands for the next frame on TheCommandList.
                self.relay_commands_to_command_list(frame);
                // Tell the GameEngine to run the commands for the new frame.
                self.frame_data_ready = true;
            }
        }
    }

    /// `Network::liteupdate()` (:726): the manager's update, "in game" unless pre-game.
    pub fn liteupdate(&mut self) {
        if !self.network_on {
            return;
        }
        let in_game = self.local_status != NetLocalStatus::PreGame;
        if let Some(c) = self.con_mgr.as_mut() {
            c.update(in_game);
        }
    }

    /// `Network::endOfGameCheck()` (:743): when the manager says this player can leave, disconnects it, tells the logic
    /// to clear the game data and sets `NETLOCALSTATUS_POSTGAME`. (`debugPrintConnectionCommands` otherwise: `_DEBUG`.)
    pub fn end_of_game_check(&mut self) {
        let can_leave = self.con_mgr.as_ref().map(|c| c.can_i_leave());
        if can_leave == Some(true) {
            if let Some(c) = self.con_mgr.as_mut() {
                c.disconnect_local_player();
            }
            self.host.message_stream_append(MSG_CLEAR_GAME_DATA);
            self.local_status = NetLocalStatus::PostGame;
        }
    }

    /// `Network::timeForNewFrame()` (:760): whether the next logic frame is due by the performance counter. "If we're
    /// pushing up against the edge of our run ahead, we should slow the framerate down a bit": the manager's minimum
    /// cushion below `runAhead * networkRunAheadSlack / 100` lengthens the frame delay by a tenth and sets
    /// `m_didSelfSlug`. A clock that fell 2 frames behind is reset to now.
    pub fn time_for_new_frame(&mut self) -> bool {
        let cur_time = self.host.performance_counter();
        let mut frame_delay = self.perf_count_freq / self.frame_rate as i64;
        if let Some(c) = self.con_mgr.as_ref() {
            let cushion = c.get_minimum_cushion() as f32;
            let run_ahead_percentage = self.run_ahead as f32 * (self.host.network_run_ahead_slack() as f32 / 100.0);
            if cushion < run_ahead_percentage {
                frame_delay += frame_delay / 10; // temporarily decrease the frame rate by 20%.
                self.did_self_slug = true;
            }
        }
        if cur_time >= self.next_frame_time {
            if self.next_frame_time + (2 * frame_delay) < cur_time {
                self.next_frame_time = cur_time;
            } else {
                self.next_frame_time += frame_delay;
            }
            return true;
        }
        false
    }

    /// `Network::isFrameDataReady()` (:807).
    pub fn is_frame_data_ready(&self) -> bool {
        self.frame_data_ready || self.local_status == NetLocalStatus::Left
    }

    /// `Network::getIncomingBytesPerSecond()` (:814) and the five like it: the manager's, 0 without one.
    pub fn get_incoming_bytes_per_second(&self) -> f32 {
        self.con_mgr.as_ref().map_or(0.0, |c| c.get_incoming_bytes_per_second())
    }
    pub fn get_incoming_packets_per_second(&self) -> f32 {
        self.con_mgr.as_ref().map_or(0.0, |c| c.get_incoming_packets_per_second())
    }
    pub fn get_outgoing_bytes_per_second(&self) -> f32 {
        self.con_mgr.as_ref().map_or(0.0, |c| c.get_outgoing_bytes_per_second())
    }
    pub fn get_outgoing_packets_per_second(&self) -> f32 {
        self.con_mgr.as_ref().map_or(0.0, |c| c.get_outgoing_packets_per_second())
    }
    pub fn get_unknown_bytes_per_second(&self) -> f32 {
        self.con_mgr.as_ref().map_or(0.0, |c| c.get_unknown_bytes_per_second())
    }
    pub fn get_unknown_packets_per_second(&self) -> f32 {
        self.con_mgr.as_ref().map_or(0.0, |c| c.get_unknown_packets_per_second())
    }
    /// `Network::getPacketArrivalCushion()` (:880).
    pub fn get_packet_arrival_cushion(&mut self) -> u32 {
        self.con_mgr.as_mut().map_or(0, |c| c.get_packet_arrival_cushion())
    }

    /// `Network::sendChat(text, playerMask)` (:891): at the execution frame.
    pub fn send_chat(&mut self, text: UnicodeString, player_mask: i32) {
        let execution_frame = self.get_execution_frame();
        if let Some(c) = self.con_mgr.as_mut() {
            c.send_chat(text, player_mask, execution_frame as u32);
        }
    }
    /// `Network::sendDisconnectChat(text)` (:899).
    pub fn send_disconnect_chat(&mut self, text: UnicodeString) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.send_disconnect_chat(text);
        }
    }
    /// `Network::sendFile(path, playerMask, commandID)` (:904).
    pub fn send_file(&mut self, path: &[u8], player_mask: u8, command_id: u16) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.send_file(path, player_mask, command_id);
        }
    }
    /// `Network::sendFileAnnounce(path, playerMask)` (:910).
    pub fn send_file_announce(&mut self, path: &[u8], player_mask: u8) -> u16 {
        self.con_mgr.as_mut().map_or(0, |c| c.send_file_announce(path, player_mask))
    }
    /// `Network::getFileTransferProgress(playerID, path)` (:915).
    pub fn get_file_transfer_progress(&mut self, player_id: i32, path: &[u8]) -> i32 {
        self.con_mgr.as_mut().map_or(0, |c| c.get_file_transfer_progress(player_id, path))
    }
    /// `Network::areAllQueuesEmpty()` (:920): `m_conMgr->canILeave()`.
    pub fn are_all_queues_empty(&self) -> bool {
        self.con_mgr.as_ref().is_some_and(|c| c.can_i_leave())
    }

    /// `Network::quitGame()` (:928): "quit the game now. This should only be called from the disconnect screen."
    pub fn quit_game(&mut self) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.quit_game();
            self.host.message_stream_append_self_destruct(true);
            self.host.message_stream_append(MSG_CLEAR_GAME_DATA);
            self.local_status = NetLocalStatus::PostGame;
        }
    }

    /// `Network::selfDestructPlayer(index)` (:942): `m_playersToDisconnect.push_back(index)`.
    pub fn self_destruct_player(&mut self, index: i32) {
        self.players_to_disconnect.push(index);
    }
    /// `Network::isPacketRouter()` (:948).
    pub fn is_packet_router(&self) -> bool {
        self.con_mgr.as_ref().is_some_and(|c| c.is_packet_router())
    }
    /// `Network::voteForPlayerDisconnect(slot)` (:956).
    pub fn vote_for_player_disconnect(&mut self, slot: i32) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.vote_for_player_disconnect(slot);
        }
    }
    /// `Network::updateLoadProgress(percent)` (:962).
    pub fn update_load_progress(&mut self, percent: i32) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.update_load_progress(percent);
        }
    }
    /// `Network::loadProgressComplete()` (:969).
    pub fn load_progress_complete(&mut self) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.load_progress_complete();
        }
    }
    /// `Network::sendTimeOutGameStart()` (:976).
    pub fn send_time_out_game_start(&mut self) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.send_time_out_game_start();
        }
    }
    /// `Network::getLocalPlayerID()` (:984): 49 without a manager.
    pub fn get_local_player_id(&self) -> u32 {
        self.con_mgr.as_ref().map_or(49, |c| c.get_local_player_id())
    }
    /// `Network::getPlayerName(playerNum)` (:991): the local player's name only while in the game (upstream
    /// dereferences `m_conMgr` for the comparison before it checks it); empty without a manager.
    pub fn get_player_name(&self, player_num: i32) -> UnicodeString {
        if let Some(c) = self.con_mgr.as_ref() {
            if player_num as u32 == c.get_local_player_id() && self.local_status != NetLocalStatus::InGame {
                return UnicodeString::default();
            }
            return c.get_player_name(player_num);
        }
        UnicodeString::default()
    }
    /// `Network::getNumPlayers()` (:1003): -1 without a manager.
    pub fn get_num_players(&self) -> i32 {
        self.con_mgr.as_ref().map_or(-1, |c| c.get_num_players())
    }
    /// `Network::getSlotAverageFPS(slot)` (:1011): -1 without a manager.
    pub fn get_slot_average_fps(&self, slot: i32) -> i32 {
        self.con_mgr.as_ref().map_or(-1, |c| c.get_slot_average_fps(slot))
    }
    /// `Network::getAverageFPS()` (header): `m_conMgr->getAverageFPS()`.
    pub fn get_average_fps(&self) -> i32 {
        self.con_mgr.as_ref().map_or(0, |c| c.get_average_fps())
    }
    /// `Network::toggleNetworkOn()` (:1019, `_DEBUG`/`_INTERNAL`): flips `m_networkOn`.
    pub fn toggle_network_on(&mut self) {
        self.network_on = !self.network_on;
    }
    /// `Network::notifyOthersOfCurrentFrame()` (:1028).
    pub fn notify_others_of_current_frame(&mut self) {
        let frame = self.host.game_logic_frame() as i32;
        if let Some(c) = self.con_mgr.as_mut() {
            c.notify_others_of_current_frame(frame);
        }
    }
    /// `Network::notifyOthersOfNewFrame(frame)` (:1034).
    pub fn notify_others_of_new_frame(&mut self, frame: u32) {
        if let Some(c) = self.con_mgr.as_mut() {
            c.notify_others_of_new_frame(frame);
        }
    }
}

/// `LOGICFRAMES_PER_SECOND`, used by callers to convert `m_frameRate`.
pub const NETWORK_LOGIC_FPS: i32 = LOGICFRAMES_PER_SECOND as i32;

/// `MSG_SELF_DESTRUCT`, re-exported for hosts.
pub const NETWORK_MSG_SELF_DESTRUCT: MessageType = MSG_SELF_DESTRUCT;
