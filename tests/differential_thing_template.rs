// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 5's `ThingTemplate` and `ThingFactory`
//! (`common::thing_template`, `common::thing_factory`, with `logic::weapon_set`'s
//! `WeaponTemplateSet` and `common::production_prerequisite`) against upstream
//! `ThingTemplate.cpp`, `ThingFactory.cpp` and `INIObject.cpp` (commit 0a05454d), compiled
//! UNCHANGED as `differential/wrap_thingtemplate.cpp` describes (with the cut-out members of
//! `WeaponSet.cpp`, `ProductionPrerequisite.cpp` and `ModuleFactory.cpp`, and two field parsers of
//! `INI.cpp`), behind the real `INI::load`, the real generic field parsers, `parseAndTranslateLabel`,
//! `scanScience`, `NameKeyGenerator`, `ArmorStore`, `DamageFXStore`, `ScienceStore`, `ModuleData`,
//! `SparseMatchFinder` and `GameLogicRandomValue` underneath.
//!
//! No game INI files are available, so every file is **synthetic**: `Object` and `ObjectReskin` blocks
//! (every one of the 110 fields of the `Object` table and the 10 of the `ObjectReskin` table, in several
//! forms, well-formed and malformed: signs, partial and overflowing numbers, bad names, empty values,
//! `NONE`/`+`/`-` bit lists, nested `ArmorSet`/`WeaponSet`/`Prerequisites`/`UnitSpecificSounds`/
//! `UnitSpecificFX` blocks, `Locomotor`, names of armors, damage effects, weapons and sciences that
//! exist and that do not, a `DefaultThingTemplate` that every later template starts from, repeated names,
//! names differing only in case), the `Armor`, `DamageFX` and `Science` blocks those names refer to,
//! comments, `CRLF`, a missing `End`, unknown fields and blocks. The game text is a stand-in on both
//! sides: a label translates to `T:` and its bytes, and to nothing if it starts with `NOTEXT`.
//!
//! **The modules.** The 205 module classes of `ModuleFactory::init` are not translated (layer 9) and
//! cannot be compiled here, so the object blocks are generated **without** the module sub-blocks of those
//! classes for the field comparison; what the Rust does with one is checked by the unit test
//! `a_module_that_is_not_translated_is_an_error_never_skipped` in `common::thing_factory` (an error at template load, never a skip), and what the C++ does there is stated in
//! `wrap_thingtemplate.cpp`: it makes the module's real data class from the block; the wrapper has none of
//! the 205, and upstream's `newModuleDataFromINI` answers `NULL` for a name the factory does not know,
//! which `parseModuleName` then calls `isAiModuleData` on. What is compared instead is everything the
//! template does with module blocks, through **test modules** that both sides register the same way (the
//! real `addModuleInternal` / `ModuleFactory::addModuleInternal`): eight behavior modules (two with the
//! `UPDATE` interface, two with `DIE`, two with `BODY`, one with both `UPDATE` and `DIE`, one `UPGRADE`),
//! two draw modules and a client update module, whose blocks are `End` (behavior) or any lines with the
//! four nested-block fields of `W3DModelDraw` (client). With them, compared: `Behavior`/`Body`/`Draw`/
//! `ClientUpdate` lines (the `Body` rule both ways, tags required), `AddModule`, `RemoveModule`,
//! `ReplaceModule` (same class, new tag), `InheritableModule`, `OverrideableByLikeKind`, a template made
//! from `DefaultThingTemplate` dropping the default modules of the same interface (except the inheritable
//! ones, `ModuleTag_DefaultAutoHealBehavior` unless trainable, and the like-kind ones by class and kind),
//! the parsing mode left set by a throw, and the module lists of override copies.
//!
//! 1. `object_blocks_...` (INI sessions): each session is 2-5 steps against one pair of systems, each step
//!    a file loaded with `INI_LOAD_OVERWRITE` (or `MULTIFILE`), a file loaded with
//!    `INI_LOAD_CREATE_OVERRIDES` (override copies of the final override, templates made for this map only),
//!    `ThingFactory::reset()` (which deletes exactly those), `postProcessLoad()`, `init()` or `update()`.
//!    After every step, compared: what `INI::load` throws (nothing, an `INIException` and its whole
//!    message, an `INI_*` status code, an `ErrorCode`), the INI `XferCRC`, **every member of every template
//!    object of every override chain** (the override flag, 100+ fields, the module lists with every
//!    entry's flags, the armor and weapon sets, the prerequisites, the per-unit maps, the getters that
//!    compute), the name map, `m_nextTemplateID`, the module factory's kept data, and the lookups
//!    (`findTemplate`, `findByTemplateID`, `isEquivalentTo` on node pairs, `findArmorTemplateSet` and
//!    `findWeaponTemplateSet` for random flags, `getPerUnitSound`/`getPerUnitFX`, `newObject`'s template
//!    side with the game logic random value it draws, `newDrawable`). The name keys are lockstep state, so
//!    every lookup assigns them in the same order on both sides and the next key is compared after every
//!    step.
//! 2. `module_lists_...`: the same with only the module-related fields, many more of them, and a
//!    `DefaultThingTemplate` with inheritable and like-kind modules in most sessions.
//! 3. `tables_...`: the tokens of the `Object` and `ObjectReskin` field tables, in order.
//!
//! Inputs for which the Rust reports undefined behaviour upstream (an integer out of range of `%d`, a
//! `Shadow = +NAME` list, a field line in a module's `End`-only block, a template copied from one that has
//! an override chain, a `reset()` that deletes a template from the middle of the list) and the module
//! errors the Rust gives where upstream would dereference a null `ModuleData` (a behavior module name
//! the wrapper's factory has never heard of) are **not** given to the C++; their counts are printed, and a
//! session stops at such a step (the steps before it are compared).
//!
//! Each comparison also runs a planted bug, which must be caught.
//!
//! # Results (2026-10-18, upstream commit 0a05454d, seeds `0x7A11` / `0xB0DE` / the `Gen` of this file)
//!
//! - `object_blocks_...`: 3000 sessions, 7562 loads (992 read to the end, 6570 threw, all 7562 outcomes,
//!   messages and CRCs equal), 17699 template objects in 16920 chains (779 override copies, 2884 made for
//!   a map, 512 reskins), 9247 whole-store dumps, 464481 lookups: **0 mismatches** in every comparison.
//! - `module_lists_...`: 3000 sessions, 8171 loads (703 ok), 18467 template objects, 29493 modules of
//!   which 13774 client, 9868 dumps, 488325 lookups: **0 mismatches**.
//! - `tables_...`: both field tables equal, token by token.
//! - Stopped at an undefined or module-error input (not given to the C++): 470 and 269 steps.
//! - **Planted bugs**, all caught: the load outcome (every load), skill points swapped with experience
//!   (9048 of 9247), `WeaponTemplateSet` owner, first sound, inheritable flag, `reskinnedFrom`,
//!   `findTemplate`, `findByTemplateID`, `isEquivalentTo`, `newObject`, set lookups, per-unit sounds; and,
//!   in the Rust source, a swapped parser for `VisionRange`, a skipped `setCopiedFromDefault` in
//!   `newOverride`, and the byte wrap below put back to a mask (each failed this test, then removed).
//! - **Findings.** (1) `TransportSlotCount` and `StructureRubbleHeight` are `parseUnsignedByte` into a
//!   signed `Byte`: 133 reads back as -123, which the first run of this test found and the Rust now
//!   does. (2) A copy of a template carries the `SparseMatchFinder` cache of its source, whose pointers
//!   point into the source's vector: `findArmorTemplateSet` can answer with another template's set
//!   (seen once in 3000 sessions; compared as "there is an answer", the index is not comparable).
//! - A first run crashed in `~ThingFactory`: the Rust had reported the dangling link of `reset()`
//!   (undefined upstream) after the harness had already run the C++ `reset()`. The C++ is now not given
//!   that `reset()`.
//!
//! Not tested against the C++ (layer 7/8 cut points, see the module docs): `calcCostToBuild`,
//! `calcTimeToBuild`, `getBuildFacilityTemplate`, `friend_getAIModuleInfo` (no AI module exists),
//! `getBuildable`'s script override, `ProductionPrerequisite`'s `Player` members, and everything of
//! `newObject` after the template is chosen.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::bit_flags::BitFlags;
use rts_lockstep_engine::common::damage_fx::DamageFxStore;
use rts_lockstep_engine::common::game_common::VeterancyLevel;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE};
use rts_lockstep_engine::common::kind_of::KindOfMaskType;
use rts_lockstep_engine::common::module::{
    Module, ModuleData, ModuleDataKind, ModuleType, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_CLIENT_UPDATE, MODULEINTERFACE_DIE,
    MODULEINTERFACE_DRAW, MODULEINTERFACE_UPDATE, MODULEINTERFACE_UPGRADE,
};
use rts_lockstep_engine::common::module_factory::{ModuleFactory, ModuleTemplate};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::overridable::Overridable;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore};
use rts_lockstep_engine::common::thing_factory::{ThingFactory, ThingFactoryError};
use rts_lockstep_engine::common::thing_template::{
    is_equivalent_to, ModuleInfo, ModuleInfoData, ThingParseEnv, ThingTemplate, ThingTemplateAudioType, OBJECT_FIELD_NAMES, RESKIN_FIELD_NAMES,
    TTAUDIO_COUNT,
};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::armor::ArmorStore;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::weapon_set::WeaponTemplateTable;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};

/// The C++ side has process-global state (the name keys, the stores, `strtok`, the game logic random
/// value), so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------
// The Rust side's systems
// ---------------------------------------------------------------------------

/// The stand-in game text of the Science fixture.
struct StandInText;

