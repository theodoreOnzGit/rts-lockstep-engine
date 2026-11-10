#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ScoreScreen.cpp (populatePlayerInfo up to its
//   skirmish tail, grabMultiPlayerInfo, grabSinglePlayerInfo, hideWindows, setObserverWindows, populateSideInfo)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The score screen's grid (`client::score_screen::grab_multi_player_info`, `grab_single_player_info` and the functions
//! they call) against the C++, #89 part 2.
//!
//! # Method
//!
//! The C++ side is the pieces build.rs cuts verbatim from `ScoreScreen.cpp` (`struct ScoreGather`, `populatePlayerInfo`
//! up to its skirmish tail, and the file from `grabMultiPlayerInfo` to its end), compiled by
//! `differential/wrap_score_grid.cpp` on stand-ins for the windows (hidden, text, enabled text colour, enabled image,
//! the status' image bit), the name keys, the game's text (`T:<label>`), the mapped images (the case's names), the
//! players (`ThePlayerList`, each player's score keeper totals, relationships, template), the academy's advice (the
//! case's tips) and `TheGameLogic`'s mode. The Rust side runs the translated functions on a real window manager made
//! from the synthetic `ScoreScreen.wnd` (`shell_gui::synthetic::score_screen_wnd`), with a [`ScoreGridHost`] over the
//! same data. Before the call both sides hide the same windows (a random mask) and give the parent the same image bit;
//! the C++ windows start with the Rust windows' colours. Both dump the parent's image and image bit, then each grid
//! window (rows 0 .. 7: the name, the observer marker, the seven numbers, the winner marker), the academy's list box
//! (its rows) and title: hidden, text, colour, image.
//!
//! A case: single player (`grabSinglePlayerInfo`) or multiplayer (`grabMultiPlayerInfo`), the screen type (single
//! player, LAN, internet or replay: a skirmish's tail is not in the cut), the game mode (single player, LAN, skirmish,
//! replay, internet: the academy's advice is given in a skirmish or internet game), the name override; 1 to 16 players
//! listed, each named from `player0` .. `player7`, `Neutral` or nothing (names repeat), observer or not, human or not,
//! listed on the score screen or not, a template (four, each with an icon and a score screen image that may be missing
//! from the collection) or none (then its base side is empty or `Observer`), a base side (`USA`, `China`, `GLA`,
//! `Observer`, `usa`), totals small enough that scores tie often (and now and then near `i32::MAX`, to wrap), a
//! relationship to every player; the local player one of them, or none.
//!
//! # Predictions (written before the first run)
//!
//! 0 mismatches. Planted bugs (`L9_PLANT=<id>`, each in its own run), all caught:
//!
//! | id | planted bug | predicted |
//! |---|---|---|
//! | 8921 | a tie's `adder` not counted up | caught when three scores tie (the third replaces the second) |
//! | 8922 | the scores lowest first | caught in nearly every multiplayer case with two rows |
//! | 8923 | allies and enemies swapped | caught in most single player cases with a listed player of a side |
//! | 8924 | the side's marker shown without an image | caught when a side's template's icon is missing |
//!
//! # Results (2026-11-10, `SCORE_GRID_CASES=20000`, the default)
//!
//! As predicted, on the first run: 20,000 cases (10,069 single player), 79,843 named rows and 5,854 academy rows
//! matched, **0 mismatches**.
//!
//! Planted bugs: 4 of 4 caught: 8921 in 831 cases, 8922 in 8,849, 8923 in 8,969, 8924 in 4,071. (8923 is caught in
//! almost every single player case, more than predicted: a case's relationships are random, so a listed player of some
//! side is nearly always an ally or an enemy.)

use rts_lockstep_engine::client::campaign_manager::CampaignManager;
use rts_lockstep_engine::client::challenge_generals::ChallengeGenerals;
use rts_lockstep_engine::client::gadget;
use rts_lockstep_engine::client::gadget_list_box as lb;
use rts_lockstep_engine::client::score_screen::{grab_multi_player_info, grab_single_player_info, ScoreGridHost, ScorePlayer, ScoreScreen, ScoreScreenGame, ScoreTemplate};
use rts_lockstep_engine::client::shell_gui::synthetic::score_screen_wnd;
use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::client::window_script::{win_status, WindowScript, WndHost};
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::recorder::RecorderMode;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::score_keeper::ScoreKeeper;
use std::collections::{BTreeMap, BTreeSet};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

