// SPDX-License-Identifier: GPL-3.0-only
//! Shared by the differential tests of the modules that mix an `UpgradeMux` in (`p1w12-sonnet`, update batch M on): the byte dump helpers, the dump of
//! a mux (`putMux` of `differential/wrap_module.cpp`), the generators of upgrade names, lists, masks and frames, and the xfer runner over a module slot.
#![allow(dead_code)]

use rts_lockstep_engine::common::module::ObjectId;
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::logic::module::upgrade_module::UpgradeMuxData;
use rts_lockstep_engine::logic::world::game_logic::World;
use std::collections::BTreeMap;

use crate::support::Gen;
use crate::up_common::pick;

/// The ids the driver gives the FX lists it meets, in order of first appearance (0 for none).
#[derive(Default)]
pub struct FxIds(BTreeMap<Vec<u8>, i32>);
impl FxIds {
    pub fn id(&mut self, name: &Option<Vec<u8>>) -> i32 {
        match name {
            None => 0,
            Some(n) => {
                let next = self.0.len() as i32 + 1;
                *self.0.entry(n.clone()).or_insert(next)
            }
        }
    }
}

pub fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
pub fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
pub fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}
pub fn put_names(v: &mut Vec<u8>, names: &[Vec<u8>]) {
    put_u32(v, names.len() as u32);
    for n in names {
        put_str(v, n);
    }
}
pub fn put_words(v: &mut Vec<u8>, w: [u64; 4]) {
    for x in w {
        v.extend_from_slice(&x.to_le_bytes());
    }
}
/// The mux data as the driver's `putMux` writes it.
pub fn put_mux(v: &mut Vec<u8>, m: &UpgradeMuxData, fx: &mut FxIds) {
    put_names(v, &m.trigger_upgrade_names);
    put_names(v, &m.activation_upgrade_names);
    put_names(v, &m.conflicting_upgrade_names);
    put_names(v, &m.removal_upgrade_names);
    put_i32(v, fx.id(&m.fx_list_upgrade));
    put_words(v, m.activation_mask().words());
    put_words(v, m.conflicting_mask().words());
    v.push(u8::from(m.requires_all_triggers));
}

/// A reader over a dump.
pub struct Reader<'a> {
    pub b: &'a [u8],
    pub p: usize,
}
impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Reader { b, p: 0 }
    }
    pub fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.b[self.p..self.p + 4].try_into().unwrap());
        self.p += 4;
        v
    }
    pub fn u64(&mut self) -> u64 {
        let v = u64::from_le_bytes(self.b[self.p..self.p + 8].try_into().unwrap());
        self.p += 8;
        v
    }
    pub fn str(&mut self) -> String {
        let n = u32::from_le_bytes(self.b[self.p..self.p + 4].try_into().unwrap()) as usize;
        self.p += 4;
        let s = String::from_utf8_lossy(&self.b[self.p..self.p + n]).into_owned();
        self.p += n;
        s
    }
    pub fn at_end(&self) -> bool {
        self.p >= self.b.len()
    }
    /// The mux dump, as four lines of text.
    pub fn mux(&mut self, out: &mut Vec<String>) {
        for what in ["trigger", "activation", "conflicting", "removal"] {
            let n = self.u32();
            let names: Vec<String> = (0..n).map(|_| self.str()).collect();
            out.push(format!("mux {what} {names:?}"));
        }
        let fx = self.u32() as i32;
        let act: Vec<u64> = (0..4).map(|_| self.u64()).collect();
        let con: Vec<u64> = (0..4).map(|_| self.u64()).collect();
        let all = self.b[self.p];
        self.p += 1;
        out.push(format!("mux fx {fx} act {act:x?} con {con:x?} all {all}"));
    }
    /// One op's xfer part: the status, the bytes (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler) and the CRC.
    pub fn xfer(&mut self) -> String {
        self.xfer_after_versions(5)
    }

    /// The byte count of the xfer record at the reader (its status word is first), without reading it.
    pub fn peek_xfer_len(&self) -> usize {
        let mut c = Reader { b: self.b, p: self.p };
        let _ = c.u32();
        c.u32() as usize
    }

    /// [`Self::xfer`] for a module whose scheduling word follows `versions` version bytes (a derived class adds one, batch R46).
    pub fn xfer_after_versions(&mut self, versions: usize) -> String {
        let xst = self.u32() as i32;
        let blen = self.u32();
        let mut bytes: Vec<u32> = (0..blen).map(|_| self.u32()).collect();
        if bytes.len() >= versions + 4 {
            // the scheduling half's frame-and-phase word (after the versions) is the sleepy list's
            bytes[versions..versions + 4].iter_mut().for_each(|b| *b = 0);
        }
        let crc = self.u32();
        format!("xfer {xst} bytes {bytes:?} crc {crc:#x}")
    }
}

