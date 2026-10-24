// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Money.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Money.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Westwood Studios Pacific, Steven Johnson, October 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Money`: how much money a player has. A plain counter, with a deposit and
//! a withdrawal that each play a cash sound.
//!
//! # Cut points
//!
//! - **The cash sound** (`TheAudio->addAudioEvent` of `MiscAudio`'s money
//!   sound, when `playSound`): becomes a [`ClientEvent::MoneySound`] pushed
//!   onto the caller's queue. The sound's name is the client's business.
//! - **The academy statistics** (`ThePlayerList->getNthPlayer(m_playerIndex)`
//!   then `recordIncome()`, on a deposit): the player belongs to layer 7, so
//!   [`Money::deposit`] returns `true` where upstream calls `recordIncome`,
//!   and the owning player makes the call.
//!
//! # Integer semantics
//!
//! `m_money` is an `UnsignedInt`. Upstream's `+=` wraps (unsigned overflow is
//! defined), and so does [`Money::deposit`]. A withdrawal never exceeds the
//! balance, so it cannot wrap.

use super::ini::{Ini, IniResult, UserData};
use super::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::client_event::{ClientEvent, MoneySound};

/// `class Money`: the amount of money, and the player it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Money {
    /// `m_money`: the amount.
    money: u32,
    /// `m_playerIndex`: whose money it is (the sound's player).
    player_index: i32,
}

impl Default for Money {
    /// `Money()`: no money, player 0.
    fn default() -> Self {
        Money { money: 0, player_index: 0 }
    }
}

impl Money {
    /// `Money()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `init()`: no money (the player index is kept).
    pub fn init(&mut self) {
        self.money = 0;
    }

    /// `countMoney()`.
    pub fn count_money(&self) -> u32 {
        self.money
    }

    /// `setPlayerIndex(ndx)`.
    pub fn set_player_index(&mut self, ndx: i32) {
        self.player_index = ndx;
    }

    /// The player index (`m_playerIndex`).
    pub fn player_index(&self) -> i32 {
        self.player_index
    }

    /// `amountEqual(that)`: whether the amounts match (the player index is
    /// not compared).
    pub fn amount_equal(&self, that: &Money) -> bool {
        self.money == that.money
    }

    /// `withdraw(amountToWithdraw, playSound)`: returns the amount actually
    /// taken, which is less than asked when the balance is lower ("sorry,
    /// can't go into debt"). A non-zero withdrawal with `play_sound` pushes
    /// the withdrawal sound onto `events`.
    pub fn withdraw(&mut self, amount_to_withdraw: u32, play_sound: bool, events: &mut Vec<ClientEvent>) -> u32 {
        let amount = amount_to_withdraw.min(self.money);
        if amount == 0 {
            return amount;
        }
        if play_sound {
            events.push(ClientEvent::MoneySound { sound: MoneySound::Withdraw, player_index: self.player_index });
        }
        self.money -= amount;
        amount
    }

    /// `deposit(amountToDeposit, playSound)`. Returns `true` when the owning
    /// player must call `recordIncome()` on its academy statistics (upstream
    /// does so for every non-zero deposit, see the module docs). A non-zero
    /// deposit with `play_sound` pushes the deposit sound onto `events`.
    pub fn deposit(&mut self, amount_to_deposit: u32, play_sound: bool, events: &mut Vec<ClientEvent>) -> bool {
        if amount_to_deposit == 0 {
            return false;
        }
        if play_sound {
            events.push(ClientEvent::MoneySound { sound: MoneySound::Deposit, player_index: self.player_index });
        }
        self.money = self.money.wrapping_add(amount_to_deposit);
        true
    }
}

/// `Money::parseMoneyAmount`: `DefaultStartingMoney = 10000`, an unsigned
/// integer into the amount.
pub fn parse_money_amount(ini: &mut Ini<'_>, m: &mut Money, _ud: &UserData) -> IniResult<()> {
    m.money = ini.parse_unsigned_int()?;
    Ok(())
}

impl Snapshot for Money {
    /// `crc`: empty upstream.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer`, version 1: the amount (the player index is not saved).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_unsigned_int(&mut self.money)
    }

    /// `loadPostProcess`: empty upstream.
    fn load_post_process(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn withdraw_is_clamped_to_the_balance_and_plays_a_sound() {
        let mut m = Money::new();
        m.set_player_index(2);
        let mut ev = Vec::new();
        assert!(m.deposit(5, false, &mut ev));
        assert!(ev.is_empty(), "no sound when playSound is off");
        assert_eq!(m.withdraw(9, true, &mut ev), 5);
        assert_eq!(m.count_money(), 0);
        assert_eq!(ev, vec![ClientEvent::MoneySound { sound: MoneySound::Withdraw, player_index: 2 }]);
        // nothing to take: no sound, nothing returned
        assert_eq!(m.withdraw(1, true, &mut ev), 0);
        assert_eq!(ev.len(), 1);
    }

    #[test]
    fn zero_deposit_does_nothing_and_records_no_income() {
        let mut m = Money::new();
        let mut ev = Vec::new();
        assert!(!m.deposit(0, true, &mut ev));
        assert_eq!(m.count_money(), 0);
        assert!(ev.is_empty());
        assert!(m.deposit(3, true, &mut ev));
        assert_eq!(ev, vec![ClientEvent::MoneySound { sound: MoneySound::Deposit, player_index: 0 }]);
    }

    #[test]
    fn init_clears_the_amount_and_keeps_the_player() {
        let mut m = Money::new();
        m.set_player_index(4);
        m.deposit(100, false, &mut Vec::new());
        m.init();
        assert_eq!(m.count_money(), 0);
        assert_eq!(m.player_index(), 4);
    }

    #[test]
    fn amount_equal_ignores_the_player() {
        let mut a = Money::default();
        let mut b = Money::new();
        a.set_player_index(1);
        b.set_player_index(7);
        assert!(a.amount_equal(&b));
        a.deposit(1, false, &mut Vec::new());
        assert!(!a.amount_equal(&b));
    }
}
