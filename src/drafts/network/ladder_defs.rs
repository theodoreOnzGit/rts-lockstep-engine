//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/LadderDefs.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/LadderDefs.cpp
//!
//! The online ladder list: `<Ladders>` / `<SpecialLadders>` blocks of the GameSpy config's leftover text parsed into
//! [`LadderInfo`]s, plus "local ladders" read from `*.ini` files in the user's `GeneralsOnline\Ladders\` folder.
//! The player-template store, map cache, game-state map-path conversions, file system and string conversion are the
//! [`LadderHost`].
//!
//! # Upstream quirks kept
//!
//! * The `<Ladder ...>` header: `tokenName` is declared but never filled, so the ladder's name starts empty (a
//!   later `Name ` line sets it); the header text after `<Ladder ` is split at `"` and spaces into address, port,
//!   homepage, with `line.str() + 7` (one char short of the 8-character prefix, the skipped space is a separator).
//! * The `QM` and `Custom` lines are commented out upstream, so `validQM = TRUE` and `validCustom = FALSE` stay.
//! * Any line that matches none of the keys kills the ladder being parsed (`delete lad`); the lines after it are
//!   ignored until the file ends and `parseLadder` returns NULL.
//! * A ladder with `playersPerTeam` outside `1..=MAX_SLOTS/2` is dropped.

use crate::common::ascii_string::AsciiString;

use super::net_command_msg::MAX_SLOTS;
use super::persistent_storage_thread::atoi;

/// `class LadderInfo`.
#[derive(Clone, Debug)]
pub struct LadderInfo {
    /// `name` (wide).
    pub name: Vec<u16>,
    /// `description`.
    pub description: Vec<u16>,
    /// `location`.
    pub location: Vec<u16>,
    /// `playersPerTeam`.
    pub players_per_team: i32,
    /// `minWins`.
    pub min_wins: i32,
    /// `maxWins`.
    pub max_wins: i32,
    /// `randomMaps`.
    pub random_maps: bool,
    /// `randomFactions`.
    pub random_factions: bool,
    /// `validQM`.
    pub valid_qm: bool,
    /// `validCustom`.
    pub valid_custom: bool,
    /// `validMaps`.
    pub valid_maps: Vec<AsciiString>,
    /// `validFactions`.
    pub valid_factions: Vec<AsciiString>,
    /// `cryptedPassword`.
    pub crypted_password: AsciiString,
    /// `address`.
    pub address: AsciiString,
    /// `port`.
    pub port: u16,
    /// `homepageURL`.
    pub homepage_url: AsciiString,
    /// `submitReplay`.
    pub submit_replay: bool,
    /// `index`.
    pub index: i32,
}

impl LadderInfo {
    /// `LadderInfo::LadderInfo()` (LadderDefs.cpp): 1 player per team, 0 wins bounds, random maps and factions,
    /// valid in QM only, port 0, no replay, index -1.
    pub fn new() -> LadderInfo {
        LadderInfo {
            name: Vec::new(),
            description: Vec::new(),
            location: Vec::new(),
            players_per_team: 1,
            min_wins: 0,
            max_wins: 0,
            random_maps: true,
            random_factions: true,
            valid_qm: true,
            valid_custom: false,
            valid_maps: Vec::new(),
            valid_factions: Vec::new(),
            crypted_password: AsciiString::default(),
            address: AsciiString::default(),
            port: 0,
            homepage_url: AsciiString::default(),
            submit_replay: false,
            index: -1,
        }
    }
}

impl Default for LadderInfo {
    fn default() -> Self {
        LadderInfo::new()
    }
}

