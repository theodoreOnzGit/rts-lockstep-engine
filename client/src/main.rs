// SPDX-License-Identifier: GPL-3.0-only
//! The client's binary: native and web (`wasm32-unknown-unknown`).
//!
//! Natively, `rts-client --zh <folder>` plays a skirmish on the player's Zero Hour folder; without it (and on the web,
//! where the page imports the folder) the demo game runs.
//!
//! `main` glue: reached by running the app, not by a test (the view model is tested in `view`, the import in `import`).

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let zh = args.iter().position(|a| a == "--zh").and_then(|i| args.get(i + 1)).map(std::path::PathBuf::from);
    rts_client::app::run(zh);
}
