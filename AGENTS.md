# AGENTS.md

Rules for AI agents (Claude Code and others) working in `rts-lockstep-engine`. **Every agent reads this file before
doing any work in this repository.** The rules marked HARD RULE are not optional.

## Track all work in GitHub issues (HARD RULE)

Owner direction, 2026-11-03. All work is tracked in the GitHub issues of `theodoreOnzGit/rts-lockstep-engine`, not in
TODO lists, markdown plans or an agent's own task tools alone.

- **File before you start.** Every piece of work has an issue before code is written. That covers a batch, a milestone,
  a defect found on the way, or a gap you decide to leave for later. If the work belongs under an epic, file it as the
  epic's sub-issue.
- **Name the upstream source** the work translates, and say what was checked. A reader must be able to tell what is
  translated and what is new.
- **Keep the issue current.** Comment progress with the numbers that were measured: tests run and passed, the gate's
  CRC, what changed. Comment the results when the work lands, and reference the issue (`#N`) in the commit message.
- **A defect you find is an issue** the moment you find it, even if you fix it in the same change. A gap you leave is
  an issue, with what blocks it.
- **Do not close issues on your own initiative.** Propose the closure with its evidence; the owner decides.
- **Roadmap questions** ("how far along are we?") are answered from the issues.

The epics: #1 (phase 2, the client). The phase 1 issues are #7 (comparison with the C++) and #8 (integration batches).

## How the port is done

- **Translate, don't rewrite.** Where upstream (`upstream/`, EA's GPLv3 release) has the code, it is translated
  function by function, with upstream's names in the doc comments. Only what upstream does not have is new. That
  means the DirectX 8 device (replaced by Bevy/wgpu) and Miles audio and Bink video (never released).
- **Read upstream first** when something misbehaves. In a translation, a difference from upstream is the likely
  cause.
- **Predictions before runs.** Before a test or a full run, write down what you expect, then report what happened,
  including the misses.
- **Correct, don't rewrite history.** A doc claim the code contradicts is fixed in the same change, struck through with
  the date and batch: `~~old~~ **Corrected YYYY-MM-DD (batch N):** new`.
- **Determinism.** All logic maths goes through `petir::Real`, never `std` floats' own functions, so native and wasm
  compute the same bits. Gate (a) (`scripts/gate_a.sh`) compares their CRCs every frame.
- **The client never changes the simulation** (`client/`, phase 2). It reads the `World`, consumes the logic's client
  events, and sends orders only as the game's own command messages. Each client milestone keeps a test that the
  logic's CRCs are unchanged with it attached.

## The game's files (HARD RULE)

The art, sounds, maps, INI data and UI layouts of *Generals* and *Zero Hour* are EA's. They are **never committed,
shipped, hosted or uploaded** by this project. Tests that need them read the owner's copy from a path given at run
time and are skipped without it, never faked. In the browser, the player's files stay in their own browser's storage.

## Mechanics

- Do not run `cargo fmt`.
- Do not put model identifiers in commits or anything pushed.
- The logic crate (root) keeps its toolchain and its one dependency (`petir`). `client/` pins its own toolchain
  (`client/rust-toolchain.toml`) because Bevy needs a newer `rustc`.
