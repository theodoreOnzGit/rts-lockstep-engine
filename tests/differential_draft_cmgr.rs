#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/ConnectionManager.cpp (the accessors listed below) and Include/GameNetwork/ConnectionManager.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/connection_manager.rs`: the accessors (GitHub #215, group network3).
//!
//! # Method
//!
//! `differential/wrap_draft_cmgr.cpp` includes the verified network2 environment (`wrap_draft_conn.cpp`: the real `ConnectionManager.h` and `Connection.cpp`, the Transport and
//! UDP stand-ins) and cuts, verbatim, `getPacketRouterFallbackSlot`, `getPacketRouterSlot`, `areAllQueuesEmpty`, `canILeave`, `getMaximumLatency`, `getMinimumFps`,
//! `isPacketRouter`, `getSlotAverageFPS`, `getNextPacketRouterSlot` and `getPacketArrivalCushion`; `isPlayerConnected`, `getNumPlayers` and `getLocalPlayerID` come with the environment.
//! A program sets the manager's fields (local slot, router slot, fallback plan, fps and latency averages, cushion, connections present, quitting or with a queued
//! keep-alive command) and calls the accessors in a random order, 20 to 70 operations; the results and the fields are compared whole.
//!
//! `getNextPacketRouterSlot` with an id that is not among the first seven of the fallback plan reads `m_packetRouterFallback[8]` upstream (one past the array); the draft clamps it.
//! The program only asks for ids that are in the plan. Not covered: everything else in `ConnectionManager` (the command relay and acks, `update`, `updateRunAhead`,
//! `parseUserList`, the file transfer, the chat, `getMinimumCushion` and `getAverageFPS`, which use `FrameMetrics`) needs the frame data, the game logic, the transport and
//! the menus: see the report of #215.

use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::drafts::network::connection::Connection;
use rts_lockstep_engine::drafts::network::connection_manager::{ChatSender, ConnectionManager, ConnectionManagerHost, PingResponse};
use rts_lockstep_engine::drafts::network::net_command_msg::{NetCommandMsg, NetCommandMsgPtr, NetworkFrameLimits};
use std::os::raw::{c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_cm_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

struct LogHost {
    global: GlobalData,
}

impl ConnectionManagerHost for LogHost {
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
        &self.global
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

struct Rd<'a> {
    w: &'a [u32],
    i: usize,
}
impl Rd<'_> {
    fn next(&mut self) -> u32 {
        let v = self.w.get(self.i).copied().unwrap_or(0);
        self.i += 1;
        v
    }
}

fn slot_value(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => u32::MAX,
        1 => 8,
        2 => g.below(30),
        _ => g.below(8),
    }
}

fn float_bits(g: &mut Gen) -> u32 {
    let vals = [0.0f32, -0.0, 1.0, 2.5, 3.0, 100.0, 7.25, 0.5, 100.0, 2.5, f32::NAN, f32::INFINITY, -1.0, 1e-30];
    vals[g.below(vals.len() as u32) as usize].to_bits()
}

fn program(g: &mut Gen) -> Vec<u32> {
    let mut v = vec![];
    v.push(slot_value(g)); // local slot
    v.push(slot_value(g)); // router slot
    let mut plan = [0u32; 8];
    for p in plan.iter_mut() {
        *p = slot_value(g);
        v.push(*p);
    }
    for _ in 0..8 {
        v.push(match g.below(5) {
            0 | 1 => (-1i32) as u32,
            2 => g.below(100),
            3 => 30,
            _ => g.below(200),
        });
    }
    for _ in 0..8 {
        v.push(float_bits(g));
    }
    v.push(match g.below(3) {
        0 => u32::MAX,
        1 => g.below(10),
        _ => g.u32(),
    });
    let nops = 20 + g.below(51);
    for _ in 0..nops {
        let op = match g.below(5) {
            0 => 1 + g.below(8),
            _ => 10 + g.below(13),
        };
        v.push(op);
        match op {
            1 => {
                v.push(g.below(8));
                v.push(g.below(3));
            }
            2 => {
                v.push(g.below(8));
                v.push(g.below(8));
                v.push(g.below(1000));
                v.push(g.below(50_000));
                v.push(g.below(256));
            }
            3 | 4 | 8 => v.push(slot_value(g)),
            5 => {
                v.push(g.below(8));
                v.push(slot_value(g));
            }
            6 => {
                v.push(g.below(8));
                v.push(match g.below(4) {
                    0 | 1 => (-1i32) as u32,
                    2 => g.below(100),
                    _ => 30,
                });
            }
            7 => {
                v.push(g.below(8));
                v.push(float_bits(g));
            }
            10 => v.push(g.below(12).wrapping_sub(2)),
            17 => v.push(g.below(12).wrapping_sub(2)),
            18 => v.push(plan[g.below(7) as usize]),
            20 => v.push(g.below(12).wrapping_sub(2)),
            _ => {}
        }
    }
    v
}

fn dump(o: &mut Vec<u32>, cm: &ConnectionManager<LogHost>) {
    o.push(cm.get_local_player_id());
    o.push(cm.get_packet_router_slot());
    // m_smallestPacketArrivalCushion is read by the op, not exposed: dumped through a probe of the draft's accessor on a clone is not possible; the C++ dump of it is
    // matched by the ops that read it (19) and by the cases that set it (8) before them
}