impl GameTextFetch for StandInText {
    fn fetch(&self, label: &[u8]) -> UnicodeString {
        if label.len() >= 6 && label[..6].eq_ignore_ascii_case(b"NOTEXT") {
            return UnicodeString::new();
        }
        let mut units: Vec<u16> = vec![u16::from(b'T'), u16::from(b':')];
        units.extend(label.iter().map(|&b| u16::from(b)));
        UnicodeString::try_from_chars(&units).unwrap()
    }
}

/// The test modules, in the order `wrap_thingtemplate.cpp` registers them (each assigns a decorated name
/// key): name, type, interface mask.
const TEST_MODULES: [(&str, ModuleType, i32); 11] = [
    ("TestUpdate", ModuleType::Behavior, MODULEINTERFACE_UPDATE),
    ("TestUpdate2", ModuleType::Behavior, MODULEINTERFACE_UPDATE),
    ("TestDie", ModuleType::Behavior, MODULEINTERFACE_DIE),
    ("TestDie2", ModuleType::Behavior, MODULEINTERFACE_DIE),
    ("TestBody", ModuleType::Behavior, MODULEINTERFACE_BODY),
    ("TestBody2", ModuleType::Behavior, MODULEINTERFACE_BODY),
    ("TestMulti", ModuleType::Behavior, MODULEINTERFACE_UPDATE | MODULEINTERFACE_DIE),
    ("TestUpgrade", ModuleType::Behavior, MODULEINTERFACE_UPGRADE),
    ("TestDraw", ModuleType::Draw, MODULEINTERFACE_DRAW),
    ("TestDraw2", ModuleType::Draw, MODULEINTERFACE_DRAW),
    ("TestClient", ModuleType::ClientUpdate, MODULEINTERFACE_CLIENT_UPDATE),
];
const TEST_WEAPONS: [&str; 3] = ["Rifle", "Cannon", "Missile"];

fn never(_: ObjectId, _: Option<ModuleDataKind>) -> Result<Module, IniError> {
    Err(IniError::new(IniErrorKind::Bug))
}
fn base_data(ini: Option<&mut Ini<'_>>) -> Result<ModuleDataKind, IniError> {
    Ok(ModuleDataKind::Module(Arc::new(ModuleData::friend_new_module_data(ini)?)))
}

struct Ctx {
    factory: ThingFactory,
    names: NameKeyGenerator,
    modules: ModuleFactory,
    armors: ArmorStore,
    dfx: DamageFxStore,
    weapons: WeaponTemplateTable,
    sciences: ScienceStore,
    occlusion: u32,
    superweapon: u32,
}

impl Ctx {
    /// `ref_tt_reset(occlusion, superweapon)`, in the same order: the name keys start at 1, the test modules
    /// and weapons are registered.
    fn new(occlusion: u32, superweapon: u32) -> Self {
        unsafe { cpp::ref_tt_reset(occlusion, superweapon) };
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut modules = ModuleFactory::new();
        for (name, module_type, mask) in TEST_MODULES {
            if module_type == ModuleType::Behavior {
                let t = ModuleTemplate { create_proc: Some(never), create_data_proc: Some(base_data), which_interfaces: Some(mask) };
                modules.add_module_internal(&mut names, t, module_type, name).unwrap();
            } else {
                // the client's modules are not in the Rust factory; the C++ registration assigned the key
                ModuleFactory::make_decorated_name_key(&mut names, name.as_bytes(), module_type).unwrap();
            }
        }
        let mut weapons = WeaponTemplateTable::new();
        for w in TEST_WEAPONS {
            weapons.add(names.name_to_key(w.as_bytes()));
        }
        Ctx {
            factory: ThingFactory::new(),
            names,
            modules,
            armors: ArmorStore::new(),
            dfx: DamageFxStore::new(),
            weapons,
            sciences: ScienceStore::new(),
            occlusion,
            superweapon,
        }
    }
}

