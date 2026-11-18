// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215) of the manager drafts `src/drafts/sweep_ww3d/{htreemgr,hanimmgr}.rs`:
//! `HTreeManagerClass::Free_All_Trees_With_Exclusion_List`, `Get_Tree_Name`; `HAnimManagerClass::
//! Free_All_Anims_With_Exclusion_List`, `Create_Asset_List` (with the exclusion list's `Is_Excluded` for hierarchies and
//! animations, `w3dexclusionlist.cpp`) against upstream's htreemgr.cpp, hanimmgr.cpp and w3dexclusionlist.cpp compiled
//! whole under renamed classes (build.rs `drafts_anim`, differential/wrap_draft_mgr.cpp).
//!
//! # Method
//!
//! Trees: 1 to 9 hierarchies (one pivot each, from `tree_chunk`) with distinct lower-cased names in random case are loaded by
//! the real `Load_Tree`; one or two `Free_All_Trees_With_Exclusion_List` with random exclusion lists (names in the loaded or
//! another case, and names not loaded). Compared: `NumTrees`, the first 32 slots (stale ones included), what the rebuilt
//! `TreeHash` holds (probed with every pool name in both cases), and `Get_Tree_Name(0..40)`. Animations: 1 to 12 stand-in
//! `HAnimClass`es named `<skeleton>.<file>` (or without a `.`) added with `Add_Anim`, half with one more holder (`Num_Refs`
//! 2); `Free_All_Anims_With_Exclusion_List`; compared: which animations are still in the table, and `Create_Asset_List`'s
//! strings sorted (the table's order is the hash's).
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! 0 mismatches for the trees (the draft documents the stale slots). For the animations I expect 0 too, with some doubt: upstream
//! removes from the hash table while iterating it, which the draft's `retain` does not model.
//! Upstream defect expected and NOT a mismatch: the tree manager's destructor deletes a tree twice after a free that moves a
//! survivor (the wrapper never destroys its managers).

mod draft_anim_bytes;
mod support;

use draft_anim_bytes::*;
use rts_lockstep_engine::drafts::sweep_ww3d::hanimmgr::{AnimEntry, HAnimManagerState};
use rts_lockstep_engine::drafts::sweep_ww3d::htreemgr::HTreeManagerState;
use rts_lockstep_engine::drafts::ww3d::w3dexclusionlist::W3DExclusionListClass;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dmg_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 50_000];
    let n = unsafe { ref_dmg_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

fn push_str(v: &mut Vec<u32>, s: &str) {
    v.push(s.len() as u32);
    v.extend(s.bytes().map(u32::from));
}

fn random_case(g: &mut Gen, s: &str) -> String {
    s.chars().map(|c| if g.below(3) == 0 { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() }).collect()
}

const POOL: [&str; 12] = ["skel", "bones", "hero", "tank", "jet", "plane", "ship", "gun", "rig", "arms", "head", "legs"];

fn exclusion(g: &mut Gen, pool: &[String]) -> Vec<String> {
    (0..g.below(6))
        .map(|_| {
            let base = pool[g.below(pool.len() as u32) as usize].clone();
            match g.below(4) {
                0 => base.to_uppercase(),
                1 => format!("{base}x"),
                _ => base,
            }
        })
        .collect()
}

fn slots(s: &HTreeManagerState) -> Vec<u32> {
    let mut v = vec![s.num_trees as u32];
    for i in 0..32 {
        match s.tree_ptr.get(i).and_then(|x| x.as_ref()) {
            Some(n) => {
                v.push(n.len() as u32 + 1);
                v.extend(n.bytes().map(u32::from));
            }
            None => v.push(0),
        }
    }
    v
}

#[test]
fn tree_manager_exclusion_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0020);
    let mut chk = Check::new("HTreeManagerClass exclusion list");
    let (mut moved, mut freed) = (0u64, 0u64);
    for case in 0..cases() {
        run(&[0]);
        let mut st = HTreeManagerState::new();
        let k = 1 + g.below(9) as usize;
        let mut pool = POOL.to_vec();
        for i in (1..pool.len()).rev() {
            pool.swap(i, g.below(i as u32 + 1) as usize);
        }
        let names: Vec<String> = pool[..k].iter().map(|n| random_case(&mut g, n)).collect();
        for n in &names {
            let bytes = tree_chunk(n.as_bytes(), &[PivotSpec { name: b"ROOT".to_vec(), parent: 0xFFFF_FFFF, t: [0.0; 3], e: [0.0; 3], q: [0.0, 0.0, 0.0, 1.0] }]);
            let mut inp = vec![1];
            words_blob(&bytes, &mut inp);
            assert_eq!(run(&inp), vec![0], "Load_Tree of {n}");
            let i = st.num_trees as usize;
            st.tree_ptr[i] = Some(n.clone());
            st.num_trees += 1;
        }
        let lower_pool: Vec<String> = POOL.iter().map(|s| s.to_string()).collect();
        // ONE round: a second Free_All_Trees_With_Exclusion_List walks the stale slots the first left (a moved survivor stays
        // in its old slot too) and deletes a tree twice when it is not excluded: an upstream defect (#215), SIGSEGV measured.
        for round in 0..1 {
            let ex = exclusion(&mut g, &names.iter().chain(lower_pool.iter()).cloned().collect::<Vec<_>>());
            let before = st.num_trees;
            let survivors = st.free_all_trees_with_exclusion_list(&W3DExclusionListClass::new(ex.clone()));
            freed += (before - st.num_trees) as u64;
            moved += u64::from(survivors.len() as i32 != before && !survivors.is_empty());
            let mut inp = vec![2, ex.len() as u32];
            for e in &ex {
                push_str(&mut inp, e);
            }
            let c = run(&inp);
            chk.eq(slots(&st), c, || format!("case {case} round {round}: trees {names:?} excluded {ex:?}"));
            let probes: Vec<String> = POOL.iter().flat_map(|p| [p.to_string(), p.to_uppercase()]).chain(names.iter().cloned()).collect();
            let mut inp = vec![3, probes.len() as u32];
            for p in &probes {
                push_str(&mut inp, p);
            }
            let c = run(&inp);
            let r: Vec<u32> = probes.iter().map(|p| u32::from(survivors.contains(p))).collect();
            chk.eq(r, c, || format!("case {case} round {round}: hash probes after {ex:?}"));
            for idx in 0..40 {
                let c = run(&[4, idx]);
                let r = match st.get_tree_name(idx as i32) {
                    Some(n) => {
                        let mut v = vec![n.len() as u32 + 1];
                        v.extend(n.bytes().map(u32::from));
                        v
                    }
                    None => vec![0],
                };
                chk.eq(r, c, || format!("case {case} round {round}: Get_Tree_Name({idx})"));
            }
        }
    }
    println!("{} cases: {freed} trees freed, {moved} rounds with a survivor", cases());
    finish(chk);
}

