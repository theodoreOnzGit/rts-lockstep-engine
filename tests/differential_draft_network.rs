#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/Network.cpp (the functions of a Network without a connection manager)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::network` against upstream's, for a `Network` with no connection manager (#215).
//!
//! # Method
//!
//! Eleven functions of `Network.cpp` are cut verbatim (build.rs, `drafts_guilogic4`) into `differential/wrap_draft_network.cpp`, built on upstream's own
//! AsciiString.cpp; the class around them, the message stream, the command list, the player list, the game logic and the clock are stand-ins (listed in its
//! header) that log every call. They are run with `m_conMgr == NULL`, which `Network::Network()` leaves and which these functions handle. 20,000 random programs:
//! a setup (local status, run ahead, frame rate, last execution frame, last frame completed, the self slug flag, the counter's frequency and next frame time,
//! the frame data flag, the game frame and counter, which `player<k>` exist), then 1 to 24 operations drawn from: `getExecutionFrame`, `timeForNewFrame` (with
//! a counter value), `isFrameDataReady`, `AllCommandsReady`, `quitGame`, `selfDestructPlayer`, `processDestroyPlayerCommand` (index 0 to 12 and some huge),
//! `isTransferCommand` (types -3 to 3000 around the two markers, and NULL), the seven statistic getters, `isPacketRouter` / `getLocalPlayerID` /
//! `getNumPlayers` / `getSlotAverageFPS` with the calls that do nothing without a manager, and a new game frame. Every answer, the log of stand-in calls and
//! the final state (status, the frame counters, the slug flag, the next frame time, the list of players to disconnect) are compared. `processRunAheadCommand`'s
//! arithmetic (`frame_grouping_for`) is compared over 20,000 run ahead and frame rate pairs, with a stand-in connection manager that logs `setFrameGrouping`.
//!
//! Not tested, and why: everything that needs a connection manager, because the draft's is the first-pass `ConnectionManager` of `drafts/network` (another
//! group's, not to be touched) whose state the C++ stand-in cannot mirror: `init`, `deinit`, `reset`, `parseUserList`, `setSawCRCMismatch`, `processCommand`,
//! `GetCommandsFromCommandList`, `RelayCommandsToCommandList`, `processFrameSynchronizedNetCommand`, `update`, `liteupdate`, `endOfGameCheck`, the statistics
//! with a manager, `sendChat` and its siblings, `getPlayerName`, `isPlayerConnected`, the cushion (slug) branch of `timeForNewFrame`, `areAllQueuesEmpty`, and
//! `setLocalAddress`, `initTransport`, `attachTransport`. `startGame` and `toggleNetworkOn` (a `_DEBUG` function; absent in the release build; the draft keeps a
//! `network_on` flag nothing here reads) are empty or debug-only. Excluded inputs: a frame rate of 0 (a division by zero upstream) in `timeForNewFrame` and
//! `processRunAheadCommand`, `INT_MIN / -1`, and counter values whose sums would overflow 64 bits (|value| < 2^61 here; the draft has overflow checks on).
//!
//! # Prediction (before the first run, 2026-11-20)
//!
//! One mismatch, found by reading upstream: `Network::quitGame` appends `MSG_SELF_DESTRUCT` (argument TRUE) and `MSG_CLEAR_GAME_DATA` to `TheMessageStream` and
//! sets `NETLOCALSTATUS_POSTGAME` even when `m_conMgr` is NULL; the draft does all three only inside `if let Some(c) = con_mgr`. Everything else: 0.
//! The run ahead arithmetic: the draft first computed `1000 * runAhead` in i64 where upstream's `Int` product wraps (plant 15904 is that first version).
//!
//! # Results
//!
//! 2026-11-20: first run, 20,000 programs, **9,586 mismatches** in the answers and state (prediction met: all in the status after `quitGame`; the draft did nothing
//! without a connection manager, upstream appends the two messages and sets `NETLOCALSTATUS_POSTGAME` anyway; fixed in network.rs); then **0 mismatches** in the answers
//! and state, in the call logs, and in the frame grouping over 20,000 run ahead and frame rate pairs. Planted bugs, 11 of 11 caught (mismatches of 20,000):
//! 15894 4,016; 15895 1,406; 15896 1,443; 15897 9,500; 15898 2,420 (in the logs: the lookup of `player8`); 15899 109; 15900 4,672 (the first version of the generator caught it
//! once: the counter equal to the next frame time was added); 15901 11,914; 15902 11,914; 15903 6,087; 15904 3,967 (all in the frame grouping: the first draft's i64
//! product, plant 15904, against upstream's `Int` product).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 15894 `getExecutionFrame` never keeps the old frame; 15895 `isTransferCommand` includes the begin marker; 15896 includes the end marker;
//! 15897 `quitGame` does not set the post game status; 15898 `processDestroyPlayerCommand` accepts index `MAX_SLOTS`; 15899 `timeForNewFrame` resets after
//! one frame delay, not two; 15900 `timeForNewFrame` waits for `>` not `>=`; 15901 `getLocalPlayerID` without a manager is 0; 15902 `getNumPlayers` is 0;
//! 15903 `isFrameDataReady` needs both; 15904 the frame grouping's product in i64 (the first draft).