fn object_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    let mut env = ThingParseEnv {
        names: &mut c.names,
        modules: &mut c.modules,
        armors: &c.armors,
        damage_fx: &c.dfx,
        weapons: &c.weapons,
        sciences: &c.sciences,
        text: &StandInText,
    };
    c.factory.parse_object_block(ini, &mut env, c.occlusion)
}
fn reskin_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    let mut env = ThingParseEnv {
        names: &mut c.names,
        modules: &mut c.modules,
        armors: &c.armors,
        damage_fx: &c.dfx,
        weapons: &c.weapons,
        sciences: &c.sciences,
        text: &StandInText,
    };
    c.factory.parse_object_reskin_block(ini, &mut env, c.occlusion)
}
fn armor_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.armors.parse_armor_definition(ini, &mut c.names)
}
fn dfx_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.dfx.parse_damage_fx_definition(ini, &mut c.names)
}
fn science_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.sciences.parse_science_definition(ini, &mut c.names, &StandInText)
}
fn stub_block(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// The block table: the five blocks of the test are the handlers under test or the translated stores they
/// refer to; the other 57 throw `ERROR_BUG`, as the stubs of `wrap_ini.cpp` (the files never name them,
/// except as noise).
fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: match token {
                "Object" => object_block,
                "ObjectReskin" => reskin_block,
                "Armor" => armor_block,
                "DamageFX" => dfx_block,
                "Science" => science_block,
                _ => stub_block,
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The dumps (the format of wrap_thingtemplate.cpp)
// ---------------------------------------------------------------------------

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_u64(v: &mut Vec<u8>, x: u64) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_f32(v: &mut Vec<u8>, x: f32) {
    v.extend_from_slice(&x.to_bits().to_le_bytes());
}
fn put_ascii(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}
fn put_uni(v: &mut Vec<u8>, s: &UnicodeString) {
    put_u32(v, s.str().len() as u32);
    for u in s.str() {
        v.extend_from_slice(&u.to_le_bytes());
    }
}
fn put_flags<T: rts_lockstep_engine::common::bit_flags::BitFlagNames>(v: &mut Vec<u8>, f: &BitFlags<T>) {
    for w in f.words() {
        put_u64(v, w);
    }
}

/// The list positions of the roots, newest first, by node id.
fn root_ids(c: &Ctx) -> Vec<u64> {
    c.factory.templates().map(|t| t.data.node_id()).collect()
}

fn dump_module_info(v: &mut Vec<u8>, mi: &ModuleInfo, bug: u32) {
    put_u32(v, mi.get_count() as u32);
    for n in mi.nuggets() {
        put_ascii(v, &n.first);
        put_ascii(v, &n.module_tag);
        put_i32(v, n.interface_mask);
        v.push(u8::from(n.copied_from_default));
        v.push(u8::from(n.inheritable ^ (bug == 4)));
        v.push(u8::from(n.overrideable_by_like_kind));
        match &n.second {
            ModuleInfoData::Logic(d) => {
                v.push(0);
                v.push(u8::from(d.is_ai_module_data()));
                put_u32(v, d.module_tag_name_key().expect("the factory set the tag").0);
            }
            ModuleInfoData::Client(c) => {
                v.push(1);
                v.push(0);
                put_u32(v, c.module_tag_name_key.0);
                put_u32(v, c.lines.len() as u32);
                for l in &c.lines {
                    put_i32(v, l.depth as i32);
                    put_ascii(v, &l.field);
                    put_u32(v, l.args.len() as u32);
                    for a in &l.args {
                        put_ascii(v, a);
                    }
                }
            }
        }
    }
}

/// `dumpTemplate`. `bug` plants a wrong Rust: 1 swaps the skill point and experience arrays, 2 forgets
/// the override flag's template in the weapon sets (its thing template id), 3 leaves out the first sound
/// of the array, 4 flips the inheritable flag of every module entry, 5 numbers `reskinnedFrom` from 1.
fn dump_template(v: &mut Vec<u8>, c: &Ctx, t: &ThingTemplate, roots: &[u64], bug: u32) {
    put_ascii(v, t.get_name());
    put_uni(v, t.get_display_name());
    put_ascii(v, t.get_default_owning_side());
    put_ascii(v, t.friend_get_command_set_string());
    put_ascii(v, t.selected_portrait_image_name());
    put_ascii(v, t.button_image_name());
    for i in 0..5 {
        put_ascii(v, t.get_upgrade_cameo_name(i));
    }
    put_ascii(v, t.get_shadow_texture_name());
    put_ascii(v, t.module_being_replaced_name());
    put_ascii(v, t.module_being_replaced_tag());
    let g = t.get_template_geometry_info();
    put_i32(v, g.get_geom_type() as i32);
    v.push(u8::from(g.get_is_small()));
    put_f32(v, g.get_height());
    put_f32(v, g.get_major_radius());
    put_f32(v, g.get_minor_radius());
    put_f32(v, g.get_bounding_circle_radius());
    put_f32(v, g.get_bounding_sphere_radius());
    put_flags(v, t.kindof());
    const AUDIO: [ThingTemplateAudioType; TTAUDIO_COUNT] = {
        use ThingTemplateAudioType as A;
        [
            A::VoiceSelect,
            A::VoiceGroupSelect,
            A::VoiceSelectElite,
            A::VoiceMove,
            A::VoiceAttack,
            A::VoiceEnter,
            A::VoiceFear,
            A::VoiceCreated,
            A::VoiceNearEnemy,
            A::VoiceTaskUnable,
            A::VoiceTaskComplete,
            A::VoiceMeetEnemy,
            A::SoundMoveStart,
            A::SoundMoveStartDamaged,
            A::SoundMoveLoop,
            A::SoundMoveLoopDamaged,
            A::SoundAmbient,
            A::SoundAmbientDamaged,
            A::SoundAmbientReallyDamaged,
            A::SoundAmbientRubble,
            A::SoundStealthOn,
            A::SoundStealthOff,
            A::SoundCreated,
            A::SoundOnDamaged,
            A::SoundOnReallyDamaged,
            A::SoundEnter,
            A::SoundExit,
            A::SoundPromotedVeteran,
            A::SoundPromotedElite,
            A::SoundPromotedHero,
            A::VoiceGarrison,
            A::SoundFalling,
            A::VoiceDefect,
            A::VoiceAttackSpecial,
            A::VoiceAttackAir,
            A::VoiceGuard,
        ]
    };
    for (i, a) in AUDIO.iter().enumerate() {
        let present = t.has_audio(*a) && !(bug == 3 && i == 0);
        v.push(u8::from(present));
        if present {
            put_ascii(v, &t.get_audio(*a).event_name);
        }
        v.push(u8::from(present));
        put_ascii(v, if present { &t.get_audio(*a).event_name } else { b"" });
    }
    dump_module_info(v, t.get_behavior_module_info(), bug);
    dump_module_info(v, t.get_draw_module_info(), bug);
    dump_module_info(v, t.get_client_update_module_info(), bug);
    const LEVELS: [VeterancyLevel; 4] = [VeterancyLevel::Regular, VeterancyLevel::Veteran, VeterancyLevel::Elite, VeterancyLevel::Heroic];
    for l in LEVELS {
        let (skill, exp) = if bug == 1 { (t.get_experience_value(l), t.raw_skill_point_value(l)) } else { (t.raw_skill_point_value(l), t.get_experience_value(l)) };
        put_i32(v, skill);
        put_i32(v, exp);
        put_i32(v, t.get_experience_required(l));
        put_i32(v, t.get_skill_point_value(l));
        put_i32(v, t.get_experience_value(l));
        put_i32(v, t.get_experience_required(l));
    }
    put_u32(v, t.get_prereq_count() as u32);
    for p in t.prereq_info() {
        put_u32(v, p.prereq_units().len() as u32);
        for u in p.prereq_units() {
            put_i32(v, u.unit.map_or(-1, i32::from));
            put_i32(v, u.flags);
            put_ascii(v, &u.name);
        }
        put_u32(v, p.prereq_sciences().len() as u32);
        for s in p.prereq_sciences() {
            put_i32(v, s.0);
        }
        let facilities = p.all_possible_build_facility_templates(32);
        put_i32(v, facilities.len() as i32);
        for f in facilities {
            put_i32(v, f.map_or(-1, i32::from));
        }
    }
    put_u32(v, t.get_build_variations().len() as u32);
    for b in t.get_build_variations() {
        put_ascii(v, b);
    }
    put_u32(v, t.get_weapon_template_sets().len() as u32);
    for w in t.get_weapon_template_sets() {
        put_i32(v, if bug == 2 { -1 } else { w.thing_template().map_or(-1, i32::from) });
        put_flags(v, w.weapon_set_flags());
        for s in 0..3 {
            put_i32(v, w.get_nth(s).map_or(0, |k| k.0 as i32));
            put_u32(v, w.get_nth_command_source_mask(s));
            put_flags(v, w.get_nth_preferred_against_mask(s));
        }
        v.push(u8::from(w.is_shared_reload_time()));
        v.push(u8::from(w.is_weapon_lock_shared_across_sets()));
        v.push(u8::from(w.has_any_weapons()));
    }
    put_u32(v, t.armor_template_sets().len() as u32);
    for a in t.armor_template_sets() {
        put_flags(v, a.conditions());
        put_i32(v, a.get_armor_template().map_or(0, |k| k.0 as i32));
        put_i32(v, a.get_damage_fx().map_or(0, |k| k.0 as i32));
    }
    put_u32(v, t.get_all_per_unit_sounds().len() as u32);
    for (k, e) in t.get_all_per_unit_sounds() {
        put_ascii(v, k);
        put_ascii(v, &e.event_name);
    }
    put_u32(v, t.per_unit_fx().len() as u32);
    for (k, fx) in t.per_unit_fx() {
        put_ascii(v, k);
        v.push(u8::from(fx.is_some()));
        if let Some(fx) = fx {
            put_ascii(v, fx);
        }
    }
    match t.reskinned_from() {
        None => put_i32(v, -2),
        Some(id) => put_i32(v, roots.iter().position(|&r| r == id).map_or(-1, |p| p as i32 + i32::from(bug == 5))),
    }
    for f in [
        t.get_fence_width(),
        t.get_fence_x_offset(),
        t.friend_calc_vision_range(),
        t.friend_calc_shroud_clearing_range(),
        t.get_shroud_reveal_to_all_range(),
        t.get_placement_view_angle(),
        t.get_factory_exit_width(),
        t.get_factory_extra_bib_width(),
        t.get_build_time(),
        t.get_asset_scale(),
        t.get_instance_scale_fuzziness(),
        t.get_shadow_size_x(),
        t.get_shadow_size_y(),
        t.get_shadow_offset_x(),
        t.get_shadow_offset_y(),
    ] {
        put_f32(v, f);
    }
    put_i32(v, t.get_energy_production());
    put_i32(v, t.get_energy_bonus());
    put_i32(v, t.get_display_color());
    put_u32(v, t.get_occlusion_delay());
    put_u32(v, t.get_max_simultaneous_link_key().0);
    put_u32(v, u32::from(t.get_template_id()));
    put_u32(v, t.friend_get_build_cost() as u32);
    put_u32(v, t.get_refund_value() as u32);
    put_u32(v, t.get_threat_value());
    // m_maxSimultaneousOfType, raw: the getter with no game logic
    put_u32(v, t.get_max_simultaneous_of_type(None));
    for b in [
        t.max_simultaneous_determined_by_superweapon_restriction(),
        t.is_prerequisite(),
        t.is_bridge(),
        t.is_build_facility(),
        t.is_trainable(),
        t.is_enter_guard(),
        t.is_hijack_guard(),
        t.is_forbidden(),
        t.armor_copied_from_default(),
        t.weapons_copied_from_default(),
    ] {
        v.push(u8::from(b));
    }
    v.push(t.get_default_radar_priority());
    v.push(t.get_raw_transport_slot_count() as u8);
    v.push(t.buildable());
    v.push(t.get_build_completion());
    v.push(t.get_editor_sorting() as u8);
    v.push(t.get_structure_rubble_height() as i8 as u8);
    v.push(t.get_shadow_type());
    v.push(t.module_parsing_mode() as u8);
    v.push(t.get_crusher_level());
    v.push(t.get_crushable_level());
    // the getters that compute something
    put_u32(v, t.get_max_simultaneous_of_type(Some(c.superweapon)));
    v.push(u8::from(t.is_buildable_item()));
    v.push(u8::from(t.can_possibly_have_any_weapon()));
    put_f32(v, t.get_structure_rubble_height());
    put_i32(v, i32::from(t.buildable()));
    put_u32(v, t.get_threat_value());
}

fn dump_chain(v: &mut Vec<u8>, c: &Ctx, root: &Overridable<ThingTemplate>, roots: &[u64], bug: u32) {
    let mut n = 0;
    let mut cur = Some(root);
    while let Some(o) = cur {
        n += 1;
        cur = o.get_next_override();
    }
    put_i32(v, n);
    let mut cur = Some(root);
    while let Some(o) = cur {
        v.push(u8::from(o.is_override()));
        dump_template(v, c, &o.data, roots, bug);
        cur = o.get_next_override();
    }
}

/// `ref_tt_dump`.
fn rust_dump(c: &Ctx, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    let roots = root_ids(c);
    put_u32(&mut v, u32::from(c.factory.next_template_id()));
    put_i32(&mut v, c.factory.template_count() as i32);
    for t in c.factory.templates() {
        dump_chain(&mut v, c, t, &roots, bug);
    }
    let entries: Vec<(&[u8], usize)> = c.factory.template_map_entries().collect();
    put_u32(&mut v, entries.len() as u32);
    for (name, pos) in entries {
        put_ascii(&mut v, name);
        put_i32(&mut v, pos as i32);
    }
    let kept = c.modules.module_data_list();
    put_u32(&mut v, kept.len() as u32);
    for d in kept {
        put_u32(&mut v, d.module_tag_name_key().unwrap().0);
    }
    v
}

fn cpp_buf(f: impl FnOnce(*mut u8, i32) -> i32) -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 22];
    let n = f(buf.as_mut_ptr(), buf.len() as i32);
    assert!((n as usize) <= buf.len(), "dump overflow");
    buf.truncate(n as usize);
    buf
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

fn pick<'a, T: ?Sized>(g: &mut Gen, xs: &[&'a T]) -> &'a T {
    xs[g.below(xs.len() as u32) as usize]
}

/// The names of templates. Names differing only in case are different templates.
const POOL: [&str; 7] = ["T1", "T2", "T3", "T4", "t1", "Boss", "DefaultThingTemplate"];
const ARMORS: [&str; 4] = ["ArmorA", "ArmorB", "None", "ArmorGhost"];
const FXS: [&str; 4] = ["FxA", "FxB", "None", "FxGhost"];
const WEAPONS: [&str; 5] = ["Rifle", "Cannon", "Missile", "None", "WeaponGhost"];
const SCIENCES: [&str; 4] = ["SCIENCE_A", "SCIENCE_B", "SCIENCE_Ghost", "science_a"];
const TAGS: [&str; 9] = [
    "ModuleTag_A",
    "ModuleTag_B",
    "ModuleTag_C",
    "ModuleTag_D",
    "ModuleTag_E",
    "ModuleTag_DefaultAutoHealBehavior",
    "ModuleTag_Draw",
    "ModuleTag_Client",
    "ModuleTag_F",
];
const BEHAVIORS: [&str; 8] = ["TestUpdate", "TestUpdate2", "TestDie", "TestDie2", "TestMulti", "TestUpgrade", "TestUpdate", "TestDie"];
const BODIES: [&str; 2] = ["TestBody", "TestBody2"];
const DRAWS: [&str; 2] = ["TestDraw", "TestDraw2"];
const STRINGS: [&str; 8] = ["Alpha", "Name_1", "\"quoted words\"", "x=y", "a/b.tga", "\"unterminated", "", "Menu.wnd"];
const SOUNDS: [&str; 6] = ["Boom", "NoSound", "nosound", "Voice_1", "", "NOSOUND"];

