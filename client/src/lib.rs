// SPDX-License-Identifier: GPL-3.0-only
//! The client of `rts-lockstep-engine`: phase 2, milestone C1 (`docs/phase2-client.md`).
//!
//! **The rule the client keeps: it never changes the simulation.** The game logic decides everything, on fixed 30 Hz
//! frames, with CRCs identical between the native and wasm builds. The client reads the `World` after each logic frame
//! ([`view::snapshot`]) and draws an interpolation between the last two snapshots. Bevy's own `f32` maths, frame timing
//! and system order therefore never reach a CRC. `view`'s tests check that taking the snapshots leaves the logic's CRCs
//! exactly as the gate game's without them.
//!
//! - [`view`]: the view model. What is drawn, read from the `World`; plain Rust, no Bevy.
//! - `app` (feature `app`, on by default): the Bevy application.

pub mod view;

#[cfg(feature = "app")]
pub mod app;
