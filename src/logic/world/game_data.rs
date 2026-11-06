// SPDX-License-Identifier: GPL-3.0-or-later
//! Integration batch I12: a game's INI read into the world's statics, as `GameEngine::init` reads it.
//!
//! `GameEngine::init` (GameEngine.cpp) makes each subsystem in turn and loads its INI files through
//! `SubsystemInterfaceList::initSubsystem`: `init()`, then `INI::load(path1)`, `INI::load(path2)` and
//! `INI::loadDirectory(dirpath)`, every load with `INI_LOAD_OVERWRITE`. Any file may hold any block (`INI::load` looks each
//! token up in one table, `theTypeTable`). After every subsystem, `postProcessLoadAll()` runs their `postProcessLoad()`
//! in the same order; of the logic's, the weapon store's and then the thing factory's.
//!
//! [`load_game_data`] does that for the subsystems the logic owns, in upstream's order and with upstream's paths
//! ([`LOGIC_SUBSYSTEMS`]), over a file system given as [`GameDataFiles`]. The blocks it reads are the logic's:
//! `GameData`, `Science`, `Rank`, `PlayerTemplate`, `Weapon`, `ObjectCreationList`, `Locomotor`, `SpecialPower`,
//! `DamageFX`, `Armor`, `Object`, `ObjectReskin`, `Upgrade`, `AIData` and `CrateData`; **Corrected 2026-11-03 (batch I50):** also two of the client's
//! the logic reads: `CommandButton` and `CommandSet` (since batch I19) and `Road` and `Bridge` (`TheTerrainRoads`, batch I50).
//! **The other client subsystems are not loaded** (the FX lists, particle systems, ~~audio,~~ the UI, the terrain types,
//! the multiplayer settings, the map cache; ~~the terrain roads, the command sets and buttons~~): a block of theirs in a file
//! the logic loads is an unknown token, as it would be upstream for a token missing from the table.
//! **Corrected 2026-11-06 (GitHub #50):** the audio's descriptions are read too, after the list, by the audio's own
//! loader (`AudioManager::init`'s files into [`GameDataLoad::audio`]); their blocks stay unknown tokens in the logic's files.
//!
//! **Subsystem `init()`s (batch I54):** of the logic's, only `UpgradeCenter::init` makes data (the veterancy upgrades) and
//! is run, in its place before its files; the module factory's runs when the load is made.
//!
//! **Missing files:** upstream's `INI::load` throws on a file it cannot open (the shipped data has every one). With
//! [`MissingFiles::Error`] so does the loader; [`MissingFiles::Skip`] skips one, for a test's or a mod's partial data.
//!
//! `p1w13-sonnet`, integration batch I12, 2026-11-02.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::common::damage_fx::DamageFxStore;
use crate::common::global_data::GlobalDataStore;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, UserData};
use crate::common::module_factory::ModuleFactory;
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::player_template::PlayerTemplateStore;
use crate::common::science::{GameTextFetch, ScienceStore, ScienceType};
use crate::common::special_power::SpecialPowerStore;
use crate::common::thing_factory::ThingFactory;
use crate::common::thing_template::ThingParseEnv;
use crate::common::unicode_string::UnicodeString;
use crate::common::upgrade::UpgradeCenter;
use crate::formats::big::FilenameList;
use crate::logic::ai::ai_data::AiDataChain;
use crate::logic::armor::ArmorStore;
use crate::logic::crate_system::CrateSystem;
use crate::logic::locomotor::LocomotorStore;
use crate::logic::object_creation_list::{ObjectCreationListStore, OclParseEnv};
use crate::logic::rank_info::RankInfoStore;
use crate::logic::weapon::WeaponStore;
use crate::logic::weapon_set::WeaponTemplateTable;
use crate::logic::world::statics::GameStatics;

/// The files a load reads: `TheFileSystem`. Paths are upstream's (`Data\\INI\\Weapon.ini`); upstream's file system ignores case.
pub trait GameDataFiles {
    /// `TheFileSystem->openFile(path)`'s contents, `None` when there is no such file.
    fn open(&self, path: &[u8]) -> Option<Vec<u8>>;
    /// `TheFileSystem->getFileListInDirectory(dir + "\\", "*.ini", list, TRUE)`: every `.ini` file under `dir`, with its full path.
    fn list_ini(&self, dir: &[u8]) -> FilenameList;
    /// GitHub #52: `TheFileSystem->doesFileExist(path)` (by default, whether it opens).
    fn file_exists(&self, path: &[u8]) -> bool {
        self.open(path).is_some()
    }
    /// GitHub #52: `TheFileSystem->getFileInfo(path)`'s size, `None` when there is no such file (by default, the opened
    /// file's length).
    fn file_size(&self, path: &[u8]) -> Option<usize> {
        self.open(path).map(|d| d.len())
    }
    /// GitHub #52: `len` bytes of the file from `offset` (fewer at its end, none without the file). A file system over
    /// archives reads only that range of the member; by default the file is opened whole.
    fn read_file_range(&self, path: &[u8], offset: usize, len: usize) -> Vec<u8> {
        self.open(path).map(|d| d[offset.min(d.len())..offset.saturating_add(len).min(d.len())].to_vec()).unwrap_or_default()
    }
}

/// A file system in memory: the files by path, compared without case.
#[derive(Clone, Debug, Default)]
pub struct MemoryFiles {
    files: BTreeMap<Vec<u8>, (Vec<u8>, Vec<u8>)>,
}

impl MemoryFiles {
    /// An empty file system.
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds (or replaces) the file `path` with `data`.
    pub fn insert(&mut self, path: &[u8], data: Vec<u8>) {
        self.files.insert(path.to_ascii_lowercase(), (path.to_vec(), data));
    }
}

