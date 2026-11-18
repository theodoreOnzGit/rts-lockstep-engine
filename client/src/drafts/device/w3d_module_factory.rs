//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/Thing/W3DModuleFactory.cpp
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/Thing/W3DThingFactory.cpp
//!
//! `W3DThingFactory` has an empty constructor and destructor (nothing to translate). `W3DModuleFactory::init` adds the
//! W3D draw modules to the module factory; the port resolves a `Draw =` name where drawables are made
//! (`crate::client::drawable`), so the list is kept here as the names it must know.

/// The draw module names `W3DModuleFactory::init` adds (W3DModuleFactory.cpp:48-66), in upstream's order, after
/// `ModuleFactory::init()`'s own.
pub const W3D_DRAW_MODULE_NAMES: [&[u8]; 19] = [
    b"W3DDefaultDraw",
    b"W3DDebrisDraw",
    b"W3DModelDraw",
    b"W3DLaserDraw",
    b"W3DOverlordTankDraw",
    b"W3DOverlordTruckDraw",
    b"W3DOverlordAircraftDraw",
    b"W3DProjectileStreamDraw",
    b"W3DPoliceCarDraw",
    b"W3DRopeDraw",
    b"W3DScienceModelDraw",
    b"W3DSupplyDraw",
    b"W3DDependencyModelDraw",
    b"W3DTankDraw",
    b"W3DTruckDraw",
    b"W3DTracerDraw",
    b"W3DTankTruckDraw",
    b"W3DTreeDraw",
    b"W3DPropDraw",
];

/// `W3DModuleFactory::init` (W3DModuleFactory.cpp:55): `add` is `addModule(...)` for the module named.
pub fn init(add_module: &mut dyn FnMut(&[u8])) {
    // extending functionality: ModuleFactory::init() is the caller's
    for name in W3D_DRAW_MODULE_NAMES.iter() {
        add_module(name);
    }
}
