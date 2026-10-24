// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/RankInfo.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/RankInfo.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, Sep 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RankInfo` (one general's rank: its name, the skill points it needs, the
//! science purchase points and the sciences it grants) and `RankInfoStore`
//! (`TheRankInfoStore`), with the `Rank` INI block.
//!
//! # Translation choices
//!
//! - `RankInfoStore::m_rankInfos` is a `vector<RankInfo*>`, index = level - 1;
//!   a later INI file hangs an override off a rank (it cannot add one).
//!   Each entry is a `Box<Overridable<RankInfo>>`.
//! - `RankInfo::m_rankName` is a `UnicodeString` filled by
//!   `parseAndTranslateLabel`; as in [`crate::common::science`], the block
//!   handler takes a [`GameTextFetch`].
//! - `SciencesGranted` goes through `INI::parseScienceVector`, so the handler
//!   takes the [`ScienceStore`] to validate the names against, and the
//!   [`NameKeyGenerator`] (an unknown name still gets a key).
//! - `RankInfo` has no constructor upstream: `newInstance` returns zeroed
//!   memory (the pool allocators zero-fill), so the numbers start at 0 and
//!   the name and sciences start empty.
//! - `SciencePurchasePointsGranted` is read with `parseUnsignedInt` into an
//!   `Int`: 4294967295 is stored as -1.
//! - A block that fails leaves what it did: a new rank is pushed only after
//!   its fields were read (a failing block adds nothing), but an override is
//!   linked before they are (a failing block leaves a half-filled override).
//!
//! # Left out: methods that take a `Player`
//!
//! None of `RankInfo.h`/`RankInfo.cpp` takes one (`class Player;` is only a
//! forward declaration); the `Player` layer reads ranks through
//! [`RankInfoStore::get_rank_info`].

use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, UserData,
};
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::overridable::Overridable;
use crate::common::science::{
    parse_and_translate_label, parse_science_vector, GameTextFetch, ScienceStore, ScienceVec,
};
use crate::common::unicode_string::UnicodeString;

/// `class RankInfo`.
#[derive(Clone, Debug, Default)]
pub struct RankInfo {
    /// `m_rankName`: the translated `RankName`.
    pub rank_name: UnicodeString,
    /// `m_skillPointsNeeded`.
    pub skill_points_needed: i32,
    /// `m_sciencePurchasePointsGranted`.
    pub science_purchase_points_granted: i32,
    /// `m_sciencesGranted`.
    pub sciences_granted: ScienceVec,
}

/// What the fields of a `Rank` block fill.
struct RankFill<'a> {
    info: &'a mut RankInfo,
    names: &'a mut NameKeyGenerator,
    sciences: &'a ScienceStore,
    text: &'a dyn GameTextFetch,
}

fn f_rank_name(ini: &mut Ini<'_>, w: &mut RankFill<'_>, _: &UserData) -> IniResult<()> {
    w.info.rank_name = parse_and_translate_label(ini, w.text)?;
    Ok(())
}
fn f_skill_points_needed(ini: &mut Ini<'_>, w: &mut RankFill<'_>, _: &UserData) -> IniResult<()> {
    w.info.skill_points_needed = ini.parse_int()?;
    Ok(())
}
fn f_sciences_granted(ini: &mut Ini<'_>, w: &mut RankFill<'_>, _: &UserData) -> IniResult<()> {
    let sciences = w.sciences;
    parse_science_vector(
        ini,
        w.names,
        |s| sciences.is_valid_science(s),
        &mut w.info.sciences_granted,
    )
}
fn f_science_purchase_points_granted(
    ini: &mut Ini<'_>,
    w: &mut RankFill<'_>,
    _: &UserData,
) -> IniResult<()> {
    w.info.science_purchase_points_granted = ini.parse_unsigned_int()? as i32;
    Ok(())
}

/// `friend_parseRankDefinition`'s `myFieldParse`.
fn rank_fields<'a>() -> [FieldParse<RankFill<'a>>; 5] {
    [
        FieldParse::new("RankName", f_rank_name, UserData::None),
        FieldParse::new("SkillPointsNeeded", f_skill_points_needed, UserData::None),
        FieldParse::new("SciencesGranted", f_sciences_granted, UserData::None),
        FieldParse::new(
            "SciencePurchasePointsGranted",
            f_science_purchase_points_granted,
            UserData::None,
        ),
        FieldParse::end(),
    ]
}

/// `class RankInfoStore` (`TheRankInfoStore`).
#[derive(Debug, Default)]
pub struct RankInfoStore {
    /// `m_rankInfos`: level - 1 is the index.
    rank_infos: Vec<Overridable<RankInfo>>,
}

impl RankInfoStore {
    /// `RankInfoStore()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `init()`.
    pub fn init(&mut self) {
        self.rank_infos.clear();
    }

    /// `reset()`: delete the overrides of `INI_LOAD_CREATE_OVERRIDES`.
    pub fn reset(&mut self) {
        let old = std::mem::take(&mut self.rank_infos);
        for head in old {
            if let Some(kept) = Box::new(head).delete_overrides() {
                self.rank_infos.push(*kept);
            }
        }
    }

    /// `getRankLevelCount()`.
    pub fn get_rank_level_count(&self) -> i32 {
        self.rank_infos.len() as i32
    }

    /// `getRankInfo(level)`: the final override of rank `level`, which is
    /// **1..=n, not 0..=n-1**; `None` outside that range.
    pub fn get_rank_info(&self, level: i32) -> Option<&RankInfo> {
        if level >= 1 && level <= self.get_rank_level_count() {
            return Some(
                &self.rank_infos[(level - 1) as usize]
                    .get_final_override()
                    .data,
            );
        }
        None
    }