impl GameDataFiles for MemoryFiles {
    fn open(&self, path: &[u8]) -> Option<Vec<u8>> {
        self.files.get(&path.to_ascii_lowercase()).map(|(_, d)| d.clone())
    }
    fn list_ini(&self, dir: &[u8]) -> FilenameList {
        let mut prefix = dir.to_ascii_lowercase();
        prefix.push(b'\\');
        let mut list = FilenameList::new();
        for (k, (path, _)) in &self.files {
            if k.starts_with(&prefix) && k.ends_with(b".ini") {
                list.insert(path.clone());
            }
        }
        list
    }
}

/// What a load does about a file it cannot open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingFiles {
    /// Fail, as upstream's `INI::load` does.
    Error,
    /// Skip the file.
    Skip,
}

/// The game text, by label (`TheGameText->fetch(label)`); ~~a label it does not hold fetches the empty string~~ **corrected
/// 2026-11-04 (batch I56):** a label it does not hold fetches upstream's `MISSING: 'label'` (GameText.cpp:1301). The empty string
/// was not upstream's, and it was not harmless: `parseAndTranslateLabel` treats an empty translation as `INI_INVALID_DATA`, and
/// `parseMapCacheDefinition` keeps no map whose display name is empty, so a game read without its string file lost both.
#[derive(Clone, Debug, Default)]
pub struct TextTable {
    /// The strings by label (a test's own; looked up first, by the exact label).
    pub strings: BTreeMap<Vec<u8>, UnicodeString>,
    /// Batch I57: the game's string file (`GameTextManager::init`'s `Data\<language>\Generals.csf`), looked up as
    /// upstream looks it up (`stricmp`).
    pub csf: Option<std::sync::Arc<crate::formats::csf::GameText>>,
}

impl TextTable {
    /// Batch I57: the table of a string file.
    pub fn from_csf(csf: crate::formats::csf::GameText) -> Self {
        TextTable { strings: BTreeMap::new(), csf: Some(std::sync::Arc::new(csf)) }
    }
}

impl GameTextFetch for TextTable {
    fn fetch(&self, label: &[u8]) -> UnicodeString {
        if let Some(s) = self.strings.get(label) {
            return s.clone();
        }
        if let Some(t) = self.csf.as_ref().and_then(|c| c.fetch(label)) {
            return UnicodeString::try_from_chars(t).unwrap_or_default();
        }
        // `missingString.format(L"MISSING: '%hs'", label)` (`csf::missing_text`; a text too long for upstream's buffer
        // throws there, and is the empty string here)
        let units = crate::formats::csf::missing_text(label).unwrap_or_default();
        UnicodeString::try_from_chars(&units).unwrap_or_default()
    }
}

/// One logic subsystem's INI: its name, `path1`, `path2` and `dirpath` as `GameEngine::init` gives them.
pub struct SubsystemFiles {
    /// The subsystem (`TheWeaponStore`).
    pub name: &'static str,
    /// `path1` (the defaults).
    pub path1: Option<&'static [u8]>,
    /// `path2`.
    pub path2: Option<&'static [u8]>,
    /// `dirpath`.
    pub dirpath: Option<&'static [u8]>,
}

/// The logic's subsystems in `GameEngine::init`'s order, with their files (GameEngine.cpp:363-490).
pub const LOGIC_SUBSYSTEMS: &[SubsystemFiles] = &[
    SubsystemFiles { name: "TheWritableGlobalData", path1: Some(b"Data\\INI\\Default\\GameData.ini"), path2: Some(b"Data\\INI\\GameData.ini"), dirpath: None },
    SubsystemFiles { name: "TheScienceStore", path1: Some(b"Data\\INI\\Default\\Science.ini"), path2: Some(b"Data\\INI\\Science.ini"), dirpath: None },
    // (batch I56: GameEngine.cpp:423, after the science store)
    SubsystemFiles { name: "TheMultiplayerSettings", path1: Some(b"Data\\INI\\Default\\Multiplayer.ini"), path2: Some(b"Data\\INI\\Multiplayer.ini"), dirpath: None },
    // (batch I59, #16: GameEngine.cpp:424, the terrain types the build assistant asks of a tile)
    SubsystemFiles { name: "TheTerrainTypes", path1: Some(b"Data\\INI\\Default\\Terrain.ini"), path2: Some(b"Data\\INI\\Terrain.ini"), dirpath: None },
    // (batch I50: the client's subsystem the logic reads, after the multiplayer settings and terrain types, GameEngine.cpp:425)
    SubsystemFiles { name: "TheTerrainRoads", path1: Some(b"Data\\INI\\Default\\Roads.ini"), path2: Some(b"Data\\INI\\Roads.ini"), dirpath: None },
    SubsystemFiles { name: "TheRankInfoStore", path1: None, path2: Some(b"Data\\INI\\Rank.ini"), dirpath: None },
    SubsystemFiles { name: "ThePlayerTemplateStore", path1: Some(b"Data\\INI\\Default\\PlayerTemplate.ini"), path2: Some(b"Data\\INI\\PlayerTemplate.ini"), dirpath: None },
    SubsystemFiles { name: "TheWeaponStore", path1: None, path2: Some(b"Data\\INI\\Weapon.ini"), dirpath: None },
    SubsystemFiles { name: "TheObjectCreationListStore", path1: Some(b"Data\\INI\\Default\\ObjectCreationList.ini"), path2: Some(b"Data\\INI\\ObjectCreationList.ini"), dirpath: None },
    SubsystemFiles { name: "TheLocomotorStore", path1: None, path2: Some(b"Data\\INI\\Locomotor.ini"), dirpath: None },
    SubsystemFiles { name: "TheSpecialPowerStore", path1: Some(b"Data\\INI\\Default\\SpecialPower.ini"), path2: Some(b"Data\\INI\\SpecialPower.ini"), dirpath: None },
    SubsystemFiles { name: "TheDamageFXStore", path1: None, path2: Some(b"Data\\INI\\DamageFX.ini"), dirpath: None },
    SubsystemFiles { name: "TheArmorStore", path1: None, path2: Some(b"Data\\INI\\Armor.ini"), dirpath: None },
    SubsystemFiles { name: "TheThingFactory", path1: Some(b"Data\\INI\\Default\\Object.ini"), path2: None, dirpath: Some(b"Data\\INI\\Object") },
    SubsystemFiles { name: "TheUpgradeCenter", path1: Some(b"Data\\INI\\Default\\Upgrade.ini"), path2: Some(b"Data\\INI\\Upgrade.ini"), dirpath: None },
    // (batch I19: `TheGameClient`'s `ControlBar::init`, between the upgrade center and the AI, GameEngine.cpp:486; its three files)
    SubsystemFiles { name: "TheControlBar", path1: Some(b"Data\\INI\\Default\\CommandButton.ini"), path2: Some(b"Data\\INI\\CommandButton.ini"), dirpath: None },
    SubsystemFiles { name: "TheControlBar (sets)", path1: None, path2: Some(b"Data\\INI\\CommandSet.ini"), dirpath: None },
    SubsystemFiles { name: "TheAI", path1: Some(b"Data\\INI\\Default\\AIData.ini"), path2: Some(b"Data\\INI\\AIData.ini"), dirpath: None },
    SubsystemFiles { name: "TheCrateSystem", path1: Some(b"Data\\INI\\Default\\Crate.ini"), path2: Some(b"Data\\INI\\Crate.ini"), dirpath: None },
];

