// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d5) of the drafts `src/drafts/ww3d/{static_sort_list, shader_presets, w3dexclusionlist,
//! matpass, rinfo}.rs` against the upstream WW3D2 files of the same names, cut verbatim by build.rs `drafts_ww3d5`
//! (differential/wrap_draft_ww3d5_misc.cpp lists the cuts and the stand-ins).
//!
//! # Method
//! `DRAFT_CASES` (default 20,000) cases per check from a fixed seed; every word compared.
//! - sort list: 1..=6 scripted objects (hook or not, `Pre_Render` answer), 1..=30 operations (`Set_Min_Sort`, `Set_Max_Sort` with values
//!   from 0 to 40, `Add_To_List` at levels 0..=34, `Render_And_Clear`), the event log (every `Pre_Render`, `Post_Render`, `Render`,
//!   `Release_Ref`, `Flush`) and the final min and max compared.
//! - presets: every preset the draft or the verified port defines against the C++'s `_Preset*Shader` bits.
//! - exclusion list: 0..=8 names (short and long, dotted, with `#`, case variants, empty, repeated), 1..=12 queries of the four kinds.
//! - material pass: a random 32-bit shader through `Set_Shader` (which applies `Enable_Fog`), the two flags, the defaults.
//! - render info: 1..=40 push and pop operations on the passes and on the override flags, the count, current flags and passes after each.
//!
//! # Excluded inputs
//! - `Set_Min_Sort(0)` (upstream's `unsigned` loop would wrap past 0; the draft stops, as its comment says).
//! - `Pop_Material_Pass` with no pass and no rejection, `Pop_Override_Flags` at level 0 (unsigned wrap), and a 32nd `Push_Override_Flags`
//!   (upstream's `WWASSERT(OverrideFlagLevel<MAX_OVERRIDE_FLAG_LEVEL)` is compiled out and then writes `OverrideFlag[32]`, past the
//!   32-entry array: an upstream off-by-one; the draft's `assert!` passes at level 31 and the array index then panics).
//! - Reference counts (`Add_Ref`, `Release_Ref` on the passes) are not modelled by the draft and are no-ops in the wrapper.
//! - Bumpenvmap preset: neither the draft nor the verified port defines it (listed as a gap).
//!
//! # Predictions (2026-11-20, before the first run)
//! 0 mismatches. Doubts: the exclusion list (the draft's `HashMap` against the real `HashTemplateClass` with its string hash; both are
//! exact-match sets, so I expect equal) and `Set_Max_Sort(0)` with `Render_And_Clear` (the draft's explicit stop at 0).
//!
//! # Results
//! 2026-11-20, `DRAFT_CASES` default: **0 mismatches on the first run**, in every check: sort list 20,000 cases; presets 21 words (the
//! 21st preset, Bumpenvmap, is not defined by the draft or the port: listed, not compared); exclusion list 20,000; material pass 20,000;
//! render info 20,000. **Prediction held**, including the two doubts (the exclusion list's exact match and the draft's stop at level 0).
//! Not covered: `MaterialPassClass`'s texture, material and cull-volume accessors and `Install_Materials` (device); `RenderInfoClass`'s
//! public fields and `SpecialRenderInfoClass` (their defaults are not dumped), so those two draft files keep their UNTESTED banner. The
//! preset constants cannot carry a plant (`plant()` is not `const`); the check compares all 21 words with the C++.
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching cases per check)
//! 18675 (render from min up) sort 5,123; 18676 (level 32 refused) 1,332; 18677 (max cap 31) 3,289; 18678 (Post_Render only if rendered) 5,397;
//! 18679 (flush when a hook refused) 5,350; 18680 (min level skipped) 2,300; 18681 (`#` not checked) exclusion 1,049; 18682 (chop at the
//! last dot) 1,280; 18683 (hanim: before the dot) 5,291; 18684 (case-insensitive) 832; 18685 (`Set_Shader` without `Enable_Fog`) material
//! pass 2,873; 18686 (default translucent false) 20,000; 18687 (30 passes fit) render info 3,656; 18688 (pop ignores rejections) 2,242;
//! 18689 (push flags stores nothing) 16,975; 18690 (pop flags does not pop) 13,879. **16 of 16 caught.**

mod support;

