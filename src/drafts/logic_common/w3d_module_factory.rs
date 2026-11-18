//! UNTESTED DRAFT (#202, #208, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/Thing/W3DModuleFactory.cpp
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/Thing/W3DThingFactory.cpp (the class's constructor and destructor are
//!   empty: nothing to translate)
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameLogic/W3DGameLogic.cpp (no code after the banner: nothing to translate)
//!
//! `W3DModuleFactory::init()` registers the draw modules on top of the logic modules (`ModuleFactory::init()`). The logic factory
//! ([`crate::common::module_factory`]) deliberately holds no `MODULETYPE_DRAW` module (a test pins that), so the draw names live
//! here for the client's factory to register in this order. Each `addModule(X)` expands (via `MAKE_STANDARD_MODULE_MACRO`'s
//! `friend_newModuleData`/`friend_newInstance`) to the name `"X"`, type `MODULETYPE_DRAW`.

/// The `addModule(...)` calls of `W3DModuleFactory::init()`, in upstream's order.
pub const W3D_DRAW_MODULE_NAMES: [&str; 19] = [
    "W3DDefaultDraw",
    "W3DDebrisDraw",
    "W3DModelDraw",
    "W3DLaserDraw",
    "W3DOverlordTankDraw",
    "W3DOverlordTruckDraw",
    "W3DOverlordAircraftDraw",
    "W3DProjectileStreamDraw",
    "W3DPoliceCarDraw",
    "W3DRopeDraw",
    "W3DScienceModelDraw",
    "W3DSupplyDraw",
    "W3DDependencyModelDraw",
    "W3DTankDraw",
    "W3DTruckDraw",
    "W3DTracerDraw",
    "W3DTankTruckDraw",
    "W3DTreeDraw",
    "W3DPropDraw",
];

/// `W3DModuleFactory::init()`: `ModuleFactory::init()` (the caller's), then each draw module name in order. Returns the names to register.
pub fn w3d_module_factory_init() -> Vec<&'static str> {
    W3D_DRAW_MODULE_NAMES.to_vec()
}
