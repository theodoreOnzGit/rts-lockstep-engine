# Liberation Sans (the client's text font)

Owner decision 2026-11-12 (GitHub #155): the client draws its window text, the subtitle, the popup message, the cinematic
text and the mouse tooltip in Liberation Sans, a font made to Arial's metrics, which is what the game's text is measured
as (`client/src/text_metrics.rs`). Both files are compiled into the client (`include_bytes!`, `client/src/fonts.rs`), so
the native and the web builds carry them; the web build also ships the licence (`scripts/build_web.sh`).

| File | Face | Version | SHA-256 |
|---|---|---|---|
| `LiberationSans-Regular.ttf` | Liberation Sans Regular (weight 400) | 2.1.5 | `4659bc0c58c5028dd488ec928d41d9265db43d9b669fc14ca8b0832daca7b144` |
| `LiberationSans-Bold.ttf` | Liberation Sans Bold (weight 700) | 2.1.5 | `3973aa5054fb467dd5627245d3dc82e37bf16fe075756156a570455871351582` |

- **Source:** the Debian package `fonts-liberation` 1:2.1.5-3 (`/usr/share/fonts/truetype/liberation/`), unmodified;
  upstream is the Liberation Fonts project, https://github.com/liberationfonts/liberation-fonts.
- **Licence:** SIL Open Font License 1.1 ([`OFL.txt`](OFL.txt), with the copyright notice). Checked 2026-11-12 in the
  package's `copyright` file (`License: SIL-OFL-1.1` for every font file) and in each font's own name table (name 13:
  "Licensed under the SIL Open Font License, Version 1.1"; name 5: "Version 2.1.5"; `fsType` 0, installable embedding).
- **Not EA's.** These files are not part of the game and are licensed for redistribution with software (OFL 1.1,
  condition 2). The fonts stay under the OFL; the program that carries them stays under the GPL-3.0 (`LICENSE`).
