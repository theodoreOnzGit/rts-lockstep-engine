//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankTruckDraw.cpp
//!
//! Sweep 2 (#213): `W3DTankTruckDraw::tossEmitters` (W3DTankTruckDraw.cpp:187) has the same body as
//! `W3DTruckDraw::tossEmitters`: dust, dirt and powerslide effects detached and destroyed. It is
//! `super::w3d_truck_draw::TruckEmitters::toss_emitters`; the tread debris pair of this class is `super::w3d_tank_draw`'s.

pub use super::w3d_truck_draw::TruckEmitters as TankTruckEmitters;
