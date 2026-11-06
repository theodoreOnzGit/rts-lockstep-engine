// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/ChallengeGenerals.h (GeneralPersona, ChallengeGenerals)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ChallengeGenerals.cpp (the whole file)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TheChallengeGenerals` (GitHub #61): the Generals' Challenge personas of `Data\INI\ChallengeMode.ini` (the
//! `ChallengeGenerals` block, its `GeneralPersona0` to `GeneralPersona11` blocks), the player template the challenge
//! plays (which `CampaignManager::xfer` saves) and its difficulty. The logic reads it: `populateRandomSideAndColor` leaves
//! out a locked general's side, and the challenge campaign's start and save use the template number.
//!
//! **Translated whole:** `ChallengeGenerals::init`, `parseGeneralPersona`, `s_fieldParseTable`,
//! `INI::parseChallengeModeDefinition`, `getChallengeGenerals`, `getPlayerGeneralByCampaignName`,
//! `getGeneralByGeneralName`, `getGeneralByTemplateName`, the template number's and difficulty's accessors, and
//! `GeneralPersona`'s accessors.
//!
//! **Where this differs from upstream:** a persona's images (`parseMappedImage`) keep their names, looked up by the
//! client when drawn (as `control_bar_scheme`'s); `getRandomTauntSound` is not translated (the client's, with `rand()`).
//! Upstream's constructor leaves `m_bStartsEnabled`, `m_PlayerTemplateNum` and `m_currentDifficulty` uninitialised; here
//! they are FALSE, 0 (the port's saves wrote 0 before, GitHub #58) and normal.

use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};

/// `NUM_GENERALS`.
pub const NUM_GENERALS: usize = 12;
/// The file `ChallengeGenerals::init` loads.
pub const CHALLENGE_MODE_INI: &[u8] = b"Data\\INI\\ChallengeMode.ini";

/// `class GeneralPersona`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeneralPersona {
    pub starts_enabled: bool,
    pub bio_name: Vec<u8>,
    pub bio_dob: Vec<u8>,
    pub bio_birthplace: Vec<u8>,
    pub bio_strategy: Vec<u8>,
    pub bio_rank: Vec<u8>,
    pub bio_branch: Vec<u8>,
    pub bio_class_number: Vec<u8>,
    /// `m_imageBioPortraitSmall` (the image's name).
    pub bio_portrait_small: Vec<u8>,
    /// `m_imageBioPortraitLarge` (the image's name).
    pub bio_portrait_large: Vec<u8>,
    pub campaign: Vec<u8>,
    pub player_template_name: Vec<u8>,
    pub portrait_movie_left_name: Vec<u8>,
    pub portrait_movie_right_name: Vec<u8>,
    /// `m_imageDefeated` (the image's name).
    pub image_defeated: Vec<u8>,
    /// `m_imageVictorious` (the image's name).
    pub image_victorious: Vec<u8>,
    pub defeated: Vec<u8>,
    pub victorious: Vec<u8>,
    pub selection_sound: Vec<u8>,
    pub taunt_sound1: Vec<u8>,
    pub taunt_sound2: Vec<u8>,
    pub taunt_sound3: Vec<u8>,
    pub win_sound: Vec<u8>,
    pub loss_sound: Vec<u8>,
    pub preview_sound: Vec<u8>,
    pub name_sound: Vec<u8>,
}

impl GeneralPersona {
    /// `isStartingEnabled()`.
    pub fn is_starting_enabled(&self) -> bool {
        self.starts_enabled
    }
    /// `getCampaign()`.
    pub fn get_campaign(&self) -> &[u8] {
        &self.campaign
    }
    /// `getPlayerTemplateName()`.
    pub fn get_player_template_name(&self) -> &[u8] {
        &self.player_template_name
    }
    /// `getBioName()`.
    pub fn get_bio_name(&self) -> &[u8] {
        &self.bio_name
    }
}

/// `class ChallengeGenerals`.
#[derive(Clone, Debug, PartialEq)]
pub struct ChallengeGenerals {
    /// `m_position[NUM_GENERALS]`.
    pub position: [GeneralPersona; NUM_GENERALS],
    /// `m_PlayerTemplateNum`.
    player_template_num: i32,
    /// `m_currentDifficulty` (a `GameDifficulty`).
    current_difficulty: i32,
}

impl Default for ChallengeGenerals {
    fn default() -> Self {
        ChallengeGenerals { position: Default::default(), player_template_num: 0, current_difficulty: 1 }
    }
}

impl ChallengeGenerals {
    /// `createChallengeGenerals()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `init()`: `Data\INI\ChallengeMode.ini` loaded (`INI_LOAD_OVERWRITE`), its `ChallengeGenerals` block.
    pub fn init(&mut self, data: &[u8]) -> IniResult<()> {
        Ini::load(CHALLENGE_MODE_INI, data, IniLoadType::Overwrite, None, CHALLENGE_BLOCKS, self)
    }

    /// `getChallengeGenerals()`.
    pub fn get_challenge_generals(&self) -> &[GeneralPersona; NUM_GENERALS] {
        &self.position
    }

