//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 8,000 operation sequences (test: tests/differential_draft_netlogic.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/FrameMetrics.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/FrameMetrics.cpp
//!
//! `timeGetTime()` is passed in as milliseconds, and `TheDisplay->getAverageFPS()` as the FPS value, so the
//! metrics are deterministic functions of their inputs. The history lengths are `TheGlobalData`'s
//! `m_networkFPSHistoryLength`, `m_networkLatencyHistoryLength` and `m_networkCushionHistoryLength`.

use crate::common::global_data::GlobalData;
use crate::logic::modules::plant;

/// `class FrameMetrics`.
#[derive(Debug)]
pub struct FrameMetrics {
    /// `m_fpsList[m_networkFPSHistoryLength]`.
    fps_list: Vec<f32>,
    /// `m_lastFpsTimeThing` (`time_t`, ms).
    last_fps_time_thing: i64,
    /// `m_fpsListIndex`.
    fps_list_index: i32,
    /// `m_averageFps`.
    average_fps: f32,
    /// `m_latencyList[m_networkLatencyHistoryLength]`.
    latency_list: Vec<f32>,
    /// `m_pendingLatencies[MAX_FRAMES_AHEAD]`.
    pending_latencies: Vec<i64>,
    /// `m_averageLatency`.
    average_latency: f32,
    /// `m_cushionIndex`.
    cushion_index: u32,
    /// `m_minimumCushion`.
    minimum_cushion: i32,
    /// `MAX_FRAMES_AHEAD`.
    max_frames_ahead: i32,
    fps_history_length: i32,
    latency_history_length: i32,
    cushion_history_length: u32,
}

impl FrameMetrics {
    /// `FrameMetrics::FrameMetrics()` (FrameMetrics.cpp:30): the lists are allocated uninitialised (zero here);
    /// `init()` fills them.
    pub fn new(max_frames_ahead: i32, g: &GlobalData) -> FrameMetrics {
        FrameMetrics {
            fps_list: vec![0.0; g.network_fps_history_length as usize],
            last_fps_time_thing: 0,
            fps_list_index: 0,
            average_fps: 0.0,
            latency_list: vec![0.0; g.network_latency_history_length as usize],
            pending_latencies: vec![0; max_frames_ahead.max(0) as usize],
            average_latency: 0.0,
            cushion_index: 0,
            minimum_cushion: 0,
            max_frames_ahead,
            fps_history_length: g.network_fps_history_length as i32,
            latency_history_length: g.network_latency_history_length as i32,
            cushion_history_length: g.network_cushion_history_length,
        }
    }

    /// `FrameMetrics::init` (FrameMetrics.cpp:55).
    pub fn init(&mut self) {
        self.average_fps = 30.0;
        self.average_latency = 0.2f32;
        self.minimum_cushion = -1;
        for v in self.fps_list.iter_mut() {
            *v = 30.0;
        }
        self.fps_list_index = 0;
        for v in self.latency_list.iter_mut() {
            *v = if plant(11878) { 0.0 } else { 0.2f32 };
        }
        self.cushion_index = 0;
    }

    /// `reset`: `init()`.
    pub fn reset(&mut self) {
        self.init();
    }

    /// `FrameMetrics::doPerFrameMetrics` (FrameMetrics.cpp:75): once a second replaces the oldest fps sample in
    /// the running average; records when this frame's info packet left.
    pub fn do_per_frame_metrics(&mut self, frame: u32, cur_time_ms: i64, display_average_fps: f32) {
        if cur_time_ms - self.last_fps_time_thing >= if plant(11871) { 1001 } else { 1000 } {
            let n = if plant(11873) { self.latency_history_length } else { self.fps_history_length } as f32;
            if plant(11872) {
                self.average_fps += display_average_fps / n;
                self.average_fps -= self.fps_list[self.fps_list_index as usize] / n;
                self.fps_list[self.fps_list_index as usize] = display_average_fps;
            } else {
                self.average_fps -= self.fps_list[self.fps_list_index as usize] / n; // subtract out the old value
                self.fps_list[self.fps_list_index as usize] = display_average_fps;
                self.average_fps += self.fps_list[self.fps_list_index as usize] / n; // add the new value
            }
            self.fps_list_index += 1;
            self.fps_list_index %= self.fps_history_length;
            if !plant(11879) {
                self.last_fps_time_thing = cur_time_ms;
            }
        }
        let pending_latencies_index = (frame % self.max_frames_ahead as u32) as usize;
        self.pending_latencies[pending_latencies_index] = cur_time_ms;
    }

