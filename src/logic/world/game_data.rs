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
//! `DamageFX`, `Armor`, `Object`, `ObjectReskin`, `Upgrade`, `AIData` and `CrateData`. **The client's subsystems are not
//! loaded** (the FX lists, particle systems, audio, the UI, the terrain types and roads, the multiplayer settings, the
//! command sets and buttons, the map cache): a block of theirs in a file the logic loads is an unknown token, as it would
//! be upstream for a token missing from the table.
//!
//! **Missing files:** upstream's `INI::load` throws on a file it cannot open (the shipped data has every one). With
//! [`MissingFiles::Error`] so does the loader; [`MissingFiles::Skip`] skips one, for a test's or a mod's partial data.
//!
//! `p1w13-sonnet`, integration batch I12, 2026-11-02.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::common::damage_fx::DamageFxStore;
use crate::common::global_data::GlobalDataStore;
use crate::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult};
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

/// The game text, by label (`TheGameText->fetch(label)`); a label it does not hold fetches the empty string.
#[derive(Clone, Debug, Default)]
pub struct TextTable {
    /// The strings by label.
    pub strings: BTreeMap<Vec<u8>, UnicodeString>,
}

impl GameTextFetch for TextTable {
    fn fetch(&self, label: &[u8]) -> UnicodeString {
        self.strings.get(label).cloned().unwrap_or_else(UnicodeString::new)
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
}

impl GameDataLoad {
    /// The stores empty, after each subsystem's `init()`, with `text` as the game text.
    pub fn new(text: TextTable) -> IniResult<Self> {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut modules = ModuleFactory::new();
        modules.init(&mut names).map_err(|_| IniError::new(IniErrorKind::Bug))?;
        Ok(GameDataLoad {
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
        })
    }

    /// `INI::load(path, INI_LOAD_OVERWRITE, NULL)` of one file's text.
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, &BLOCKS, self)
    }

    /// `postProcessLoadAll()` for the logic's subsystems, in their order: the weapon store's, then the thing factory's.
    pub fn post_process_load(&mut self) {
        weapon_store_post_process(&mut self.weapon_store, &self.factory, &self.ocls, &mut self.names);
        self.factory.post_process_load();
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
    load.post_process_load();
    Ok(load)
}

// -------------------------------------------------------------------------------------------------------------------------------
// the blocks (`theTypeTable`'s logic entries)
// -------------------------------------------------------------------------------------------------------------------------------

static BLOCKS: [BlockParse<GameDataLoad>; 15] = [
    BlockParse { token: "AIData", parse: ai_data_block },
    BlockParse { token: "Armor", parse: armor_block },
    BlockParse { token: "CrateData", parse: crate_block },
    BlockParse { token: "DamageFX", parse: damage_fx_block },
    BlockParse { token: "GameData", parse: game_data_block },
    BlockParse { token: "Locomotor", parse: locomotor_block },
    BlockParse { token: "Object", parse: object_block },
    BlockParse { token: "ObjectCreationList", parse: ocl_block },
    BlockParse { token: "ObjectReskin", parse: object_reskin_block },
    BlockParse { token: "PlayerTemplate", parse: player_template_block },
    BlockParse { token: "Science", parse: science_block },
    BlockParse { token: "Rank", parse: rank_block },
    BlockParse { token: "SpecialPower", parse: special_power_block },
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
}
