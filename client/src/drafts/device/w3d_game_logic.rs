//! UNTESTED DRAFT (#202, #205, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameLogic/W3DGameLogic.cpp
//!
//! The file is the license header and an include of `W3DGameLogic.h`: `W3DGameLogic` adds nothing to `GameLogic`
//! beyond `createTerrainLogic` / `createGhostObjectManager` (the header), which make the W3D-flavoured terrain logic
//! and ghost object manager. The port's logic crate has one of each, so nothing is translated.
