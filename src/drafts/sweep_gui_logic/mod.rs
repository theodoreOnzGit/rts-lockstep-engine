//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Sweep 2 (#213) group `gui_logic`: the `Class::method` definitions of GameClient/GUI, GameLogic (Object, AI, System,
//! Map) and GameNetwork whose names appear nowhere in the verified port or the first-pass drafts.

//!
//! # Sweep 2 classification (200 entries)
//!
//! - **Drafted, 136** (in this directory, one `.rs` per upstream `.cpp`).
//! - **Translated already, 38**: `AIStateMachine::setGoalPath` / `getGoalWaypoint` / `getGoalSquad` (`AiMachineExtras`
//!   fields, ai_commands.rs); `TurretAI::getTurretFireAngleSweepForWeaponSlot` / `getTurretSweepSpeedModifierForWeaponSlot`
//!   (`turret.rs` data arrays); `OverlordContain::activate/deactivateRedirectedContain` (`redirection_activated`,
//!   overlord_contain.rs:368, 388, 488); the four `SimpleObjectIterator::sort*` (object_iter.rs:170-175);
//!   `MissileAIUpdate::doPrelaunchState` / `doDeadState` (missile_ai_update.rs:664); `AIUpdateInterface::privateFacePosition`,
//!   `RappelInto`, `FollowWaypointPathExact`, `AsTeam`, `AsTeamExact`, `GetHealed`, `WanderInPlace`, `Busy` (arms of
//!   `base_ai_do_command`, ai_commands.rs:343-475); `PartitionData::calcMaxCoiForObject` (manager.rs:667);
//!   `CrateSystem::friend_findCrateTemplate` (`find_crate_template`, crate_system.rs:196); `TrainTrack::incReference` /
//!   `releaseReference` (the `Arc`); `GameWindow::winSetDisabledTextColors` / `winSetHiliteTextColors`
//!   (`win_set_text_colors`); `winSetNext`, `winSetPrev`, `winGetPrev`, `winSet/GetNext/PrevInLayout` (the arena's lists);
//!   `GameWindowManager::winGetWindowList` (`window_list`); the three `create*Interface` factories (`GameSpyInfo::new`,
//!   `GameResultsQueue::new`, `Pinger::new`).
//! - **Excluded, 26**: `PathfindLayer::doDebugIcons`, `Pathfinder::doDebugIcons`, `Pathfinder::debugShowSearch`,
//!   `DumbProjectileBehavior::displayFlightPath` (`_DEBUG`/`_INTERNAL` icons); `AIStateMachine::getCurrentStateName`
//!   (`STATE_MACHINE_DEBUG`); `AIGroup::groupSurrender`, `groupPickUpPrisoner`, `groupReturnToPrison`,
//!   `AIUpdateInterface::doSurrenderUpdateStuff` (`ALLOW_SURRENDER`, never defined); `TurretAI::getOtherTurretWeaponInfo`,
//!   `friend_getInterTurretDelay` (`INTER_TURRET_DELAY`, never defined); `CellOutwardIterator::nextCell` (`FASTER_GCO` is
//!   defined, PartitionManager.h:75); `PartitionCell::validateCoiList` (`_DEBUG`); `PartitionManager::getPMStats`,
//!   `GameLogic::getAIMetricsStatistics` (`DUMP_PERF_STATS`); `FlightDeckBehavior::validateAssignments` (only
//!   `DEBUG_CRASH`); `IMEManager::printConversionStatus`, `printSentenceStatus` (`DEBUG_IME`); `GameWindowManager::dumpWindow`
//!   (`#ifndef FINAL`, logs), `initTestGUI` (returns first); `GameWindow::winSetEditData` / `winGetEditData` (GUI editor);
//!   `TextureFilenameParameterClass::Copy_Value` (WWSaveLoad's editor parameters); the three `Game*MessageQueue::getThread`
//!   (the drafted threads are run by their owner, no `m_thread`).
//! - Also drafted, not on the list: all of `Network.cpp`, `GameMessageParser.cpp`, `FileTransfer.cpp`, `GameSpy/Chat.cpp`.

pub mod ai_group;
pub mod ai_states;
pub mod battle_plan_update;
pub mod chinook_ai_update;
pub mod misc_object_accessors;
pub mod partition_manager;
pub mod physics_update;
pub mod network;
pub mod game_message_parser;
pub mod file_transfer;
pub mod chat;
pub mod game_info;
pub mod lan_api_callbacks;
pub mod lan_game_info;
pub mod udp;
pub mod game_window;
pub mod game_font;
pub mod game_window_global;
pub mod header_template;
pub mod lan_lobby_menu;
pub mod w3d_game_window;
pub mod ime_manager;
pub mod shell_menu_scheme;
pub mod control_bar;
pub mod control_bar_command;
pub mod control_bar_command_processing;
pub mod control_bar_resizer;
pub mod control_bar_scheme;
