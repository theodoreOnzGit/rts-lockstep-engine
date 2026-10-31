#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp (Object::updateObjValuesFromMapProperties)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `Object::updateObjValuesFromMapProperties` (`logic::world::object_map_props`) against the C++, layer 8 batch O1.
//!
//! # Method
//!
//! The C++ side is the function cut verbatim from `Object.cpp` by build.rs and compiled by
//! `differential/wrap_object_map_props.cpp` on stand-ins: a `Dict` keyed by name, the object (its body, experience
//! tracker, AI and locomotor, selectability, script status, ranges, upgrades), the upgrade center, the drawable and the
//! audio system. Every call that acts logs a line; at the end the two ranges are logged. The Rust side runs
//! `update_obj_values_from_map_properties` on a `MapPropsHost` that answers and logs as the stand-ins do, and runs the
//! client's `apply_ambient_sound_properties` on the drawable's ambient-sound event with a client that does the same.
//!
//! A program draws the object (which of body, trainable tracker, AI, locomotor, drawable it has; whether it starts
//! selectable; the drawable's base ambient sound and whether its sound is enabled from script), which of three upgrades
//! and two audio events exist, and up to 24 dictionary entries over the 31 keys the function reads (four
//! `objectGrantUpgrade<n>`), each of a random type (a wrong type reads as absent on both sides) and a value from a small
//! range around the boundaries the function tests (negative ranges and HPs, stopping distance near 0.5, veterancy -1..4,
//! time and weather 0..3, empty and unknown names).
//!
//! A veterancy outside `0..4` on a trainable object is undefined upstream (`setVeterancyLevel` indexes the experience
//! table out of bounds): the Rust side stops there and the program is not compared.
//!
//! # Results (2026-10-31, `OBJMP_CASES=20000`, the default)
//!
//! 20,000 programs, 19,073 compared (927 cut at an out-of-range veterancy on a trainable object), 75,181 log lines:
//! **0 mismatches**.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 6 of 6 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 4041 | veterancy set on an object that is not trainable | 1,898 programs |
//! | 4042 | a negative `objectMaxHPs` accepted | 679 programs |
//! | 4043 | `setSelectable` called when the value is unchanged | 2,707 programs |
//! | 4044 | a negative shroud-clearing distance kept | 1,297 programs |
//! | 4045 | the upgrade loop stops at an unknown upgrade | 280 programs |
//! | 4046 | the loop count overridden on a sound that does not loop | 115 programs |

use rts_lockstep_engine::common::dict::Dict;
use rts_lockstep_engine::common::game_common::VeterancyLevel;
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::object_events::AmbientSoundProperties;
use rts_lockstep_engine::logic::world::object_map_props::*;
use rts_lockstep_engine::logic::world::ObjectId;
use std::ffi::c_char;