/// What the INI is read into: the logic's stores, the name keys, and what their parsers look up.
pub struct GameDataLoad {
    /// `TheNameKeyGenerator`.
    pub names: NameKeyGenerator,
    /// `TheModuleFactory`.
    pub modules: ModuleFactory,
    /// `TheGameText`.
    pub text: TextTable,
    /// `TheWritableGlobalData`.
    pub global_data: GlobalDataStore,
    /// `TheScienceStore`.
    pub sciences: ScienceStore,
    /// `TheRankInfoStore`.
    pub ranks: RankInfoStore,
    /// `ThePlayerTemplateStore`.
    pub player_templates: PlayerTemplateStore,
    /// `TheWeaponStore`.
    pub weapon_store: WeaponStore,
    /// The weapons' names, as the thing templates' weapon sets look them up.
    pub weapons: WeaponTemplateTable,
    /// `TheObjectCreationListStore`.
    pub ocls: ObjectCreationListStore,
    /// `TheLocomotorStore`.
    pub locomotors: LocomotorStore,
    /// `TheSpecialPowerStore`.
    pub special_powers: SpecialPowerStore,
    /// `TheDamageFXStore`.
    pub damage_fx: DamageFxStore,
    /// `TheArmorStore`.
    pub armors: ArmorStore,
    /// `TheThingFactory`.
    pub factory: ThingFactory,
    /// `TheUpgradeCenter`.
    pub upgrades: UpgradeCenter,
    /// `TheAI->getAiData()`'s chain.
    pub ai_data: AiDataChain,
    /// `TheCrateSystem`.
    pub crates: CrateSystem,
    /// Integration batch I19: `TheControlBar`'s command buttons and sets.
    pub command_buttons: crate::logic::command_button::CommandButtonStore,
    /// Integration batch I50: `TheTerrainRoads`.
    pub terrain_roads: crate::common::terrain_roads::TerrainRoadCollection,
    /// Integration batch I56: `TheMultiplayerSettings`.
    pub multiplayer_settings: crate::common::multiplayer_settings::MultiplayerSettings,
    /// Integration batch I56: `TheMapCache`, which no INI of this load reads (it is made after the load, by
    /// `updateCache`); kept so that a load made again from the statics gives it back.
    pub map_cache: crate::common::map_cache::MapCache,
    /// Integration batch I59: `TheTerrainTypes`.
    pub terrain_types: crate::common::terrain_types::TerrainTypeCollection,
    /// Batch C3: `GameSpyColor[GSCOLOR_MAX]` as an `OnlineChatColors` block sets it (the online lobby's colours, which the
    /// game's `Multiplayer.ini` carries beside the logic's settings). Only what the files set: upstream's defaults
    /// (Chat.cpp) are the lobby's, which this engine has not.
    pub online_chat_colors: [i32; GSCOLOR_MAX],
    /// GitHub #50: `TheAudio`'s descriptions (`AudioManager::init`'s files; see [`load_game_data`]).
    pub audio: crate::client::audio_info::AudioTables,
    /// GitHub #52: `getAudioLengthMS` of each event ([`GameStatics::audio_lengths_ms`]), made by
    /// [`GameDataLoad::load_audio_lengths`].
    pub audio_lengths_ms: BTreeMap<Vec<u8>, f32>,
    /// What a button's fields look up, ~~made at the first button (the stores it names are read before the control bar)~~ **corrected
    /// in integration batch I27:** remade when the stores it is made from have grown since (upstream's `find*` calls are live at every
    /// field: a power, template or upgrade read after the first button was invisible to the later buttons); the stores' sizes when made.
    button_lookups: Option<(Arc<crate::logic::command_button::CommandButtonLookups>, [usize; 4])>,
}