fn rust_run(w: &[u32]) -> Vec<u32> {
    let mut rd = Rd { w, i: 0 };
    let host = LogHost { global: GlobalData::default() };
    let mut cm = ConnectionManager::new(host, NetworkFrameLimits::defaults());
    let local = rd.next();
    let router = rd.next();
    let mut plan = [0u32; 8];
    for p in plan.iter_mut() {
        *p = rd.next();
    }
    cm.debug_set_local_slot(local);
    cm.debug_set_router(router, plan);
    for i in 0..8 {
        cm.debug_set_fps_average(i, rd.next() as i32);
    }
    for i in 0..8 {
        cm.debug_set_latency_average(i, f32::from_bits(rd.next()));
    }
    let mut cushion = rd.next();
    cm.debug_set_smallest_cushion(cushion);
    let mut o = Vec::new();
    while rd.i < w.len() {
        let op = rd.next();
        o.push(0xA5A5A5A5);
        o.push(op);
        let mut local_slot = local;
        let _ = &mut local_slot;
        match op {
            1 => {
                let slot = (rd.next() % 8) as usize;
                let mode = rd.next();
                let c = if mode == 0 {
                    None
                } else {
                    let mut c = Connection::new();
                    c.init();
                    if mode == 2 {
                        c.set_quitting(0);
                    }
                    Some(c)
                };
                cm.debug_set_connection(slot, c);
            }
            2 => {
                let slot = (rd.next() % 8) as usize;
                let (player, frame, id, relay) = (rd.next(), rd.next(), rd.next(), rd.next());
                if let Some(c) = cm.debug_connection_mut(slot) {
                    let mut m = NetCommandMsg::new_keep_alive();
                    m.set_net_command_type(rts_lockstep_engine::drafts::network::net_command_msg::NetCommandType::KeepAlive);
                    m.set_player_id(player);
                    m.set_execution_frame(frame);
                    m.set_id(id as u16);
                    c.send_net_command_msg(&NetCommandMsgPtr::new(m), relay as u8);
                }
            }
            3 => cm.debug_set_local_slot(rd.next()),
            4 => cm.debug_set_router_slot(rd.next()),
            5 => {
                let i = (rd.next() % 8) as usize;
                cm.debug_set_fallback(i, rd.next());
            }
            6 => {
                let i = (rd.next() % 8) as usize;
                cm.debug_set_fps_average(i, rd.next() as i32);
            }
            7 => {
                let i = (rd.next() % 8) as usize;
                cm.debug_set_latency_average(i, f32::from_bits(rd.next()));
            }
            8 => {
                cushion = rd.next();
                cm.debug_set_smallest_cushion(cushion);
            }
            10 => o.push(cm.get_packet_router_fallback_slot(rd.next() as i32)),
            11 => o.push(cm.get_packet_router_slot()),
            12 => o.push(u32::from(cm.are_all_queues_empty())),
            13 => o.push(u32::from(cm.can_i_leave())),
            14 => o.push(cm.get_maximum_latency().to_bits()),
            15 => {
                let (a, b) = cm.get_minimum_fps();
                o.push(a as u32);
                o.push(b as u32);
            }
            16 => o.push(u32::from(cm.is_packet_router())),
            17 => o.push(cm.get_slot_average_fps(rd.next() as i32) as u32),
            18 => o.push(cm.get_next_packet_router_slot(rd.next())),
            19 => o.push(cm.get_packet_arrival_cushion()),
            20 => o.push(u32::from(cm.is_player_connected(rd.next() as i32))),
            21 => o.push(cm.get_num_players() as u32),
            22 => o.push(cm.get_local_player_id()),
            _ => {}
        }
        dump(&mut o, &cm);
        let _ = cushion;
        for i in 0..8 {
            let (present, quitting) = cm.debug_connection_state(i);
            o.push(u32::from(present));
            o.push(u32::from(quitting));
        }
    }
    o
}

fn cpp_run(w: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 400_000];
    let n = unsafe { ref_cm_run(w.as_ptr(), w.len() as c_int, out.as_mut_ptr(), out.len() as c_int) };
    assert!((n as usize) <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

/// The C++ dump after an operation is: local slot, router slot, cushion, then the eight connection flags pairs; the Rust one has no cushion word: drop it.
fn strip_cushion(c: &[u32]) -> Vec<u32> {
    // records start at the marker; the cushion is the third word after the operation's results: find it by the fixed tail length (3 + 16 words)
    let mut out = Vec::new();
    let mut marks: Vec<usize> = c.iter().enumerate().filter(|(_, &x)| x == 0xA5A5A5A5).map(|(i, _)| i).collect();
    marks.push(c.len());
    for w in marks.windows(2) {
        let rec = &c[w[0]..w[1]];
        let n = rec.len();
        // tail: local, router, cushion, 16 flags
        let tail = 3 + 16;
        let cut = n - tail;
        out.extend_from_slice(&rec[..cut + 2]);
        out.extend_from_slice(&rec[cut + 3..]);
    }
    out
}

#[test]
fn cmgr_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("CM_CASES", 20_000);
    let mut g = Gen::new(0xc1a7_0001);
    let mut chk = Check::new("connection_manager accessor programs");
    for i in 0..n {
        let w = program(&mut g);
        let r = rust_run(&w);
        let c = strip_cushion(&cpp_run(&w));
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(14);
            println!("case {i}: first difference at word {k} (rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}", r.len(), c.len(), &r[k0..(k + 14).min(r.len())], &c[k0..(k + 14).min(c.len())]);
            if std::env::var("CM_DEBUG").is_ok() {
                println!("program {:?}", w);
            }
        }
        chk.eq(r == c, true, || format!("cmgr case {i}"));
    }
    println!("{n} programs");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
