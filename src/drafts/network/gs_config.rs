//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/GSConfig.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/GSConfig.cpp
//!
//! `GameSpyConfig`: the text blob the (defunct) GameSpy config server sent, parsed into ping servers and timings,
//! quick-match maps and bot id, VIP ids, manglers, NAT timeouts, custom-match flags, and the text left over for the
//! ladder list. The MapCache/GameState lookups of the `<QMMaps>` section are the `qm_map_lookup` closure of
//! [`GameSpyConfig::new`].
//!
//! # Notes
//!
//! * `m_natManglerRetryInterval` etc. are `time_t` upstream; `i64` here.
//! * The per-key "missing val" debug logs of the NAT and Custom blocks print an uninitialised `val` upstream; they
//!   are dropped.
//! * `getPointsForRank` returns a hard-coded table (0, 5, 10, 20, 50, 100, 200, 500, 1000, 2000); the config text
//!   has no section for it.

use std::collections::BTreeSet;

use crate::common::ascii_string::AsciiString;

use super::persistent_storage_thread::atoi;

/// `MAX_RANKS` (RankPointValue.h).
pub const MAX_RANKS: usize = 10;

/// `class GameSpyConfig`.
#[derive(Clone, Debug)]
pub struct GameSpyConfig {
    /// `m_pingServers`.
    ping_servers: Vec<AsciiString>,
    /// `m_pingReps`.
    ping_reps: i32,
    /// `m_pingTimeout`.
    ping_timeout: i32,
    /// `m_pingCutoffGood`.
    ping_cutoff_good: i32,
    /// `m_pingCutoffBad`.
    ping_cutoff_bad: i32,
    /// `m_natRetryInterval`.
    nat_retry_interval: i32,
    /// `m_natMaxManglerRetries`.
    nat_max_mangler_retries: i32,
    /// `m_natManglerRetryInterval`.
    nat_mangler_retry_interval: i64,
    /// `m_natKeepaliveInterval`.
    nat_keepalive_interval: i64,
    /// `m_natPortTimeout`.
    nat_port_timeout: i64,
    /// `m_natRoundTimeout`.
    nat_round_timeout: i64,
    /// `m_manglerHosts`.
    mangler_hosts: Vec<AsciiString>,
    /// `m_manglerPorts`.
    mangler_ports: Vec<u16>,
    /// `m_qmMaps`.
    qm_maps: Vec<AsciiString>,
    /// `m_qmBotID`.
    qm_bot_id: i32,
    /// `m_qmChannel`.
    qm_channel: i32,
    /// `m_restrictGamesToLobby`.
    restrict_games_to_lobby: bool,
    /// `m_vip`.
    vip: BTreeSet<i32>,
    /// `m_rankPoints`.
    rank_points: [i32; MAX_RANKS],
    /// `m_leftoverConfig`.
    leftover_config: AsciiString,
}

/// The section flags of the constructor (`SectionChecker` over the eight `in*` bools).
#[derive(Clone, Copy, Debug, Default)]
struct Sections {
    ping_servers: bool,
    ping_duration: bool,
    qm_maps: bool,
    qm_bot: bool,
    manglers: bool,
    vip: bool,
    nat: bool,
    custom: bool,
}

impl Sections {
    /// `SectionChecker::isInSection`: any flag set.
    fn is_in_section(&self) -> bool {
        self.ping_servers || self.ping_duration || self.qm_maps || self.qm_bot || self.manglers || self.vip || self.nat || self.custom
    }
}

/// `key == "reps"`-style tokenising of a `key = value` line: `line.nextToken(&key, " =")` then `nextToken(&val, " =")`.
fn key_val(line: &mut AsciiString) -> Option<(AsciiString, Option<AsciiString>)> {
    let mut key = AsciiString::default();
    if !line.next_token(&mut key, Some(b" =")).unwrap_or(false) {
        return None;
    }
    let mut val = AsciiString::default();
    let got = line.next_token(&mut val, Some(b" =")).unwrap_or(false);
    Some((key, if got { Some(val) } else { None }))
}