fn gen_real(g: &mut Gen, clean: bool) -> String {
    match g.below(14) {
        0..=5 => format!("{:.2}", g.uniform(-50.0, 500.0)),
        6 => format!("{}", g.below(300)),
        7 => pick(g, &["0", "-0", "0.0", "1", "100.5", "-1", "3.4028235e38", "1e-45"]).to_string(),
        _ if clean => format!("{:.1}", g.uniform(0.0, 100.0)),
        8 => pick(g, &["abc", "", "1.2.3", "12abc", "--1", "."]).to_string(),
        9 => pick(g, &["1e39", "inf", "nan", "0x10"]).to_string(),
        _ => format!("{:.3}", g.uniform(-5.0, 5.0)),
    }
}
fn gen_int(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=5 => format!("{}", g.below(2000)),
        6 => format!("-{}", g.below(200)),
        _ if clean => format!("{}", g.below(100)),
        7 => pick(g, &["abc", "", "1e3", "12abc", "+5"]).to_string(),
        8 => pick(g, &["99999999999", "2147483648"]).to_string(),
        _ => format!("{}", g.below(70000)),
    }
}
fn gen_small(g: &mut Gen, clean: bool, max: u32) -> String {
    if clean || g.below(10) < 7 {
        return format!("{}", g.below(max + 1));
    }
    pick(g, &["-1", "256", "65536", "x", "", "70000", "1.5"]).to_string()
}
fn gen_bool(g: &mut Gen, clean: bool) -> String {
    if clean || g.below(10) < 7 {
        return pick(g, &["Yes", "No", "yes", "NO"]).to_string();
    }
    pick(g, &["1", "0", "True", "maybe", ""]).to_string()
}
fn gen_string(g: &mut Gen, clean: bool) -> String {
    if clean {
        return pick(g, &STRINGS[..2]).to_string();
    }
    pick(g, &STRINGS).to_string()
}
fn gen_index(g: &mut Gen, names: &[&str], clean: bool) -> String {
    if clean || g.below(10) < 8 {
        let n = pick(g, names);
        return if g.below(8) == 0 { n.to_ascii_lowercase() } else { n.to_string() };
    }
    pick(g, &["NOPE", "", "5", "None"]).to_string()
}
fn gen_names(g: &mut Gen, names: &[&str], max: u32) -> String {
    let n = g.below(max + 1);
    let v: Vec<&str> = (0..n).map(|_| pick(g, names)).collect();
    v.join(" ")
}
fn gen_color(g: &mut Gen, clean: bool) -> String {
    let c = |g: &mut Gen| g.below(256);
    if clean || g.below(4) != 0 {
        return format!("R:{} G:{} B:{}", c(g), c(g), c(g));
    }
    match g.below(5) {
        0 => format!("R:{} G:{} B:{}", 256 + g.below(50), c(g), c(g)),
        1 => format!("G:{} R:{} B:{}", c(g), c(g), c(g)),
        2 => format!("R:{} G:{}", c(g), c(g)),
        3 => pick(g, &["", "R:", "R:x G:1 B:2"]).to_string(),
        _ => format!("R:{} G:{} B:{} A:{}", c(g), c(g), c(g), c(g)),
    }
}
/// A bit list: plain names, or `+`/`-` edits, or `NONE`, or a mistake.
fn gen_bits(g: &mut Gen, names: &[&str], clean: bool, allow_edit: bool) -> String {
    let n = 1 + g.below(4);
    let pick_name = |g: &mut Gen| names[g.below(names.len() as u32) as usize].to_string();
    match g.below(12) {
        0 => "NONE".to_string(),
        1 if allow_edit => (0..n).map(|_| format!("{}{}", pick(g, &["+", "-"]), pick_name(g))).collect::<Vec<_>>().join(" "),
        2 if !clean => format!("{} NONE", pick_name(g)),
        3 if !clean => format!("{} +{}", pick_name(g), pick_name(g)),
        4 if !clean => pick(g, &["NOT_A_BIT", "", "+NOT_A_BIT"]).to_string(),
        _ => (0..n).map(|_| pick_name(g)).collect::<Vec<_>>().join(" "),
    }
}

fn ind(g: &mut Gen) -> &'static str {
    pick(g, &["  ", " ", "\t", "    ", "", "  ", "  "])
}
fn sep(g: &mut Gen) -> &'static str {
    pick(g, &[" = ", "=", " ", " =", "= ", "\t= ", " = ", " = "])
}
fn comment(g: &mut Gen) -> &'static str {
    if g.below(14) == 0 {
        pick(g, &[" ; comment", ";x", "  ; Yes"])
    } else {
        ""
    }
}

/// What the generator may use.
struct Gx<'a> {
    g: &'a mut Gen,
    /// Only well-formed values.
    clean: bool,
    /// Only the fields that concern modules (and what the module rules read).
    modules_only: bool,
    /// Whether the module blocks may use an unknown name (the wrapper's factory has none: not run on the C++).
    allow_ghost_modules: bool,
}