/// What ladder parsing needs from the rest of the game.
pub trait LadderHost {
    /// `ThePlayerTemplateStore->getAllSideStrings(&list)`.
    fn all_side_strings(&self) -> Vec<AsciiString>;
    /// The sides of the templates that are playable and not "Boss", in template order
    /// (`getNthPlayerTemplate(i)`, skipping NULL; `isPlayableSide() && getSide().compare("Boss") != 0`).
    fn playable_non_boss_sides(&self) -> Vec<AsciiString>;
    /// `TheGameSpyConfig->getQMMaps()`.
    fn qm_maps(&self) -> Vec<AsciiString>;
    /// `"%s\\%s\\%s.map"` of `TheMapCache->getMapDir()` and the name, then
    /// `portableMapPathToRealMapPath(realMapPathToPortableMapPath(..))` and `toLower()`.
    fn map_path_for(&self, map_name: &AsciiString) -> AsciiString;
    /// `TheMapCache->findMap(path)->m_numPlayers`, None if the map is unknown.
    fn map_num_players(&self, map_path: &AsciiString) -> Option<i32>;
    /// `TheFileSystem->getFileListInDirectory("<user data>GeneralsOnline\\Ladders\\", "*.ini", list, TRUE)`.
    fn local_ladder_files(&self) -> Vec<AsciiString>;
    /// The whole text of a file (`openFile(READ | TEXT)`, read in 1023-byte chunks); empty if it cannot be opened.
    fn read_text_file(&self, name: &AsciiString) -> Vec<u8>;
    /// `MultiByteToWideCharSingleLine`.
    fn multi_byte_to_wide_single_line(&self, s: &[u8]) -> Vec<u16>;
}

fn strip_cr_trim(line: &mut AsciiString) {
    let len = line.get_length();
    if len > 0 && line.try_get_char_at(len - 1).unwrap_or(0) == b'\r' {
        let _ = line.remove_last_char(); // there is a trailing '\r'
    }
    let _ = line.trim();
}

fn from_bytes(b: &[u8]) -> AsciiString {
    AsciiString::try_from_bytes(b).unwrap_or_default()
}