struct Gen(u64);
impl Gen {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

const KEYS: [&str; 31] = [
    "objectName",
    "objectMaxHPs",
    "objectInitialHealth",
    "objectVeterancy",
    "objectAggressiveness",
    "objectRecruitableAI",
    "objectSelectable",
    "objectStoppingDistance",
    "objectEnabled",
    "objectPowered",
    "objectIndestructible",
    "objectUnsellable",
    "objectTargetable",
    "objectVisualRange",
    "objectShroudClearingDistance",
    "objectGrantUpgrade0",
    "objectGrantUpgrade1",
    "objectGrantUpgrade2",
    "objectGrantUpgrade3",
    "objectTime",
    "objectWeather",
    "objectSoundAmbient",
    "objectSoundAmbientCustomized",
    "objectSoundAmbientEnabled",
    "objectSoundAmbientLooping",
    "objectSoundAmbientLoopCount",
    "objectSoundAmbientMinVolume",
    "objectSoundAmbientVolume",
    "objectSoundAmbientMinRange",
    "objectSoundAmbientMaxRange",
    "objectSoundAmbientPriority",
];
/// The type each key is read as (0 bool, 1 int, 2 real, 3 string).
const KEY_TYPES: [u32; 31] = [3, 1, 1, 1, 1, 0, 0, 2, 0, 0, 0, 0, 0, 1, 1, 3, 3, 3, 3, 1, 1, 3, 0, 0, 0, 1, 2, 2, 2, 2, 1];
const STRINGS: [&[u8]; 9] = [b"", b"UpA", b"UpB", b"UpC", b"UpX", b"SndA", b"SndB", b"SndX", b"Bob"];

/// The audio table entry for audio event `i` (as `audio_entry` in the wrapper).
fn audio_entry(i: i32, name: &[u8]) -> AmbientAudioInfo {
    AmbientAudioInfo {
        name: name.to_vec(),
        control: (i & 1) as u32,
        loop_count: if i >= 1 { 0 } else { 3 },
        volume: 0.25 * (i + 1) as f32,
        min_volume: 0.1 * (i + 1) as f32,
        min_distance: 10.0 * (i + 1) as f32,
        max_distance: 100.0 * (i + 1) as f32,
        priority: i + 1,
        overridden: 0,
    }
}

/// The object, the stores and the log: the Rust side's stand-ins.
struct Host {
    names: NameKeyGenerator,
    flags: u32,
    selectable: bool,
    vision: f32,
    shroud: f32,
    upgrades: u32,
    audio: u32,
    base: AmbientAudioInfo,
    log: Vec<String>,
}

impl Host {
    fn flag(&self, b: u32) -> bool {
        (self.flags >> b) & 1 != 0
    }
}

impl MapPropsHost for Host {
    fn mp_names(&mut self) -> &mut NameKeyGenerator {
        &mut self.names
    }
    fn mp_set_name(&mut self, _id: ObjectId, name: Vec<u8>) {
        self.log.push(format!("name [{}]", String::from_utf8_lossy(&name)));
    }
    fn mp_has_body(&self, _id: ObjectId) -> bool {
        self.flag(0)
    }
    fn mp_body_set_max_health(&mut self, _id: ObjectId, m: f32) {
        self.log.push(format!("body maxhealth {:08x}", m.to_bits()));
    }
    fn mp_body_set_initial_health(&mut self, _id: ObjectId, p: i32) {
        self.log.push(format!("body initialhealth {p}"));
    }
    fn mp_body_set_indestructible(&mut self, _id: ObjectId, on: bool) {
        self.log.push(format!("body indestructible {}", i32::from(on)));
    }
    fn mp_is_trainable(&self, _id: ObjectId) -> bool {
        self.flag(1)
    }
    fn mp_set_veterancy_level(&mut self, _id: ObjectId, level: VeterancyLevel) {
        self.log.push(format!("veterancy {} 1", level as i32));
    }
    fn mp_has_ai(&self, _id: ObjectId) -> bool {
        self.flag(2)
    }
    fn mp_ai_set_attitude(&mut self, _id: ObjectId, a: i32) {
        self.log.push(format!("ai attitude {a}"));
    }
    fn mp_ai_set_is_recruitable(&mut self, _id: ObjectId, on: bool) {
        self.log.push(format!("ai recruitable {}", i32::from(on)));
    }
    fn mp_ai_has_cur_locomotor(&mut self, _id: ObjectId) -> bool {
        self.flag(3)
    }
    fn mp_ai_set_close_enough_dist(&mut self, _id: ObjectId, d: f32) {
        self.log.push(format!("loco closeenough {:08x}", d.to_bits()));
    }
    fn mp_is_selectable(&self, _id: ObjectId) -> bool {
        self.selectable
    }
    fn mp_set_selectable(&mut self, _id: ObjectId, on: bool) {
        self.log.push(format!("selectable {}", i32::from(on)));
        self.selectable = on;
    }
    fn mp_set_script_status(&mut self, _id: ObjectId, bit: u8, on: bool) {
        self.log.push(format!("scriptstatus {bit} {}", i32::from(on)));
    }
    fn mp_set_vision_range_field(&mut self, _id: ObjectId, r: f32) {
        self.vision = r;
    }
    fn mp_set_shroud_clearing_range_field(&mut self, _id: ObjectId, r: f32) {
        self.shroud = r;
    }
    fn mp_find_upgrade(&mut self, name: &[u8]) -> Option<UpgradeMaskType> {
        let i = (0..3).find(|&i| self.upgrades >> i & 1 != 0 && STRINGS[1 + i] == name)?;
        let mut m = UpgradeMaskType::new();
        m.set(i as i32, 1);
        Some(m)
    }
    fn mp_give_upgrade(&mut self, _id: ObjectId, mask: &UpgradeMaskType) {
        let i = (0..3).find(|&i| mask.test(i as i32)).expect("one of the three");
        self.log.push(format!("upgrade [{}]", String::from_utf8_lossy(STRINGS[1 + i])));
    }
    fn mp_has_drawable(&self, _id: ObjectId) -> bool {
        self.flag(5)
    }
    fn mp_drawable_time_or_weather(&mut self, _id: ObjectId, night: Option<bool>, snow: Option<bool>) {
        for (v, n) in [(night, "NIGHT"), (snow, "SNOW")] {
            match v {
                Some(true) => self.log.push(format!("setmc {n}")),
                Some(false) => self.log.push(format!("clearmc {n}")),
                None => {}
            }
        }
    }
    fn mp_drawable_ambient_sound(&mut self, id: ObjectId, properties: AmbientSoundProperties) {
        let mut client = Client { host: self, id };
        apply_ambient_sound_properties(&properties, &mut client);
    }
}

/// The drawable and the audio system for `apply_ambient_sound_properties`.
struct Client<'a> {
    host: &'a mut Host,
    id: ObjectId,
}