impl GameDataLoad {
    /// The stores empty, after each subsystem's `init()`, with `text` as the game text.
    pub fn new(text: TextTable) -> IniResult<Self> {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut modules = ModuleFactory::new();
        modules.init(&mut names).map_err(|_| IniError::new(IniErrorKind::Bug))?;
        Ok(GameDataLoad {
            online_chat_colors: [0; GSCOLOR_MAX],
            names,
            modules,
            text,
            global_data: GlobalDataStore::new(),
            sciences: ScienceStore::new(),
            ranks: RankInfoStore::new(),
            player_templates: PlayerTemplateStore::new(),
            weapon_store: WeaponStore::new(),
            weapons: WeaponTemplateTable::new(),
            ocls: ObjectCreationListStore::new(),
            locomotors: LocomotorStore::new(),
            special_powers: SpecialPowerStore::new(),
            damage_fx: DamageFxStore::new(),
            armors: ArmorStore::new(),
            factory: ThingFactory::new(),
            upgrades: UpgradeCenter::new(),
            ai_data: Default::default(),
            crates: CrateSystem::new(),
            command_buttons: Default::default(),
            terrain_roads: Default::default(),
            multiplayer_settings: Default::default(),
            map_cache: Default::default(),
            terrain_types: Default::default(),
            audio: Default::default(),
            audio_lengths_ms: BTreeMap::new(),
            button_lookups: None,
        })
    }

    /// `INI::load(path, INI_LOAD_OVERWRITE, NULL)` of one file's text.
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, &BLOCKS, self)
    }

    /// Integration batch I33: `INI::load(path, loadType, NULL)` of one file's text (`INI_LOAD_CREATE_OVERRIDES` for a map's
    /// `map.ini`).
    pub fn load_text_as(&mut self, path: &[u8], text: &[u8], load_type: IniLoadType) -> IniResult<()> {
        Ini::load(path, text, load_type, None, &BLOCKS, self)
    }

    /// GitHub #52: `getAudioLengthMS` of every event of the audio tables read, from the headers of the files `files`
    /// holds (`crate::logic::audio_length`), with `language` as `GetRegistryLanguage()` for the localized files. A load
    /// from an install does it after [`load_game_data`] (`World::from_install`); every machine reads the same files, so
    /// every machine gets the same lengths.
    pub fn load_audio_lengths(&mut self, files: &impl GameDataFiles, language: &[u8]) {
        self.audio_lengths_ms = crate::logic::audio_length::audio_lengths_ms(&self.audio, files, language);
    }

    /// `postProcessLoadAll()` for the logic's subsystems, in their order: the weapon store's, then the thing factory's.
    pub fn post_process_load(&mut self) {
        weapon_store_post_process(&mut self.weapon_store, &self.factory, &self.ocls, &mut self.names);
        self.factory.post_process_load();
    }

    /// Integration batch I33: the load again from a world's statics and name keys (the inverse of [`GameDataLoad::into_statics`]), to
    /// read more INI into the stores the world already has (a map's `map.ini`, `INI_LOAD_OVERWRITE`). What the statics do not keep is
    /// made again: the module factory (its `init`), the weapons' name table (the weapon store's templates), the AI data's chain (the
    /// current data only: `into_statics` keeps no more), ~~no game text~~ the statics' game text (batch I57), and the buttons' lookups (made at the next button).
    pub fn from_statics(st: GameStatics, mut names: NameKeyGenerator) -> IniResult<Self> {
        let mut modules = ModuleFactory::new();
        modules.init(&mut names).map_err(|_| IniError::new(IniErrorKind::Bug))?;
        let mut weapons = WeaponTemplateTable::new();
        for t in st.weapon_store.templates() {
            weapons.add(t.name_key);
        }
        Ok(GameDataLoad {
            online_chat_colors: [0; GSCOLOR_MAX],
            names,
            modules,
            text: st.game_text,
            global_data: st.global_data,
            sciences: st.science_store,
            ranks: st.rank_info_store,
            player_templates: st.player_template_store,
            weapon_store: st.weapon_store,
            weapons,
            ocls: st.object_creation_list_store,
            locomotors: st.locomotor_store,
            special_powers: st.special_power_store,
            damage_fx: st.damage_fx_store,
            armors: st.armor_store,
            factory: st.thing_factory,
            upgrades: st.upgrade_center,
            ai_data: crate::logic::ai::ai_data::AiDataChain { entries: vec![st.ai_data] },
            crates: st.crate_system,
            command_buttons: st.command_buttons,
            terrain_roads: st.terrain_roads,
            multiplayer_settings: st.multiplayer_settings,
            map_cache: st.map_cache,
            terrain_types: st.terrain_types,
            audio: st.audio,
            audio_lengths_ms: st.audio_lengths_ms,
            button_lookups: None,
        })
    }

    /// The world's statics of the load (and its name keys, which the world keeps beside them).
    pub fn into_statics(self) -> (GameStatics, NameKeyGenerator) {
        let statics = GameStatics {
            upgrade_center: self.upgrades,
            science_store: self.sciences,
            special_power_store: self.special_powers,
            thing_factory: self.factory,
            object_creation_list_store: self.ocls,
            weapon_store: self.weapon_store,
            armor_store: self.armors,
            damage_fx_store: self.damage_fx,
            crate_system: self.crates,
            global_data: self.global_data,
            player_template_store: self.player_templates,
            rank_info_store: self.ranks,
            locomotor_store: self.locomotors,
            ai_data: self.ai_data.current().clone(),
            command_buttons: self.command_buttons,
            terrain_roads: self.terrain_roads,
            multiplayer_settings: self.multiplayer_settings,
            map_cache: self.map_cache,
            game_text: self.text,
            terrain_types: self.terrain_types,
            audio: self.audio,
            audio_lengths_ms: self.audio_lengths_ms,
        };
        (statics, self.names)
    }
}