    /// `FrameMetrics::processLatencyResponse` (FrameMetrics.cpp:97): the round trip of the frame info packet,
    /// in seconds, replaces its slot in the running average.
    pub fn process_latency_response(&mut self, frame: u32, cur_time_ms: i64) {
        let pending_index = (frame % if plant(11874) { self.latency_history_length as u32 } else { self.max_frames_ahead as u32 }) as usize;
        let pending_index = pending_index % self.pending_latencies.len().max(1);
        let time_diff = cur_time_ms - self.pending_latencies[pending_index];
        let latency_list_index = (frame % self.latency_history_length as u32) as usize;
        let n = self.latency_history_length as f32;
        if !plant(11880) {
            self.average_latency -= self.latency_list[latency_list_index] / n;
        }
        self.latency_list[latency_list_index] = if plant(11875) { (time_diff / 1000) as f32 } else { (time_diff as f32) / 1000.0f32 }; // milliseconds to seconds
        self.average_latency += self.latency_list[latency_list_index] / n;
        // (every 16th frame upstream logs the average)
    }

    /// `FrameMetrics::addCushion` (FrameMetrics.cpp:117).
    pub fn add_cushion(&mut self, cushion: i32) {
        self.cushion_index = self.cushion_index.wrapping_add(1);
        self.cushion_index %= self.cushion_history_length;
        if self.cushion_index == 0 && !plant(11877) {
            self.minimum_cushion = -1;
        }
        if (if plant(11876) { cushion > self.minimum_cushion } else { cushion < self.minimum_cushion }) || self.minimum_cushion == -1 {
            self.minimum_cushion = cushion;
        }
    }

    /// `getAverageFPS`: `(Int)m_averageFps`.
    pub fn get_average_fps(&self) -> i32 {
        if plant(11881) { (self.average_fps + 0.5) as i32 } else { self.average_fps as i32 }
    }

    /// `getAverageLatency`.
    pub fn get_average_latency(&self) -> f32 {
        self.average_latency
    }

    /// `getMinimumCushion`.
    pub fn get_minimum_cushion(&self) -> i32 {
        self.minimum_cushion
    }

    /// Test support, not upstream (`FrameMetrics` keeps these members protected, the differential test reads them as the wrapper's
    /// `#define protected public` does): the average fps and latency, the minimum cushion, the fps index, the last fps time (low
    /// 32 bits), the cushion index, the running average fps, the fps list, the latency list and the pending latencies (low and high
    /// words), all as the words `op_metrics` writes (tests/differential_draft_netlogic.rs).
    #[doc(hidden)]
    pub fn dump_state(&self, o: &mut Vec<u32>) {
        o.push(self.get_average_fps() as u32);
        o.push(self.get_average_latency().to_bits());
        o.push(self.get_minimum_cushion() as u32);
        o.push(self.fps_list_index as u32);
        o.push(self.last_fps_time_thing as u32);
        o.push(self.cushion_index);
        o.push(self.average_fps.to_bits());
        for v in &self.fps_list {
            o.push(v.to_bits());
        }
        for v in &self.latency_list {
            o.push(v.to_bits());
        }
        for v in &self.pending_latencies {
            o.push(*v as u32);
            o.push((*v as u64 >> 32) as u32);
        }
    }
}
