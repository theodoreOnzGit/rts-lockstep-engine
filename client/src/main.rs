// SPDX-License-Identifier: GPL-3.0-only
//! The client's binary: native and web (`wasm32-unknown-unknown`).
//!
//! `main` glue: reached by running the app, not by a test (the view model is tested in `view`).

fn main() {
    rts_client::app::run();
}
