// SPDX-License-Identifier: GPL-3.0-only
//! Translations from the Westwood libraries, `Libraries/Source/WWVegas`.
//! So far: the parts of `WWMath` that game logic leans on, and the WWLib
//! chunk reader the W3D files are read through; since #181 item 3 also WWLib's
//! `strtrim` ([`trim`]) and INI database (`INIClass`, [`ini`]), which the W3D
//! library reads `w3danimsound.ini` with.

pub mod chunkio;
pub mod matrix3d;
pub mod quat;
pub mod random;
pub mod realcrc;
pub mod trim;
pub mod ini;
pub mod vector2;
pub mod vector3;
pub mod vector4;
pub mod wwmath;
// --- p1w4-terrain ---
pub mod plane;
pub mod tri;
// --- end p1w4-terrain ---
