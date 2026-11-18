//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/NetMessageStream.cpp
//!
//! Nothing to translate: the whole body of `NetMessageStream.cpp` (`AddToNetCommandList`, `GetCommandMsg`,
//! `ClearCommandPacket`, `AddCommandToPacket`, `GetCommandPacket`, the `CommandHead`/`CommandTail` lists) is
//! inside one `/* ... */` comment in upstream's release (the file is compiled but defines nothing); the live
//! lockstep path is `NetPacket`/`ConnectionManager`. Checked 2026-11-18: no uncommented definition in the file.