impl Gx<'_> {
    fn line(&mut self, body: String) -> String {
        format!("{}{}{}", ind(self.g), body, comment(self.g))
    }

    /// One module sub-block: the line and its `End`.
    fn module_block(&mut self, out: &mut Vec<String>, kind: &str) {
        let g = &mut *self.g;
        let tag = if g.below(10) == 0 { String::new() } else { pick(g, &TAGS).to_string() };
        let (field, name): (&str, String) = match kind {
            "Body" => ("Body", if g.below(8) == 0 { pick(g, &BEHAVIORS).to_string() } else { pick(g, &BODIES).to_string() }),
            "Draw" => ("Draw", pick(g, &DRAWS).to_string()),
            "ClientUpdate" => ("ClientUpdate", "TestClient".to_string()),
            _ => (
                "Behavior",
                if g.below(14) == 0 && !self.clean {
                    pick(g, &BODIES).to_string()
                } else if self.allow_ghost_modules && g.below(30) == 0 {
                    pick(g, &["NoSuchModule", "AutoHealBehavior", "TestDraw"]).to_string()
                } else {
                    pick(g, &BEHAVIORS).to_string()
                },
            ),
        };
        let s = sep(g);
        let head = if tag.is_empty() { format!("{field}{s}{name}") } else { format!("{field}{s}{name} {tag}") };
        out.push(self.line(head));
        if kind == "Draw" || kind == "ClientUpdate" {
            for _ in 0..self.g.below(5) {
                match self.g.below(5) {
                    0 | 1 => {
                        let nested = pick(self.g, &["DefaultConditionState", "ConditionState", "AliasConditionState", "TransitionState"]);
                        let arg = pick(self.g, &["", " = DAMAGED", " NIGHT", " = A B"]);
                        out.push(self.line(format!("{nested}{arg}")));
                        for _ in 0..self.g.below(3) {
                            let (k, v) = (pick(self.g, &["Model", "Animation", "Flags", "WeaponFireFXBone"]), pick(self.g, &["X", "A B", "1 2 3"]));
                        out.push(self.line(format!("{k} = {v}")));
                        }
                        if self.g.below(30) != 0 {
                            out.push(self.line("End".to_string()));
                        }
                    }
                    _ => {
                        let f = pick(self.g, &["OkToChangeModelColor", "Foo", "Bar", "ConditionStates", "TrackMarks", "Bones"]);
                        let a = pick(self.g, &["Yes", "1 2", "", "x y z", "End2"]);
                        out.push(self.line(format!("{f} = {a}")));
                    }
                }
            }
        } else if !self.clean && self.g.below(40) == 0 {
            out.push(self.line("Garbage = 1".to_string()));
        }
        if self.clean || self.g.below(60) != 0 {
            out.push(self.line("End".to_string()));
        }
    }

    /// The lines of one field of the `Object` table (by its index in `OBJECT_FIELD_NAMES`).
    fn field(&mut self, out: &mut Vec<String>, name: &str) {
        let clean = self.clean;
        let s = sep(self.g);
        let g = &mut *self.g;
        let body = match name {
            "DisplayName" => format!("{name}{s}{}", if g.below(10) == 0 && !clean { "NOTEXT_Label" } else { pick(g, &["OBJECT:Tank", "Label_A", "X"]) }),
            "RadarPriority" => format!("{name}{s}{}", gen_index(g, &["INVALID", "NOT_ON_RADAR", "STRUCTURE", "UNIT", "LOCAL_UNIT_ONLY"], clean)),
            "TransportSlotCount" | "StructureRubbleHeight" | "CrusherLevel" | "CrushableLevel" => format!("{name}{s}{}", gen_small(g, clean, 255)),
            "BuildCost" | "RefundValue" | "ThreatValue" => format!("{name}{s}{}", gen_small(g, clean, 65535)),
            "FenceWidth" | "FenceXOffset" | "VisionRange" | "ShroudClearingRange" | "ShroudRevealToAllRange" | "PlacementViewAngle"
            | "FactoryExitWidth" | "FactoryExtraBibWidth" | "BuildTime" | "Scale" | "ShadowSizeX" | "ShadowSizeY" | "ShadowOffsetX"
            | "ShadowOffsetY" | "InstanceScaleFuzziness" | "GeometryMajorRadius" | "GeometryMinorRadius" | "GeometryHeight" => {
                format!("{name}{s}{}", gen_real(g, clean))
            }
            "IsBridge" | "IsTrainable" | "EnterGuard" | "HijackGuard" | "IsForbidden" | "IsPrerequisite" | "GeometryIsSmall" => {
                format!("{name}{s}{}", gen_bool(g, clean))
            }
            "SkillPointValue" | "ExperienceValue" | "ExperienceRequired" => {
                let n = if clean || g.below(10) < 8 { 4 } else { g.below(7) };
                let v: Vec<String> = (0..n).map(|_| gen_int(g, clean)).collect();
                format!("{name}{s}{}", v.join(" "))
            }
            "Side" | "CommandSet" | "SelectPortrait" | "ButtonImage" | "ShadowTexture" | "UpgradeCameo1" | "UpgradeCameo2" | "UpgradeCameo3"
            | "UpgradeCameo4" | "UpgradeCameo5" => format!("{name}{s}{}", gen_string(g, clean)),
            "Buildable" => format!("{name}{s}{}", gen_index(g, &["Yes", "Ignore_Prerequisites", "No", "Only_By_AI"], clean)),
            "BuildCompletion" => format!("{name}{s}{}", gen_index(g, &["INVALID", "APPEARS_AT_RALLY_POINT", "PLACED_BY_PLAYER"], clean)),
            "EditorSorting" => format!("{name}{s}{}", gen_index(g, &rts_lockstep_engine::common::thing_sort::EDITOR_SORTING_NAMES, clean)),
            "EnergyProduction" | "EnergyBonus" => format!("{name}{s}{}", gen_int(g, clean)),
            "DisplayColor" => format!("{name}{s}{}", gen_color(g, clean)),
            "KindOf" => format!("{name}{s}{}", gen_bits(g, KindOfMaskType::get_bit_names(), clean, true)),
            "BuildVariations" => format!("{name}{s}{}", gen_names(g, &POOL, 3)),
            "OcclusionDelay" => format!("{name}{s}{}", if clean || g.below(10) < 8 { format!("{}", g.below(5000)) } else { gen_int(g, false) }),
            "Shadow" => format!("{name}{s}{}", gen_bits(g, &rts_lockstep_engine::common::thing_template::SHADOW_NAMES, clean, false)),
            "Geometry" => format!("{name}{s}{}", gen_index(g, &["SPHERE", "CYLINDER", "BOX"], clean)),
            "MaxSimultaneousOfType" => format!(
                "{name}{s}{}",
                if g.below(5) == 0 { pick(g, &["DeterminedBySuperweaponRestriction", "determinedbysuperweaponrestriction"]).to_string() } else { gen_small(g, clean, 65535) }
            ),
            "MaxSimultaneousLinkKey" => format!("{name}{s}{}", pick(g, &["LinkA", "LinkB", "linka", ""])),
            "Locomotor" => format!("{name}{s}{}", pick(g, &["SET_NORMAL Loco", "", "SET_WANDER None"])),
            "RemoveModule" => format!("{name}{s}{}", if g.below(10) == 0 { String::new() } else { pick(g, &TAGS).to_string() }),
            n if n.starts_with("Voice") || n.starts_with("Sound") => format!("{name}{s}{}", pick(g, &SOUNDS)),
            _ => format!("{name}{s}X"),
        };
        match name {
            "ArmorSet" => {
                out.push(self.line("ArmorSet".to_string()));
                for _ in 0..self.g.below(4) {
                    let g = &mut *self.g;
                    let l = match g.below(6) {
                        0 | 1 => format!("Conditions = {}", gen_bits(g, &rts_lockstep_engine::logic::armor_set::ARMOR_SET_NAMES, clean, true)),
                        2 | 3 => format!("Armor = {}", pick(g, &ARMORS)),
                        4 => format!("DamageFX = {}", pick(g, &FXS)),
                        _ => format!("{} = 1", pick(g, &["Foo", "armor", "Conditions2"])),
                    };
                    out.push(self.line(l));
                }
                if self.clean || self.g.below(40) != 0 {
                    out.push(self.line("End".to_string()));
                }
            }
            "WeaponSet" => {
                out.push(self.line("WeaponSet".to_string()));
                for _ in 0..self.g.below(6) {
                    let g = &mut *self.g;
                    let slot = gen_index(g, &["PRIMARY", "SECONDARY", "TERTIARY"], clean);
                    let l = match g.below(8) {
                        0 => format!("Conditions = {}", gen_bits(g, &rts_lockstep_engine::logic::weapon_set_flags::WEAPON_SET_NAMES, clean, true)),
                        1..=3 => format!("Weapon = {slot} {}", pick(g, &WEAPONS)),
                        4 => format!(
                            "AutoChooseSources = {slot} {}",
                            gen_bits(g, &rts_lockstep_engine::logic::weapon_set::COMMAND_SOURCE_MASK_NAMES, clean, true)
                        ),
                        5 => format!("PreferredAgainst = {slot} {}", gen_bits(g, KindOfMaskType::get_bit_names(), clean, true)),
                        6 => format!("{} = {}", pick(g, &["ShareWeaponReloadTime", "WeaponLockSharedAcrossSets"]), gen_bool(g, clean)),
                        _ => format!("{} = 1", pick(g, &["Foo", "weapon"])),
                    };
                    out.push(self.line(l));
                }
                if self.clean || self.g.below(40) != 0 {
                    out.push(self.line("End".to_string()));
                }
            }
            "Prerequisites" => {
                out.push(self.line("Prerequisites".to_string()));
                for _ in 0..self.g.below(4) {
                    let g = &mut *self.g;
                    let l = match g.below(7) {
                        0..=3 => format!("Object = {}", gen_names(g, &POOL, 3).trim().to_string() + if g.below(14) == 0 && !clean { "" } else { " T2" }),
                        4 | 5 => format!("Science = {}", pick(g, &SCIENCES)),
                        _ => format!("{} = 1", pick(g, &["Foo", "Object2"])),
                    };
                    out.push(self.line(l));
                }
                if self.clean || self.g.below(40) != 0 {
                    out.push(self.line("End".to_string()));
                }
            }
            "UnitSpecificSounds" | "UnitSpecificFX" => {
                out.push(self.line(name.to_string()));
                for _ in 0..self.g.below(5) {
                    let g = &mut *self.g;
                    let key = pick(g, &["TankTurretMove", "tankturretmove", "Tracks", "Boom", "A"]);
                    let value = if name == "UnitSpecificFX" { pick(g, &["FX_A", "None", "none", "FX_Boom", ""]) } else { pick(g, &SOUNDS) };
                    out.push(self.line(format!("{key} = {value}")));
                }
                if self.clean || self.g.below(40) != 0 {
                    out.push(self.line("End".to_string()));
                }
            }
            "Behavior" => self.module_block(out, "Behavior"),
            "Body" => self.module_block(out, "Body"),
            "Draw" => self.module_block(out, "Draw"),
            "ClientUpdate" => self.module_block(out, "ClientUpdate"),
            "AddModule" | "InheritableModule" | "OverrideableByLikeKind" => {
                out.push(self.line(name.to_string()));
                for _ in 0..1 + self.g.below(3) {
                    let kind = pick(self.g, &["Behavior", "Behavior", "Body", "Draw", "ClientUpdate"]);
                    self.module_block(out, kind);
                    if self.g.below(12) == 0 {
                        let vr = 1 + self.g.below(9);
                out.push(self.line(format!("VisionRange = {vr}")));
                    }
                }
                if self.clean || self.g.below(40) != 0 {
                    out.push(self.line("End".to_string()));
                }
            }
            "ReplaceModule" => {
                let tag = pick(self.g, &TAGS);
                let sp = sep(self.g);
                out.push(self.line(format!("ReplaceModule{sp}{tag}")));
                for _ in 0..self.g.below(3) {
                    let kind = pick(self.g, &["Behavior", "Behavior", "Body", "Draw", "ClientUpdate"]);
                    self.module_block(out, kind);
                }
                if self.clean || self.g.below(40) != 0 {
                    out.push(self.line("End".to_string()));
                }
            }
            _ => out.push(self.line(body)),
        }
    }

    /// An `Object` block; `name` is the template's name.
    fn object(&mut self, out: &mut Vec<String>, name: &str) {
        out.push(format!("Object {name}{}", comment(self.g)));
        let n = if self.modules_only { 1 + self.g.below(7) } else { 1 + self.g.below(22) };
        let module_fields = ["Behavior", "Body", "Draw", "ClientUpdate", "AddModule", "RemoveModule", "ReplaceModule", "InheritableModule", "OverrideableByLikeKind"];
        let other_in_module_mode = ["KindOf", "IsTrainable", "ArmorSet", "WeaponSet", "Side", "VisionRange"];
        for _ in 0..n {
            let field: &str = if self.modules_only {
                if self.g.below(6) == 0 { pick(self.g, &other_in_module_mode) } else { pick(self.g, &module_fields) }
            } else {
                OBJECT_FIELD_NAMES[self.g.below(OBJECT_FIELD_NAMES.len() as u32) as usize]
            };
            self.field(out, field);
        }
        if self.clean || self.g.below(60) != 0 {
            out.push("End".to_string());
        }
    }

    /// An `ObjectReskin` block.
    fn reskin(&mut self, out: &mut Vec<String>, name: &str, from: &str) {
        out.push(format!("ObjectReskin {name} {from}{}", comment(self.g)));
        for _ in 0..self.g.below(5) {
            let field = RESKIN_FIELD_NAMES[self.g.below(RESKIN_FIELD_NAMES.len() as u32) as usize];
            self.field(out, field);
        }
        if self.clean || self.g.below(60) != 0 {
            out.push("End".to_string());
        }
    }

    fn name(&mut self) -> &'static str {
        // the default template is the common one in the module sessions
        if self.modules_only && self.g.below(4) == 0 {
            return "DefaultThingTemplate";
        }
        if self.g.below(6) == 0 {
            "DefaultThingTemplate"
        } else {
            pick(self.g, &POOL[..6])
        }
    }
}