    /// The first persona whose `get` equals `name` without regard to case.
    fn find(&self, name: &[u8], get: fn(&GeneralPersona) -> &[u8]) -> Option<&GeneralPersona> {
        self.position.iter().find(|p| get(p).eq_ignore_ascii_case(name))
    }

    /// `getPlayerGeneralByCampaignName(name)` (`NULL`, a debug assertion upstream, when none).
    pub fn get_player_general_by_campaign_name(&self, name: &[u8]) -> Option<&GeneralPersona> {
        self.find(name, GeneralPersona::get_campaign)
    }
    /// `getGeneralByGeneralName(name)`.
    pub fn get_general_by_general_name(&self, name: &[u8]) -> Option<&GeneralPersona> {
        self.find(name, GeneralPersona::get_bio_name)
    }
    /// `getGeneralByTemplateName(name)`.
    pub fn get_general_by_template_name(&self, name: &[u8]) -> Option<&GeneralPersona> {
        self.find(name, GeneralPersona::get_player_template_name)
    }
    /// `setCurrentPlayerTemplateNum(n)`.
    pub fn set_current_player_template_num(&mut self, n: i32) {
        self.player_template_num = n;
    }
    /// `getCurrentPlayerTemplateNum()`.
    pub fn get_current_player_template_num(&self) -> i32 {
        self.player_template_num
    }
    /// `setCurrentDifficulty(diff)`.
    pub fn set_current_difficulty(&mut self, diff: i32) {
        self.current_difficulty = diff;
    }
    /// `getCurrentDifficulty()`.
    pub fn get_current_difficulty(&self) -> i32 {
        self.current_difficulty
    }
}

/// `parseGeneralPersona`'s `dataFieldParse`.
pub static PERSONA_FIELDS: [FieldParse<GeneralPersona>; 27] = [
    FieldParse::new("StartsEnabled", |i, p, _| i.parse_bool().map(|v| p.starts_enabled = v != crate::logic::modules::plant(4423)), UserData::None),
    FieldParse::new("BioNameString", |i, p, _| i.parse_ascii_string().map(|v| p.bio_name = v), UserData::None),
    FieldParse::new("BioDOBString", |i, p, _| i.parse_ascii_string().map(|v| p.bio_dob = v), UserData::None),
    FieldParse::new("BioBirthplaceString", |i, p, _| i.parse_ascii_string().map(|v| p.bio_birthplace = v), UserData::None),
    FieldParse::new("BioStrategyString", |i, p, _| i.parse_ascii_string().map(|v| p.bio_strategy = v), UserData::None),
    FieldParse::new("BioRankString", |i, p, _| i.parse_ascii_string().map(|v| p.bio_rank = v), UserData::None),
    FieldParse::new("BioBranchString", |i, p, _| i.parse_ascii_string().map(|v| p.bio_branch = v), UserData::None),
    FieldParse::new("BioClassNumberString", |i, p, _| i.parse_ascii_string().map(|v| p.bio_class_number = v), UserData::None),
    // INI::parseMappedImage: the token (its image looked up when drawn)
    FieldParse::new("BioPortraitSmall", |i, p, _| i.get_next_token(None).map(|v| p.bio_portrait_small = v), UserData::None),
    FieldParse::new("BioPortraitLarge", |i, p, _| i.get_next_token(None).map(|v| p.bio_portrait_large = v), UserData::None),
    FieldParse::new("Campaign", |i, p, _| i.parse_ascii_string().map(|v| p.campaign = v), UserData::None),
    FieldParse::new("PlayerTemplate", |i, p, _| i.parse_ascii_string().map(|v| p.player_template_name = v), UserData::None),
    FieldParse::new("PortraitMovieLeftName", |i, p, _| i.parse_ascii_string().map(|v| p.portrait_movie_left_name = v), UserData::None),
    FieldParse::new("PortraitMovieRightName", |i, p, _| i.parse_ascii_string().map(|v| p.portrait_movie_right_name = v), UserData::None),
    FieldParse::new("DefeatedImage", |i, p, _| i.get_next_token(None).map(|v| p.image_defeated = v), UserData::None),
    FieldParse::new("VictoriousImage", |i, p, _| i.get_next_token(None).map(|v| p.image_victorious = v), UserData::None),
    FieldParse::new("DefeatedString", |i, p, _| i.parse_ascii_string().map(|v| p.defeated = v), UserData::None),
    FieldParse::new("VictoriousString", |i, p, _| i.parse_ascii_string().map(|v| p.victorious = v), UserData::None),
    FieldParse::new("SelectionSound", |i, p, _| i.parse_ascii_string().map(|v| p.selection_sound = v), UserData::None),
    FieldParse::new("TauntSound1", |i, p, _| i.parse_ascii_string().map(|v| p.taunt_sound1 = v), UserData::None),
    FieldParse::new("TauntSound2", |i, p, _| i.parse_ascii_string().map(|v| p.taunt_sound2 = v), UserData::None),
    FieldParse::new("TauntSound3", |i, p, _| i.parse_ascii_string().map(|v| p.taunt_sound3 = v), UserData::None),
    FieldParse::new("WinSound", |i, p, _| i.parse_ascii_string().map(|v| p.win_sound = v), UserData::None),
    FieldParse::new("LossSound", |i, p, _| i.parse_ascii_string().map(|v| p.loss_sound = v), UserData::None),
    FieldParse::new("PreviewSound", |i, p, _| i.parse_ascii_string().map(|v| p.preview_sound = v), UserData::None),
    FieldParse::new("NameSound", |i, p, _| i.parse_ascii_string().map(|v| p.name_sound = v), UserData::None),
    FieldParse::end(),
];

