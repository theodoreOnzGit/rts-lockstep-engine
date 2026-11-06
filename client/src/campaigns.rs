// SPDX-License-Identifier: GPL-3.0-only
//! Campaign missions and the Generals' Challenge from the client (GitHub #62). Plain Rust, no Bevy: the application's
//! panel calls these, and the tests drive them the same way.
//!
//! **The list** is the World's `TheCampaignManager` (`Data\INI\Campaign.ini`: USA, China, GLA and the training campaign,
//! each with its missions) and `TheChallengeGenerals` (`Data\INI\ChallengeMode.ini`: the challenge's generals), read by
//! the install ([`sections`]). The game's own menus (the main menu's campaign buttons, the difficulty menu, the challenge
//! menu, the score screen) are milestone C5; this list is plain, and offers every mission, not only a campaign's first
//! (upstream's menus start the first; the others by Continue, or a cheat's `MissionSelect` menu).
//!
//! **A mission's map** must be on the World's disk, and the client keeps no logic file system once the game has started
//! (the archives the logic read whole are dropped). It is read from the art's file system, which holds every archive the
//! page handed over (the logic's ones by their directory and their page key): in the browser by range, as the art is
//! (`crate::import`'s ranged reads), natively from the `--zh` folder. The choice waits ([`Flow::poll`]) until every file
//! of the map's folder has arrived, and only then mounts them all (`World::mount_map_when_ready`), so every machine
//! starts the mission from the same bytes. The World it starts in is a copy of the install's (`crate::import::Started::base`),
//! as a save's load is.
//!
//! **The game's end** ([`Flow::check_game_over`]): when the script engine's end-game timer runs out (upstream's
//! `MSG_CLEAR_GAME_DATA`, `World::single_player_game_over`), the score screen's `finishSinglePlayerInit` runs on a copy
//! of the World's campaign (the client never changes the simulation): after a victory the next mission is the campaign's
//! next (`gotoNextMission`), after a defeat the same one (Continue reads Retry); its Continue starts it
//! (`startNextCampaignGame`), read by range as the first was. At a campaign's end there is none. **#64 (2026-11-06):** the
//! message now reaches `clearGameData`, which clears the World that played; the next mission starts in that World (the
//! client's copy of it, which the application puts in place of its game, as it did the install's copy before).
//!
//! **Briefings.** The briefing movies are Bink videos, which cannot be played (Bink was never released); the briefing's
//! text is shown instead ([`briefing`]): the mission's location and objective lines from the game text.

use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice};
use rts_lockstep_engine::logic::world::campaign_world::{CampaignStartError, MissionChoice, PendingMission};
use rts_lockstep_engine::common::science::GameTextFetch;
use rts_lockstep_engine::logic::world::game_logic::World;

/// The campaign difficulties (`GameDifficulty`: `DIFFICULTY_EASY`, `DIFFICULTY_NORMAL`, `DIFFICULTY_HARD`) by their value.
pub const DIFFICULTIES: [&str; 3] = ["Easy", "Normal", "Hard"];

/// A line of the list: what it starts, and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub choice: MissionChoice,
    pub line: String,
}

/// A campaign's missions, or the challenge's generals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub entries: Vec<Entry>,
}

/// `TheGameText->fetch(label)`, or `None` for an empty label or one the text lacks (upstream shows `MISSING: '...'`).
fn text(world: &World, label: &[u8]) -> Option<String> {
    if label.is_empty() {
        return None;
    }
    let s = String::from_utf16_lossy(world.statics.game_text.fetch(label).str());
    (!s.is_empty() && !s.starts_with("MISSING:")).then_some(s)
}

