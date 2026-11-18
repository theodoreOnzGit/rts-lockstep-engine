//! UNTESTED DRAFT (#202, #208): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/Common/registry.h
//!   GeneralsMD/Code/GameEngine/Source/Common/System/registry.cpp
//!
//! "Simple interface for storing/retrieving registry values." The Windows registry itself (`RegOpenKeyEx`, `RegQueryValueEx`,
//! `RegCreateKeyEx`, `RegSetValueEx`, `RegCloseKey`) is the host's: [`RegistryHost`] is the five calls' outcome, so the
//! logic upstream wraps (the two key prefixes, the local-machine-then-current-user order, the defaults and the language cache) is here.

/// `HKEY_LOCAL_MACHINE` / `HKEY_CURRENT_USER`: the two roots registry.cpp reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryRoot {
    /// `HKEY_LOCAL_MACHINE`.
    LocalMachine,
    /// `HKEY_CURRENT_USER`.
    CurrentUser,
}

/// What `RegOpenKeyEx` + `RegQueryValueEx` / `RegCreateKeyEx` + `RegSetValueEx` answer (upstream: `ERROR_SUCCESS` or not).
pub trait RegistryHost {
    /// `getStringFromRegistry` body: the value as a REG_SZ read into a 256-byte buffer, `None` for any error.
    fn query_string(&mut self, root: RegistryRoot, path: &[u8], key: &[u8]) -> Option<Vec<u8>>;
    /// `getUnsignedIntFromRegistry` body: four bytes read as a `UnsignedInt`, `None` for any error.
    fn query_u32(&mut self, root: RegistryRoot, path: &[u8], key: &[u8]) -> Option<u32>;
    /// `setStringInRegistry` body: `RegCreateKeyEx` then `RegSetValueEx` (REG_SZ, size `getLength()+1`); true for `ERROR_SUCCESS`.
    fn set_string(&mut self, root: RegistryRoot, path: &[u8], key: &[u8], val: &[u8]) -> bool;
    /// `setUnsignedIntInRegistry` body (REG_DWORD, size 4).
    fn set_u32(&mut self, root: RegistryRoot, path: &[u8], key: &[u8], val: u32) -> bool;
}

/// `"SOFTWARE\\Electronic Arts\\EA Games\\Generals"`.
const GENERALS_KEY: &[u8] = b"SOFTWARE\\Electronic Arts\\EA Games\\Generals";
/// `"SOFTWARE\\Electronic Arts\\EA Games\\Command and Conquer Generals Zero Hour"`.
const ZERO_HOUR_KEY: &[u8] = b"SOFTWARE\\Electronic Arts\\EA Games\\Command and Conquer Generals Zero Hour";

/// `getStringFromRegistry(root, path, key, val)`: `val` is set only on success.
pub fn get_string_from_registry<H: RegistryHost>(host: &mut H, root: RegistryRoot, path: &[u8], key: &[u8], val: &mut Vec<u8>) -> bool {
    match host.query_string(root, path, key) {
        Some(v) => {
            *val = v;
            true
        }
        None => false,
    }
}

/// `getUnsignedIntFromRegistry(root, path, key, val)`.
pub fn get_unsigned_int_from_registry<H: RegistryHost>(host: &mut H, root: RegistryRoot, path: &[u8], key: &[u8], val: &mut u32) -> bool {
    match host.query_u32(root, path, key) {
        Some(v) => {
            *val = v;
            true
        }
        None => false,
    }
}

/// `setStringInRegistry(root, path, key, val)`.
pub fn set_string_in_registry<H: RegistryHost>(host: &mut H, root: RegistryRoot, path: &[u8], key: &[u8], val: &[u8]) -> bool {
    host.set_string(root, path, key, val)
}

/// `setUnsignedIntInRegistry(root, path, key, val)`.
pub fn set_unsigned_int_in_registry<H: RegistryHost>(host: &mut H, root: RegistryRoot, path: &[u8], key: &[u8], val: u32) -> bool {
    host.set_u32(root, path, key, val)
}