mod support;

use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::common::message_stream::{GameMessageArgument, MessageType};
use rts_lockstep_engine::common::message_types::{MSG_CLEAR_GAME_DATA, MSG_SELF_DESTRUCT};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::drafts::network::connection_manager::{ChatSender, ConnectionManagerHost, PingResponse};
use rts_lockstep_engine::drafts::network::net_command_msg::{NetCommandMsgPtr, NetLocalStatus, NetworkFrameLimits};
use rts_lockstep_engine::drafts::sweep_gui_logic::network::{Network, NetworkHost};
use std::cell::Cell;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_net_run(input: *const i64, n: i32, out: *mut i64, cap: i32, log: *mut std::os::raw::c_char, logcap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

/// The game's side of the draft's `NetworkHost`: the stand-ins of the C++ wrapper, as the same log lines.
struct Host {
    frame: u32,
    counter: Cell<i64>,
    player_mask: i64,
    slack: u32,
    log: std::cell::RefCell<Vec<String>>,
}

fn type_name(t: MessageType) -> &'static str {
    if t == MSG_SELF_DESTRUCT {
        "SELF_DESTRUCT"
    } else if t == MSG_CLEAR_GAME_DATA {
        "CLEAR_GAME_DATA"
    } else {
        "other"
    }
}

impl NetworkHost for Host {
    type Msg = u32;
    fn game_logic_frame(&self) -> u32 {
        self.frame
    }
    fn command_list_first(&self) -> Option<u32> {
        None
    }
    fn command_list_next(&self, _msg: u32) -> Option<u32> {
        None
    }
    fn message_arguments(&self, _msg: u32) -> Vec<GameMessageArgument> {
        Vec::new()
    }
    fn message_type(&self, _msg: u32) -> MessageType {
        MessageType(0)
    }
    fn command_list_remove(&mut self, _msg: u32) {}
    fn command_list_append_game_command(&mut self, _command: &NetCommandMsgPtr) {}
    fn command_list_append_self_destruct(&mut self, player_index: i32) {
        // the C++: `newInstance(GameMessage)(MSG_SELF_DESTRUCT)`, `appendBooleanArgument(FALSE)`, `friend_setPlayerIndex(index)`, then `TheCommandList->appendMessage`
        self.log.borrow_mut().push("bool 0".to_string());
        self.log.borrow_mut().push(format!("player {player_index}"));
        self.log.borrow_mut().push("commandlist.append SELF_DESTRUCT".to_string());
    }
    fn message_stream_append(&mut self, ty: MessageType) {
        self.log.borrow_mut().push(format!("stream.append {}", type_name(ty)));
    }
    fn message_stream_append_self_destruct(&mut self, arg: bool) {
        self.log.borrow_mut().push("stream.append SELF_DESTRUCT".to_string());
        self.log.borrow_mut().push(format!("bool {}", arg as i32));
    }
    fn find_player_index_by_slot_name(&self, slot: u32) -> Option<i32> {
        // the C++ stand-in logs the name it is asked for
        self.log.borrow_mut().push(format!("find player{slot}"));
        (slot < 63 && (self.player_mask >> slot) & 1 != 0).then(|| 100 + slot as i32)
    }
    fn script_actions_close_windows(&mut self, _immediate: bool) {}
    fn create_window_from_script(&mut self, _name: &[u8]) -> Option<u32> {
        None
    }
    fn destroy_window(&mut self, _window: u32) {}
    fn script_engine_start_end_game_timer(&mut self) {}
    fn recorder_log_crc_mismatch(&mut self) {}
    fn network_run_ahead_slack(&self) -> u32 {
        self.slack
    }
    fn performance_counter(&self) -> i64 {
        self.counter.get()
    }
    fn performance_frequency(&self) -> i64 {
        0
    }
}

