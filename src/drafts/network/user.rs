//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/User.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/User.cpp

use crate::common::unicode_string::UnicodeString;

/// `class User`: a player as the network layer knows it (name, address, port).
#[derive(Clone, Debug, Default)]
pub struct User {
    /// `m_name`.
    name: UnicodeString,
    /// `m_port`.
    port: u16,
    /// `m_ipaddr`.
    ipaddr: u32,
}

impl User {
    /// `User::User()`: upstream leaves `m_port` and `m_ipaddr` uninitialised; 0 here.
    pub fn new() -> User {
        User::default()
    }

    /// `User::User(UnicodeString name, UnsignedInt addr, UnsignedInt port)` (User.cpp:30): the port is truncated
    /// to an `UnsignedShort`.
    pub fn with_address(name: UnicodeString, addr: u32, port: u32) -> User {
        let mut m_name = UnicodeString::new();
        m_name.set_unicode(&name);
        User { name: m_name, ipaddr: addr, port: port as u16 }
    }

    /// `User::operator=(const User *other)` (User.cpp:40).
    pub fn assign(&mut self, other: &User) {
        self.name = other.name.clone();
        self.ipaddr = other.ipaddr;
        self.port = other.port;
    }

    /// `User::operator==` (User.cpp:50): equal names (the address and port are not compared).
    pub fn is_equal(&self, other: &User) -> bool {
        self.name.compare_unicode(&other.name) == std::cmp::Ordering::Equal
    }

    /// `User::operator!=` (User.cpp:58).
    pub fn is_not_equal(&self, other: &User) -> bool {
        self.name.compare_unicode(&other.name) != std::cmp::Ordering::Equal
    }

    /// `GetName`.
    pub fn get_name(&self) -> UnicodeString {
        self.name.clone()
    }

    /// `User::setName` (User.cpp:66).
    pub fn set_name(&mut self, name: UnicodeString) {
        self.name.set_unicode(&name);
    }

    /// `GetPort`.
    pub fn get_port(&self) -> u16 {
        self.port
    }

    /// `GetIPAddr`.
    pub fn get_ip_addr(&self) -> u32 {
        self.ipaddr
    }

    /// `SetPort`.
    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    /// `SetIPAddr`.
    pub fn set_ip_addr(&mut self, ipaddr: u32) {
        self.ipaddr = ipaddr;
    }
}