impl GameSpyConfig {
    /// `GameSpyConfigInterface::create(config)` / `GameSpyConfig::GameSpyConfig` (GSConfig.cpp): the defaults
    /// (NAT retry 1000, mangler retries 25, mangler interval 300, keepalive 15000, port timeout 10000, round
    /// timeout 10000, ping reps 1, timeout 1000, cutoffs 300/600, no restriction, bot 0, channel 0), then the
    /// config text one `\n`-separated line at a time (trailing `\r` removed, trimmed, empties skipped); a section
    /// opens only outside another one. `qm_map_lookup(lowercase line)` stands for
    /// `TheMapCache->getMapDir()` + `"\\<line>\\<line>.map"` -> portable -> real -> lowercased, returning it only if
    /// `TheMapCache->findMap` finds the map.
    pub fn new(config: &[u8], qm_map_lookup: &dyn Fn(&[u8]) -> Option<Vec<u8>>) -> GameSpyConfig {
        let mut c = GameSpyConfig {
            ping_servers: Vec::new(),
            ping_reps: 1,
            ping_timeout: 1000,
            ping_cutoff_good: 300,
            ping_cutoff_bad: 600,
            nat_retry_interval: 1000,
            nat_max_mangler_retries: 25,
            nat_mangler_retry_interval: 300,
            nat_keepalive_interval: 15000,
            nat_port_timeout: 10000,
            nat_round_timeout: 10000,
            mangler_hosts: Vec::new(),
            mangler_ports: Vec::new(),
            qm_maps: Vec::new(),
            qm_bot_id: 0,
            qm_channel: 0,
            restrict_games_to_lobby: false,
            vip: BTreeSet::new(),
            rank_points: [0, 5, 10, 20, 50, 100, 200, 500, 1000, 2000],
            leftover_config: AsciiString::default(),
        };

        let mut sec = Sections::default();
        let mut cfg = AsciiString::try_from_bytes(config).unwrap_or_default();
        let mut line = AsciiString::default();
        while cfg.next_token(&mut line, Some(b"\n")).unwrap_or(false) {
            let len = line.get_length();
            if len > 0 && line.try_get_char_at(len - 1).unwrap_or(0) == b'\r' {
                let _ = line.remove_last_char(); // there is a trailing '\r'
            }
            let _ = line.trim();
            if line.is_empty() {
                continue;
            }
            let l = line.str().to_vec();
            let not_in = !sec.is_in_section();

            if not_in && l == b"<PingServers>" {
                sec.ping_servers = true;
            } else if sec.ping_servers && l == b"</PingServers>" {
                sec.ping_servers = false;
            } else if not_in && l == b"<PingDuration>" {
                sec.ping_duration = true;
            } else if sec.ping_duration && l == b"</PingDuration>" {
                sec.ping_duration = false;
            } else if not_in && l == b"<QMMaps>" {
                sec.qm_maps = true;
            } else if sec.qm_maps && l == b"</QMMaps>" {
                sec.qm_maps = false;
            } else if not_in && l == b"<Manglers>" {
                sec.manglers = true;
            } else if sec.manglers && l == b"</Manglers>" {
                sec.manglers = false;
            } else if not_in && l == b"<QMBot>" {
                sec.qm_bot = true;
            } else if sec.qm_bot && l == b"</QMBot>" {
                sec.qm_bot = false;
            } else if not_in && l == b"<VIP>" {
                sec.vip = true;
            } else if sec.vip && l == b"</VIP>" {
                sec.vip = false;
            } else if not_in && l == b"<NAT>" {
                sec.nat = true;
            } else if sec.nat && l == b"</NAT>" {
                sec.nat = false;
            } else if not_in && l == b"<Custom>" {
                sec.custom = true;
            } else if sec.custom && l == b"</Custom>" {
                sec.custom = false;
            } else if sec.vip {
                let _ = line.to_lower();
                if line.get_length() != 0 {
                    let val = atoi(line.str());
                    if val > 0 {
                        c.vip.insert(val);
                    }
                }
            } else if sec.ping_servers {
                let _ = line.to_lower();
                c.ping_servers.push(line.clone());
            } else if sec.ping_duration {
                let _ = line.to_lower();
                if let Some((key, val)) = key_val(&mut line) {
                    if let Some(v) = val {
                        match key.str() {
                            b"reps" => c.ping_reps = atoi(v.str()),
                            b"timeout" => c.ping_timeout = atoi(v.str()),
                            b"low" => c.ping_cutoff_good = atoi(v.str()),
                            b"med" => c.ping_cutoff_bad = atoi(v.str()),
                            _ => {}
                        }
                    }
                }
            } else if sec.manglers {
                let _ = line.trim();
                let _ = line.to_lower();
                let mut host_str = AsciiString::default();
                let mut port_str = AsciiString::default();
                let _ = line.next_token(&mut host_str, Some(b":"));
                let _ = line.next_token(&mut port_str, Some(b":\n\r"));
                if host_str.is_not_empty() && port_str.is_not_empty() {
                    c.mangler_hosts.push(host_str);
                    c.mangler_ports.push(atoi(port_str.str()) as u16);
                }
            } else if sec.qm_maps {
                let _ = line.to_lower();
                if let Some(map_name) = qm_map_lookup(line.str()) {
                    c.qm_maps.push(AsciiString::try_from_bytes(&map_name).unwrap_or_default());
                }
            } else if sec.qm_bot {
                let _ = line.to_lower();
                if let Some((key, val)) = key_val(&mut line) {
                    if key.str() == b"id" {
                        if let Some(v) = val {
                            c.qm_bot_id = atoi(v.str());
                        }
                    }
                }
            } else if sec.nat {
                let _ = line.to_lower();
                if let Some((key, val)) = key_val(&mut line) {
                    if let Some(v) = val {
                        let n = atoi(v.str());
                        match key.str() {
                            b"retryinterval" => c.nat_retry_interval = n,
                            b"manglerretries" => c.nat_max_mangler_retries = n,
                            b"manglerinterval" => c.nat_mangler_retry_interval = n as i64,
                            b"keepaliveinterval" => c.nat_keepalive_interval = n as i64,
                            b"porttimeout" => c.nat_port_timeout = n as i64,
                            b"roundtimeout" => c.nat_round_timeout = n as i64,
                            _ => {} // Unknown key in NAT block
                        }
                    }
                }
            } else if sec.custom {
                let _ = line.to_lower();
                let mut key = AsciiString::default();
                let mut val = AsciiString::default();
                if line.next_token(&mut key, Some(b" =")).unwrap_or(false) && line.next_token(&mut val, Some(b" =")).unwrap_or(false) {
                    if key.str() == b"restricted" {
                        c.restrict_games_to_lobby = atoi(val.str()) != 0;
                    }
                    // else Unknown key in Custom block
                }
            } else {
                let _ = c.leftover_config.concat(&l);
                let _ = c.leftover_config.concat_char(b'\n');
            }
        }
        c
    }

