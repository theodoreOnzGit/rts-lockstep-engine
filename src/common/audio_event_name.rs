// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp (lines 1170-1182:
//   `INI::parseAudioEventRTS`, the token handling only)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The name of an `AudioEventRTS`, the only part of one that the templates
//! (`UpgradeTemplate`, `SpecialPowerTemplate`) set from INI.
//!
//! # Cut point: sounds are names
//!
//! `INI::parseAudioEventRTS` reads one token, calls
//! `AudioEventRTS::setEventName` on it unless it is `NoSound` (any case), and
//! then asks `TheAudio->getInfoForAudioEvent` to fill in the event's
//! description. The client owns the audio system, so a parsed sound is the
//! **name as written** and nothing here resolves it. `NoSound` leaves the name
//! as it was (the default empty name, or what an earlier line set), as
//! upstream leaves the event.

use super::ini::{Ini, IniResult};

/// The event name of an `AudioEventRTS` (empty until set).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioEventName {
    /// `m_eventName`.
    pub event_name: Vec<u8>,
}

impl AudioEventName {
    /// `INI::parseAudioEventRTS`: the next token is the event name, except
    /// `NoSound`, which changes nothing. `INI_INVALID_DATA` if there is no
    /// token.
    pub fn parse(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        let token = ini.get_next_token(None)?;
        if !token.eq_ignore_ascii_case(b"NoSound") {
            self.event_name = token;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, FieldParse, IniErrorKind, IniLoadType, UserData};

    fn field(ini: &mut Ini<'_>, a: &mut AudioEventName, _: &UserData) -> IniResult<()> {
        a.parse(ini)
    }
    static FIELDS: [FieldParse<AudioEventName>; 2] = [
        FieldParse::new("Sound", field, UserData::None),
        FieldParse::end(),
    ];
    fn block(ini: &mut Ini<'_>, a: &mut AudioEventName) -> IniResult<()> {
        ini.get_next_token(None)?;
        ini.init_from_ini(a, &FIELDS)
    }
    static BLOCKS: [BlockParse<AudioEventName>; 1] = [BlockParse {
        token: "Event",
        parse: block,
    }];

    fn load(text: &[u8], start: &[u8]) -> (AudioEventName, IniResult<()>) {
        let mut a = AudioEventName {
            event_name: start.to_vec(),
        };
        let r = Ini::load(
            b"a.ini",
            text,
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut a,
        );
        (a, r)
    }

    #[test]
    fn name_is_kept_and_nosound_changes_nothing() {
        assert_eq!(
            load(b"Event E\n Sound = Boom\nEnd\n", b"").0.event_name,
            b"Boom"
        );
        assert_eq!(
            load(b"Event E\n Sound = nosound\nEnd\n", b"Old")
                .0
                .event_name,
            b"Old"
        );
        assert_eq!(
            load(b"Event E\n Sound = Boom\n Sound = NOSOUND\nEnd\n", b"")
                .0
                .event_name,
            b"Boom"
        );
        // a missing token throws
        assert_eq!(
            load(b"Event E\n Sound =\nEnd\n", b"").1.unwrap_err().kind,
            IniErrorKind::InvalidData
        );
    }
}