/// The list: each campaign that is not a challenge's, in the file's order, with its missions (the name, the location's
/// text when the game text has it, and the map's folder); then the Generals' Challenge's generals that have a campaign
/// (the persona's name from the game text, else its position, and its campaign). Upstream's challenge menu locks the
/// generals a player has not unlocked (`StartsEnabled`, the battle honours); the port keeps no honours yet, so every
/// general is offered, the locked ones marked.
pub fn sections(world: &World) -> Vec<Section> {
    let mut out = Vec::new();
    for c in world.campaign.campaign_list.iter().filter(|c| !c.is_challenge_campaign) {
        let name = String::from_utf8_lossy(&c.name).into_owned();
        let title = text(world, &c.campaign_name_label).map_or_else(|| format!("Campaign {name}"), |t| format!("{t} ({name})"));
        let entries = c
            .missions
            .iter()
            .map(|m| {
                let folder = m.map_name.rsplit(|&b| b == b'\\').nth(1).map(|f| String::from_utf8_lossy(f).into_owned()).unwrap_or_default();
                let place = text(world, &m.location_name_label).map_or(String::new(), |t| format!(": {t}"));
                Entry { choice: MissionChoice::Campaign { campaign: c.name.clone(), mission: m.name.clone() }, line: format!("{}{place} ({folder})", String::from_utf8_lossy(&m.name)) }
            })
            .collect();
        out.push(Section { title, entries });
    }
    let generals: Vec<Entry> = world
        .challenge_generals
        .get_challenge_generals()
        .iter()
        .enumerate()
        .filter(|(_, g)| !g.get_campaign().is_empty())
        .map(|(i, g)| {
            let name = text(world, g.get_bio_name()).unwrap_or_else(|| format!("General {i}"));
            let locked = if g.starts_enabled { "" } else { ", locked in the game" };
            Entry { choice: MissionChoice::Challenge(i as i32), line: format!("{name} ({}{locked})", String::from_utf8_lossy(g.get_campaign())) }
        })
        .collect();
    if !generals.is_empty() {
        out.push(Section { title: "Generals' Challenge".to_string(), entries: generals });
    }
    out
}

/// The current mission's briefing text: its location, then its objective lines (`ObjectiveLine0` to `4`), each from the
/// game text; empty outside a campaign.
pub fn briefing(world: &World) -> Vec<String> {
    let Some(m) = world.campaign.get_current_mission() else { return Vec::new() };
    std::iter::once(&m.location_name_label).chain(m.mission_objectives_label.iter()).filter_map(|l| text(world, l)).collect()
}

/// A mission started: its World (the game started, the local player made), the local player and a label for the status
/// line.
pub struct Started {
    pub world: World,
    pub player: i32,
    pub label: String,
}

/// The score screen's state at a game's end.
pub struct GameOver {
    pub victorious: bool,
    /// What Continue (or Retry) starts, waiting for its map; `Err(NoMission)` at the campaign's end.
    pub next: Result<PendingMission, CampaignStartError>,
    /// After a victory with a mission to follow, `finishSinglePlayerInit`'s automatic mission save (`GameState::missionSave`,
    /// the button's "Save and Continue"): its description and bytes, for the save folder.
    pub mission_save: Option<(String, Vec<u8>)>,
}

/// The flow from a choice to a game: the mission waiting for its map, and the score screen of the game that ended.
#[derive(Default)]
pub struct Flow {
    pub pending: Option<PendingMission>,
    pub over: Option<GameOver>,
    /// The game being played is a mission this flow started (its end opens the score screen once).
    pub watching: bool,
}

/// Why a choice did not start, in words.
pub fn describe(e: &CampaignStartError) -> String {
    match e {
        CampaignStartError::NoMission(c) => format!("the campaign {:?} has no such mission", String::from_utf8_lossy(c)),
        CampaignStartError::NoGeneral(i) => format!("no general {i}, or its player template is missing"),
    }
}

impl Flow {
    /// A choice from the list, at `difficulty`, in a copy of `base` (the install's World): it waits for its map.
    pub fn choose(&mut self, base: &World, choice: MissionChoice, difficulty: i32) -> Result<(), String> {
        self.pending = Some(PendingMission::new(base, choice, difficulty).map_err(|e| describe(&e))?);
        self.over = None;
        Ok(())
    }