/// The `Armor`, `DamageFX` and `Science` blocks that the generated names refer to (a share of them).
fn preamble(g: &mut Gen) -> Vec<String> {
    let mut out = Vec::new();
    if g.below(10) < 8 {
        for (n, c) in [("ArmorA", "FLAME 50%"), ("ArmorB", "CRUSH 25%")] {
            if g.below(4) != 0 {
                out.push(format!("Armor {n}\n Armor = Default 100%\n Armor = {c}\nEnd"));
            }
        }
        for n in ["FxA", "FxB"] {
            if g.below(4) != 0 {
                out.push(format!("DamageFX {n}\n AmountForMajorFX = Default 10\nEnd"));
            }
        }
        for n in ["SCIENCE_A", "SCIENCE_B"] {
            if g.below(4) != 0 {
                out.push(format!("Science {n}\n DisplayName = NAME:{n}\n IsGrantable = No\nEnd"));
            }
        }
    }
    out
}

/// One file of object blocks.
fn gen_file(g: &mut Gen, modules_only: bool, with_preamble: bool) -> Vec<u8> {
    let clean = g.below(10) < 4;
    let allow_ghost_modules = !modules_only && g.below(4) == 0;
    let mut lines = Vec::new();
    if with_preamble {
        lines.extend(preamble(g));
    }
    let mut gx = Gx { g, clean, modules_only, allow_ghost_modules };
    let blocks = 1 + gx.g.below(if modules_only { 4 } else { 3 });
    for _ in 0..blocks {
        match gx.g.below(20) {
            0..=15 => {
                let n = gx.name();
                gx.object(&mut lines, n);
            }
            16 | 17 => {
                let n = gx.name();
                let from = pick(gx.g, &POOL[..6]);
                gx.reskin(&mut lines, n, from);
            }
            18 => lines.push(pick(gx.g, &["; a comment", "", "   ", ";", "\t"]).to_string()),
            _ if !clean => lines.push(pick(gx.g, &["Unknown block", "=", "End", "AIData"]).to_string()),
            _ => lines.push("; noise".to_string()),
        }
    }
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        out.extend_from_slice(l.as_bytes());
        let last = i + 1 == lines.len();
        if !(last && g.below(4) == 0) {
            out.extend_from_slice(if g.below(8) == 0 { b"\r\n" } else { b"\n" });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Running both sides
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    detail: Vec<u8>,
    crc: u32,
}

/// What the Rust says about a load; the `Err` is a failure upstream does not have (undefined upstream, or a
/// module error).
fn rust_outcome(r: IniResult<()>, crc: u32) -> Result<Outcome, IniError> {
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown() {
            None => return Err(e),
            Some(Thrown::Exception(m)) => (1, m),
            Some(Thrown::Status(c)) => (2, c.to_le_bytes().to_vec()),
            Some(Thrown::ErrorCode(c)) => (3, c.to_le_bytes().to_vec()),
            Some(Thrown::OutOfRange) => (4, Vec::new()),
        },
    };
    Ok(Outcome { status, detail, crc })
}

fn cpp_load(name: &[u8], data: &[u8], load_type: IniLoadType, with_xfer: bool) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut msg = vec![0u8; 4096];
    let (mut crc, mut code) = (0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_tt_load(cname.as_ptr(), load_type as i32, i32::from(with_xfer), &mut crc, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
    };
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail, crc: if with_xfer { crc } else { 0 } }
}

