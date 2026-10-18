// SPDX-License-Identifier: GPL-3.0-only
//! The module-interface headers of `GameEngine/Include/GameLogic/Module`
//! (with their `.cpp` files in `Source/GameLogic/Object`): the base classes
//! of the modules an `Object` is made of. `common::module` holds the
//! [`Module`](crate::common::module::Module) enum that aggregates them and
//! `common::module_factory` the registry; concrete modules (layer 9) become
//! variants beside these. Layer plan: `docs/phase1-porting-graph.md`, "5. Thing
//! and module system".

pub mod behavior_module;
pub mod body_module;
pub mod collide_module;
pub mod contain_module;
pub mod create_module;
pub mod damage_module;
pub mod destroy_module;
pub mod die_module;
pub mod special_power_module;
pub mod special_power_update_module;
pub mod update_module;
pub mod upgrade_module;

/// Helpers for the unit tests of the module files: the `xfer` of a snapshot
/// to bytes and back.
#[cfg(test)]
pub(crate) mod test_support {
    use crate::common::xfer::{Snapshot, Xfer, XferResult};
    use crate::common::xfer_crc::XferCrc;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;

    /// `xfer` through an `XferSave`: the file's bytes.
    pub(crate) fn save_bytes(s: &mut dyn Snapshot) -> XferResult<Vec<u8>> {
        let mut x = XferSave::new();
        x.open(b"t")?;
        s.xfer(&mut x)?;
        x.close()?;
        Ok(x.bytes().to_vec())
    }

    /// `xfer` through an `XferLoad` over `bytes`.
    pub(crate) fn load_bytes(s: &mut dyn Snapshot, bytes: Vec<u8>) -> XferResult<()> {
        let mut x = XferLoad::new(bytes);
        x.open(b"t")?;
        s.xfer(&mut x)?;
        x.close()
    }

    /// `crc` through an `XferCRC`: its CRC.
    pub(crate) fn crc_of(s: &mut dyn Snapshot) -> XferResult<u32> {
        let mut x = XferCrc::new_without_nan_guard();
        x.open(b"t")?;
        s.crc(&mut x)?;
        Ok(x.get_crc())
    }
}
