// SPDX-License-Identifier: GPL-3.0-only
//! The client's binary: native and web (`wasm32-unknown-unknown`).
//!
//! Natively, `rts-client --zh <folder>` plays a skirmish on the player's Zero Hour folder; without it (and on the web,
//! where the page imports the folder) the demo game runs. `rts-client --model NAME [--anim HIERARCHY.ANIM] [--zh
//! <folder>]` runs the model viewer instead (#10, step 4): the named W3D model from the folder, or the generated
//! placeholder robot without `--zh`.
//!
//! `main` glue: reached by running the app, not by a test (the view model is tested in `view`, the import in `import`,
//! the viewer's scene in `model`).

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let zh = arg("--zh").map(std::path::PathBuf::from);
    if let Some(model) = arg("--model") {
        #[cfg(not(target_arch = "wasm32"))]
        let source = match zh {
            Some(folder) => rts_client::app::ViewerSource::Folder { folder, model, anim: arg("--anim") },
            None => rts_client::app::ViewerSource::Demo,
        };
        // (the web page passes no arguments: the generated robot)
        #[cfg(target_arch = "wasm32")]
        let source = {
            let _ = model;
            rts_client::app::ViewerSource::Demo
        };
        rts_client::app::run_viewer(source);
        return;
    }
    rts_client::app::run(zh);
}