fn show(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[derive(Default)]
struct Excluded(BTreeMap<String, u64>);
impl Excluded {
    fn add(&mut self, e: &IniError) {
        let key = match &e.kind {
            IniErrorKind::ModuleUnknown(_) => "ModuleUnknown".to_string(),
            IniErrorKind::ModuleNotTranslated(_) => "ModuleNotTranslated".to_string(),
            k => format!("{k:?}"),
        };
        *self.0.entry(key).or_default() += 1;
    }
    fn total(&self) -> u64 {
        self.0.values().sum()
    }
}

/// The comparisons of the sessions.
struct Checks {
    outcome: Check,
    outcome_bug: Check,
    dump: Check,
    dump_bug_skill: Check,
    dump_bug_weapon_owner: Check,
    dump_bug_audio: Check,
    dump_bug_inheritable: Check,
    dump_bug_reskin: Check,
    keys: Check,
    find: Check,
    find_bug: Check,
    by_id: Check,
    by_id_bug: Check,
    equiv: Check,
    equiv_bug: Check,
    new_object: Check,
    new_object_bug: Check,
    new_drawable: Check,
    sets: Check,
    sets_bug: Check,
    per_unit: Check,
    per_unit_bug: Check,
}

impl Checks {
    fn new() -> Self {
        Checks {
            outcome: Check::new("Object INI::load (throw, message, CRC)"),
            outcome_bug: Check::new("INI::load"),
            dump: Check::new("ThingFactory (every member of all)"),
            dump_bug_skill: Check::new("skill points / experience"),
            dump_bug_weapon_owner: Check::new("WeaponTemplateSet owner"),
            dump_bug_audio: Check::new("first sound"),
            dump_bug_inheritable: Check::new("module inheritable flag"),
            dump_bug_reskin: Check::new("reskinnedFrom"),
            keys: Check::new("NAMEKEY of the probe names"),
            find: Check::new("findTemplate"),
            find_bug: Check::new("findTemplate"),
            by_id: Check::new("findByTemplateID"),
            by_id_bug: Check::new("findByTemplateID"),
            equiv: Check::new("isEquivalentTo"),
            equiv_bug: Check::new("isEquivalentTo"),
            new_object: Check::new("newObject (template, random value)"),
            new_object_bug: Check::new("newObject"),
            new_drawable: Check::new("newDrawable"),
            sets: Check::new("findArmor/WeaponTemplateSet"),
            sets_bug: Check::new("findArmor/WeaponTemplateSet"),
            per_unit: Check::new("getPerUnitSound / getPerUnitFX"),
            per_unit_bug: Check::new("getPerUnitSound"),
        }
    }
}

#[derive(Default)]
struct Tally {
    stale_cache: u64,
    sessions: u64,
    loads: u64,
    loads_ok: u64,
    loads_failed: u64,
    override_loads: u64,
    resets: u64,
    post_process: u64,
    other_ops: u64,
    chains: u64,
    templates: u64,
    override_nodes: u64,
    marked_roots: u64,
    reskins: u64,
    with_modules: u64,
    modules: u64,
    client_modules: u64,
    armor_sets: u64,
    weapon_sets: u64,
    prereqs: u64,
    audio: u64,
    per_unit: u64,
    lookups: u64,
    finds_found: u64,
    equivalent: u64,
    new_objects: u64,
    variations_taken: u64,
    stopped_undefined: u64,
    threw_message: u64,
}

fn position_of(c: &Ctx, name: &[u8]) -> i32 {
    match c.factory.find_template(name) {
        None => -1,
        Some(t) => c.factory.templates().position(|r| r.data.node_id() == t.data.node_id()).unwrap() as i32,
    }
}

/// The node at (position, depth), depth -1 the final override.
fn node_at(c: &Ctx, position: i32, depth: i32) -> Option<&Overridable<ThingTemplate>> {
    let root = c.factory.templates().nth(position as usize)?;
    if depth < 0 {
        return Some(root.get_final_override());
    }
    let mut cur = root;
    for _ in 0..depth {
        cur = cur.get_next_override()?;
    }
    Some(cur)
}

fn chain_len(root: &Overridable<ThingTemplate>) -> i32 {
    let mut n = 1;
    let mut cur = root;
    while let Some(next) = cur.get_next_override() {
        n += 1;
        cur = next;
    }
    n
}

/// The lookups after a step. They give unseen names their keys, in the same order on both sides.
fn probe(c: &mut Ctx, ck: &mut Checks, t: &mut Tally, g: &mut Gen, tag: &str, salt: u32) {
    // the next key
    let probe_name = format!("Probe_{tag}_{salt}");
    let cn = CString::new(probe_name.as_str()).unwrap();
    let rk = c.names.name_to_key(probe_name.as_bytes());
    ck.keys.eq(rk.0, unsafe { cpp::ref_tt_name_key(cn.as_ptr()) }, || format!("{tag}: key"));
    t.lookups += 1;

    // findTemplate, by every name of the pool and the names in the store
    let mut names: Vec<String> = POOL.iter().map(|s| s.to_string()).collect();
    names.push("Nobody".into());
    names.push("T1 ".into());
    names.push(String::new());
    for (name, _) in c.factory.template_map_entries().map(|(n, p)| (show(n), p)).collect::<Vec<_>>() {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    for n in &names {
        let cn = CString::new(n.as_str()).unwrap();
        let cx = unsafe { cpp::ref_tt_find(cn.as_ptr()) };
        let r = position_of(c, n.as_bytes());
        ck.find.eq(r, cx, || format!("{tag}: find {n:?}"));
        ck.find_bug.eq(if r < 0 { r } else { r + 1 }, cx, || format!("{tag}: find {n:?}"));
        t.lookups += 1;
        if r >= 0 {
            t.finds_found += 1;
        }
    }
    // findByTemplateID
    for id in 0..c.factory.next_template_id().saturating_add(2) {
        let cx = unsafe { cpp::ref_tt_find_by_id(u32::from(id)) };
        let r = c
            .factory
            .find_by_template_id(id)
            .map_or(-1, |t| c.factory.templates().position(|x| x.data.node_id() == t.data.node_id()).unwrap() as i32);
        ck.by_id.eq(r, cx, || format!("{tag}: id {id}"));
        ck.by_id_bug.eq(if r < 0 { r } else { r.max(1) - 1 + i32::from(r == 0) }, cx, || format!("{tag}: id {id}"));
        t.lookups += 1;
    }
    // per node
    let n_roots = c.factory.template_count() as i32;
    let mut nodes: Vec<(i32, i32)> = Vec::new();
    for p in 0..n_roots {
        let len = chain_len(node_at(c, p, 0).unwrap());
        nodes.push((p, 0));
        if len > 1 {
            nodes.push((p, -1));
        }
        t.chains += 1;
    }
    // isEquivalentTo on pairs
    for _ in 0..12.min(nodes.len() * nodes.len()) {
        let (a, b) = (nodes[g.below(nodes.len() as u32) as usize], nodes[g.below(nodes.len() as u32) as usize]);
        let cx = unsafe { cpp::ref_tt_equivalent(a.0, a.1, b.0, b.1) };
        let (na, nb) = (node_at(c, a.0, a.1).unwrap(), node_at(c, b.0, b.1).unwrap());
        let r = i32::from(is_equivalent_to(na, nb));
        ck.equiv.eq(r, cx, || format!("{tag}: equivalent {a:?} {b:?}"));
        ck.equiv_bug.eq(1 - r, cx, || format!("{tag}: equivalent {a:?} {b:?}"));
        t.lookups += 1;
        t.equivalent += u64::from(r == 1 && a != b);
    }
    let _ = unsafe { cpp::ref_tt_equivalent(n_roots, 0, 0, 0) };
    // newObject / newDrawable on a few nodes: the random value is seeded the same on both sides
    for _ in 0..3.min(nodes.len()) {
        let (p, d) = nodes[g.below(nodes.len() as u32) as usize];
        let seed = g.u32();
        let mut words = [0u64; 4];
        for w in words.iter_mut() {
            *w = g.u64() & g.u64();
        }
        let mask = ObjectStatusMaskType::from_words(words);
        let node = node_at(c, p, d).unwrap();
        let mut rng = RandomValues::new(1);
        rng.init_game_logic_random(seed);
        let _guard = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { cpp::ref_init_game_logic_random(seed) };
        let plan = c.factory.new_object(Some(&node.data), &mut rng, mask).unwrap();
        let (mut made, mut status) = (0i32, [0u64; 4]);
        let status_code = unsafe { cpp::ref_tt_new_object(p, d, 0, words.as_ptr(), &mut made, status.as_mut_ptr()) };
        assert_eq!(status_code, 0);
        let rust_view = (i32::from(plan.template_id), plan.status_bits.words(), rng.get_game_logic_random_seed_crc());
        let cpp_view = (made, status, unsafe { cpp::ref_get_game_logic_random_seed_crc() });
        ck.new_object.eq(rust_view, cpp_view, || format!("{tag}: newObject {p} {d}"));
        let bad = (rust_view.0 ^ 1, rust_view.1, rust_view.2);
        ck.new_object_bug.eq(bad, cpp_view, || format!("{tag}: newObject {p} {d}"));
        t.new_objects += 1;
        if !node.data.get_build_variations().is_empty() {
            t.variations_taken += u64::from(rust_view.0 != i32::from(node.data.get_template_id()));
        }
        // newDrawable
        let bits = (g.u32() & 0xffff) as i32;
        let mut events = Vec::new();
        c.factory.new_drawable(Some(&node.data), bits as u32, 7, &mut events).unwrap();
        let (mut dmade, mut dstatus) = (0i32, 0i32);
        let dcode = unsafe { cpp::ref_tt_new_drawable(p, d, 0, bits, &mut dmade, &mut dstatus) };
        assert_eq!(dcode, 0);
        let expect = [ClientEvent::ObjectCreated { template_id: dmade as u16, drawable_status_bits: dstatus as u32, object_id: 7 }];
        ck.new_drawable.eq(&events[..], &expect[..], || format!("{tag}: newDrawable {p} {d}"));
    }
    // the null templates
    let mut rng = RandomValues::new(1);
    assert_eq!(c.factory.new_object(None, &mut rng, ObjectStatusMaskType::new()).unwrap_err(), ThingFactoryError::BadArg);
    let (mut made, mut status) = (0i32, [0u64; 4]);
    assert_eq!(unsafe { cpp::ref_tt_new_object(0, 0, 1, status.as_ptr(), &mut made, status.as_mut_ptr()) }, 2);
    assert_eq!(unsafe { cpp::ref_tt_new_drawable(0, 0, 1, 0, &mut made, &mut (status[0] as i32)) }, 2);
    // findArmorTemplateSet / findWeaponTemplateSet, per-unit lookups
    for &(p, d) in nodes.iter().take(4) {
        for which in 0..2 {
            let flags = g.u64() & if which == 0 { 0xff } else { 0x1ffff };
            let cx = unsafe { cpp::ref_tt_find_set(p, d, which, flags) };
            let mut tmpl = node_at(c, p, d).unwrap().data.clone();
            let r = if which == 0 {
                let mut f = rts_lockstep_engine::logic::armor_set::ArmorSetFlags::new();
                for i in 0..8 {
                    if flags >> i & 1 == 1 {
                        f.set(i, 1);
                    }
                }
                let found = tmpl.find_armor_template_set(&f).map(|s| s as *const _);
                found.map_or(-1, |s| tmpl.armor_template_sets().iter().position(|x| std::ptr::eq(x, s)).unwrap() as i32)
            } else {
                let mut f = rts_lockstep_engine::logic::weapon_set_flags::WeaponSetFlags::new();
                for i in 0..17 {
                    if flags >> i & 1 == 1 {
                        f.set(i, 1);
                    }
                }
                let found = tmpl.find_weapon_template_set(&f).map(|s| s as *const _);
                found.map_or(-1, |s| tmpl.get_weapon_template_sets().iter().position(|x| std::ptr::eq(x, s)).unwrap() as i32)
            };
            // -3: upstream's cache, copied with the template, answers with a pointer into the SOURCE's vector (the
            // same set, by content, as long as the source lives); the index is not comparable, the answer is that
            // there is one
            let (r, cx) = if cx == -3 {
                t.stale_cache += 1;
                assert!(r != -1 || tmpl.armor_template_sets().len() + tmpl.get_weapon_template_sets().len() > 0, "{tag}: stale cache but no sets");
                (0, 0)
            } else {
                (r, cx)
            };
            ck.sets.eq(r, cx, || format!("{tag}: set {which} {flags:#x} at {p} {d}"));
            ck.sets_bug.eq(if r < 0 { 0 } else { r + 1 }, cx, || format!("{tag}: set {which} {flags:#x} at {p} {d}"));
            t.lookups += 1;
        }
        let node = node_at(c, p, d).unwrap();
        for key in ["TankTurretMove", "tankturretmove", "Tracks", "Boom", "A", "Zzz", ""] {
            let ckey = CString::new(key).unwrap();
            let mut out = vec![0u8; 256];
            let mut has = 0;
            let n = unsafe { cpp::ref_tt_per_unit(p, d, 0, ckey.as_ptr(), out.as_mut_ptr() as *mut _, 256, &mut has) };
            let cx_sound = out[..n as usize].to_vec();
            let r_sound = node.data.get_per_unit_sound(key.as_bytes()).event_name.clone();
            ck.per_unit.eq(r_sound.clone(), cx_sound.clone(), || format!("{tag}: sound {key:?} at {p} {d}"));
            let mut wrong = r_sound.clone();
            wrong.push(b'!');
            ck.per_unit_bug.eq(wrong, cx_sound, || format!("{tag}: sound {key:?}"));
            let n = unsafe { cpp::ref_tt_per_unit(p, d, 1, ckey.as_ptr(), out.as_mut_ptr() as *mut _, 256, &mut has) };
            let cx_fx = if has == 1 { Some(out[..n as usize].to_vec()) } else { None };
            ck.per_unit.eq(node.data.get_per_unit_fx(key.as_bytes()).map(|s| s.to_vec()), cx_fx, || format!("{tag}: fx {key:?} at {p} {d}"));
            t.lookups += 2;
        }
    }
}

/// Counts what a step's dump held.
fn tally_dump(c: &Ctx, t: &mut Tally) {
    for root in c.factory.templates() {
        let mut cur = Some(root);
        let mut first = true;
        while let Some(o) = cur {
            t.templates += 1;
            if !first {
                t.override_nodes += 1;
            }
            if first && o.is_override() {
                t.marked_roots += 1;
            }
            first = false;
            let d = &o.data;
            if d.reskinned_from().is_some() {
                t.reskins += 1;
            }
            let n = d.get_behavior_module_info().get_count() + d.get_draw_module_info().get_count() + d.get_client_update_module_info().get_count();
            t.modules += n as u64;
            t.client_modules += (d.get_draw_module_info().get_count() + d.get_client_update_module_info().get_count()) as u64;
            t.with_modules += u64::from(n > 0);
            t.armor_sets += d.armor_template_sets().len() as u64;
            t.weapon_sets += d.get_weapon_template_sets().len() as u64;
            t.prereqs += d.get_prereq_count() as u64;
            t.audio += (0..TTAUDIO_COUNT).filter(|&i| d_has_audio(d, i)).count() as u64;
            t.per_unit += (d.get_all_per_unit_sounds().len() + d.per_unit_fx().len()) as u64;
            cur = o.get_next_override();
        }
    }
}

fn d_has_audio(d: &ThingTemplate, i: usize) -> bool {
    // the index is the enum's discriminant
    const ALL: [ThingTemplateAudioType; TTAUDIO_COUNT] = {
        use ThingTemplateAudioType as A;
        [
            A::VoiceSelect,
            A::VoiceGroupSelect,
            A::VoiceSelectElite,
            A::VoiceMove,
            A::VoiceAttack,
            A::VoiceEnter,
            A::VoiceFear,
            A::VoiceCreated,
            A::VoiceNearEnemy,
            A::VoiceTaskUnable,
            A::VoiceTaskComplete,
            A::VoiceMeetEnemy,
            A::SoundMoveStart,
            A::SoundMoveStartDamaged,
            A::SoundMoveLoop,
            A::SoundMoveLoopDamaged,
            A::SoundAmbient,
            A::SoundAmbientDamaged,
            A::SoundAmbientReallyDamaged,
            A::SoundAmbientRubble,
            A::SoundStealthOn,
            A::SoundStealthOff,
            A::SoundCreated,
            A::SoundOnDamaged,
            A::SoundOnReallyDamaged,
            A::SoundEnter,
            A::SoundExit,
            A::SoundPromotedVeteran,
            A::SoundPromotedElite,
            A::SoundPromotedHero,
            A::VoiceGarrison,
            A::SoundFalling,
            A::VoiceDefect,
            A::VoiceAttackSpecial,
            A::VoiceAttackAir,
            A::VoiceGuard,
        ]
    };
    d.has_audio(ALL[i])
}

/// Runs `sessions` sessions; returns the tallies.
fn run_sessions(seed: u64, sessions: usize, modules_only: bool, label: &str) -> (Checks, Tally, Excluded) {
    let table = blocks();
    let mut g = Gen::new(seed);
    let mut ck = Checks::new();
    let mut excluded = Excluded::default();
    let mut t = Tally::default();
    for session in 0..sessions {
        let occlusion = g.below(400);
        let superweapon = g.below(9);
        let mut c = Ctx::new(occlusion, superweapon);
        t.sessions += 1;
        let steps = 2 + g.below(4);
        for step in 0..steps {
            // 0 overwrite load, 1 create-overrides load, 2 reset, 3 postProcessLoad, 4 init/update
            let kind = if step == 0 {
                0
            } else {
                match g.below(24) {
                    0..=10 => 0,
                    11..=17 => 1,
                    18..=19 => 2,
                    20..=21 => 3,
                    _ => 4,
                }
            };
            let tag = format!("{label} session {session} step {step}");
            match kind {
                2..=4 => match kind {
                    2 => {
                        t.resets += 1;
                        let r = c.factory.reset();
                        // the C++ reset is not run on an undefined input: it would free memory that its factory still links
                        if r.is_ok() {
                            unsafe { cpp::ref_tt_op(0) };
                        }
                        if let Err(e) = r {
                            // upstream left a dangling link: the factory is unlinked properly here, and the
                            // walks that follow would read freed memory upstream
                            assert_eq!(e.kind, IniErrorKind::Undefined(rts_lockstep_engine::common::ini::Undefined::DanglingTemplateLink));
                            excluded.add(&e);
                            t.stopped_undefined += 1;
                            break;
                        }
                    }
                    3 => {
                        t.post_process += 1;
                        c.factory.post_process_load();
                        unsafe { cpp::ref_tt_op(3) };
                    }
                    _ => {
                        t.other_ops += 1;
                        if g.below(2) == 0 {
                            c.factory.init();
                            unsafe { cpp::ref_tt_op(1) };
                        } else {
                            c.factory.update();
                            unsafe { cpp::ref_tt_op(2) };
                        }
                    }
                },
                _ => {
                    let overrides = kind == 1;
                    let fresh = step == 0 || g.below(5) == 0;
                    let data = gen_file(&mut g, modules_only, fresh);
                    let load_type = if overrides {
                        IniLoadType::CreateOverrides
                    } else if g.below(12) == 0 {
                        IniLoadType::Multifile
                    } else {
                        IniLoadType::Overwrite
                    };
                    let name = format!("Data\\INI\\Object\\Case{}.ini", session % 7).into_bytes();
                    let with_xfer = g.below(4) != 0;
                    let mut crc = XferCrc::new_without_nan_guard();
                    let r = Ini::load(&name, &data, load_type, if with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None }, &table, &mut c);
                    let crc_v = if with_xfer { crc.get_crc() } else { 0 };
                    let rust = match rust_outcome(r, crc_v) {
                        Ok(o) => o,
                        Err(e) => {
                            excluded.add(&e);
                            t.stopped_undefined += 1;
                            break;
                        }
                    };
                    t.loads += 1;
                    if overrides {
                        t.override_loads += 1;
                    }
                    if rust.status == 0 {
                        t.loads_ok += 1
                    } else {
                        t.loads_failed += 1;
                        t.threw_message += u64::from(rust.status == 1);
                    }
                    let cx = cpp_load(&name, &data, load_type, with_xfer);
                    ck.outcome.eq(&rust, &cx, || format!("{tag}: {load_type:?} file {:?}", show(&data)));
                    let wrong = Outcome { status: rust.status ^ 4, detail: rust.detail.clone(), crc: rust.crc };
                    ck.outcome_bug.eq(&wrong, &cx, || tag.clone());
                }
            }

            // the store, after the step
            let cd = cpp_buf(|b, n| unsafe { cpp::ref_tt_dump(b, n) });
            let file = || tag.clone();
            ck.dump.eq(rust_dump(&c, 0), cd.clone(), file);
            ck.dump_bug_skill.eq(rust_dump(&c, 1), cd.clone(), file);
            ck.dump_bug_weapon_owner.eq(rust_dump(&c, 2), cd.clone(), file);
            ck.dump_bug_audio.eq(rust_dump(&c, 3), cd.clone(), file);
            ck.dump_bug_inheritable.eq(rust_dump(&c, 4), cd.clone(), file);
            ck.dump_bug_reskin.eq(rust_dump(&c, 5), cd, file);
            tally_dump(&c, &mut t);
            probe(&mut c, &mut ck, &mut t, &mut g, &tag, step + session as u32);
        }
    }
    (ck, t, excluded)
}

fn finish(ck: Checks, full: bool) {
    ck.outcome.finish();
    ck.outcome_bug.finish_planted();
    ck.dump.finish();
    ck.dump_bug_skill.finish_planted();
    ck.dump_bug_weapon_owner.finish_planted();
    if full {
        ck.dump_bug_audio.finish_planted();
    }
    ck.dump_bug_inheritable.finish_planted();
    ck.dump_bug_reskin.finish_planted();
    ck.keys.finish();
    ck.find.finish();
    ck.find_bug.finish_planted();
    ck.by_id.finish();
    ck.by_id_bug.finish_planted();
    ck.equiv.finish();
    ck.equiv_bug.finish_planted();
    ck.new_object.finish();
    ck.new_object_bug.finish_planted();
    ck.new_drawable.finish();
    ck.sets.finish();
    ck.sets_bug.finish_planted();
    ck.per_unit.finish();
    if full {
        ck.per_unit_bug.finish_planted();
    }
}

fn report(label: &str, t: &Tally, excluded: &Excluded) {
    println!("{label}: set lookups answered from a copied (stale) cache, index not comparable: {}", t.stale_cache);
    println!(
        "{label}: {} sessions, {} loads ({} overrides, {} ok, {} threw, {} with a message), {} resets, {} postProcessLoad, {} init/update; \
         compared {} template objects in {} chains ({} override copies, {} templates made for a map, {} reskins, {} with modules: {} modules of which {} client, \
         {} armor sets, {} weapon sets, {} prerequisites, {} sounds set, {} per-unit entries), {} lookups ({} finds found, {} equivalent pairs, {} newObject, {} of those \
         that took a build variation); steps stopped at an undefined or module-error input: {} {:?}",
        t.sessions,
        t.loads,
        t.override_loads,
        t.loads_ok,
        t.loads_failed,
        t.threw_message,
        t.resets,
        t.post_process,
        t.other_ops,
        t.templates,
        t.chains,
        t.override_nodes,
        t.marked_roots,
        t.reskins,
        t.with_modules,
        t.modules,
        t.client_modules,
        t.armor_sets,
        t.weapon_sets,
        t.prereqs,
        t.audio,
        t.per_unit,
        t.lookups,
        t.finds_found,
        t.equivalent,
        t.new_objects,
        t.variations_taken,
        excluded.total(),
        excluded.0
    );
}

#[test]
fn object_blocks_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (ck, t, excluded) = run_sessions(0x7A11, 3_000, false, "Object INI");
    report("Object INI", &t, &excluded);
    assert!(t.loads_ok > 500 && t.loads_failed > 1500, "the generator must reach both outcomes: {} ok, {} threw", t.loads_ok, t.loads_failed);
    assert!(t.templates > 8000 && t.override_nodes > 500 && t.marked_roots > 100 && t.reskins > 100, "too few templates compared");
    assert!(t.with_modules > 1000 && t.client_modules > 300 && t.armor_sets > 500 && t.weapon_sets > 300 && t.prereqs > 500, "too few of each part compared");
    assert!(t.audio > 3000 && t.per_unit > 500 && t.equivalent > 100 && t.variations_taken > 20, "too few lookups of each kind");
    finish(ck, true);
}

