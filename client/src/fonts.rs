// SPDX-License-Identifier: GPL-3.0-only
//! (#155, owner decision 2026-11-12) The font the client draws the game's text in: Liberation Sans 2.1.5, regular and
//! bold, shipped with the client (`client/fonts/`, under the SIL Open Font License 1.1: `client/fonts/OFL.txt`, and
//! `client/fonts/README.md` for the version, the source and the checksums). New: upstream draws in the Windows fonts its
//! `Language.ini` and `.wnd` files name, which the client does not have.
//!
//! Both faces are compiled into the client (`include_bytes!`), so the native build and the web build (one `.wasm`, no
//! asset folder: `scripts/build_web.sh`) carry the same bytes. Liberation Sans is made to Arial's metrics, which is what
//! the client measures the game's text as (`crate::text_metrics`), so the drawn text fits the boxes the translated
//! window code lays out. Every face the game names is drawn in it (`crate::text_metrics::draw_face`): Arial as itself,
//! the others (Times New Roman, the narrowed `Generals`, Arial Unicode MS) as an approximation. The app registers both
//! faces with Bevy at start-up and draws the window text, the military subtitle, the popup message, the cinematic text
//! and the mouse tooltip in them. Plain Rust, no Bevy.

/// `LiberationSans-Regular.ttf` (2.1.5, weight 400).
pub const REGULAR: &[u8] = include_bytes!("../fonts/LiberationSans-Regular.ttf");

/// `LiberationSans-Bold.ttf` (2.1.5, weight 700).
pub const BOLD: &[u8] = include_bytes!("../fonts/LiberationSans-Bold.ttf");

/// The family name both faces carry (their name table's name 1).
pub const FAMILY: &str = "Liberation Sans";

/// A face of [`FAMILY`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Regular,
    Bold,
}

impl Face {
    /// The face's file.
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Face::Regular => REGULAR,
            Face::Bold => BOLD,
        }
    }

    /// The weight it is drawn at (its `OS/2` weight class).
    pub fn weight(self) -> u16 {
        match self {
            Face::Regular => 400,
            Face::Bold => 700,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TrueType file's table by tag: its bytes.
    fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> &'a [u8] {
        let be16 = |o: usize| usize::from(u16::from_be_bytes([font[o], font[o + 1]]));
        let be32 = |o: usize| u32::from_be_bytes([font[o], font[o + 1], font[o + 2], font[o + 3]]) as usize;
        let n = be16(4);
        let i = (0..n).find(|i| &font[12 + 16 * i..16 + 16 * i] == tag).expect("table");
        let (off, len) = (be32(12 + 16 * i + 8), be32(12 + 16 * i + 12));
        &font[off..off + len]
    }

    /// The name table's Windows English (platform 3, language 0x409) string `id`.
    fn name(font: &[u8], id: u16) -> String {
        let t = table(font, b"name");
        let be16 = |o: usize| u16::from_be_bytes([t[o], t[o + 1]]);
        let (count, strings) = (usize::from(be16(2)), usize::from(be16(4)));
        for r in 0..count {
            let o = 6 + 12 * r;
            if be16(o) == 3 && be16(o + 4) == 0x409 && be16(o + 6) == id {
                let (len, at) = (usize::from(be16(o + 8)), strings + usize::from(be16(o + 10)));
                let units: Vec<u16> = t[at..at + len].chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                return String::from_utf16_lossy(&units);
            }
        }
        String::new()
    }

    /// #155, predicted before the run: both compiled-in files are Liberation Sans (name 1), Regular and Bold (name 2),
    /// version 2.1.5 (name 5), under the SIL Open Font License 1.1 (name 13), at weights 400 and 700 (`OS/2`), with
    /// 2048 units to the em (`head`), installable (`fsType` 0); and Arial's widths, which `crate::text_metrics` measures
    /// with, are theirs: "S" 1366 units (667 thousandths), "i" 455 (222), "W" 1933 (944), regular; "S" 1366 (667) bold.
    ///
    /// Results (2026-11-12): all held.
    #[test]
    fn the_shipped_fonts_are_liberation_sans_2_1_5_under_the_ofl() {
        for (face, style) in [(Face::Regular, "Regular"), (Face::Bold, "Bold")] {
            let f = face.bytes();
            assert_eq!(name(f, 1), FAMILY);
            assert_eq!(name(f, 2), style);
            assert_eq!(name(f, 5), "Version 2.1.5");
            assert_eq!(name(f, 13), "Licensed under the SIL Open Font License, Version 1.1");
            let os2 = table(f, b"OS/2");
            assert_eq!(u16::from_be_bytes([os2[4], os2[5]]), face.weight());
            assert_eq!(u16::from_be_bytes([os2[8], os2[9]]), 0, "fsType");
            let head = table(f, b"head");
            assert_eq!(u16::from_be_bytes([head[18], head[19]]), 2048);
        }
        // the advance widths of a few glyphs (`cmap` format 4 lookup through `hmtx`)
        let advance = |f: &[u8], ch: u16| -> u16 {
            let cmap = table(f, b"cmap");
            let be16 = |t: &[u8], o: usize| u16::from_be_bytes([t[o], t[o + 1]]);
            let be32 = |t: &[u8], o: usize| u32::from_be_bytes([t[o], t[o + 1], t[o + 2], t[o + 3]]) as usize;
            let n = usize::from(be16(cmap, 2));
            let sub = (0..n).map(|i| (be16(cmap, 4 + 8 * i), be16(cmap, 6 + 8 * i), be32(cmap, 8 + 8 * i))).find(|&(p, e, _)| p == 3 && e == 1).expect("unicode cmap").2;
            let t = &cmap[sub..];
            assert_eq!(be16(t, 0), 4);
            let segs = usize::from(be16(t, 6)) / 2;
            let (ends, starts, deltas, offsets) = (14, 16 + 2 * segs, 16 + 4 * segs, 16 + 6 * segs);
            let s = (0..segs).find(|&s| be16(t, ends + 2 * s) >= ch).expect("segment");
            assert!(be16(t, starts + 2 * s) <= ch);
            let ro = be16(t, offsets + 2 * s);
            let glyph = if ro == 0 {
                ch.wrapping_add(be16(t, deltas + 2 * s))
            } else {
                let at = offsets + 2 * s + usize::from(ro) + 2 * usize::from(ch - be16(t, starts + 2 * s));
                be16(t, at).wrapping_add(be16(t, deltas + 2 * s))
            };
            let hhea = table(f, b"hhea");
            let metrics = be16(hhea, 34);
            let hmtx = table(f, b"hmtx");
            be16(hmtx, 4 * usize::from(glyph.min(metrics - 1)))
        };
        assert_eq!(advance(REGULAR, u16::from(b'S')), 1366);
        assert_eq!(advance(REGULAR, u16::from(b'i')), 455);
        assert_eq!(advance(REGULAR, u16::from(b'W')), 1933);
        assert_eq!(advance(BOLD, u16::from(b'S')), 1366);
    }
}