#[test]
fn anim_manager_exclusion_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0021);
    let mut chk = Check::new("HAnimManagerClass exclusion list and asset list");
    let (mut freed, mut kept_excl) = (0u64, 0u64);
    let files = ["walk", "run", "idle", "die", "fire", "reload", "jump", "swim", "aim", "turn", "wave", "sit"];
    for case in 0..cases() {
        run(&[0]);
        let mut st = HAnimManagerState::default();
        let mut added: Vec<String> = Vec::new();
        let mut order: Vec<usize> = (0..files.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, g.below(i as u32 + 1) as usize);
        }
        for i in 0..1 + g.below(12) as usize {
            let file = random_case(&mut g, files[order[i]]);
            let name = if g.below(8) == 0 { file.clone() } else { { let sk = POOL[g.below(4) as usize]; format!("{}.{}", random_case(&mut g, sk), file) } };
            let extra = g.below(2);
            let mut inp = vec![5];
            push_str(&mut inp, &name);
            inp.push(extra);
            run(&inp);
            st.anims.push(AnimEntry { name: name.clone(), num_refs: 1 + extra });
            added.push(name);
        }
        let list = |st: &HAnimManagerState| -> Vec<String> {
            let mut l = Vec::new();
            st.create_asset_list(&mut l);
            l.sort();
            l
        };
        let c = run(&[7]);
        let mut cl = decode_list(&c);
        cl.sort();
        chk.eq(list(&st), cl, || format!("case {case}: asset list before, anims {added:?}"));
        let pool: Vec<String> = files.iter().map(|s| s.to_string()).collect();
        let ex = exclusion(&mut g, &pool);
        let exl = W3DExclusionListClass::new(ex.clone());
        let before = st.anims.len();
        for a in &st.anims {
            kept_excl += u64::from(a.num_refs == 1 && exl.is_excluded_hanim(&a.name));
        }
        st.free_all_anims_with_exclusion_list(&exl);
        freed += (before - st.anims.len()) as u64;
        let mut inp = vec![6, ex.len() as u32];
        for e in &ex {
            push_str(&mut inp, e);
        }
        let c = run(&inp);
        let r: Vec<u32> = added.iter().map(|n| u32::from(st.anims.iter().any(|a| &a.name == n))).collect();
        chk.eq(r, c, || format!("case {case}: survivors of {added:?} with exclusion {ex:?}"));
        let c = run(&[7]);
        let mut cl = decode_list(&c);
        cl.sort();
        chk.eq(list(&st), cl, || format!("case {case}: asset list after"));
    }
    println!("{} cases: {freed} animations freed, {kept_excl} single-holder animations kept by the list", cases());
    finish(chk);
}

fn decode_list(w: &[u32]) -> Vec<String> {
    let mut i = 1;
    let mut v = Vec::new();
    for _ in 0..w[0] {
        let n = w[i] as usize;
        v.push(w[i + 1..i + 1 + n].iter().map(|&c| c as u8 as char).collect());
        i += 1 + n;
    }
    v
}
