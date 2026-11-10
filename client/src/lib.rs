// SPDX-License-Identifier: GPL-3.0-only
//! The client of `rts-lockstep-engine`: phase 2, milestone C1 (`docs/phase2-client.md`).
//!
//! **The rule the client keeps: it never changes the simulation.** The game logic decides everything, on fixed 30 Hz
//! frames, with CRCs identical between the native and wasm builds. The client reads the `World` after each logic frame
//! ([`view::snapshot`]) and draws an interpolation between the last two snapshots. Bevy's own `f32` maths, frame timing
//! and system order therefore never reach a CRC. `view`'s tests check that taking the snapshots leaves the logic's CRCs
//! exactly as the gate game's without them.
//!
//! - [`audio`]: the audio device (#13): the translated audio manager's voices played from the game's files; plain Rust,
//!   with the browser's Web Audio output on the web.
//! - [`campaigns`]: campaign missions and the Generals' Challenge started from a list, their maps read as the art is,
//!   and the score screen's Continue (#62); plain Rust.
//! - [`drawables`]: each object's drawable and its posed W3D model, chosen by its model condition flags (#25); plain Rust.
//! - [`demo`]: the demo game, until the player's own files are loaded (C2d).
//! - [`import`]: the player's Zero Hour files and the game made from them (C3).
//! - [`input`]: the player's input through the translated client (C2d); plain Rust, no Bevy.
//! - [`saves`]: saved games (#4): the save folder (natively the game's own, in the browser OPFS `Save/`), the list, and
//!   saving and loading through the translated save code; plain Rust.
//! - [`shell`]: the game's shell (#69): the main menu and its message boxes from their files, run by the translated
//!   shell, and its requests turned into the client's starts; plain Rust (not drawn yet).
//! - [`particles`]: the particle systems' particles as camera-facing quads by texture and blend (#43, #44); plain Rust.
//! - [`terrain`]: the terrain's look from the world and the game's files (#11); its Bevy meshes with feature `app`.
//! - [`ui`]: the game's own control bar from its files (#12): what to draw, as quads; the keyboard's keys and typed
//!   characters into the window manager (#74); plain Rust.
//! - [`view`]: the view model. What is drawn, read from the `World`; plain Rust, no Bevy.
//! - `model` (feature `app`): W3D models drawn with Bevy, and the model viewer's scene (#10, step 4).
//! - `app` (feature `app`, on by default): the Bevy application.

pub mod audio;
pub mod campaigns;
pub mod demo;
pub mod drawables;
pub mod import;
pub mod input;
pub mod particles;
pub mod saves;
pub mod shell;
pub mod terrain;
pub mod ui;
pub mod view;

#[cfg(feature = "app")]
pub mod app;
#[cfg(feature = "app")]
pub mod model;