/// A connection manager's host nothing reaches (the draft's `Network` is built with none).
struct NoConn(GlobalData);
impl ConnectionManagerHost for NoConn {
    fn time_get_time(&self) -> u32 {
        0
    }
    fn game_frame(&self) -> u32 {
        0
    }
    fn game_logic_process_progress(&mut self, _player_id: u32, _percentage: u8) {}
    fn game_logic_process_progress_complete(&mut self, _player_id: u32) {}
    fn game_logic_time_out_game_start(&mut self) {}
    fn global_data(&self) -> &GlobalData {
        &self.0
    }
    fn network_execution_frame(&self) -> u32 {
        0
    }
    fn notify_others_of_current_frame(&mut self) {}
    fn notify_others_of_new_frame(&mut self, _frame: u32) {}
    fn disconnect_menu_hide_screen(&mut self) {}
    fn disconnect_menu_show_screen(&mut self) {}
    fn disconnect_menu_set_player_name(&mut self, _slot: i32, _name: &UnicodeString) {}
    fn disconnect_menu_update_votes(&mut self, _slot: i32, _votes: i32) {}
    fn disconnect_menu_remove_player(&mut self, _slot: i32, _name: &UnicodeString) {}
    fn disconnect_menu_set_player_timeout_time(&mut self, _slot: i32, _time: i32) {}
    fn disconnect_menu_hide_packet_router_timeout(&mut self) {}
    fn disconnect_menu_show_chat(&mut self, _text: &UnicodeString) {}
    fn in_game_ui_message(&mut self, _text: &UnicodeString) {}
    fn in_game_ui_message_color(&mut self, _color: i32, _text: &UnicodeString) {}
    fn in_game_ui_player_left_game(&mut self, _name: &UnicodeString) {}
    fn audio_add_event(&mut self, _name: &[u8]) {}
    fn chat_sender(&mut self, _player_id: u32) -> ChatSender {
        ChatSender::NotFound
    }
    fn local_player_is_observer(&mut self) -> bool {
        false
    }
    fn game_info_slot_is_muted(&mut self, _slot: u32) -> bool {
        false
    }
    fn game_info_slot_last_frame_in_game(&mut self, _slot: i32) -> Option<u32> {
        None
    }
    fn game_info_slot_set_last_frame_in_game(&mut self, _slot: i32, _frame: u32) {}
    fn game_info_mark_slot_as_disconnected(&mut self, _slot: i32) {}
    fn recorder_log_player_disconnect(&mut self, _name: &UnicodeString, _slot: i32) {}
    fn populate_in_game_diplomacy_popup(&mut self) {}
    fn file_exists(&mut self, _path: &[u8]) -> bool {
        false
    }
    fn file_write(&mut self, _path: &[u8], _data: &[u8]) -> bool {
        false
    }
    fn local_file_read_nonempty(&mut self, _path: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn lan_on_chat_system(&mut self, _text: &UnicodeString) {}
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
    fn pinger_available(&self) -> bool {
        false
    }
    fn ping_server_count(&self) -> usize {
        0
    }
    fn ping_server_hostname(&self, _index: usize) -> String {
        String::new()
    }
    fn ping_add_request(&mut self, _hostname: &str, _repetitions: i32, _timeout: i32) {}
    fn ping_get_response(&mut self) -> Option<PingResponse> {
        None
    }
}

fn status_of(v: i64) -> NetLocalStatus {
    match v {
        0 => NetLocalStatus::PreGame,
        1 => NetLocalStatus::InGame,
        2 => NetLocalStatus::Leaving,
        3 => NetLocalStatus::Left,
        _ => NetLocalStatus::PostGame,
    }
}
fn status_num(s: NetLocalStatus) -> i64 {
    match s {
        NetLocalStatus::PreGame => 0,
        NetLocalStatus::InGame => 1,
        NetLocalStatus::Leaving => 2,
        NetLocalStatus::Left => 3,
        NetLocalStatus::PostGame => 4,
    }
}

fn big(g: &mut Gen) -> i64 {
    match g.below(4) {
        0 => g.i32() as i64,
        1 => (g.u64() >> 4) as i64 - (1i64 << 59),
        2 => g.below(2000) as i64,
        _ => g.below(200_000_000) as i64,
    }
}

/// The Rust run of a program in the wrapper's format: (answers, log).
fn rust_run(prog: &[i64]) -> (Vec<i64>, Vec<String>) {
    let mut k = 0;
    let mut next = || {
        k += 1;
        prog[k - 1]
    };
    let status = next();
    let run_ahead = next();
    let frame_rate = next();
    let last_exec = next();
    let last_completed = next();
    let slug = next();
    let freq = next();
    let next_time = next();
    let fdr = next();
    let slack = next();
    let game_frame = next();
    let counter = next();
    let mask = next();
    let crcs = next();
    let saw = next();
    let host = Host { frame: game_frame as u32, counter: Cell::new(counter), player_mask: mask, slack: slack as u32, log: std::cell::RefCell::new(Vec::new()) };
    let mut net: Network<Host, NoConn> = Network::new(host, NoConn(GlobalData::default()), NetworkFrameLimits::defaults());
    let mut o: Vec<i64> = vec![net.check_crcs_this_frame as i64, net.did_self_slug as i64, net.frame_data_ready as i64, net.saw_crc_mismatch as i64, net.con_mgr.is_none() as i64, net.message_window.is_none() as i64];
    net.local_status = status_of(status);
    net.run_ahead = run_ahead as i32;
    net.frame_rate = frame_rate as i32;
    net.last_execution_frame = last_exec as i32;
    net.last_frame_completed = last_completed as i32;
    net.did_self_slug = slug != 0;
    net.perf_count_freq = freq;
    net.next_frame_time = next_time;
    net.frame_data_ready = fdr != 0;
    net.check_crcs_this_frame = crcs != 0;
    net.saw_crc_mismatch = saw != 0;
    while k + 2 < prog.len() {
        let (kind, a, b) = (prog[k], prog[k + 1], prog[k + 2]);
        k += 3;
        match kind {
            0 => o.push(net.get_execution_frame() as i64),
            1 => {
                net.host.counter.set(a);
                o.push(net.time_for_new_frame() as i64);
            }
            2 => o.push(net.is_frame_data_ready() as i64),
            3 => o.push(net.all_commands_ready(a as u32) as i64),
            4 => net.quit_game(),
            5 => net.self_destruct_player(a as i32),
            6 => {
                net.process_destroy_player_command(a as u32)
            }
            7 => o.push(Network::<Host, NoConn>::is_transfer_command((b == 0).then_some(MessageType(a as i32))) as i64),
            8 => {
                for v in [net.get_incoming_bytes_per_second(), net.get_incoming_packets_per_second(), net.get_outgoing_bytes_per_second(), net.get_outgoing_packets_per_second(), net.get_unknown_bytes_per_second(), net.get_unknown_packets_per_second()] {
                    o.push(v.to_bits() as i64);
                }
                o.push(net.get_packet_arrival_cushion() as i64);
            }
            9 => {
                o.push(net.is_packet_router() as i64);
                o.push(net.get_local_player_id() as i64);
                o.push(net.get_num_players() as i64);
                o.push(net.get_slot_average_fps(a as i32) as i64);
                net.vote_for_player_disconnect(a as i32);
                net.update_load_progress(a as i32);
                net.load_progress_complete();
                net.send_time_out_game_start();
                net.notify_others_of_current_frame();
                net.notify_others_of_new_frame(a as u32);
            }
            10 => net.host.frame = a as u32,
            _ => {
                net.process_run_ahead_command(a as i32, b as i32);
                // the C++ stand-in connection manager logs what it is given
                net.host.log.borrow_mut().push(format!("frameGrouping {}", Network::<Host, NoConn>::frame_grouping_for(a as i32, b as i32)));
            }
        }
    }
    o.extend([status_num(net.local_status), net.last_execution_frame as i64, net.last_frame_completed as i64, net.did_self_slug as i64, net.next_frame_time, net.run_ahead as i64, net.frame_rate as i64, net.frame_data_ready as i64]);
    o.push(net.players_to_disconnect.len() as i64);
    o.extend(net.players_to_disconnect.iter().map(|&i| i as i64));
    let log = net.host.log.borrow().clone();
    (o, log)
}

#[test]
fn network_without_a_connection_manager() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_4E70);
    let mut answers = Check::new("Network (no connection manager): answers and state");
    let mut logs = Check::new("Network (no connection manager): stand-in call logs");
    let mut grouping = Check::new("Network::processRunAheadCommand (frame grouping)");
    let mut out = vec![0i64; 4096];
    let mut log = vec![0 as std::os::raw::c_char; 1 << 16];
    for case in 0..20_000 {
        let mut prog: Vec<i64> = Vec::new();
        let freq = [10_000_000, 3_579_545, 1_000_000_000, big(&mut g).abs() % 10_000_000_000 + 1][g.below(4) as usize];
        prog.extend([
            g.below(5) as i64,
            [30, 30, 10, 64, big(&mut g) % 2000][g.below(5) as usize],
            [30, 15, 60, 1, -7, 1 + g.below(120) as i64][g.below(6) as usize],
            big(&mut g) % 100000,
            big(&mut g) % 100000,
            g.below(2) as i64,
            freq,
            big(&mut g) % (1i64 << 61),
            g.below(2) as i64,
            g.below(300) as i64,
            g.below(100000) as i64,
            big(&mut g) % (1i64 << 61),
            (g.u32() & 0xFFFF) as i64,
            g.below(2) as i64,
            g.below(2) as i64,
        ]);
        for _ in 0..1 + g.below(24) {
            let kind = g.below(12) as i64;
            let (a, b) = match kind {
                1 => (if g.below(3) == 0 { prog[7] } else { big(&mut g) % (1i64 << 61) }, 0), // (a third of them: the counter equal to the setup's next frame time, a tie)
                3 => (g.u32() as i64, 0),
                5 => (g.i32() as i64, 0),
                6 => ([0, 1, 7, 8, 9, 12, 0xFFFF_FFFF, 0x8000_0000][g.below(8) as usize], 0),
                7 => ([-3, 0, 999, 1000, 1001, 1500, 1998, 1999, 2000, 3000, 27, 1093][g.below(12) as usize] + [0, 0, 0, 1, -1][g.below(5) as usize] * g.below(2) as i64, (g.below(8) == 0) as i64),
                9 => (g.below(20) as i64 - 3, 0),
                10 => (g.below(100000) as i64, 0),
                11 => (0, 0), // (replaced below: its own check)
                _ => (0, 0),
            };
            if kind == 11 {
                continue;
            }
            // timeForNewFrame with a frame rate of 0 divides by zero upstream: the frame rate is never 0 here (the setup's list has none)
            prog.extend([kind, a, b]);
        }
        let n = unsafe { ref_net_run(prog.as_ptr(), prog.len() as i32, out.as_mut_ptr(), out.len() as i32, log.as_mut_ptr(), log.len() as i32) };
        assert!(n >= 0, "out too small");
        let cpp_out = out[..n as usize].to_vec();
        let cpp_log: Vec<String> = unsafe { std::ffi::CStr::from_ptr(log.as_ptr()) }.to_string_lossy().lines().map(|s| s.to_string()).collect();
        let (r_out, r_log) = rust_run(&prog);
        answers.eq(r_out, cpp_out, || format!("case {case} program {prog:?}"));
        logs.eq(r_log, cpp_log, || format!("case {case} program {prog:?}"));
    }
    // processRunAheadCommand: the draft's pure part against the C++ with a stand-in manager
    for case in 0..20_000 {
        let ra = [g.i32(), g.below(200) as i32, g.below(100) as i32 - 20, (g.i32() >> g.below(24)) % 1_000_000, 2_147_483, 2_147_484, -2_147_483][g.below(7) as usize];
        let fr = loop {
            let v = [g.i32(), g.below(120) as i32 - 5, 30, 1, -1, (g.i32() >> g.below(28))][g.below(6) as usize];
            if v != 0 && !(ra == i32::MIN && v == -1) {
                break v;
            }
        };
        let prog: Vec<i64> = vec![0, 30, fr as i64, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 11, ra as i64, fr as i64];
        // (the setup's run ahead and frame rate are replaced by the operation)
        let n = unsafe { ref_net_run(prog.as_ptr(), prog.len() as i32, out.as_mut_ptr(), out.len() as i32, log.as_mut_ptr(), log.len() as i32) };
        assert!(n >= 0);
        let cpp_log = unsafe { std::ffi::CStr::from_ptr(log.as_ptr()) }.to_string_lossy().into_owned();
        let want = format!("frameGrouping {}\n", Network::<Host, NoConn>::frame_grouping_for(ra, fr));
        grouping.eq(want, cpp_log, || format!("case {case}: run ahead {ra} frame rate {fr}"));
    }
    if planted() {
        answers.report();
        logs.report();
        grouping.report();
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    answers.finish();
    logs.finish();
    grouping.finish();
}