/// `parseLadder` (LadderDefs.cpp): one ladder from its raw text, or None (a bad line, a bad team size or no
/// `</Ladder>` before the end).
pub fn parse_ladder<H: LadderHost>(host: &H, raw: &AsciiString) -> Option<LadderInfo> {
    let mut raw = raw.clone();
    let mut lad: Option<LadderInfo> = None;
    let mut line = AsciiString::default();
    while raw.next_token(&mut line, Some(b"\n")).unwrap_or(false) {
        strip_cr_trim(&mut line);
        if line.is_empty() {
            continue;
        }
        // woohoo!  got a line!
        let _ = line.trim();
        let s = line.str().to_vec();

        if lad.is_none() && line.starts_with(b"<Ladder ") {
            // start of a ladder def
            let mut l = LadderInfo::new();
            let mut token_name = AsciiString::default(); // never filled upstream
            let mut token_addr = AsciiString::default();
            let mut token_port = AsciiString::default();
            let mut token_homepage = AsciiString::default();
            let mut h = line.clone();
            let _ = h.remove_last_char(); // the '>'
            let mut rest = from_bytes(if h.str().len() >= 7 { &h.str()[7..] } else { b"" }); // the "<Ladder "
            let _ = rest.next_token(&mut token_addr, Some(b"\" "));
            let _ = rest.next_token(&mut token_port, Some(b" "));
            let _ = rest.next_token(&mut token_homepage, Some(b" "));
            let _ = &mut token_name;

            l.name = host.multi_byte_to_wide_single_line(token_name.str());
            while l.name.len() > 20 {
                l.name.pop(); // Per Harvard's request, ladder names are limited to 20 chars
            }
            l.address = token_addr;
            l.port = atoi(token_port.str()) as u16;
            l.homepage_url = token_homepage;
            lad = Some(l);
        } else if lad.is_some() && line.starts_with(b"Name ") {
            lad.as_mut().unwrap().name = host.multi_byte_to_wide_single_line(&s[5..]);
        } else if lad.is_some() && line.starts_with(b"Desc ") {
            lad.as_mut().unwrap().description = host.multi_byte_to_wide_single_line(&s[5..]);
        } else if lad.is_some() && line.starts_with(b"Loc ") {
            lad.as_mut().unwrap().location = host.multi_byte_to_wide_single_line(&s[4..]);
        } else if lad.is_some() && line.starts_with(b"TeamSize ") {
            lad.as_mut().unwrap().players_per_team = atoi(&s[9..]);
        } else if lad.is_some() && line.starts_with(b"RandomMaps ") {
            lad.as_mut().unwrap().random_maps = atoi(&s[11..]) != 0;
        } else if lad.is_some() && line.starts_with(b"RandomFactions ") {
            lad.as_mut().unwrap().random_factions = atoi(&s[15..]) != 0;
        } else if lad.is_some() && line.starts_with(b"Faction ") {
            let faction = from_bytes(&s[8..]);
            let out = host.all_side_strings();
            if out.iter().any(|f| *f == faction) {
                // valid faction - now check for dupes
                let l = lad.as_mut().unwrap();
                if !l.valid_factions.iter().any(|f| *f == faction) {
                    l.valid_factions.push(faction);
                }
            }
        } else if lad.is_some() && line.starts_with(b"MinWins ") {
            lad.as_mut().unwrap().min_wins = atoi(&s[8..]);
        } else if lad.is_some() && line.starts_with(b"MaxWins ") {
            lad.as_mut().unwrap().max_wins = atoi(&s[8..]);
        } else if lad.is_some() && line.starts_with(b"CryptedPass ") {
            lad.as_mut().unwrap().crypted_password = from_bytes(&s[12..]);
        } else if lad.is_some() && s == b"</Ladder>" {
            // end of a ladder
            let mut l = lad.take().unwrap();
            if l.players_per_team >= 1 && l.players_per_team <= MAX_SLOTS / 2 {
                if l.valid_factions.is_empty() {
                    // No factions specified.  Using all.
                    l.valid_factions.clear();
                    for side in host.playable_non_boss_sides() {
                        l.valid_factions.push(side);
                    }
                }
                // (else: debug log of each faction's INI:Faction marker text only)

                if l.valid_maps.is_empty() {
                    // No maps specified.  Using all.
                    for map_name in host.qm_maps() {
                        // check sizes on the maps before allowing them
                        if let Some(n) = host.map_num_players(&map_name) {
                            if n >= l.players_per_team * 2 {
                                l.valid_maps.push(map_name);
                            }
                        }
                    }
                }
                return Some(l);
            } else {
                // no maps?  don't play on it!
                return None;
            }
        } else if lad.is_some() && line.starts_with(b"Map ") {
            // valid map
            let mut map_name = from_bytes(&s[4..]);
            let _ = map_name.trim();
            if map_name.is_not_empty() {
                let map_name = host.map_path_for(&map_name);
                let qm_maps = host.qm_maps();
                if qm_maps.iter().any(|m| *m == map_name) {
                    // check sizes on the maps before allowing them
                    if let Some(n) = host.map_num_players(&map_name) {
                        let l = lad.as_mut().unwrap();
                        if n >= l.players_per_team * 2 {
                            l.valid_maps.push(map_name);
                        }
                    }
                }
            }
        } else {
            // bad ladder - kill it
            lad = None;
        }
    }
    None
}

/// `class LadderList`.
#[derive(Clone, Debug, Default)]
pub struct LadderList {
    /// `m_localLadders`.
    local_ladders: Vec<LadderInfo>,
    /// `m_specialLadders`.
    special_ladders: Vec<LadderInfo>,
    /// `m_standardLadders`.
    standard_ladders: Vec<LadderInfo>,
}