    /// The score screen's Continue: the next mission (or the same one, after a defeat) waits for its map. `false` at the
    /// campaign's end (nothing to start).
    pub fn continue_campaign(&mut self) -> bool {
        match self.over.take() {
            Some(GameOver { next: Ok(p), .. }) => {
                self.pending = Some(p);
                true
            }
            _ => false,
        }
    }

    /// Once a frame: while a mission waits, its map's folder is read from `fs` (the bytes asked for arrive between frames;
    /// `None` until all have); once mounted, the mission starts and its starting frames run (until the local player is
    /// made, at most 30). The map being read, for the status line, while waiting.
    pub fn poll<D: LocalDevice>(&mut self, fs: &FileSystem<D>) -> Option<Result<Started, String>> {
        let p = self.pending.as_mut()?;
        if !p.mount(fs) {
            return None;
        }
        let p = self.pending.take()?;
        let (mut world, map) = match p.start() {
            Ok(x) => x,
            Err(e) => return Some(Err(describe(&e))),
        };
        for _ in 0..30 {
            if let Err(e) = world.engine_update() {
                return Some(Err(format!("a starting frame failed: {e:?}")));
            }
            if let Some(player) = world.players.local_index().filter(|_| world.game_started()) {
                self.watching = true;
                let campaign = world.campaign.get_current_campaign().map(|c| String::from_utf8_lossy(&c.name).into_owned()).unwrap_or_default();
                let label = format!("{campaign} mission {} on {}", world.campaign.get_current_mission_number() + 1, String::from_utf8_lossy(&map));
                return Some(Ok(Started { world, player, label }));
            }
        }
        Some(Err("the mission made no local player in 30 frames".to_string()))
    }

    /// The map a waiting mission reads.
    pub fn waiting_for(&self) -> Option<&[u8]> {
        self.pending.as_ref().map(|p| p.map.as_slice())
    }

    /// After each logic frame: when the mission being played has ended (`World::single_player_game_over`), the score
    /// screen's `finishSinglePlayerInit` on a copy of ~~its campaign~~ the World (cleared by its `clearGameData`, #64), and
    /// what its Continue starts ~~in a copy of `base`~~ in that World. Whether the score screen opened now.
    pub fn check_game_over(&mut self, world: &World) -> bool {
        if !self.watching || self.over.is_some() || !world.single_player_game_over() {
            return false;
        }
        self.watching = false;
        let victorious = world.campaign.is_victorious();
        // (#64: the World that played, cleared by its `clearGameData`, is the one the next mission starts in, as upstream's
        // one engine; the client's copy of it is given `finishSinglePlayerInit`, the score screen's, and the mission save
        // is made from it, as upstream's is after the clear)
        let mut played = world.clone();
        played.finish_single_player();
        let mission_save = played
            .mission_save_description()
            .and_then(|d| played.mission_save(&d).ok().map(|b| (String::from_utf16_lossy(&d), b)));
        let next = PendingMission::next(played);
        let mission_save = mission_save.filter(|_| victorious && next.is_ok());
        self.over = Some(GameOver { victorious, next, mission_save });
        true
    }

    /// A mission save from the save folder (the load menu's, for a `SAVE_FILE_TYPE_MISSION` file): loaded into a copy of
    /// `base`, its mission waits for its map ([`PendingMission::from_mission_save`]).
    pub fn load_mission_save(&mut self, base: &World, bytes: Vec<u8>) -> Result<(), String> {
        self.pending = Some(PendingMission::from_mission_save(base, bytes)?);
        self.over = None;
        Ok(())
    }