/// `TheWeaponStore->postProcessLoad()`: each weapon template's projectile template and OCLs by name. An OCL is its list's name key in
/// `ocls` (as the world's weapon OCL calls look it up); `None` is `NULL`.
pub fn weapon_store_post_process(weapon_store: &mut WeaponStore, factory: &ThingFactory, ocls: &ObjectCreationListStore, names: &mut NameKeyGenerator) {
    let keys: BTreeMap<Vec<u8>, u32> = ocls.ocls.keys().map(|k| (names.key_to_name(*k).to_vec(), k.0)).collect();
    let find_template = |n: &[u8]| factory.find_template(n).map(|t| t.data.get_template_id());
    let find_ocl = |n: &[u8]| if n.eq_ignore_ascii_case(b"None") { None } else { keys.get(n).copied() };
    weapon_store.post_process_load(&find_template, &find_ocl);
}

/// `GameEngine::init`'s INI loads for the logic's subsystems ([`LOGIC_SUBSYSTEMS`]), then their `postProcessLoad()`.
pub fn load_game_data(files: &impl GameDataFiles, missing: MissingFiles, text: TextTable) -> IniResult<GameDataLoad> {
    let mut load = GameDataLoad::new(text)?;
    for sys in LOGIC_SUBSYSTEMS {
        // (batch I54: `initSubsystem` calls the subsystem's `init()` before its loads. The upgrade center's is the one with an
        // effect on the data: `UpgradeCenter::init` makes the three veterancy upgrades (`Upgrade_Veterancy_VETERAN`, `_ELITE`,
        // `_HEROIC`), which take mask bits 0 to 2 and name keys before any `Upgrade` block. It was never called, so no unit got
        // its veterancy upgrade and every INI upgrade's mask bit was three lower than upstream's)
        if sys.name == "TheUpgradeCenter" {
            load.upgrades.init(&mut load.names).map_err(|_| IniError::new(IniErrorKind::Bug))?;
        }
        for path in [sys.path1, sys.path2].into_iter().flatten() {
            match files.open(path) {
                Some(data) => load.load_text(path, &data)?,
                None if missing == MissingFiles::Skip => {}
                None => return Err(IniError::new(IniErrorKind::CantOpenFile)),
            }
        }
        if let Some(dir) = sys.dirpath {
            let list = files.list_ini(dir);
            Ini::load_directory(dir, true, IniLoadType::Overwrite, None, &list, |p| files.open(p), &BLOCKS, &mut load)?;
        }
    }
    // (GitHub #50: `initSubsystem(TheAudio, ...)`, GameEngine.cpp:434: `AudioManager::init` loads its own files with its own
    // blocks (`client::audio_info::AUDIO_INI_FILES`), which no other subsystem reads, so where in the list it runs changes nothing.
    // The logic reads a logical event's description from it (`logic::logical_audio`). Unlike upstream's `INI::load`, a
    // missing or bad audio file is skipped, as the client's own load skips it: every machine reads the same files, so
    // every machine skips the same ones. No user preferences are read: they set only volumes, which the logic never reads)
    let _ = load.audio.load(|p| files.open(p));
    load.post_process_load();
    Ok(load)
}

/// #51: `INI::parseAudioEventDefinition` into the audio's tables (`TheAudio->newAudioEventInfo`), recorded for the client.
fn audio_event_block(ini: &mut Ini<'_>, l: &mut GameDataLoad) -> IniResult<()> {
    crate::client::audio_info::logic_ini_audio_event(ini, &mut l.audio)
}

/// #51: `INI::parseDialogDefinition`, as [`audio_event_block`].
fn dialog_event_block(ini: &mut Ini<'_>, l: &mut GameDataLoad) -> IniResult<()> {
    crate::client::audio_info::logic_ini_dialog_event(ini, &mut l.audio)
}

/// #51: `INI::parseMusicTrackDefinition`, as [`audio_event_block`].
fn music_track_block(ini: &mut Ini<'_>, l: &mut GameDataLoad) -> IniResult<()> {
    crate::client::audio_info::logic_ini_music_track(ini, &mut l.audio)
}

// -------------------------------------------------------------------------------------------------------------------------------
// the blocks (`theTypeTable`'s logic entries)
// -------------------------------------------------------------------------------------------------------------------------------

static BLOCKS: [BlockParse<GameDataLoad>; 27] = [
    BlockParse { token: "AIData", parse: ai_data_block },
    // (#51: `theTypeTable`'s audio event rows, which a map's `map.ini` can hold; the descriptions go to the audio's tables)
    BlockParse { token: "AudioEvent", parse: audio_event_block },
    BlockParse { token: "DialogEvent", parse: dialog_event_block },
    BlockParse { token: "MusicTrack", parse: music_track_block },
    BlockParse { token: "Armor", parse: armor_block },
    BlockParse { token: "Bridge", parse: bridge_block },
    BlockParse { token: "CommandButton", parse: command_button_block },
    BlockParse { token: "CommandSet", parse: command_set_block },
    BlockParse { token: "CrateData", parse: crate_block },
    BlockParse { token: "DamageFX", parse: damage_fx_block },
    BlockParse { token: "GameData", parse: game_data_block },
    BlockParse { token: "Locomotor", parse: locomotor_block },
    BlockParse { token: "MultiplayerColor", parse: multiplayer_color_block },
    BlockParse { token: "MultiplayerSettings", parse: multiplayer_settings_block },
    BlockParse { token: "MultiplayerStartingMoneyChoice", parse: multiplayer_money_block },
    BlockParse { token: "Object", parse: object_block },
    BlockParse { token: "ObjectCreationList", parse: ocl_block },
    BlockParse { token: "ObjectReskin", parse: object_reskin_block },
    BlockParse { token: "OnlineChatColors", parse: online_chat_colors_block },
    BlockParse { token: "PlayerTemplate", parse: player_template_block },
    BlockParse { token: "Science", parse: science_block },
    BlockParse { token: "Rank", parse: rank_block },
    BlockParse { token: "Road", parse: road_block },
    BlockParse { token: "SpecialPower", parse: special_power_block },
    BlockParse { token: "Terrain", parse: terrain_block },
    BlockParse { token: "Upgrade", parse: upgrade_block },
    BlockParse { token: "Weapon", parse: weapon_block },
];