pub fn mask_from(w: &[u32]) -> UpgradeMaskType {
    let mut m = UpgradeMaskType::default();
    for i in 0..128 {
        if w[i / 32] >> (i % 32) & 1 != 0 {
            m.set(i as i32, 1);
        }
    }
    m
}

pub fn gen_name(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    if clean && !upgrades.is_empty() {
        return upgrades[g.below(upgrades.len() as u32) as usize].clone();
    }
    match g.below(100) {
        0..=64 if !upgrades.is_empty() => upgrades[g.below(upgrades.len() as u32) as usize].clone(),
        65..=74 if !upgrades.is_empty() => upgrades[g.below(upgrades.len() as u32) as usize].to_lowercase(),
        75..=76 => format!("Unknown{}", g.below(3)),
        77 => "Up9".to_string(),
        78..=79 => pick(g, &["None", "none", "NONE"]).to_string(),
        _ if !upgrades.is_empty() => upgrades[0].clone(),
        _ => format!("Unknown{}", g.below(3)),
    }
}

pub fn gen_list(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    let n = 1 + g.below(3);
    (0..n).map(|_| gen_name(g, upgrades, clean)).collect::<Vec<_>>().join(" ")
}

/// The five fields of the mux, each present with the probability the module's tests have always used.
pub fn gen_mux_lines(g: &mut Gen, upgrades: &[String], clean: bool, lines: &mut Vec<String>) {
    use crate::up_common::{gen_bool, gen_sep};
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            lines.push(format!("{name}{sep}{value}"));
        }
    };
    let v = gen_list(g, upgrades, clean);
    field(g, 70, "TriggeredBy", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 25, "ConflictsWith", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 25, "RemovesUpgrades", v);
    let v = pick(g, &["FX_Up", "FX_Other", "None"]).to_string();
    field(g, 30, "FXListUpgrade", v);
    let v = gen_bool(g, clean);
    field(g, 25, "RequiresAllTriggers", v);
}

pub fn gen_mask(g: &mut Gen, n_up: usize, at_least: u32) -> [u32; 4] {
    let mut w = [0u32; 4];
    if n_up > 0 {
        // a third of the masks name most of the upgrades, so that a TriggeredBy list is met more often than by chance
        let extra = if g.below(3) == 0 { g.below(n_up as u32 + 1) } else { g.below(3) };
        for _ in 0..at_least + extra {
            let b = g.below(n_up as u32) as usize;
            w[b / 32] |= 1 << (b % 32);
        }
    }
    w
}

pub fn gen_frame(g: &mut Gen) -> u32 {
    match g.below(12) {
        0..=6 => g.below(400),
        7..=8 => g.below(10_000),
        9 => g.below(2_000_000),
        10 => u32::MAX - g.below(40),
        _ => 0,
    }
}

/// Runs an xfer of module slot `idx` of object `me`: `how` 0 saves (to the file `file`), 1 loads `saved` (cut to 512 bytes), 2 computes the CRC.
pub fn run_xfer(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8], file: &str) -> (i32, Vec<u8>, u32) {
    run_xfer_cut(w, me, idx, how, saved, file, 512)
}

/// [`run_xfer`] with the load cut to `cut` bytes, for a driver whose load buffer is larger (batch R65's, 1,024 bytes).
pub fn run_xfer_cut(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8], file: &str, cut: usize) -> (i32, Vec<u8>, u32) {
    fn status<T>(r: Result<T, rts_lockstep_engine::common::xfer::XferStatus>) -> i32 {
        match r {
            Ok(_) => 0,
            Err(e) => e as i32,
        }
    }
    match how {
        0 => {
            let mut x = XferSave::new();
            let st = match x.open(file.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, x.bytes().to_vec(), 0)
        }
        1 => {
            let mut b = saved.to_vec();
            b.truncate(cut);
            let mut x = XferLoad::new(b);
            let st = match x.open(file.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), 0)
        }
        _ => {
            let mut x = XferCrc::new_without_nan_guard();
            let st = match x.open(b"crc") {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), x.get_crc())
        }
    }
}