impl LadderList {
    /// `LadderList::LadderList()` (LadderDefs.cpp): reads `leftover_config` (`TheGameSpyConfig->getLeftoverConfig()`)
    /// line by line; the `<Ladders>` / `<SpecialLadders>` blocks hold `<Ladder ...>`..`</Ladder>` groups that are
    /// each parsed with [`parse_ladder`] and numbered from 1 in order of acceptance (shared between the two
    /// kinds); then loads the local ladders.
    pub fn new<H: LadderHost>(host: &H, leftover_config: &AsciiString) -> LadderList {
        let mut list = LadderList::default();
        let mut raw_motd = leftover_config.clone();
        let mut line = AsciiString::default();
        let mut in_ladders = false;
        let mut in_special_ladders = false;
        let mut in_ladder = false;
        let mut index = 1;
        let mut raw_ladder = AsciiString::default();

        while raw_motd.next_token(&mut line, Some(b"\n")).unwrap_or(false) {
            strip_cr_trim(&mut line);
            if line.is_empty() {
                continue;
            }
            let s = line.str().to_vec();

            if !in_ladders && s == b"<Ladders>" {
                in_ladders = true;
                raw_ladder.clear();
            } else if in_ladders && s == b"</Ladders>" {
                in_ladders = false;
            } else if !in_special_ladders && s == b"<SpecialLadders>" {
                in_special_ladders = true;
                raw_ladder.clear();
            } else if in_special_ladders && s == b"</SpecialLadders>" {
                in_special_ladders = false;
            } else if in_ladders || in_special_ladders {
                if line.starts_with(b"<Ladder ") && !in_ladder {
                    in_ladder = true;
                    raw_ladder.clear();
                    let _ = raw_ladder.concat(&s);
                    let _ = raw_ladder.concat_char(b'\n');
                } else if s == b"</Ladder>" && in_ladder {
                    in_ladder = false;
                    let _ = raw_ladder.concat(&s);
                    let _ = raw_ladder.concat_char(b'\n');
                    if let Some(mut lad) = parse_ladder(host, &raw_ladder) {
                        lad.index = index;
                        index += 1;
                        if in_ladders {
                            list.standard_ladders.push(lad);
                        } else {
                            list.special_ladders.push(lad);
                        }
                    }
                    raw_ladder.clear();
                } else if in_ladder {
                    let _ = raw_ladder.concat(&s);
                    let _ = raw_ladder.concat_char(b'\n');
                }
            }
        }

        // look for local ladders
        list.load_local_ladders(host);
        list
    }

    /// `findLadder(addr, port)`: special, then standard, then local ladders.
    pub fn find_ladder(&self, addr: &AsciiString, port: u16) -> Option<&LadderInfo> {
        for l in self.special_ladders.iter().chain(self.standard_ladders.iter()).chain(self.local_ladders.iter()) {
            if l.address == *addr && l.port == port {
                return Some(l);
            }
        }
        None
    }

    /// `findLadderByIndex(index)`: index 0 is never found; special, standard, then local ladders.
    pub fn find_ladder_by_index(&self, index: i32) -> Option<&LadderInfo> {
        if index == 0 {
            return None;
        }
        for l in self.special_ladders.iter().chain(self.standard_ladders.iter()).chain(self.local_ladders.iter()) {
            if l.index == index {
                return Some(l);
            }
        }
        None
    }

    /// `getSpecialLadders`.
    pub fn get_special_ladders(&self) -> &Vec<LadderInfo> {
        &self.special_ladders
    }
    /// `getStandardLadders`.
    pub fn get_standard_ladders(&self) -> &Vec<LadderInfo> {
        &self.standard_ladders
    }
    /// `getLocalLadders`.
    pub fn get_local_ladders(&self) -> &Vec<LadderInfo> {
        &self.local_ladders
    }

    /// `LadderList::loadLocalLadders`: every `*.ini` in the user's ladder folder, lower-cased, numbered -1, -2, ...
    fn load_local_ladders<H: LadderHost>(&mut self, host: &H) {
        let mut index = -1;
        for mut filename in host.local_ladder_files() {
            let _ = filename.to_lower();
            self.check_ladder(host, &filename, index);
            index -= 1;
        }
    }

    /// `LadderList::checkLadder`: parses the file as one ladder and keeps it if it has an address, a port and at
    /// least one valid map; local ladders are neither QM nor custom.
    fn check_ladder<H: LadderHost>(&mut self, host: &H, fname: &AsciiString, index: i32) {
        let raw = host.read_text_file(fname);
        if raw.is_empty() {
            return;
        }
        let raw_data = from_bytes(&raw);
        let mut li = match parse_ladder(host, &raw_data) {
            Some(l) => l,
            None => return,
        };
        // sanity check
        if li.address.is_empty() {
            return;
        }
        if li.port == 0 {
            return;
        }
        if li.valid_maps.is_empty() {
            return;
        }
        li.index = index;
        // ladders are QM-only at this point, which kinda invalidates the whole concept of local ladders.  Oh well.
        li.valid_qm = false; // no local ladders in QM
        li.valid_custom = false;
        self.local_ladders.push(li);
    }
}