const NAMES: [&str; 10] = ["player0", "player1", "player2", "player3", "player4", "player5", "player6", "player7", "Neutral", ""];
const SIDES: [&str; 5] = ["USA", "China", "GLA", "Observer", "usa"];
const ICONS: [&str; 5] = ["IconUSA", "IconChina", "IconGLA", "Missing", ""];
const SCREENS: [&str; 4] = ["ScoreA", "ScoreB", "Nope", ""];
const IMAGES: [&str; 6] = ["IconUSA", "IconChina", "IconGLA", "ScoreA", "ScoreB", "MutiPlayer_ScoreScreen"];
const ROW: [&str; 10] = [
    "StaticTextPlayer",
    "StaticTextObserver",
    "StaticTextUnitsBuilt",
    "StaticTextUnitsLost",
    "StaticTextUnitsDestroyed",
    "StaticTextBuildingsBuilt",
    "StaticTextBuildingsLost",
    "StaticTextBuildingsDestroyed",
    "StaticTextResources",
    "GameWindowWinner",
];

/// A player of a case.
#[derive(Clone, Debug)]
struct P {
    name: &'static str,
    color: i32,
    observer: bool,
    human: bool,
    listed: bool,
    side: &'static str,
    template: i32,
    /// me, earned, spent, units built, lost, buildings built, lost
    totals: [i32; 7],
    units_destroyed: [i32; 4],
    buildings_destroyed: [i32; 4],
    rel: [i32; 16],
}

#[derive(Clone, Debug)]
struct Case {
    single: bool,
    screen_type: i32,
    mode: i32,
    over: bool,
    count: i32,
    local: i32,
    parent_image_bit: bool,
    images: Vec<&'static str>,
    templates: [(&'static str, &'static str); 4],
    tips: Vec<&'static str>,
    players: Vec<P>,
    /// the windows hidden before the call, in dump order
    hidden: Vec<bool>,
}

fn small(r: &mut Rng) -> i32 {
    if r.chance(3) {
        i32::MAX - r.below(3) as i32
    } else {
        r.below(3) as i32
    }
}

fn gen_case(r: &mut Rng) -> Case {
    let templates = std::array::from_fn(|_| (ICONS[r.below(ICONS.len() as u64) as usize], SCREENS[r.below(SCREENS.len() as u64) as usize]));
    let players = (0..16)
        .map(|_| {
            let template = if r.chance(25) { -1 } else { r.below(4) as i32 };
            let side = if template < 0 { ["", "Observer"][r.below(2) as usize] } else { SIDES[r.below(SIDES.len() as u64) as usize] };
            P {
                name: NAMES[r.below(NAMES.len() as u64) as usize],
                color: r.next() as i32,
                observer: r.chance(15),
                human: r.chance(50),
                listed: r.chance(70),
                side,
                template,
                totals: [r.below(16) as i32, small(r) * 100, small(r), small(r), small(r), small(r), small(r)],
                units_destroyed: std::array::from_fn(|_| small(r)),
                buildings_destroyed: std::array::from_fn(|_| small(r)),
                rel: std::array::from_fn(|_| r.below(3) as i32),
            }
        })
        .collect();
    let images = IMAGES.iter().copied().filter(|_| r.chance(60)).collect();
    let tips = ["ACADEMY:A", "ACADEMY:B", "ACADEMY:C"].iter().copied().take(r.below(4) as usize).collect();
    Case {
        single: r.chance(50),
        screen_type: [0, 2, 3, 4][r.below(4) as usize],
        mode: [0, 1, 2, 3, 5][r.below(5) as usize],
        over: r.chance(50),
        count: 1 + r.below(16) as i32,
        local: if r.chance(10) { -1 } else { r.below(16) as i32 },
        parent_image_bit: r.chance(30),
        images,
        templates,
        tips,
        players,
        hidden: (0..ROW.len() * 8 + 2).map(|_| r.chance(30)).collect(),
    }
}

/// The reader's host for the synthetic file.
struct Wnd(BTreeMap<Vec<u8>, u32>);

impl WndHost for Wnd {
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        let n = self.0.len() as u32 + 1;
        *self.0.entry(name.to_vec()).or_insert(n)
    }
    fn find_image(&self, _name: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        label.iter().map(|&b| u16::from(b)).collect()
    }
    fn display_size(&self) -> (i32, i32) {
        (800, 600)
    }
}

/// The Rust side's host: the statics, the keys the file was read with, the case's images.
struct Host {
    ss: ScoreScreen,
    keys: BTreeMap<Vec<u8>, u32>,
    images: BTreeSet<Vec<u8>>,
}

impl GuiHost for Host {}

