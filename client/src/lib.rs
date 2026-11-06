// SPDX-License-Identifier: GPL-3.0-only
//! The client of `rts-lockstep-engine`: phase 2, milestone C1 (`docs/phase2-client.md`).
//!
//! **The rule the client keeps: it never changes the simulation.** The game logic decides everything, on fixed 30 Hz
//! frames, with CRCs identical between the native and wasm builds. The client reads the `World` after each logic frame
//! ([`view::snapshot`]) and draws an interpolation between the last two snapshots. Bevy's own `f32` maths, frame timing
//! and system order therefore never reach a CRC. `view`'s tests check that taking the snapshots leaves the logic's CRCs
//! exactly as the gate game's without them.
//!
//! - [`drawables`]: each object's drawable and its posed W3D model, chosen by its model condition flags (#25); plain Rust.
//! - [`demo`]: the demo game, until the player's own files are loaded (C2d).
//! - [`import`]: the player's Zero Hour files and the game made from them (C3).
//! - [`input`]: the player's input through the translated client (C2d); plain Rust, no Bevy.
//! - [`terrain`]: the terrain's look from the world and the game's files (#11); its Bevy meshes with feature `app`.
//! - [`ui`]: the game's own control bar from its files (#12): what to draw, as quads; plain Rust.
//! - [`view`]: the view model. What is drawn, read from the `World`; plain Rust, no Bevy.
//! - `model` (feature `app`): W3D models drawn with Bevy, and the model viewer's scene (#10, step 4).
//! - `app` (feature `app`, on by default): the Bevy application.

pub mod demo;
pub mod drawables;
pub mod import;
pub mod input;
pub mod terrain;
pub mod ui;
pub mod view;

#[cfg(feature = "app")]
pub mod app;
#[cfg(feature = "app")]
pub mod model;