/// `parseGeneralPersona(ini, instance, store, ...)`: the persona at the entry's position (the user data here), its
/// fields until `End`.
fn parse_general_persona(ini: &mut Ini<'_>, c: &mut ChallengeGenerals, u: &UserData) -> IniResult<()> {
    ini.init_from_ini(&mut c.position[u.mask() as usize], &PERSONA_FIELDS)
}

/// `ChallengeGenerals::s_fieldParseTable` (`GeneralPersona0` to `GeneralPersona11`).
pub static CHALLENGE_FIELDS: [FieldParse<ChallengeGenerals>; NUM_GENERALS + 1] = [
    FieldParse::new("GeneralPersona0", parse_general_persona, UserData::Mask(0)),
    FieldParse::new("GeneralPersona1", parse_general_persona, UserData::Mask(1)),
    FieldParse::new("GeneralPersona2", parse_general_persona, UserData::Mask(2)),
    FieldParse::new("GeneralPersona3", parse_general_persona, UserData::Mask(3)),
    FieldParse::new("GeneralPersona4", parse_general_persona, UserData::Mask(4)),
    FieldParse::new("GeneralPersona5", parse_general_persona, UserData::Mask(5)),
    FieldParse::new("GeneralPersona6", parse_general_persona, UserData::Mask(6)),
    FieldParse::new("GeneralPersona7", parse_general_persona, UserData::Mask(7)),
    FieldParse::new("GeneralPersona8", parse_general_persona, UserData::Mask(8)),
    FieldParse::new("GeneralPersona9", parse_general_persona, UserData::Mask(9)),
    FieldParse::new("GeneralPersona10", parse_general_persona, UserData::Mask(10)),
    FieldParse::new("GeneralPersona11", parse_general_persona, UserData::Mask(11)),
    FieldParse::end(),
];

/// `INI::parseChallengeModeDefinition`: the block's fields into `TheChallengeGenerals` (no name token is read).
pub fn parse_challenge_mode_definition(ini: &mut Ini<'_>, c: &mut ChallengeGenerals) -> IniResult<()> {
    ini.init_from_ini(c, &CHALLENGE_FIELDS)
}

/// The `ChallengeGenerals` row of `theTypeTable`.
pub static CHALLENGE_BLOCKS: &[BlockParse<ChallengeGenerals>] =
    &[BlockParse { token: "ChallengeGenerals", parse: parse_challenge_mode_definition }];

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic `ChallengeMode.ini`: two personas, the second locked.
    pub(crate) const INI: &[u8] = b"ChallengeGenerals
  GeneralPersona0
    StartsEnabled = Yes
    BioNameString = GUI:BioNameEntry_Pos0
    BioPortraitSmall = SNPortrait0
    Campaign = CHALLENGE_0
    PlayerTemplate = FactionAmerica
    DefeatedImage = Defeated0
    WinSound = Win0
  End
  GeneralPersona11
    StartsEnabled = No
    BioNameString = GUI:BioNameEntry_Pos11
    Campaign = Challenge_11
    PlayerTemplate = FactionChina
  End
End
";

    /// GitHub #61: the personas parse into their positions, and the lookups ignore case. **Predicted before the run:**
    /// position 0 enabled, America, `CHALLENGE_0`, its images' names kept; position 11 locked, China; the others empty;
    /// found by campaign `challenge_0`, general name `gui:bionameentry_pos11`, template `factionchina`. **Result
    /// (2026-11-06):** as predicted, first run.
    #[test]
    fn a_synthetic_challenge_mode_ini_parses() {
        let mut c = ChallengeGenerals::new();
        c.init(INI).expect("the INI parses");
        let p = c.get_challenge_generals();
        assert!(p[0].is_starting_enabled());
        assert_eq!(p[0].bio_portrait_small, b"SNPortrait0");
        assert_eq!(p[0].image_defeated, b"Defeated0");
        assert_eq!(p[0].win_sound, b"Win0");
        assert!(!p[11].is_starting_enabled());
        assert!(p[5].campaign.is_empty());
        assert_eq!(c.get_player_general_by_campaign_name(b"challenge_0").map(|g| g.player_template_name.clone()), Some(b"FactionAmerica".to_vec()));
        assert_eq!(c.get_general_by_general_name(b"gui:bionameentry_pos11").map(|g| g.campaign.clone()), Some(b"Challenge_11".to_vec()));
        assert!(c.get_general_by_template_name(b"factionchina").is_some_and(|g| !g.is_starting_enabled()));
        assert!(c.get_general_by_template_name(b"FactionGLA").is_none());
    }
}