fn full_path(base: &[u8], path: &[u8]) -> Vec<u8> {
    let mut p = base.to_vec();
    p.extend_from_slice(path);
    p
}

/// `GetStringFromGeneralsRegistry(path, key, val)`: the base game's key, local machine first, then current user.
pub fn get_string_from_generals_registry<H: RegistryHost>(host: &mut H, path: &[u8], key: &[u8], val: &mut Vec<u8>) -> bool {
    let full = full_path(GENERALS_KEY, path);
    // DEBUG_LOG(("GetStringFromRegistry - looking in %s for key %s\n", ...));
    if get_string_from_registry(host, RegistryRoot::LocalMachine, &full, key, val) {
        return true;
    }
    get_string_from_registry(host, RegistryRoot::CurrentUser, &full, key, val)
}

/// `GetStringFromRegistry(path, key, val)`: Zero Hour's key, local machine first, then current user.
pub fn get_string_from_zero_hour_registry<H: RegistryHost>(host: &mut H, path: &[u8], key: &[u8], val: &mut Vec<u8>) -> bool {
    let full = full_path(ZERO_HOUR_KEY, path);
    if get_string_from_registry(host, RegistryRoot::LocalMachine, &full, key, val) {
        return true;
    }
    get_string_from_registry(host, RegistryRoot::CurrentUser, &full, key, val)
}

/// `GetUnsignedIntFromRegistry(path, key, val)`.
pub fn get_unsigned_int_from_zero_hour_registry<H: RegistryHost>(host: &mut H, path: &[u8], key: &[u8], val: &mut u32) -> bool {
    let full = full_path(ZERO_HOUR_KEY, path);
    if get_unsigned_int_from_registry(host, RegistryRoot::LocalMachine, &full, key, val) {
        return true;
    }
    get_unsigned_int_from_registry(host, RegistryRoot::CurrentUser, &full, key, val)
}

/// The `static` cache of `GetRegistryLanguage`: `cached` and `val = "english"`.
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryLanguageCache {
    /// `static Bool cached = FALSE`.
    pub cached: bool,
    /// `static AsciiString val = "english"`.
    pub val: Vec<u8>,
}

impl RegistryLanguageCache {
    /// The statics' initial values.
    pub fn new() -> RegistryLanguageCache {
        RegistryLanguageCache { cached: false, val: b"english".to_vec() }
    }

    /// `GetRegistryLanguage()`: the cached value, else (the first call) the registry's `Language`, left at "english" if absent.
    pub fn get_registry_language<H: RegistryHost>(&mut self, host: &mut H) -> Vec<u8> {
        if self.cached {
            return self.val.clone();
        }
        self.cached = true;
        let mut v = self.val.clone();
        get_string_from_zero_hour_registry(host, b"", b"Language", &mut v);
        self.val = v;
        self.val.clone()
    }
}

impl Default for RegistryLanguageCache {
    fn default() -> Self {
        RegistryLanguageCache::new()
    }
}

/// `GetRegistryGameName()`: the `SKU` value, default `"GeneralsMPTest"`.
pub fn get_registry_game_name<H: RegistryHost>(host: &mut H) -> Vec<u8> {
    let mut val = b"GeneralsMPTest".to_vec();
    get_string_from_zero_hour_registry(host, b"", b"SKU", &mut val);
    val
}

/// `GetRegistryVersion()`: the `Version` value, default 65536.
pub fn get_registry_version<H: RegistryHost>(host: &mut H) -> u32 {
    let mut val: u32 = 65536;
    get_unsigned_int_from_zero_hour_registry(host, b"", b"Version", &mut val);
    val
}

/// `GetRegistryMapPackVersion()`: the `MapPackVersion` value, default 65536.
pub fn get_registry_map_pack_version<H: RegistryHost>(host: &mut H) -> u32 {
    let mut val: u32 = 65536;
    get_unsigned_int_from_zero_hour_registry(host, b"", b"MapPackVersion", &mut val);
    val
}