impl ScoreGridHost for Host {
    fn score_screen(&mut self) -> &mut ScoreScreen {
        &mut self.ss
    }
    fn grid_name_to_key(&mut self, name: &[u8]) -> u32 {
        let n = self.keys.len() as u32 + 1;
        *self.keys.entry(name.to_vec()).or_insert(n)
    }
    fn grid_fetch_text(&self, label: &[u8]) -> Vec<u16> {
        [b"T:".as_slice(), label].concat().iter().map(|&b| u16::from(b)).collect()
    }
    fn grid_find_image(&self, name: &[u8]) -> Option<Vec<u8>> {
        self.images.contains(name).then(|| name.to_vec())
    }
    fn score_untranslated(&mut self, _what: &[u8]) {}
}

/// The windows the sides dump, in order.
fn dump_names() -> Vec<String> {
    let mut n: Vec<String> = (0..8).flat_map(|i| ROW.iter().map(move |r| format!("ScoreScreen.wnd:{r}{i}"))).collect();
    n.push("ScoreScreen.wnd:ListboxWarschoolAdvice".into());
    n.push("ScoreScreen.wnd:StaticTextWarSchool".into());
    n
}

fn rel(v: i32) -> Relationship {
    match v {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

/// The Rust side: the dump, and each dumped window's colour and border as the file made them (for the C++ windows).
fn rust_run(c: &Case) -> (String, Vec<(i32, i32)>) {
    let mut wm = GameWindowManager::new((800, 600));
    let mut wnd = Wnd(BTreeMap::new());
    let mut h = Host { ss: ScoreScreen::default(), keys: BTreeMap::new(), images: c.images.iter().map(|s| s.as_bytes().to_vec()).collect() };
    wm.win_create_from_script(&mut WindowScript::new(), b"ScoreScreen.wnd", &score_screen_wnd(), &mut wnd, &mut h).expect("the synthetic file reads");
    h.keys = wnd.0;
    let names = dump_names();
    let wins: Vec<usize> = names.iter().map(|n| wm.tree.find_by_name(n.as_bytes()).unwrap_or_else(|| panic!("{n}"))).collect();
    let initial: Vec<(i32, i32)> = wins.iter().map(|&w| (wm.tree.windows[w].inst.enabled_text.color, wm.tree.windows[w].inst.enabled_text.border_color)).collect();
    for (&w, &hide) in wins.iter().zip(&c.hidden) {
        wm.win_hide(w, hide);
    }
    let parent = wm.tree.find_by_name(b"ScoreScreen.wnd:ParentScoreScreen").expect("the parent");
    if c.parent_image_bit {
        wm.win_set_status(parent, win_status::IMAGE);
    } else {
        wm.win_clear_status(parent, win_status::IMAGE);
    }
    let templates: Vec<ScoreTemplate> = c.templates.iter().map(|(i, s)| ScoreTemplate { side_icon_image: i.as_bytes().to_vec(), score_screen_image: s.as_bytes().to_vec(), ..Default::default() }).collect();
    let players: Vec<ScorePlayer> = c
        .players
        .iter()
        .map(|p| {
            let mut k = ScoreKeeper::new();
            let mut ud = [0; 16];
            ud[..4].copy_from_slice(&p.units_destroyed);
            let mut bd = [0; 16];
            bd[..4].copy_from_slice(&p.buildings_destroyed);
            let t = p.totals;
            k.friend_set_totals(t[0], t[1], t[2], t[3], t[4], t[5], t[6], ud, bd);
            ScorePlayer {
                name: p.name.as_bytes().to_vec(),
                display_name: format!("D{}", p.color as u32 % 1000).encode_utf16().collect(),
                color: p.color,
                score_keeper: k,
                observer: p.observer,
                dead: false,
                human: p.human,
                base_side: p.side.as_bytes().to_vec(),
                template: (p.template >= 0).then_some(p.template as usize),
                list_in_score_screen: p.listed,
                skill_points: 0,
            }
        })
        .collect();
    let relations = c.players.iter().map(|p| p.rel.iter().map(|&v| rel(v)).collect()).collect();
    let game = ScoreScreenGame {
        game_mode: c.mode,
        recorder_mode: Some(RecorderMode::Record),
        recorder_multiplayer: false,
        last_replay_file_name: Vec::new(),
        players,
        player_count: c.count,
        local: (c.local >= 0).then_some(c.local as usize),
        relations,
        templates,
        global_difficulty: 1,
        game_info: None,
        local_allied_victory: false,
        local_allied_defeat: false,
        campaign: CampaignManager::new(),
        challenge_generals: ChallengeGenerals::new(),
        local_academy_advice: (!c.tips.is_empty()).then(|| c.tips.iter().map(|t| t.as_bytes().to_vec()).collect()),
    };
    h.ss.parent = Some(parent);
    h.ss.listbox_academy = wm.tree.find_by_name(b"ScoreScreen.wnd:ListboxWarschoolAdvice");
    h.ss.static_text_academy_title = wm.tree.find_by_name(b"ScoreScreen.wnd:StaticTextWarSchool");
    h.ss.override_player_display_name = c.over;
    h.ss.screen_type = c.screen_type;
    h.ss.game = Some(game);
    if c.single {
        grab_single_player_info(&mut wm, &mut h);
    } else {
        grab_multi_player_info(&mut wm, &mut h);
    }
    assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    let img = |wm: &GameWindowManager, w: usize| wm.tree.windows[w].inst.enabled_draw_data[0].image.as_ref().map_or("-".to_string(), |i| String::from_utf8_lossy(i).into_owned());
    let mut o = format!("parent img:{} image {}\n", img(&wm, parent), u8::from(wm.tree.windows[parent].status & win_status::IMAGE != 0));
    for (n, &w) in names.iter().zip(&wins) {
        let hidden = wm.tree.windows[w].status & win_status::HIDDEN != 0;
        let text = String::from_utf16_lossy(&gadget::static_text_get_text(&wm, w));
        o += &format!("{n} h{} [{text}] #{:08x} img:{}\n", u8::from(hidden), wm.tree.windows[w].inst.enabled_text.color as u32, img(&wm, w));
        if n.ends_with("ListboxWarschoolAdvice") {
            for r in 0..lb::get_num_entries(&wm, w) {
                o += &format!("  row [{}]\n", String::from_utf16_lossy(&lb::get_text(&mut wm, &mut h, w, r, 0)));
            }
        }
    }
    (o, initial)
}

fn cpp_run(c: &Case, initial: &[(i32, i32)]) -> String {
    let mut text = format!("cfg {} {} {} {} {} {} {}\n", u8::from(c.single), c.screen_type, c.mode, u8::from(c.over), c.count, c.local, if c.parent_image_bit { 0x80 } else { 0 });
    for i in &c.images {
        text += &format!("img {i}\n");
    }
    for (i, (icon, screen)) in c.templates.iter().enumerate() {
        text += &format!("tpl {i} {icon}|{screen}\n");
    }
    for t in &c.tips {
        text += &format!("tip {t}\n");
    }
    for (i, p) in c.players.iter().enumerate() {
        let nums: Vec<String> = p.totals.iter().chain(&p.units_destroyed).chain(&p.buildings_destroyed).chain(&p.rel).map(|v| v.to_string()).collect();
        text += &format!(
            "player {i} {}|D{}|{} {} {} {} {}|{} {}\n",
            p.name,
            p.color as u32 % 1000,
            p.color,
            u8::from(p.observer),
            u8::from(p.human),
            u8::from(p.listed),
            p.side,
            p.template,
            nums.join(" ")
        );
    }
    for ((n, &hide), (color, border)) in dump_names().iter().zip(&c.hidden).zip(initial) {
        text += &format!("win {n} {} {color} {border}\n", u8::from(hide));
    }
    let input = std::ffi::CString::new(text).expect("no NUL");
    let mut cap = 1 << 15;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_score_grid_run(input.as_ptr(), out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

#[test]
fn the_score_grid_matches_upstream() {
    let cases: usize = std::env::var("SCORE_GRID_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut r = Rng(0x5363_6f72_6547_7289);
    let (mut mismatches, mut rows, mut single, mut academy) = (0usize, 0usize, 0usize, 0usize);
    for case in 0..cases {
        let c = gen_case(&mut r);
        let (a, initial) = rust_run(&c);
        let b = cpp_run(&c, &initial);
        if a != b {
            mismatches += 1;
            if mismatches <= 3 {
                let diff: Vec<String> = a.lines().zip(b.lines()).filter(|(x, y)| x != y).take(6).map(|(x, y)| format!("    rust {x}\n    c++  {y}")).collect();
                println!("case {case}: {c:?}\n{}", diff.join("\n"));
            }
        } else {
            single += usize::from(c.single);
            rows += a.lines().filter(|l| l.contains("StaticTextPlayer") && l.contains(" h0 ") && !l.contains("[]")).count();
            academy += a.lines().filter(|l| l.starts_with("  row")).count();
        }
    }
    println!("#89 score grid: {cases} cases ({single} single player), {rows} named rows, {academy} academy rows matched, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
