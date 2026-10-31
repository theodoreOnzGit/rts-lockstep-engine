#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp (ScriptEngine::init's templates)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ScriptEngine::init`'s condition and action templates (`logic::script_templates`) against the C++, `p1w13-sonnet`, layer 11
//! batch E1.
//!
//! # Method
//!
//! The C++ side is `init`'s template code (lines 587-5266, the tables and the two loops that append `[index]` and make the name
//! keys), cut out unchanged by build.rs and compiled by `differential/wrap_script_templates.cpp` as the body of a member function of
//! a stand-in holding the two arrays, on the real `Template` classes of `Scripts.h` and `Template()` of `Scripts.cpp`, on zeroed
//! memory (as `NEW ScriptEngine`), with a fresh name key generator (`init()` only). It writes every field of the 453 templates.
//!
//! The Rust side is [`CONDITION_TEMPLATES`], [`ACTION_TEMPLATES`], [`TemplateText::ui_name`] (with the index), and
//! [`script_templates`] on a fresh `NameKeyGenerator` (the keys and the declared parameter types the script reader takes).
//! Compared: each template's editor name, second editor name (empty), internal name, name key, string count, the twelve strings,
//! parameter count, the twelve parameter types, the help text (empty), and the reader's parameter list (the first `m_numParameters`
//! types).
//!
//! # Results (2026-10-31)
//!
//! 453 templates (109 conditions, 344 actions), every field: **0 mismatches**. The tables keep upstream's slips (see the module
//! docs): seven types `init` never sets, `CAMERA_MOTION_BLUR_FOLLOW`'s parameter set in slot 1, more types or strings set than
//! declared, fields set twice.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 4 of 4 caught.
//!
//! | id | planted bug | templates differing |
//! |---|---|---|
//! | 3891 | the editor name's index one too high | 453 |
//! | 3892 | the action keys made before the condition keys | 453 |
//! | 3893 | an unset parameter slot read as `REAL` | 453 |
//! | 3894 | the reader's parameters the stored types (up to the last not `INT`), not the declared count | 64 |

use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::script_templates::*;
use rts_lockstep_engine::logic::scripts::MAX_PARMS;

struct Rd<'a> {
    b: &'a [u8],
    at: usize,
}

impl Rd<'_> {
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.b[self.at..self.at + 4].try_into().expect("4 bytes"));
        self.at += 4;
        v
    }
    fn str(&mut self) -> Vec<u8> {
        let n = self.u32() as usize;
        let s = self.b[self.at..self.at + n].to_vec();
        self.at += n;
        s
    }
}

#[test]
fn script_templates_match_upstream() {
    let need = unsafe { cpp::ref_script_templates(std::ptr::null_mut(), 0) };
    let mut buf = vec![0u8; need];
    let got = unsafe { cpp::ref_script_templates(buf.as_mut_ptr(), buf.len()) };
    assert_eq!(got, need);
    let mut names = NameKeyGenerator::new();
    names.init();
    let tables = script_templates(&mut names);
    let mut r = Rd { b: &buf, at: 0 };
    let mut differing = 0;
    let all = CONDITION_TEMPLATES.iter().enumerate().map(|(i, t)| (true, i, t)).chain(ACTION_TEMPLATES.iter().enumerate().map(|(i, t)| (false, i, t)));
    for (is_condition, i, t) in all {
        let st = if is_condition { tables.get_condition_template(i as i32) } else { tables.get_action_template(i as i32) };
        let mut diffs: Vec<String> = Vec::new();
        let mut check = |what: &str, c: String, rust: String| {
            if c != rust {
                diffs.push(format!("{what}: c++ {c:?} rust {rust:?}"));
            }
        };
        let s = |v: &[u8]| String::from_utf8_lossy(v).into_owned();
        check("uiName", s(&r.str()), s(&t.ui_name(i as i32)));
        check("uiName2", s(&r.str()), String::new());
        check("internalName", s(&r.str()), s(t.internal_name));
        check("key", r.u32().to_string(), st.internal_name_key.0.to_string());
        check("numUiStrings", (r.u32() as i32).to_string(), t.num_ui_strings.to_string());
        for k in 0..MAX_PARMS {
            check(&format!("uiStrings[{k}]"), s(&r.str()), s(t.ui_string(k)));
        }
        let np = r.u32() as i32;
        check("numParameters", np.to_string(), t.num_parameters.to_string());
        let mut params = Vec::new();
        for k in 0..MAX_PARMS {
            let p = r.u32() as i32;
            params.push(p);
            check(&format!("parameters[{k}]"), p.to_string(), t.parameter(k).to_string());
        }
        check("reader's parameters", format!("{:?}", &params[..np.clamp(0, MAX_PARMS as i32) as usize]), format!("{:?}", st.parameters));
        check("helpText", s(&r.str()), String::new());
        if !diffs.is_empty() {
            differing += 1;
            if differing <= 5 {
                println!("{} {i}: {}", if is_condition { "condition" } else { "action" }, diffs.join("; "));
            }
        }
    }
    assert_eq!(r.at, buf.len(), "the whole dump read");
    println!("E1: {} templates, {differing} differing", CONDITION_TEMPLATES.len() + ACTION_TEMPLATES.len());
    assert_eq!(differing, 0);
}
