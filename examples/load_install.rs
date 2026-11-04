// SPDX-License-Identifier: GPL-3.0-only
//! Loads a Zero Hour install the way the engine starts (`World::from_install`, batch I57) and reports what it read: the
//! archives that did not open, the string file, the logic's INI (the first error stops the load, as upstream's
//! `INI::load` throws), the thing templates and the map cache. With `--map <name>`, it also mounts that map and starts
//! a skirmish on it for a few frames.
//!
//! ```text
//! cargo run --release --example load_install -- <folder> [--generals <dir>] [--language English] [--map <path>]
//! ```
//!
//! `<folder>` is the Zero Hour folder (its `*.big` files). `--generals` gives the original *Generals* folder, whose
//! archives Zero Hour reads too (upstream reads its path from the registry). The game files are read, never copied
//! or uploaded.
//!
//! CLI glue: the loading itself is tested by `install_world::tests`.

use rts_lockstep_engine::common::file_system::{FileSystem, NativeDevice};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::install_world::{InstallError, DEFAULT_LANGUAGE};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(folder) = args.first() else {
        eprintln!("usage: load_install <zero hour folder> [--generals <dir>] [--language <name>] [--map <path>]");
        std::process::exit(2);
    };
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    // the device's root is `/`, so that both folders are absolute device paths
    let abs = |p: &str| -> Vec<u8> {
        let full = std::fs::canonicalize(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        let mut s = full.to_string_lossy().replace('/', "\\").into_bytes();
        if !s.ends_with(b"\\") {
            s.push(b'\\');
        }
        s
    };
    let mut fs = FileSystem::new(NativeDevice::new("/"));
    fs.local.game_dir = abs(folder);
    let generals = opt("--generals").map(|g| abs(&g));
    fs.init(generals.as_deref());
    let language = opt("--language").unwrap_or_else(|| String::from_utf8_lossy(DEFAULT_LANGUAGE).into_owned());
    match World::from_install(&fs, language.as_bytes()) {
        Ok((mut w, report)) => {
            println!("archives that did not open: {}", report.archive_errors.len());
            for e in &report.archive_errors {
                println!("  {e}");
            }
            println!("string file: {:?}", report.csf);
            println!("thing templates: {}", report.templates);
            println!("maps in the map cache: {}", report.maps);
            for (name, md) in w.statics.map_cache.maps.iter().take(20) {
                println!("  {} ({} players) {}", String::from_utf8_lossy(name), md.num_players, String::from_utf16_lossy(md.display_name.str()));
            }
            if let Some(map) = opt("--map") {
                let n = w.mount_map(&fs, map.as_bytes());
                println!("mounted {n} files of {map}");
                match w.start_skirmish(map.as_bytes(), 0, 1234) {
                    Ok(()) => {
                        for _ in 0..30 {
                            if let Err(e) = w.engine_update() {
                                println!("frame error: {e:?}");
                                break;
                            }
                        }
                        println!("after 30 frames: {} objects, live errors {:?}", w.objects.ids_in_order().len(), w.cuts.team_live_errors);
                    }
                    Err(e) => println!("the skirmish did not start: {e:?}"),
                }
            }
        }
        Err(InstallError::Ini { error, missing }) => {
            println!("the install's INI did not load: {error:?}");
            for m in &missing {
                println!("  missing {}", String::from_utf8_lossy(m));
            }
            std::process::exit(1);
        }
        Err(e) => {
            println!("the install did not load: {e:?}");
            std::process::exit(1);
        }
    }
}