    /// Whether `bytes` are a mission save (its `SaveGameInfo`'s file type).
    pub fn is_mission_save(bytes: &[u8]) -> bool {
        rts_lockstep_engine::client::save_game::read_save_summary(bytes.to_vec())
            .is_ok_and(|s| s.info.save_file_type == rts_lockstep_engine::common::game_state::SAVE_FILE_TYPE_MISSION)
    }
}

/// The page's side (for its checks): whether the campaign panel is open (it takes the pointer then) and the flow's last
/// state (`waiting <map>`, `playing <label>`, `over victorious`, `over defeated`, `error: ...`).
pub mod page {
    use std::cell::RefCell;

    #[derive(Default)]
    struct State {
        open: bool,
        status: String,
    }

    thread_local! {
        static P: RefCell<State> = RefCell::new(State::default());
    }

    pub fn set_panel_open(open: bool) {
        P.with(|p| p.borrow_mut().open = open);
    }

    pub fn panel_open() -> bool {
        P.with(|p| p.borrow().open)
    }

    pub fn set_status(s: String) {
        P.with(|p| p.borrow_mut().status = s);
    }

    pub fn status() -> String {
        P.with(|p| p.borrow().status.clone())
    }

    /// The page's call: the flow's state (see the module).
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen::prelude::wasm_bindgen]
    pub fn campaign_status() -> String {
        status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAMPAIGN: &[u8] = b"Campaign Training\n CampaignNameLabel = CAMPAIGN:Training\n FirstMission = M01\n Mission M01\n  Map = Maps\\Training01\\Training01.map\n  LocationNameLabel = LOCATION:Range\n  ObjectiveLine0 = OBJ:Move\n  ObjectiveLine1 = OBJ:Nowhere\n End\nEnd\n\
        Campaign USA\n FirstMission = M01\n Mission M01\n  Map = Maps\\USA01\\USA01.map\n  NextMission = M02\n End\n Mission M02\n  Map = Maps\\USA02\\USA02.map\n End\nEnd\n\
        Campaign Challenge_0\n IsChallengeCampaign = Yes\n FirstMission = Mission01\n Mission Mission01\n  Map = Maps\\GC01\\GC01.map\n End\nEnd\n";
    const CHALLENGE: &[u8] = b"ChallengeGenerals\n GeneralPersona0\n  StartsEnabled = Yes\n  BioNameString = GUI:Pos0\n  Campaign = Challenge_0\n  PlayerTemplate = FactionAmerica\n End\n\
        GeneralPersona1\n  Campaign = Challenge_1\n  PlayerTemplate = FactionAmerica\n End\nEnd\n";

    fn world() -> World {
        let mut w = World::new();
        w.campaign.init(CAMPAIGN).expect("Campaign.ini");
        w.challenge_generals.init(CHALLENGE).expect("ChallengeMode.ini");
        let t = &mut std::sync::Arc::make_mut(&mut w.statics).game_text;
        for (k, v) in [("CAMPAIGN:Training", "Boot Camp"), ("LOCATION:Range", "The firing range"), ("OBJ:Move", "Move the tank"), ("GUI:Pos0", "General Granger")] {
            t.strings.insert(k.as_bytes().to_vec(), rts_lockstep_engine::common::unicode_string::UnicodeString::try_from_chars(&v.encode_utf16().collect::<Vec<_>>()).unwrap());
        }
        w
    }

    /// #62: the list. **Predicted before the run:** two campaign sections, in the file's order (`Boot Camp (training)`,
    /// then `Campaign usa`, its label absent), the challenge campaign left out of them; their missions named, located
    /// when the game text has it, with the map's folder; then the Generals' Challenge with general 0 (`General Granger`,
    /// `challenge_0`) and general 1 (no name in the text: `General 1`, marked locked); the choices are the campaign and
    /// mission names as the campaign manager keeps them (lower case) and the general's index. The briefing outside a
    /// campaign is empty; in the training campaign it is the location and the one objective line the text has (the other's
    /// label is missing, and left out).
    #[test]
    fn the_list_names_the_campaigns_missions_and_generals() {
        let mut w = world();
        let s = sections(&w);
        let titles: Vec<&str> = s.iter().map(|x| x.title.as_str()).collect();
        assert_eq!(titles, ["Boot Camp (training)", "Campaign usa", "Generals' Challenge"]);
        let lines: Vec<Vec<&str>> = s.iter().map(|x| x.entries.iter().map(|e| e.line.as_str()).collect()).collect();
        assert_eq!(lines[0], ["m01: The firing range (Training01)"]);
        assert_eq!(lines[1], ["m01 (USA01)", "m02 (USA02)"]);
        assert_eq!(lines[2], ["General Granger (Challenge_0)", "General 1 (Challenge_1, locked in the game)"]);
        assert_eq!(s[1].entries[1].choice, MissionChoice::Campaign { campaign: b"usa".to_vec(), mission: b"m02".to_vec() });
        assert_eq!(s[2].entries[1].choice, MissionChoice::Challenge(1));
        assert!(briefing(&w).is_empty());
        w.campaign.set_campaign(b"Training");
        assert_eq!(briefing(&w), ["The firing range", "Move the tank"]);
    }

    /// #62, not run unless asked: with `RTS_CAMPAIGN_INSTALL=<dir>` (the engine's `write_the_campaign_install_when_asked`:
    /// the tests' install with three missions' maps in `MapsZH.big`, after a 300 KiB member), the browser's path: every
    /// archive handed over whole with its page key (none is an art archive), the game started (the quick skirmish), then
    /// the list's `usa`/`m01` chosen; the map is read from the art's device, which holds the logic's archives by their
    /// directory only, every range it asks for served from the files as the page serves them.
    ///
    /// *Predictions:* the list has the `usa` campaign (two missions) and the challenge's general 0; the choice waits one
    /// round (three ranges: `USA01`'s map, `map.ini`, `map.str`) and starts on `USA01`, local player 1, labelled
    /// `usa mission 1`; its script wins, and the score screen opens 120 to 125 frames later, victorious, its Continue
    /// waiting for `USA02`, which starts as `usa mission 2`; its automatic mission save (#62 part 2) loads and starts
    /// `usa mission 2` too, after one round (one range: `USA02`'s map), so the Continue after it waits for nothing; run twice, the two runs' logic CRCs
    /// agree on each of the first mission's 150 frames. #64 part 2, predicted: a normal save of `m01` at its 60th frame,
    /// loaded through `PendingLoad`, mounts its folder's 3 files at once (they arrived for the start) and its load reads
    /// `USA01`'s `map.ini`.
    #[test]
    fn a_mission_from_the_list_runs_on_the_browsers_import_when_asked() {
        use crate::import::Imported;
        use rts_lockstep_engine::common::file_system::RangedDevice;
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        let Ok(dir) = std::env::var("RTS_CAMPAIGN_INSTALL") else { return };
        let mut names: Vec<_> = std::fs::read_dir(&dir).expect("the folder").filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".big")).collect();
        names.sort();
        let archives: Vec<(String, Vec<u8>)> = names.into_iter().map(|n| (n.clone(), std::fs::read(std::path::Path::new(&dir).join(&n)).expect("an archive"))).collect();
        let serve = |fs: &mut FileSystem<RangedDevice>| -> usize {
            let wanted = fs.local.device.take_wanted();
            for &(k, o, l) in &wanted {
                fs.local.device.provide(k, o, archives[k as usize].1[o..o + l].to_vec());
            }
            wanted.len()
        };
        let until_started = |flow: &mut Flow, fs: &mut FileSystem<RangedDevice>| -> (Started, Vec<usize>) {
            let mut served = Vec::new();
            loop {
                if let Some(r) = flow.poll(fs) {
                    return (r.expect("the mission starts"), served);
                }
                served.push(serve(fs));
                assert!(served.len() < 5, "{served:?}");
            }
        };
        let run = || -> Vec<u32> {
            let files: Vec<(Vec<u8>, Imported)> = archives.iter().enumerate().map(|(k, (n, b))| (n.as_bytes().to_vec(), Imported::Whole { key: k as u32, bytes: b.clone() })).collect();
            let (started, mut fs) = crate::import::start_from_files(files, None).expect("the game starts");
            let base = started.base;
            let s = sections(&base);
            let titles: Vec<&str> = s.iter().map(|x| x.title.as_str()).collect();
            assert_eq!(titles, ["Campaign usa", "Generals' Challenge"]);
            assert_eq!(s[0].entries.len(), 2);
            let mut flow = Flow::default();
            flow.choose(&base, s[0].entries[0].choice.clone(), 1).expect("the choice");
            let (mut m1, served) = until_started(&mut flow, &mut fs);
            eprintln!("m1: {} after {served:?}", m1.label);
            assert_eq!(served, [3]);
            assert!(m1.label.starts_with("usa mission 1"), "{}", m1.label);
            assert_eq!(m1.player, 1);
            let mut crcs = Vec::new();
            let mut over_at = None;
            for f in 1..=150 {
                if f == 60 {
                    // (#64 part 2: a normal save of the listed mission, loaded through the save panel's path: the save's map
                    // folder mounted from the art's files first, so the load reads its `map.ini`)
                    let mut controls = crate::input::Controls::new(m1.world.clone(), m1.player);
                    let mut particles = rts_lockstep_engine::client::particle_sys::ParticleSystemManager::new();
                    let save = crate::saves::save_game(&mut controls, &mut particles, &mut crate::saves::ClientState::default(), None, "m1").expect("the save");
                    let mut p = crate::saves::PendingLoad::new(save, Some(&base)).expect("the pending load");
                    assert_eq!(p.mount(&fs), Ok(true), "USA01's folder arrived for the mission's start");
                    assert_eq!(p.mounted, Some(3));
                    p.world.cuts.trace = Some(Vec::new());
                    let (c, ..) = crate::saves::client_from_pending(p, None).expect("the load");
                    let read = c.client.world.cuts.trace.as_ref().is_some_and(|t| t.iter().any(|l| l.contains("\"INI::load\"") && l.contains(&format!("{:?}", b"maps\\usa01\\map.ini".to_vec()))));
                    assert!(read, "the save's load read USA01's map.ini");
                }
                m1.world.engine_update().expect("a frame");
                crcs.push(m1.world.logic_crc(CRC_RECALC, None).expect("the CRC"));
                if over_at.is_none() && flow.check_game_over(&m1.world) {
                    over_at = Some(f);
                }
            }
            eprintln!("the score screen at frame {over_at:?} after the start");
            assert!(over_at.is_some_and(|f| (120..=125).contains(&f)), "{over_at:?}");
            assert!(flow.over.as_ref().is_some_and(|o| o.victorious));
            let (desc, save) = flow.over.as_ref().and_then(|o| o.mission_save.clone()).expect("the mission save");
            assert!(Flow::is_mission_save(&save), "{desc}");
            let mut loaded = Flow::default();
            loaded.load_mission_save(&base, save).expect("the mission save's load");
            let (from_save, served) = until_started(&mut loaded, &mut fs);
            assert_eq!(served, [1]);
            assert!(from_save.label.starts_with("usa mission 2"), "{}", from_save.label);
            assert!(flow.continue_campaign());
            let (m2, served) = until_started(&mut flow, &mut fs);
            eprintln!("m2: {} after {served:?}", m2.label);
            assert!(served.is_empty(), "the mission save's load read USA02's range already: {served:?}");
            assert!(m2.label.starts_with("usa mission 2"), "{}", m2.label);
            crcs
        };
        let (a, b) = (run(), run());
        assert_eq!(a, b, "two runs' CRCs");
    }
}
