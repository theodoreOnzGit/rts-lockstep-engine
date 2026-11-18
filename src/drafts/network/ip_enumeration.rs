//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/IPEnumeration.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/IPEnumeration.cpp
//!
//! Lists the local machine's IPv4 addresses. `gethostname`/`gethostbyname` (and Winsock start-up) are the
//! [`LocalAddressSource`]'s, so the browser build can answer with its own idea of the local addresses (usually
//! none). `EnumeratedIP`'s `m_next` chain is the `Vec` order (ascending by address).

use crate::common::ascii_string::AsciiString;

/// `gethostname` and `gethostbyname`.
pub trait LocalAddressSource {
    /// `gethostname`: the machine's name; None on failure.
    fn host_name(&mut self) -> Option<Vec<u8>>;
    /// `gethostbyname(name)->h_addr_list`: the machine's IPv4 addresses, four bytes each in network order; None
    /// on failure.
    fn host_addresses(&mut self, name: &[u8]) -> Option<Vec<[u8; 4]>>;
}

/// `class EnumeratedIP`.
#[derive(Clone, Debug, Default)]
pub struct EnumeratedIP {
    /// `m_IPstring`: "a.b.c.d".
    ip_string: AsciiString,
    /// `m_IP`: host byte order.
    ip: u32,
}

impl EnumeratedIP {
    /// `EnumeratedIP::EnumeratedIP()`.
    pub fn new() -> EnumeratedIP {
        EnumeratedIP::default()
    }
    /// `getIPstring`.
    pub fn get_ip_string(&self) -> &AsciiString {
        &self.ip_string
    }
    /// `setIPstring`.
    pub fn set_ip_string(&mut self, name: AsciiString) {
        self.ip_string = name;
    }
    /// `getIP`.
    pub fn get_ip(&self) -> u32 {
        self.ip
    }
    /// `setIP`.
    pub fn set_ip(&mut self, ip: u32) {
        self.ip = ip;
    }
}

/// `class IPEnumeration`.
#[derive(Debug, Default)]
pub struct IPEnumeration {
    /// `m_IPlist`, ascending by address.
    ip_list: Vec<EnumeratedIP>,
}

impl IPEnumeration {
    /// `IPEnumeration::IPEnumeration()`.
    pub fn new() -> IPEnumeration {
        IPEnumeration::default()
    }

    /// `IPEnumeration::getAddresses` (IPEnumeration.cpp): the local addresses, looked up once and kept (an empty
    /// list is looked up again next time). Each address enters the list in ascending order: before the head if
    /// smaller than it, else after the head and after every following smaller address.
    pub fn get_addresses<S: LocalAddressSource>(&mut self, src: &mut S) -> &[EnumeratedIP] {
        if !self.ip_list.is_empty() {
            return &self.ip_list;
        }
        let hostname = match src.host_name() {
            Some(h) => h,
            None => return &self.ip_list,
        };
        let addrs = match src.host_addresses(&hostname) {
            Some(a) => a,
            None => return &self.ip_list,
        };
        for entry in addrs {
            let mut new_ip = EnumeratedIP::new();
            let s = format!("{}.{}.{}.{}", entry[0], entry[1], entry[2], entry[3]);
            let mut str_ = AsciiString::default();
            let _ = str_.set(s.as_bytes());
            let ip = u32::from_be_bytes(entry); // ntohl of the in-memory bytes
            new_ip.set_ip_string(str_);
            new_ip.set_ip(ip);
            if self.ip_list.is_empty() {
                self.ip_list.push(new_ip);
            } else if new_ip.get_ip() < self.ip_list[0].get_ip() {
                self.ip_list.insert(0, new_ip);
            } else {
                let mut p = 0usize;
                while p + 1 < self.ip_list.len() && self.ip_list[p + 1].get_ip() < new_ip.get_ip() {
                    p += 1;
                }
                self.ip_list.insert(p + 1, new_ip);
            }
        }
        &self.ip_list
    }

    /// `IPEnumeration::getMachineName` (IPEnumeration.cpp): the machine's name (empty on failure).
    pub fn get_machine_name<S: LocalAddressSource>(&mut self, src: &mut S) -> AsciiString {
        match src.host_name() {
            Some(h) => AsciiString::try_from_bytes(&h).unwrap_or_default(),
            None => AsciiString::default(),
        }
    }
}