/// `TheGlobalData->m_defaultOcclusionDelay` as `INI::parseObjectDefinition` passes it (0 before a `GameData` block is read).
fn default_occlusion_delay(c: &GameDataLoad) -> u32 {
    c.global_data.the_global_data().map_or(0, |g| g.default_occlusion_delay)
}

fn object_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    let delay = default_occlusion_delay(c);
    let mut env = ThingParseEnv {
        names: &mut c.names,
        modules: &mut c.modules,
        armors: &c.armors,
        damage_fx: &c.damage_fx,
        weapons: &c.weapons,
        sciences: &c.sciences,
        text: &c.text,
        locomotors: &c.locomotors,
    };
    c.factory.parse_object_block(ini, &mut env, delay)
}

fn object_reskin_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    let delay = default_occlusion_delay(c);
    let mut env = ThingParseEnv {
        names: &mut c.names,
        modules: &mut c.modules,
        armors: &c.armors,
        damage_fx: &c.damage_fx,
        weapons: &c.weapons,
        sciences: &c.sciences,
        text: &c.text,
        locomotors: &c.locomotors,
    };
    c.factory.parse_object_reskin_block(ini, &mut env, delay)
}

fn ai_data_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    crate::logic::ai::ai_data::parse_ai_data_definition(ini, &mut c.ai_data, &c.sciences, &mut c.names).unwrap_or_else(|_| Err(IniError::new(IniErrorKind::Bug)))
}

/// `INI::parseObjectCreationListDefinition`; a nugget's weapon is the weapon store's by index (the weapons are read first).
fn ocl_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    let weapon_templates = c.weapon_store.templates().iter().enumerate().map(|(i, t)| (t.name.clone(), i as u32)).collect();
    let env = Arc::new(OclParseEnv { preload_assets: false, weapon_templates });
    c.ocls.parse_object_creation_list_definition(ini, &mut c.names, &env)
}

fn weapon_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.weapon_store.parse_weapon_template_definition(ini, &mut c.names)?;
    // (the thing templates' weapon sets name the weapons by key)
    for t in c.weapon_store.templates() {
        c.weapons.add(t.name_key);
    }
    Ok(())
}

fn locomotor_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.locomotors.parse_locomotor_template_definition(ini, &mut c.names)
}

fn armor_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.armors.parse_armor_definition(ini, &mut c.names)
}

fn damage_fx_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.damage_fx.parse_damage_fx_definition(ini, &mut c.names)
}

/// `INI::parseTerrainBridgeDefinition` (batch I50).
fn bridge_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.terrain_roads.parse_terrain_bridge_definition(ini)
}

/// `INI::parseTerrainRoadDefinition` (batch I50).
fn road_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.terrain_roads.parse_terrain_road_definition(ini)
}

fn crate_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.crates.parse_crate_template_definition(ini, &mut c.names)
}

fn game_data_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    crate::common::global_data::parse_game_data_definition(ini, &mut c.global_data)
}

fn science_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.sciences.parse_science_definition(ini, &mut c.names, &c.text)
}

fn rank_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.ranks.parse_rank_definition(ini, &mut c.names, &c.sciences, &c.text)
}

fn player_template_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.player_templates.parse_player_template_definition(ini, &mut c.names, &c.sciences, &c.text)
}

fn special_power_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    let sciences = &c.sciences;
    c.special_powers.parse_special_power_definition(ini, &mut c.names, &|k| sciences.is_valid_science(ScienceType::from_name_key(k)))
}

/// (batch I19) `INI::parseCommandButtonDefinition`, its fields' lookups made from the stores read before it.
fn command_button_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    let sizes = [c.factory.templates().count(), c.upgrades.get_upgrade_names().len(), c.special_powers.templates().count(), c.sciences.infos().count()];
    if c.button_lookups.as_ref().is_none_or(|(_, s)| *s != sizes) {
        let mut l = crate::logic::command_button::CommandButtonLookups::default();
        for t in c.factory.templates() {
            l.templates.insert(t.data.get_name().to_vec(), t.data.get_template_id());
        }
        for name in c.upgrades.get_upgrade_names() {
            let key = c.names.name_to_key(&name);
            l.upgrades.insert(name, key);
        }
        for sp in c.special_powers.templates() {
            l.special_powers.insert(sp.name().to_vec(), sp.id());
        }
        l.sciences = c.sciences.infos().map(|i| i.science()).collect();
        c.button_lookups = Some((Arc::new(l), sizes));
    }
    let lookups = c.button_lookups.as_ref().map(|(l, _)| l.clone()).expect("made above");
    c.command_buttons.parse_command_button_definition(ini, &mut c.names, &lookups)
}

/// (batch I19) `INI::parseCommandSetDefinition`.
fn command_set_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.command_buttons.parse_command_set_definition(ini)
}

fn terrain_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.terrain_types.parse_terrain_definition(ini)
}

fn multiplayer_settings_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.multiplayer_settings.parse_multiplayer_settings_definition(ini)
}