use rts_lockstep_engine::drafts::ww3d::handles::RenderObjHandle;
use rts_lockstep_engine::drafts::ww3d::matpass::{MaterialPassClass, MaterialPassStatics};
use rts_lockstep_engine::drafts::ww3d::rinfo::RenderInfoClass;
use rts_lockstep_engine::drafts::ww3d::shader_presets as sp;
use rts_lockstep_engine::drafts::ww3d::static_sort_list::{DefaultStaticSortListClass, StaticSortRenderHost};
use rts_lockstep_engine::drafts::ww3d::w3dexclusionlist::W3DExclusionListClass;
use rts_lockstep_engine::formats::w3d::shader as port;
use rts_lockstep_engine::formats::w3d::shader::ShaderClass;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dw5_misc(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 100_000];
    let n = unsafe { ref_dw5_misc(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "wrapper refused op {}", input[0]);
    out.truncate(n as usize);
    out
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in &chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        for e in c.examples.iter().take(2) {
            println!("EX {}: {}", c.name, &e[..e.len().min(1200)]);
        }
        bad += c.mismatches;
    }
    if !planted {
        assert_eq!(bad, 0, "mismatches against the C++");
    } else {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

// ---------------- sort list ----------------
struct Host {
    hook: Vec<(bool, bool)>,
    log: Vec<u32>,
}

impl StaticSortRenderHost for Host {
    fn has_render_hook(&self, robj: RenderObjHandle) -> bool {
        self.hook[robj].0
    }
    fn pre_render(&mut self, robj: RenderObjHandle, _rinfo: &mut RenderInfoClass) -> bool {
        self.log.extend([1, robj as u32]);
        self.hook[robj].1
    }
    fn post_render(&mut self, robj: RenderObjHandle, _rinfo: &mut RenderInfoClass) {
        self.log.extend([2, robj as u32]);
    }
    fn render(&mut self, robj: RenderObjHandle, _rinfo: &mut RenderInfoClass) {
        self.log.extend([3, robj as u32]);
    }
    fn release_ref(&mut self, robj: RenderObjHandle) {
        self.log.extend([4, robj as u32]);
    }
    fn flush_mesh_renderer(&mut self) {
        self.log.push(5);
    }
}

fn sort_case(g: &mut Gen, c: &mut Check, case: u64) {
    let n = 1 + g.below(6);
    let hooks: Vec<(bool, bool)> = (0..n).map(|_| (g.below(2) == 0, g.below(2) == 0)).collect();
    let nops = 1 + g.below(30);
    let mut ops: Vec<[u32; 3]> = Vec::new();
    for _ in 0..nops {
        ops.push(match g.below(8) {
            0 => [0, 1 + g.below(40), 0],
            1 => [1, g.below(41), 0],
            2..=4 => [2, g.below(n), g.below(35)],
            _ => [3, 0, 0],
        });
    }
    let mut inp = vec![0, n];
    for h in &hooks {
        inp.extend([u32::from(h.0), u32::from(h.1)]);
    }
    inp.push(nops);
    for o in &ops {
        inp.extend(o);
    }
    let cpp = run(&inp);
    let mut host = Host { hook: hooks, log: Vec::new() };
    let mut list = DefaultStaticSortListClass::new();
    let mut ri = RenderInfoClass::new(0);
    for o in &ops {
        match o[0] {
            0 => list.set_min_sort(o[1]),
            1 => list.set_max_sort(o[1]),
            2 => list.add_to_list(o[1] as usize, o[2]),
            _ => {
                list.render_and_clear(&mut host, &mut ri);
                host.log.push(0xFFFF_FFFF);
            }
        }
    }
    host.log.push(list.get_min_sort());
    host.log.push(list.get_max_sort());
    c.eq(host.log, cpp, || format!("case {case} ops {ops:?}"));
}

// ---------------- presets ----------------
fn preset_check(c: &mut Check) {
    let table: [(&str, Option<ShaderClass>); 22] = [
        ("Opaque", Some(sp::PRESET_OPAQUE_SHADER)),
        ("Additive", Some(port::PRESET_ADDITIVE_SHADER)),
        ("Bumpenvmap", None),
        ("Alpha", Some(sp::PRESET_ALPHA_SHADER)),
        ("Multiplicative", Some(sp::PRESET_MULTIPLICATIVE_SHADER)),
        ("Opaque2D", Some(sp::PRESET_OPAQUE_2D_SHADER)),
        ("OpaqueSprite", Some(sp::PRESET_OPAQUE_SPRITE_SHADER)),
        ("Additive2D", Some(sp::PRESET_ADDITIVE_2D_SHADER)),
        ("Alpha2D", Some(sp::PRESET_ALPHA_2D_SHADER)),
        ("AdditiveSprite", Some(port::PRESET_ADDITIVE_SPRITE_SHADER)),
        ("AlphaSprite", Some(port::PRESET_ALPHA_SPRITE_SHADER)),
        ("OpaqueSolid", Some(sp::PRESET_OPAQUE_SOLID_SHADER)),
        ("AdditiveSolid", Some(sp::PRESET_ADDITIVE_SOLID_SHADER)),
        ("AlphaSolid", Some(sp::PRESET_ALPHA_SOLID_SHADER)),
        ("ATest2D", Some(sp::PRESET_ATEST_2D_SHADER)),
        ("ATestSprite", Some(sp::PRESET_ATEST_SPRITE_SHADER)),
        ("ATestBlend2D", Some(sp::PRESET_ATEST_BLEND_2D_SHADER)),
        ("ATestBlendSprite", Some(sp::PRESET_ATEST_BLEND_SPRITE_SHADER)),
        ("Screen2D", Some(sp::PRESET_SCREEN_2D_SHADER)),
        ("ScreenSprite", Some(sp::PRESET_SCREEN_SPRITE_SHADER)),
        ("Multiplicative2D", Some(sp::PRESET_MULTIPLICATIVE_2D_SHADER)),
        ("MultiplicativeSprite", Some(sp::PRESET_MULTIPLICATIVE_SPRITE_SHADER)),
    ];
    for (i, (name, s)) in table.iter().enumerate() {
        let cpp = run(&[1, i as u32]);
        if let Some(s) = s {
            c.eq(s.0, cpp[0], || format!("preset {name}"));
        } else {
            println!("NOT COVERED: preset {name} (C++ bits {:#010x}); neither the draft nor the verified port defines it", cpp[0]);
        }
    }
}

// ---------------- exclusion list ----------------
const WORDS: [&str; 14] = ["", "A", "tank", "Tank", "TANK", "tank.skin", "tank.skin.lod", "a#b", "#", "longer_name_01", "LONGER_NAME_01", "longer_name_01.x", ".", "x.longer_name_01"];

fn word(g: &mut Gen) -> String {
    match g.below(4) {
        0 => WORDS[g.below(WORDS.len() as u32) as usize].to_string(),
        1 => format!("{}.{}", WORDS[g.below(WORDS.len() as u32) as usize], WORDS[g.below(WORDS.len() as u32) as usize]),
        2 => {
            let n = g.below(14);
            (0..n).map(|_| (b'a' + g.below(3) as u8) as char).collect()
        }
        _ => {
            let n = g.below(14);
            (0..n).map(|_| ['a', 'B', '.', '#', '_'][g.below(5) as usize]).collect()
        }
    }
}

fn push_str(v: &mut Vec<u32>, s: &str) {
    v.push(s.len() as u32);
    v.extend(s.bytes().map(u32::from));
}

fn excl_case(g: &mut Gen, c: &mut Check, case: u64) {
    let names: Vec<String> = (0..g.below(9)).map(|_| word(g)).collect();
    let queries: Vec<(u32, String)> = (0..1 + g.below(12))
        .map(|_| {
            let w = if !names.is_empty() && g.below(2) == 0 { names[g.below(names.len() as u32) as usize].clone() } else { word(g) };
            let w = match g.below(4) {
                0 => format!("{w}.sub"),
                1 => format!("pre.{w}"),
                _ => w,
            };
            (g.below(4), w)
        })
        .collect();
    let mut inp = vec![2, names.len() as u32];
    for n in &names {
        push_str(&mut inp, n);
    }
    inp.push(queries.len() as u32);
    for (k, q) in &queries {
        inp.push(*k);
        push_str(&mut inp, q);
    }
    let cpp = run(&inp);
    let list = W3DExclusionListClass::new(names.clone());
    let mine: Vec<u32> = queries
        .iter()
        .map(|(k, q)| {
            u32::from(match k {
                0 => list.is_excluded_prototype(q),
                1 => list.is_excluded_htree(q),
                2 => list.is_excluded_hanim(q),
                _ => list.is_excluded(q),
            })
        })
        .collect();
    c.eq(mine, cpp, || format!("case {case} names {names:?} queries {queries:?}"));
}

// ---------------- material pass ----------------
fn matpass_case(g: &mut Gen, c: &mut Check, case: u64) {
    let bits = if g.below(3) == 0 { g.u32() } else { g.u32() & 0x1F_FFFF | (g.below(8) << 21) };
    let (t, p) = (g.below(2), g.below(2));
    let cpp = run(&[3, bits, t, p]);
    let mut mp = MaterialPassClass::new();
    let mut st = MaterialPassStatics::default();
    let mut mine = vec![u32::from(mp.is_enabled_on_translucent_meshes()), u32::from(st.is_per_polygon_culling_enabled())];
    mp.set_shader(ShaderClass(bits));
    mp.enable_on_translucent_meshes(t != 0);
    st.enable_per_polygon_culling(p != 0);
    mine.push(mp.peek_shader().0);
    mine.push(u32::from(mp.is_enabled_on_translucent_meshes()));
    mine.push(u32::from(st.is_per_polygon_culling_enabled()));
    c.eq(mine, cpp, || format!("case {case} bits {bits:#010x} {t} {p}"));
}

// ---------------- render info ----------------
fn rinfo_case(g: &mut Gen, c: &mut Check, case: u64) {
    let mut ops: Vec<[u32; 2]> = Vec::new();
    let (mut passes, mut rejected, mut level) = (0usize, 0usize, 0usize);
    let heavy = g.below(3) == 0;
    for _ in 0..1 + g.below(if heavy { 90 } else { 40 }) {
        let k = if heavy && g.below(4) != 0 { 0 } else { g.below(5) };
        match k {
            0 => {
                let h = if g.below(5) == 0 { 0 } else { 1 + g.below(1000) };
                ops.push([0, h]);
                if passes < 31 {
                    passes += 1;
                } else {
                    rejected += 1;
                }
            }
            1 => {
                if passes == 0 && rejected == 0 {
                    continue;
                }
                ops.push([1, 0]);
                if rejected == 0 {
                    passes -= 1;
                } else {
                    rejected -= 1;
                }
            }
            2 => {
                if level >= 31 {
                    continue;
                }
                ops.push([2, g.below(16)]);
                level += 1;
            }
            3 => {
                if level == 0 {
                    continue;
                }
                ops.push([3, 0]);
                level -= 1;
            }
            _ => ops.push([4, g.below(16)]),
        }
    }
    let mut inp = vec![4, ops.len() as u32];
    for o in &ops {
        inp.extend(o);
    }
    let cpp = run(&inp);
    let mut ri = RenderInfoClass::new(0);
    let mut mine = Vec::new();
    for o in &ops {
        match o[0] {
            0 => ri.push_material_pass(if o[1] == 0 { None } else { Some(o[1] as usize) }),
            1 => ri.pop_material_pass(),
            2 => ri.push_override_flags(o[1]),
            3 => ri.pop_override_flags(),
            _ => *ri.current_override_flags() = o[1],
        }
        mine.push(ri.additional_pass_count() as u32);
        mine.push(*ri.current_override_flags());
        let n = ri.additional_pass_count();
        mine.push(if n > 0 { ri.peek_additional_pass(n - 1).map_or(0, |h| h as u32) } else { 0xFFFF_FFFF });
    }
    for k in 0..ri.additional_pass_count() {
        mine.push(ri.peek_additional_pass(k).map_or(0, |h| h as u32));
    }
    c.eq(mine, cpp, || format!("case {case} ops {ops:?}"));
}

#[test]
fn ww3d5_misc_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_15_02);
    let mut c_sort = Check::new("DefaultStaticSortListClass");
    let mut c_pre = Check::new("shader presets");
    let mut c_ex = Check::new("W3DExclusionListClass");
    let mut c_mp = Check::new("MaterialPassClass");
    let mut c_ri = Check::new("RenderInfoClass");
    preset_check(&mut c_pre);
    for case in 0..cases() {
        sort_case(&mut g, &mut c_sort, case);
        excl_case(&mut g, &mut c_ex, case);
        matpass_case(&mut g, &mut c_mp, case);
        rinfo_case(&mut g, &mut c_ri, case);
    }
    finish(vec![c_sort, c_pre, c_ex, c_mp, c_ri]);
}