impl AmbientSoundClient for Client<'_> {
    fn set_custom_sound_ambient_off(&mut self) {
        self.host.log.push("soundoff".into());
    }
    fn find_audio_event_info(&mut self, name: &[u8]) -> Option<AmbientAudioInfo> {
        (0..2).find(|&i| self.host.audio >> i & 1 != 0 && STRINGS[5 + i] == name).map(|i| audio_entry(i as i32, STRINGS[5 + i]))
    }
    fn base_sound_ambient_info(&mut self) -> Option<AmbientAudioInfo> {
        self.host.flag(6).then(|| self.host.base.clone())
    }
    fn enable_ambient_sound_from_script(&mut self, on: bool) {
        self.host.log.push(format!("enable {}", i32::from(on)));
    }
    fn ambient_sound_enabled_from_script(&mut self) -> bool {
        self.host.flag(7)
    }
    fn install_custom_sound_ambient(&mut self, mut a: AmbientAudioInfo) {
        // Drawable::mangleCustomAudioName
        let mut name = format!(" CUSTOM {} ", self.id).into_bytes();
        name.extend_from_slice(&a.name);
        a.name = name;
        a.overridden |= override_bits::NAME;
        self.host.log.push(format!(
            "install [{}] {} {} {:08x} {:08x} {:08x} {:08x} {} {}",
            String::from_utf8_lossy(&a.name),
            a.control,
            a.loop_count,
            a.volume.to_bits(),
            a.min_volume.to_bits(),
            a.min_distance.to_bits(),
            a.max_distance.to_bits(),
            a.priority,
            a.overridden
        ));
    }
}