fn multiplayer_color_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.multiplayer_settings.parse_multiplayer_color_definition(ini)
}

fn multiplayer_money_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.multiplayer_settings.parse_multiplayer_starting_money_choice_definition(ini)
}

/// `GSCOLOR_MAX` (PeerDefs.h).
pub const GSCOLOR_MAX: usize = 27;

fn parse_chat_color(ini: &mut Ini<'_>, colors: &mut [i32; GSCOLOR_MAX], which: &UserData) -> IniResult<()> {
    colors[which.mask() as usize] = ini.parse_color_int()?;
    Ok(())
}

/// `GameSpyColorFieldParse` (Chat.cpp): every field `INI::parseColorInt` into its `GameSpyColor` slot, in `GSColors`' order.
static ONLINE_CHAT_COLOR_FIELDS: [FieldParse<[i32; GSCOLOR_MAX]>; GSCOLOR_MAX] = [
    FieldParse::new("Default", parse_chat_color, UserData::Mask(0)),
    FieldParse::new("CurrentRoom", parse_chat_color, UserData::Mask(1)),
    FieldParse::new("ChatRoom", parse_chat_color, UserData::Mask(2)),
    FieldParse::new("Game", parse_chat_color, UserData::Mask(3)),
    FieldParse::new("GameFull", parse_chat_color, UserData::Mask(4)),
    FieldParse::new("GameCRCMismatch", parse_chat_color, UserData::Mask(5)),
    FieldParse::new("PlayerNormal", parse_chat_color, UserData::Mask(6)),
    FieldParse::new("PlayerOwner", parse_chat_color, UserData::Mask(7)),
    FieldParse::new("PlayerBuddy", parse_chat_color, UserData::Mask(8)),
    FieldParse::new("PlayerSelf", parse_chat_color, UserData::Mask(9)),
    FieldParse::new("PlayerIgnored", parse_chat_color, UserData::Mask(10)),
    FieldParse::new("ChatNormal", parse_chat_color, UserData::Mask(11)),
    FieldParse::new("ChatEmote", parse_chat_color, UserData::Mask(12)),
    FieldParse::new("ChatOwner", parse_chat_color, UserData::Mask(13)),
    FieldParse::new("ChatOwnerEmote", parse_chat_color, UserData::Mask(14)),
    FieldParse::new("ChatPriv", parse_chat_color, UserData::Mask(15)),
    FieldParse::new("ChatPrivEmote", parse_chat_color, UserData::Mask(16)),
    FieldParse::new("ChatPrivOwner", parse_chat_color, UserData::Mask(17)),
    FieldParse::new("ChatPrivOwnerEmote", parse_chat_color, UserData::Mask(18)),
    FieldParse::new("ChatBuddy", parse_chat_color, UserData::Mask(19)),
    FieldParse::new("ChatSelf", parse_chat_color, UserData::Mask(20)),
    FieldParse::new("AcceptTrue", parse_chat_color, UserData::Mask(21)),
    FieldParse::new("AcceptFalse", parse_chat_color, UserData::Mask(22)),
    FieldParse::new("MapSelected", parse_chat_color, UserData::Mask(23)),
    FieldParse::new("MapUnselected", parse_chat_color, UserData::Mask(24)),
    FieldParse::new("MOTD", parse_chat_color, UserData::Mask(25)),
    FieldParse::new("MOTDHeading", parse_chat_color, UserData::Mask(26)),
];

/// `INI::parseOnlineChatColorDefinition` (Chat.cpp): `ini->initFromINI(GameSpyColor, GameSpyColorFieldParse)`. The block sits
/// in the game's `Multiplayer.ini`, which the logic reads; without it the load stopped at an unknown block, where upstream,
/// whose table holds every block, reads on (batch C3, found reading upstream's block table before a real install's load).
fn online_chat_colors_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    ini.init_from_ini(&mut c.online_chat_colors, &ONLINE_CHAT_COLOR_FIELDS)
}