#[test]
fn module_lists_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (ck, t, excluded) = run_sessions(0xB0DE, 3_000, true, "module lists");
    report("module lists", &t, &excluded);
    assert!(t.loads_ok > 500 && t.loads_failed > 1000, "the generator must reach both outcomes: {} ok, {} threw", t.loads_ok, t.loads_failed);
    assert!(t.modules > 8000 && t.client_modules > 1500 && t.override_nodes > 800, "too few modules compared");
    finish(ck, false);
}

#[test]
fn tables_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { cpp::ref_tt_reset(0, 0) };
    let mut ck = Check::new("field tables (tokens, in order)");
    for (which, names) in [(0, &OBJECT_FIELD_NAMES[..]), (1, &RESKIN_FIELD_NAMES[..])] {
        let b = cpp_buf(|b, n| unsafe { cpp::ref_tt_fields(which, b, n) });
        let count = i32::from_le_bytes(b[0..4].try_into().unwrap()) as usize;
        let mut at = 4;
        let mut tokens = Vec::new();
        for _ in 0..count {
            let len = u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize;
            tokens.push(show(&b[at + 4..at + 4 + len]));
            at += 4 + len;
        }
        // upstream's table also names VoiceSurrender under ALLOW_SURRENDER, which no build defines
        let rust: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        ck.eq(rust.len(), tokens.len(), || format!("table {which}: count"));
        ck.eq(rust, tokens, || format!("table {which}"));
    }
    ck.finish();
}
