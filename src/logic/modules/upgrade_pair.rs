// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AutoDepositUpdate.h   (`struct upgradePair`)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AutoDepositUpdate.cpp   (`parseUpgradePair`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `upgradePair` and `parseUpgradePair`, the `UpgradedBoost = UpgradeType:<name> Boost:<amount>` field `AutoDepositUpdate` and
//! `MoneyCrateCollide` share (the function lives in `AutoDepositUpdate.cpp`, the struct in its header).
//!
//! Upstream reads the word after the type with `stricmp(token, "Boost")` without testing the token for `NULL`: a line that ends after the
//! upgrade name is a `NULL` dereference ([`Undefined::NullTokenCompared`]).
//!
//! `p1w11-sonnet`, 2026-10-21.

use crate::common::ini::{Ini, IniError, IniErrorKind, IniResult, Undefined, SEPS_COLON};

/// `struct upgradePair`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradePair {
    /// `type`: the upgrade's name.
    pub type_name: Vec<u8>,
    /// `amount`.
    pub amount: i32,
}

/// `parseUpgradePair(ini, instance, store, userData)`: reads one pair and appends it to `store` (`std::list<upgradePair>`).
pub fn parse_upgrade_pair(ini: &mut Ini<'_>, store: &mut Vec<UpgradePair>) -> IniResult<()> {
    let mut info = UpgradePair {
        type_name: Vec::new(),
        amount: 0,
    };
    let token = ini.get_next_token(Some(SEPS_COLON))?;
    if token.eq_ignore_ascii_case(b"UpgradeType") {
        let token = ini
            .get_next_token_or_null(Some(SEPS_COLON))
            .ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        info.type_name = token;
    } else {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    let token = ini
        .get_next_token_or_null(Some(SEPS_COLON))
        .ok_or_else(|| IniError::undefined(Undefined::NullTokenCompared))?;
    if token.eq_ignore_ascii_case(b"Boost") {
        info.amount = Ini::scan_int(&ini.get_next_token(Some(SEPS_COLON))?)?;
        if crate::logic::modules::plant(774) {
            info.amount = info.amount.wrapping_abs();
        }
    } else {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    // insert the info into the upgrade list
    store.push(info);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};

    fn read(text: &[u8]) -> (IniResult<()>, Vec<UpgradePair>) {
        fn block(ini: &mut Ini<'_>, out: &mut Vec<UpgradePair>) -> IniResult<()> {
            parse_upgrade_pair(ini, out)
        }
        static BLOCKS: [BlockParse<Vec<UpgradePair>>; 1] = [BlockParse {
            token: "P",
            parse: block,
        }];
        let mut out = Vec::new();
        let r = Ini::load(
            b"p.ini",
            text,
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut out,
        );
        (r, out)
    }

    #[test]
    fn a_pair_is_read_and_appended() {
        let (r, out) = read(b"P UpgradeType:Up_A Boost:25\nP UpgradeType = Up_B Boost = -3\n");
        assert!(r.is_ok());
        assert_eq!(
            out,
            vec![
                UpgradePair {
                    type_name: b"Up_A".to_vec(),
                    amount: 25
                },
                UpgradePair {
                    type_name: b"Up_B".to_vec(),
                    amount: -3
                }
            ]
        );
    }

    #[test]
    fn a_wrong_word_is_invalid_data_and_a_missing_one_is_undefined() {
        assert_eq!(
            read(b"P Type:Up_A Boost:1\n").0.unwrap_err().kind,
            IniErrorKind::InvalidData
        );
        assert_eq!(
            read(b"P UpgradeType:Up_A Bonus:1\n").0.unwrap_err().kind,
            IniErrorKind::InvalidData
        );
        assert_eq!(
            read(b"P UpgradeType\n").0.unwrap_err().kind,
            IniErrorKind::InvalidData
        );
        assert!(read(b"P UpgradeType:Up_A\n").0.unwrap_err().is_undefined());
    }
}