fn upgrade_block(ini: &mut Ini<'_>, c: &mut GameDataLoad) -> IniResult<()> {
    c.upgrades.parse_upgrade_definition(ini, &mut c.names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> MemoryFiles {
        let mut f = MemoryFiles::new();
        f.insert(b"Data\\INI\\GameData.ini", b"GameData\n PartitionCellSize = 25\n DefaultOcclusionDelay = 2000\nEnd\n".to_vec());
        f.insert(b"Data\\INI\\Weapon.ini", b"Weapon Zap\n PrimaryDamage = 5\n AttackRange = 50\n FireOCL = OCL_Spark\nEnd\n".to_vec());
        f.insert(b"Data\\INI\\ObjectCreationList.ini", b"ObjectCreationList OCL_Spark\n FireWeapon\n  Weapon = Zap\n End\nEnd\n".to_vec());
        f.insert(
            b"Data\\INI\\Object\\Unit.ini",
            b"Object Base\n KindOf = VEHICLE\n WeaponSet\n  Conditions = None\n  Weapon = PRIMARY Zap\n End\nEnd\n".to_vec(),
        );
        // (a directory loads in its sorted list's order, upstream's `FilenameList` set: the reskin's file sorts after its parent's, or the
        // reskin finds no parent, "ObjectReskin must come after the original Object"; first written as `Reskin.ini`, which fails so)
        f.insert(b"Data\\INI\\Object\\Unit_Skins.ini", b"ObjectReskin Skin Base\n FenceWidth = 3\nEnd\n".to_vec());
        f
    }

    /// Batch I12: the logic's INI read in `GameEngine::init`'s order from upstream's paths. Predicted before the run: the
    /// `GameData` file (first) sets the partition cell size and the occlusion delay (2000 ms: 60 frames), so the objects read later
    /// take it; the weapon's `FireOCL` is the list's name key after the post-process (the list is read after the weapon); the list's
    /// `FireWeapon` nugget names the weapon store's first weapon (index 0); the reskin, in a later file of the object directory, is
    /// a second template with its parent's weapon set and its own fence width.
    #[test]
    fn the_logic_ini_loads_in_upstreams_order() {
        let mut load = load_game_data(&files(), MissingFiles::Skip, TextTable::default()).expect("the INI");
        let g = load.global_data.the_global_data().expect("the global data");
        assert_eq!((g.partition_cell_size, g.default_occlusion_delay), (25.0, 60));
        let key = load.names.name_to_key(b"OCL_Spark").0;
        assert_eq!(load.weapon_store.templates()[0].fire_ocls[0], Some(key));
        let list = load.ocls.find_object_creation_list(&mut load.names, b"OCL_Spark").expect("the list");
        match &*list.nuggets[0] {
            crate::logic::object_creation_list::Nugget::FireWeapon(n) => assert_eq!(n.weapon, Some(0)),
            n => panic!("{n:?}"),
        }
        let base = load.factory.find_template(b"Base").expect("Base").data.clone();
        let skin = load.factory.find_template(b"Skin").expect("Skin").data.clone();
        assert_ne!(base.get_template_id(), skin.get_template_id());
        assert_eq!(skin.get_weapon_template_sets(), base.get_weapon_template_sets());
        assert_eq!((base.get_fence_width(), skin.get_fence_width()), (0.0, 3.0));
    }

    /// Batch I56: `TheMultiplayerSettings` is read in the logic's load, after the science store, from `Multiplayer.ini` (it was
    /// never read: the World answered its questions from tables). Predicted: the block's beacon count and shroud flag, two
    /// colours with the alpha byte set (`0xFF` << 24 on the RGB), the default starting money, and all of it kept by
    /// [`GameDataLoad::into_statics`].
    #[test]
    fn the_multiplayer_settings_load_into_the_statics() {
        let mut f = files();
        f.insert(
            b"Data\\INI\\Multiplayer.ini",
            b"MultiplayerSettings\n MaxBeaconsPerPlayer = 5\n UseShroud = No\nEnd\n\
              MultiplayerColor Gold\n TooltipName = Color:Gold\n RGBColor = R:255 G:200 B:0\n RGBNightColor = R:128 G:100 B:0\nEnd\n\
              MultiplayerColor Red\n TooltipName = Color:Red\n RGBColor = R:255 G:0 B:0\n RGBNightColor = R:128 G:0 B:0\nEnd\n\
              MultiplayerStartingMoneyChoice\n Value = 10000\n Default = Yes\nEnd\n"
                .to_vec(),
        );
        let load = load_game_data(&f, MissingFiles::Skip, TextTable::default()).expect("the INI");
        let (st, _) = load.into_statics();
        let m = &st.multiplayer_settings;
        assert_eq!((m.get_max_beacons_per_player(), m.is_shroud_in_multiplayer(), m.get_num_colors()), (5, false, 2));
        let red = m.color_at(1).expect("the second colour");
        assert_eq!((red.color() as u32, red.night_color() as u32), (0xFFFF_0000, 0xFF80_0000));
        assert_eq!(m.get_default_starting_money().count_money(), 10000);
        assert!(m.color_at(2).is_none() && m.color_at(crate::common::multiplayer_settings::PLAYERTEMPLATE_OBSERVER).is_some());
    }

    /// Batch I12: upstream's `INI::load` throws on a file it cannot open; a client block (`FXList`) in a logic file is an unknown
    /// token to the logic's table. Predicted: `CantOpenFile` with [`MissingFiles::Error`] (the default files are absent), and
    /// `UnknownToken` for the `FXList` block.
    #[test]
    fn a_missing_file_or_a_client_block_fails_the_load() {
        let r = load_game_data(&files(), MissingFiles::Error, TextTable::default());
        assert_eq!(r.err().map(|e| e.kind), Some(IniErrorKind::CantOpenFile));
        let mut f = files();
        f.insert(b"Data\\INI\\Armor.ini", b"FXList Boom\nEnd\n".to_vec());
        let r = load_game_data(&f, MissingFiles::Skip, TextTable::default());
        assert_eq!(r.err().map(|e| e.kind), Some(IniErrorKind::UnknownToken));
    }

    /// Batch C3: a `Multiplayer.ini` with an `OnlineChatColors` block, as the game's carries one beside the logic's settings.
    /// Predicted: the load reads on past it (before, `UnknownToken`), sets the colours it names (`Default` R:255 G:255 B:255,
    /// `ChatSelf` R:0 G:255 B:0, as `parseColorInt` packs them) and refuses a field the table does not name, as upstream's
    /// `initFromINI` does.
    #[test]
    fn a_multiplayer_ini_with_online_chat_colors_loads() {
        let mut f = files();
        let block = b"OnlineChatColors\n  Default = R:255 G:255 B:255\n  ChatSelf = R:0 G:255 B:0\nEnd\n";
        f.insert(b"Data\\INI\\Multiplayer.ini", block.to_vec());
        let load = load_game_data(&f, MissingFiles::Skip, TextTable::default()).expect("the INI");
        assert_eq!(load.online_chat_colors[20], crate::common::ini::game_make_color(0, 255, 0, 255));
        assert_ne!(load.online_chat_colors[0], 0);
        let mut g = files();
        g.insert(b"Data\\INI\\Multiplayer.ini", b"OnlineChatColors\n  NoSuchColour = R:1 G:2 B:3\nEnd\n".to_vec());
        assert!(load_game_data(&g, MissingFiles::Skip, TextTable::default()).is_err());
    }
}