/// A value for an entry of key `k` read as type `t`.
fn value(g: &mut Gen, k: usize, t: u32) -> u32 {
    match t {
        0 => g.below(2),
        1 => {
            let v: i32 = match k {
                3 => g.below(6) as i32 - 1,      // veterancy -1..4
                19 | 20 => g.below(4) as i32,    // time and weather 0..3
                _ => g.below(400) as i32 - 100,
            };
            v as u32
        }
        2 => match g.below(4) {
            0 => 0.5f32.to_bits(),
            1 => 0.4999f32.to_bits(),
            _ => (g.below(4000) as f32 / 37.0 - 20.0).to_bits(),
        },
        _ => match k {
            15..=18 => [0u32, 1, 2, 3, 4][g.below(5) as usize],
            21 => [0u32, 5, 6, 7][g.below(4) as usize],
            _ => g.below(9),
        },
    }
}

#[test]
fn update_obj_values_from_map_properties_matches_the_cpp() {
    let cases: u32 = std::env::var("OBJMP_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x0b1e_c7a5_5eed);
    let (mut compared, mut undefined, mut lines, mut mismatches) = (0u32, 0u32, 0usize, 0u32);
    let mut out = vec![0u8; 1 << 16];
    for case in 0..cases {
        let flags = g.next() & 0xff;
        let base_control = g.below(2);
        let base_loop = if g.below(2) == 0 { 0 } else { g.below(5) };
        let base = [
            base_control,
            base_loop,
            (g.below(100) as f32 / 100.0).to_bits(),
            (g.below(100) as f32 / 200.0).to_bits(),
            (g.below(500) as f32).to_bits(),
            (g.below(2000) as f32).to_bits(),
            g.below(4),
        ];
        let upgrades = g.below(8);
        let audio = g.below(4);
        let n = g.below(25) as usize;
        let mut words: Vec<u32> = vec![flags];
        words.extend_from_slice(&base);
        words.push(upgrades);
        words.push(audio);
        words.push(n as u32);
        let mut names = NameKeyGenerator::new();
        let mut d = Dict::new();
        for _ in 0..n {
            let k = g.below(31) as usize;
            let t = if g.below(8) == 0 { g.below(4) } else { KEY_TYPES[k] };
            let v = value(&mut g, k, t);
            words.extend_from_slice(&[k as u32, t, v]);
            let key = names.name_to_key(KEYS[k].as_bytes());
            match t {
                0 => d.set_bool(key, v != 0).unwrap(),
                1 => d.set_int(key, v as i32).unwrap(),
                2 => d.set_real(key, f32::from_bits(v)).unwrap(),
                _ => d.set_ascii_string(key, STRINGS[(v % 9) as usize]).unwrap(),
            }
        }
        let base_info = AmbientAudioInfo {
            name: b"Base".to_vec(),
            control: base[0],
            loop_count: base[1] as i32,
            volume: f32::from_bits(base[2]),
            min_volume: f32::from_bits(base[3]),
            min_distance: f32::from_bits(base[4]),
            max_distance: f32::from_bits(base[5]),
            priority: base[6] as i32,
            overridden: 0,
        };
        let mut h = Host { names, flags, selectable: flags >> 4 & 1 != 0, vision: -1.0, shroud: -1.0, upgrades, audio, base: base_info, log: Vec::new() };
        if update_obj_values_from_map_properties(&mut h, 7, &d).is_err() {
            undefined += 1;
            continue;
        }
        h.log.push(format!("ranges {:08x} {:08x}", h.vision.to_bits(), h.shroud.to_bits()));
        let len = unsafe { cpp::ref_objmp_run(words.as_ptr(), words.len() as i32, out.as_mut_ptr() as *mut c_char, out.len() as i32) };
        let theirs = String::from_utf8_lossy(&out[..len as usize]).into_owned();
        let ours = h.log.join("\n") + "\n";
        compared += 1;
        lines += h.log.len();
        if ours != theirs {
            mismatches += 1;
            if mismatches <= 3 {
                eprintln!("case {case}: words {words:?}\n--- rust\n{ours}--- c++\n{theirs}");
            }
        }
    }
    eprintln!("objmp: {cases} programs, {compared} compared, {undefined} undefined, {lines} lines, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