    /// Every rank's chain, level 1 first.
    pub fn chains(&self) -> impl Iterator<Item = &Overridable<RankInfo>> {
        self.rank_infos.iter()
    }

    /// `friend_parseRankDefinition(ini)` (the `Rank` block, reached through
    /// `INI::parseRankDefinition`): the level, then the fields until `End`.
    ///
    /// - `INI_LOAD_CREATE_OVERRIDES`: "we aren't allowed to add ranks in
    ///   overrides, only to override existing ones": a level outside
    ///   `1..=n` is `INI_INVALID_DATA`.
    /// - Any other load: "ranks must increase monotonically": a level other
    ///   than n + 1 is `INI_INVALID_DATA`.
    pub fn parse_rank_definition(
        &mut self,
        ini: &mut Ini<'_>,
        names: &mut NameKeyGenerator,
        sciences: &ScienceStore,
        text: &dyn GameTextFetch,
    ) -> IniResult<()> {
        let rank = Ini::scan_int(&ini.get_next_token(None)?)?;
        let invalid = || IniError::new(IniErrorKind::InvalidData);

        if ini.get_load_type() == IniLoadType::CreateOverrides {
            if rank < 1 || i64::from(rank) > self.rank_infos.len() as i64 {
                return Err(invalid());
            }
            let fin = self.rank_infos[(rank - 1) as usize].friend_get_final_override();
            let mut new_info = Overridable::new(fin.data.clone());
            new_info.mark_as_override();
            let _ = fin.set_next_override(Some(Box::new(new_info)));
            let target = &mut self.rank_infos[(rank - 1) as usize]
                .friend_get_final_override()
                .data;
            let mut fill = RankFill {
                info: target,
                names,
                sciences,
                text,
            };
            ini.init_from_ini(&mut fill, &rank_fields())
        } else {
            if i64::from(rank) != self.rank_infos.len() as i64 + 1 {
                return Err(invalid());
            }
            let mut info = RankInfo::default();
            {
                let mut fill = RankFill {
                    info: &mut info,
                    names,
                    sciences,
                    text,
                };
                ini.init_from_ini(&mut fill, &rank_fields())?;
            }
            self.rank_infos.push(Overridable::new(info));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::BlockParse;
    use crate::common::science::SCIENCE_INVALID;

    struct Text;
    impl GameTextFetch for Text {
        fn fetch(&self, label: &[u8]) -> UnicodeString {
            let u: Vec<u16> = label.iter().map(|&b| u16::from(b)).collect();
            UnicodeString::try_from_chars(&u).unwrap()
        }
    }

    struct Ctx {
        sciences: ScienceStore,
        ranks: RankInfoStore,
        names: NameKeyGenerator,
    }
    fn sci(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.sciences
            .parse_science_definition(ini, &mut c.names, &Text)
    }
    fn rank(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.ranks
            .parse_rank_definition(ini, &mut c.names, &c.sciences, &Text)
    }
    static BLOCKS: [BlockParse<Ctx>; 2] = [
        BlockParse {
            token: "Science",
            parse: sci,
        },
        BlockParse {
            token: "Rank",
            parse: rank,
        },
    ];

    fn load(c: &mut Ctx, text: &[u8], t: IniLoadType) -> IniResult<()> {
        Ini::load(b"r.ini", text, t, None, &BLOCKS, c)
    }

    #[test]
    fn ranks_are_monotonic_and_overridable() {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut c = Ctx {
            sciences: ScienceStore::new(),
            ranks: RankInfoStore::new(),
            names,
        };
        let txt = b"Science S\nEnd\nRank 1\n RankName = One\n SkillPointsNeeded = 0\n SciencesGranted = S\n SciencePurchasePointsGranted = 1\nEnd\nRank 2\n SkillPointsNeeded = 5\nEnd\n";
        load(&mut c, txt, IniLoadType::Overwrite).unwrap();
        assert_eq!(c.ranks.get_rank_level_count(), 2);
        let r1 = c.ranks.get_rank_info(1).unwrap();
        assert_eq!(
            (
                r1.skill_points_needed,
                r1.science_purchase_points_granted,
                r1.sciences_granted.len()
            ),
            (0, 1, 1)
        );
        assert!(c.ranks.get_rank_info(0).is_none() && c.ranks.get_rank_info(3).is_none());
        // out of order, and a failing block adds nothing
        assert!(load(&mut c, b"Rank 5\nEnd\n", IniLoadType::Overwrite).is_err());
        assert!(load(
            &mut c,
            b"Rank 3\n SciencesGranted = Nope\nEnd\n",
            IniLoadType::Overwrite
        )
        .is_err());
        assert_eq!(c.ranks.get_rank_level_count(), 2);
        // overrides only for existing levels, copied from the final override
        assert!(load(&mut c, b"Rank 3\nEnd\n", IniLoadType::CreateOverrides).is_err());
        load(
            &mut c,
            b"Rank 2\n SkillPointsNeeded = 50\n SciencePurchasePointsGranted = 4294967295\nEnd\n",
            IniLoadType::CreateOverrides,
        )
        .unwrap();
        let r2 = c.ranks.get_rank_info(2).unwrap();
        assert_eq!(
            (r2.skill_points_needed, r2.science_purchase_points_granted),
            (50, -1)
        );
        assert_eq!(
            c.ranks
                .chains()
                .filter(|h| h.get_next_override().is_some())
                .count(),
            1
        );
        c.ranks.reset();
        assert_eq!(c.ranks.get_rank_info(2).unwrap().skill_points_needed, 5);
        assert!(c.sciences.find_science_info(SCIENCE_INVALID).is_none());
        c.ranks.init();
        assert_eq!(c.ranks.get_rank_level_count(), 0);
    }
}