    /// `getPingServers`.
    pub fn get_ping_servers(&self) -> Vec<AsciiString> {
        self.ping_servers.clone()
    }
    /// `getNumPingRepetitions`.
    pub fn get_num_ping_repetitions(&self) -> i32 {
        self.ping_reps
    }
    /// `getPingTimeoutInMs`.
    pub fn get_ping_timeout_in_ms(&self) -> i32 {
        self.ping_timeout
    }
    /// `getPingCutoffGood`.
    pub fn get_ping_cutoff_good(&self) -> i32 {
        self.ping_cutoff_good
    }
    /// `getPingCutoffBad`.
    pub fn get_ping_cutoff_bad(&self) -> i32 {
        self.ping_cutoff_bad
    }
    /// `getQMMaps`.
    pub fn get_qm_maps(&self) -> Vec<AsciiString> {
        self.qm_maps.clone()
    }
    /// `getQMBotID`.
    pub fn get_qm_bot_id(&self) -> i32 {
        self.qm_bot_id
    }
    /// `getQMChannel`.
    pub fn get_qm_channel(&self) -> i32 {
        self.qm_channel
    }
    /// `setQMChannel`.
    pub fn set_qm_channel(&mut self, channel: i32) {
        self.qm_channel = channel;
    }
    /// `getPointsForRank`: the rank clamped to `0..MAX_RANKS`.
    pub fn get_points_for_rank(&self, rank: i32) -> i32 {
        let mut r = rank;
        if r >= MAX_RANKS as i32 {
            r = MAX_RANKS as i32 - 1;
        }
        if r < 0 {
            r = 0;
        }
        self.rank_points[r as usize]
    }
    /// `isPlayerVIP`.
    pub fn is_player_vip(&self, id: i32) -> bool {
        self.vip.contains(&id)
    }
    /// `getManglerLocation(index, host, port)`: None if `index` is out of range.
    pub fn get_mangler_location(&self, index: i32) -> Option<(AsciiString, u16)> {
        if index < 0 || index as usize >= self.mangler_hosts.len() {
            return None;
        }
        Some((self.mangler_hosts[index as usize].clone(), self.mangler_ports[index as usize]))
    }
    /// `getLeftoverConfig`.
    pub fn get_leftover_config(&self) -> AsciiString {
        self.leftover_config.clone()
    }
    /// `getTimeBetweenRetries`.
    pub fn get_time_between_retries(&self) -> i32 {
        self.nat_retry_interval
    }
    /// `getMaxManglerRetries`.
    pub fn get_max_mangler_retries(&self) -> i32 {
        self.nat_max_mangler_retries
    }
    /// `getRetryInterval`.
    pub fn get_retry_interval(&self) -> i64 {
        self.nat_mangler_retry_interval
    }
    /// `getKeepaliveInterval`.
    pub fn get_keepalive_interval(&self) -> i64 {
        self.nat_keepalive_interval
    }
    /// `getPortTimeout`.
    pub fn get_port_timeout(&self) -> i64 {
        self.nat_port_timeout
    }
    /// `getRoundTimeout`.
    pub fn get_round_timeout(&self) -> i64 {
        self.nat_round_timeout
    }
    /// `restrictGamesToLobby`.
    pub fn restrict_games_to_lobby(&self) -> bool {
        self.restrict_games_to_lobby
    }
}
