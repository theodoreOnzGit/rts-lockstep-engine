// SPDX-License-Identifier: GPL-3.0-only
//
// Generated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/MessageStream.h (`enum GameMessage::Type`)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameMessage::Type` of the release build (no `_DEBUG`, `_INTERNAL`, `_ALLOW_DEBUG_CHEATS_IN_RELEASE`,
//! `ALLOW_SURRENDER` or `DUMP_PERF_STATS`): the header's enumeration run through the preprocessor with none of them
//! defined, each value counted as C++ counts it. `tests/differential_message_stream.rs` compares every value with the
//! compiled header's (`differential/msg_types.inc`, the same names).

use super::message_stream::MessageType;

/// `GameMessage::MSG_INVALID`.
pub const MSG_INVALID: MessageType = MessageType(0);
/// `GameMessage::MSG_FRAME_TICK`.
pub const MSG_FRAME_TICK: MessageType = MessageType(1);
/// `GameMessage::MSG_RAW_MOUSE_BEGIN`.
pub const MSG_RAW_MOUSE_BEGIN: MessageType = MessageType(2);
/// `GameMessage::MSG_RAW_MOUSE_POSITION`.
pub const MSG_RAW_MOUSE_POSITION: MessageType = MessageType(3);
/// `GameMessage::MSG_RAW_MOUSE_LEFT_BUTTON_DOWN`.
pub const MSG_RAW_MOUSE_LEFT_BUTTON_DOWN: MessageType = MessageType(4);
/// `GameMessage::MSG_RAW_MOUSE_LEFT_DOUBLE_CLICK`.
pub const MSG_RAW_MOUSE_LEFT_DOUBLE_CLICK: MessageType = MessageType(5);
/// `GameMessage::MSG_RAW_MOUSE_LEFT_BUTTON_UP`.
pub const MSG_RAW_MOUSE_LEFT_BUTTON_UP: MessageType = MessageType(6);
/// `GameMessage::MSG_RAW_MOUSE_LEFT_CLICK`.
pub const MSG_RAW_MOUSE_LEFT_CLICK: MessageType = MessageType(7);
/// `GameMessage::MSG_RAW_MOUSE_LEFT_DRAG`.
pub const MSG_RAW_MOUSE_LEFT_DRAG: MessageType = MessageType(8);
/// `GameMessage::MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN`.
pub const MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN: MessageType = MessageType(9);
/// `GameMessage::MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK`.
pub const MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK: MessageType = MessageType(10);
/// `GameMessage::MSG_RAW_MOUSE_MIDDLE_BUTTON_UP`.
pub const MSG_RAW_MOUSE_MIDDLE_BUTTON_UP: MessageType = MessageType(11);
/// `GameMessage::MSG_RAW_MOUSE_MIDDLE_DRAG`.
pub const MSG_RAW_MOUSE_MIDDLE_DRAG: MessageType = MessageType(12);
/// `GameMessage::MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN`.
pub const MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN: MessageType = MessageType(13);
/// `GameMessage::MSG_RAW_MOUSE_RIGHT_DOUBLE_CLICK`.
pub const MSG_RAW_MOUSE_RIGHT_DOUBLE_CLICK: MessageType = MessageType(14);
/// `GameMessage::MSG_RAW_MOUSE_RIGHT_BUTTON_UP`.
pub const MSG_RAW_MOUSE_RIGHT_BUTTON_UP: MessageType = MessageType(15);
/// `GameMessage::MSG_RAW_MOUSE_RIGHT_DRAG`.
pub const MSG_RAW_MOUSE_RIGHT_DRAG: MessageType = MessageType(16);
/// `GameMessage::MSG_RAW_MOUSE_WHEEL`.
pub const MSG_RAW_MOUSE_WHEEL: MessageType = MessageType(17);
/// `GameMessage::MSG_RAW_MOUSE_END`.
pub const MSG_RAW_MOUSE_END: MessageType = MessageType(18);
/// `GameMessage::MSG_RAW_KEY_DOWN`.
pub const MSG_RAW_KEY_DOWN: MessageType = MessageType(19);
/// `GameMessage::MSG_RAW_KEY_UP`.
pub const MSG_RAW_KEY_UP: MessageType = MessageType(20);
/// `GameMessage::MSG_MOUSE_LEFT_CLICK`.
pub const MSG_MOUSE_LEFT_CLICK: MessageType = MessageType(21);
/// `GameMessage::MSG_MOUSE_LEFT_DOUBLE_CLICK`.
pub const MSG_MOUSE_LEFT_DOUBLE_CLICK: MessageType = MessageType(22);
/// `GameMessage::MSG_MOUSE_MIDDLE_CLICK`.
pub const MSG_MOUSE_MIDDLE_CLICK: MessageType = MessageType(23);
/// `GameMessage::MSG_MOUSE_MIDDLE_DOUBLE_CLICK`.
pub const MSG_MOUSE_MIDDLE_DOUBLE_CLICK: MessageType = MessageType(24);
/// `GameMessage::MSG_MOUSE_RIGHT_CLICK`.
pub const MSG_MOUSE_RIGHT_CLICK: MessageType = MessageType(25);
/// `GameMessage::MSG_MOUSE_RIGHT_DOUBLE_CLICK`.
pub const MSG_MOUSE_RIGHT_DOUBLE_CLICK: MessageType = MessageType(26);
/// `GameMessage::MSG_CLEAR_GAME_DATA`.
pub const MSG_CLEAR_GAME_DATA: MessageType = MessageType(27);
/// `GameMessage::MSG_NEW_GAME`.
pub const MSG_NEW_GAME: MessageType = MessageType(28);
/// `GameMessage::MSG_BEGIN_META_MESSAGES`.
pub const MSG_BEGIN_META_MESSAGES: MessageType = MessageType(29);
/// `GameMessage::MSG_META_SAVE_VIEW1`.
pub const MSG_META_SAVE_VIEW1: MessageType = MessageType(30);
/// `GameMessage::MSG_META_SAVE_VIEW2`.
pub const MSG_META_SAVE_VIEW2: MessageType = MessageType(31);
/// `GameMessage::MSG_META_SAVE_VIEW3`.
pub const MSG_META_SAVE_VIEW3: MessageType = MessageType(32);
/// `GameMessage::MSG_META_SAVE_VIEW4`.
pub const MSG_META_SAVE_VIEW4: MessageType = MessageType(33);
/// `GameMessage::MSG_META_SAVE_VIEW5`.
pub const MSG_META_SAVE_VIEW5: MessageType = MessageType(34);
/// `GameMessage::MSG_META_SAVE_VIEW6`.
pub const MSG_META_SAVE_VIEW6: MessageType = MessageType(35);
/// `GameMessage::MSG_META_SAVE_VIEW7`.
pub const MSG_META_SAVE_VIEW7: MessageType = MessageType(36);
/// `GameMessage::MSG_META_SAVE_VIEW8`.
pub const MSG_META_SAVE_VIEW8: MessageType = MessageType(37);
/// `GameMessage::MSG_META_VIEW_VIEW1`.
pub const MSG_META_VIEW_VIEW1: MessageType = MessageType(38);
/// `GameMessage::MSG_META_VIEW_VIEW2`.
pub const MSG_META_VIEW_VIEW2: MessageType = MessageType(39);
/// `GameMessage::MSG_META_VIEW_VIEW3`.
pub const MSG_META_VIEW_VIEW3: MessageType = MessageType(40);
/// `GameMessage::MSG_META_VIEW_VIEW4`.
pub const MSG_META_VIEW_VIEW4: MessageType = MessageType(41);
/// `GameMessage::MSG_META_VIEW_VIEW5`.
pub const MSG_META_VIEW_VIEW5: MessageType = MessageType(42);
/// `GameMessage::MSG_META_VIEW_VIEW6`.
pub const MSG_META_VIEW_VIEW6: MessageType = MessageType(43);
/// `GameMessage::MSG_META_VIEW_VIEW7`.
pub const MSG_META_VIEW_VIEW7: MessageType = MessageType(44);
/// `GameMessage::MSG_META_VIEW_VIEW8`.
pub const MSG_META_VIEW_VIEW8: MessageType = MessageType(45);
/// `GameMessage::MSG_META_CREATE_TEAM0`.
pub const MSG_META_CREATE_TEAM0: MessageType = MessageType(46);
/// `GameMessage::MSG_META_CREATE_TEAM1`.
pub const MSG_META_CREATE_TEAM1: MessageType = MessageType(47);
/// `GameMessage::MSG_META_CREATE_TEAM2`.
pub const MSG_META_CREATE_TEAM2: MessageType = MessageType(48);
/// `GameMessage::MSG_META_CREATE_TEAM3`.
pub const MSG_META_CREATE_TEAM3: MessageType = MessageType(49);
/// `GameMessage::MSG_META_CREATE_TEAM4`.
pub const MSG_META_CREATE_TEAM4: MessageType = MessageType(50);
/// `GameMessage::MSG_META_CREATE_TEAM5`.
pub const MSG_META_CREATE_TEAM5: MessageType = MessageType(51);
/// `GameMessage::MSG_META_CREATE_TEAM6`.
pub const MSG_META_CREATE_TEAM6: MessageType = MessageType(52);
/// `GameMessage::MSG_META_CREATE_TEAM7`.
pub const MSG_META_CREATE_TEAM7: MessageType = MessageType(53);
/// `GameMessage::MSG_META_CREATE_TEAM8`.
pub const MSG_META_CREATE_TEAM8: MessageType = MessageType(54);
/// `GameMessage::MSG_META_CREATE_TEAM9`.
pub const MSG_META_CREATE_TEAM9: MessageType = MessageType(55);
/// `GameMessage::MSG_META_SELECT_TEAM0`.
pub const MSG_META_SELECT_TEAM0: MessageType = MessageType(56);
/// `GameMessage::MSG_META_SELECT_TEAM1`.
pub const MSG_META_SELECT_TEAM1: MessageType = MessageType(57);
/// `GameMessage::MSG_META_SELECT_TEAM2`.
pub const MSG_META_SELECT_TEAM2: MessageType = MessageType(58);
/// `GameMessage::MSG_META_SELECT_TEAM3`.
pub const MSG_META_SELECT_TEAM3: MessageType = MessageType(59);
/// `GameMessage::MSG_META_SELECT_TEAM4`.
pub const MSG_META_SELECT_TEAM4: MessageType = MessageType(60);
/// `GameMessage::MSG_META_SELECT_TEAM5`.
pub const MSG_META_SELECT_TEAM5: MessageType = MessageType(61);
/// `GameMessage::MSG_META_SELECT_TEAM6`.
pub const MSG_META_SELECT_TEAM6: MessageType = MessageType(62);
/// `GameMessage::MSG_META_SELECT_TEAM7`.
pub const MSG_META_SELECT_TEAM7: MessageType = MessageType(63);
/// `GameMessage::MSG_META_SELECT_TEAM8`.
pub const MSG_META_SELECT_TEAM8: MessageType = MessageType(64);
/// `GameMessage::MSG_META_SELECT_TEAM9`.
pub const MSG_META_SELECT_TEAM9: MessageType = MessageType(65);
/// `GameMessage::MSG_META_ADD_TEAM0`.
pub const MSG_META_ADD_TEAM0: MessageType = MessageType(66);
/// `GameMessage::MSG_META_ADD_TEAM1`.
pub const MSG_META_ADD_TEAM1: MessageType = MessageType(67);
/// `GameMessage::MSG_META_ADD_TEAM2`.
pub const MSG_META_ADD_TEAM2: MessageType = MessageType(68);
/// `GameMessage::MSG_META_ADD_TEAM3`.
pub const MSG_META_ADD_TEAM3: MessageType = MessageType(69);
/// `GameMessage::MSG_META_ADD_TEAM4`.
pub const MSG_META_ADD_TEAM4: MessageType = MessageType(70);
/// `GameMessage::MSG_META_ADD_TEAM5`.
pub const MSG_META_ADD_TEAM5: MessageType = MessageType(71);
/// `GameMessage::MSG_META_ADD_TEAM6`.
pub const MSG_META_ADD_TEAM6: MessageType = MessageType(72);
/// `GameMessage::MSG_META_ADD_TEAM7`.
pub const MSG_META_ADD_TEAM7: MessageType = MessageType(73);
/// `GameMessage::MSG_META_ADD_TEAM8`.
pub const MSG_META_ADD_TEAM8: MessageType = MessageType(74);
/// `GameMessage::MSG_META_ADD_TEAM9`.
pub const MSG_META_ADD_TEAM9: MessageType = MessageType(75);
/// `GameMessage::MSG_META_VIEW_TEAM0`.
pub const MSG_META_VIEW_TEAM0: MessageType = MessageType(76);
/// `GameMessage::MSG_META_VIEW_TEAM1`.
pub const MSG_META_VIEW_TEAM1: MessageType = MessageType(77);
/// `GameMessage::MSG_META_VIEW_TEAM2`.
pub const MSG_META_VIEW_TEAM2: MessageType = MessageType(78);
/// `GameMessage::MSG_META_VIEW_TEAM3`.
pub const MSG_META_VIEW_TEAM3: MessageType = MessageType(79);
/// `GameMessage::MSG_META_VIEW_TEAM4`.
pub const MSG_META_VIEW_TEAM4: MessageType = MessageType(80);
/// `GameMessage::MSG_META_VIEW_TEAM5`.
pub const MSG_META_VIEW_TEAM5: MessageType = MessageType(81);
/// `GameMessage::MSG_META_VIEW_TEAM6`.
pub const MSG_META_VIEW_TEAM6: MessageType = MessageType(82);
/// `GameMessage::MSG_META_VIEW_TEAM7`.
pub const MSG_META_VIEW_TEAM7: MessageType = MessageType(83);
/// `GameMessage::MSG_META_VIEW_TEAM8`.
pub const MSG_META_VIEW_TEAM8: MessageType = MessageType(84);
/// `GameMessage::MSG_META_VIEW_TEAM9`.
pub const MSG_META_VIEW_TEAM9: MessageType = MessageType(85);
/// `GameMessage::MSG_META_SELECT_MATCHING_UNITS`.
pub const MSG_META_SELECT_MATCHING_UNITS: MessageType = MessageType(86);
/// `GameMessage::MSG_META_SELECT_NEXT_UNIT`.
pub const MSG_META_SELECT_NEXT_UNIT: MessageType = MessageType(87);
/// `GameMessage::MSG_META_SELECT_PREV_UNIT`.
pub const MSG_META_SELECT_PREV_UNIT: MessageType = MessageType(88);
/// `GameMessage::MSG_META_SELECT_NEXT_WORKER`.
pub const MSG_META_SELECT_NEXT_WORKER: MessageType = MessageType(89);
/// `GameMessage::MSG_META_SELECT_PREV_WORKER`.
pub const MSG_META_SELECT_PREV_WORKER: MessageType = MessageType(90);
/// `GameMessage::MSG_META_VIEW_COMMAND_CENTER`.
pub const MSG_META_VIEW_COMMAND_CENTER: MessageType = MessageType(91);
/// `GameMessage::MSG_META_VIEW_LAST_RADAR_EVENT`.
pub const MSG_META_VIEW_LAST_RADAR_EVENT: MessageType = MessageType(92);
/// `GameMessage::MSG_META_SELECT_HERO`.
pub const MSG_META_SELECT_HERO: MessageType = MessageType(93);
/// `GameMessage::MSG_META_SELECT_ALL`.
pub const MSG_META_SELECT_ALL: MessageType = MessageType(94);
/// `GameMessage::MSG_META_SELECT_ALL_AIRCRAFT`.
pub const MSG_META_SELECT_ALL_AIRCRAFT: MessageType = MessageType(95);
/// `GameMessage::MSG_META_SCATTER`.
pub const MSG_META_SCATTER: MessageType = MessageType(96);
/// `GameMessage::MSG_META_STOP`.
pub const MSG_META_STOP: MessageType = MessageType(97);
/// `GameMessage::MSG_META_DEPLOY`.
pub const MSG_META_DEPLOY: MessageType = MessageType(98);
/// `GameMessage::MSG_META_CREATE_FORMATION`.
pub const MSG_META_CREATE_FORMATION: MessageType = MessageType(99);
/// `GameMessage::MSG_META_FOLLOW`.
pub const MSG_META_FOLLOW: MessageType = MessageType(100);
/// `GameMessage::MSG_META_CHAT_PLAYERS`.
pub const MSG_META_CHAT_PLAYERS: MessageType = MessageType(101);
/// `GameMessage::MSG_META_CHAT_ALLIES`.
pub const MSG_META_CHAT_ALLIES: MessageType = MessageType(102);
/// `GameMessage::MSG_META_CHAT_EVERYONE`.
pub const MSG_META_CHAT_EVERYONE: MessageType = MessageType(103);
/// `GameMessage::MSG_META_DIPLOMACY`.
pub const MSG_META_DIPLOMACY: MessageType = MessageType(104);
/// `GameMessage::MSG_META_OPTIONS`.
pub const MSG_META_OPTIONS: MessageType = MessageType(105);
/// `GameMessage::MSG_META_TOGGLE_LOWER_DETAILS`.
pub const MSG_META_TOGGLE_LOWER_DETAILS: MessageType = MessageType(106);
/// `GameMessage::MSG_META_TOGGLE_CONTROL_BAR`.
pub const MSG_META_TOGGLE_CONTROL_BAR: MessageType = MessageType(107);
/// `GameMessage::MSG_META_BEGIN_PATH_BUILD`.
pub const MSG_META_BEGIN_PATH_BUILD: MessageType = MessageType(108);
/// `GameMessage::MSG_META_END_PATH_BUILD`.
pub const MSG_META_END_PATH_BUILD: MessageType = MessageType(109);
/// `GameMessage::MSG_META_BEGIN_FORCEATTACK`.
pub const MSG_META_BEGIN_FORCEATTACK: MessageType = MessageType(110);
/// `GameMessage::MSG_META_END_FORCEATTACK`.
pub const MSG_META_END_FORCEATTACK: MessageType = MessageType(111);
/// `GameMessage::MSG_META_BEGIN_FORCEMOVE`.
pub const MSG_META_BEGIN_FORCEMOVE: MessageType = MessageType(112);
/// `GameMessage::MSG_META_END_FORCEMOVE`.
pub const MSG_META_END_FORCEMOVE: MessageType = MessageType(113);
/// `GameMessage::MSG_META_BEGIN_WAYPOINTS`.
pub const MSG_META_BEGIN_WAYPOINTS: MessageType = MessageType(114);
/// `GameMessage::MSG_META_END_WAYPOINTS`.
pub const MSG_META_END_WAYPOINTS: MessageType = MessageType(115);
/// `GameMessage::MSG_META_BEGIN_PREFER_SELECTION`.
pub const MSG_META_BEGIN_PREFER_SELECTION: MessageType = MessageType(116);
/// `GameMessage::MSG_META_END_PREFER_SELECTION`.
pub const MSG_META_END_PREFER_SELECTION: MessageType = MessageType(117);
/// `GameMessage::MSG_META_TAKE_SCREENSHOT`.
pub const MSG_META_TAKE_SCREENSHOT: MessageType = MessageType(118);
/// `GameMessage::MSG_META_ALL_CHEER`.
pub const MSG_META_ALL_CHEER: MessageType = MessageType(119);
/// `GameMessage::MSG_META_TOGGLE_ATTACKMOVE`.
pub const MSG_META_TOGGLE_ATTACKMOVE: MessageType = MessageType(120);
/// `GameMessage::MSG_META_BEGIN_CAMERA_ROTATE_LEFT`.
pub const MSG_META_BEGIN_CAMERA_ROTATE_LEFT: MessageType = MessageType(121);
/// `GameMessage::MSG_META_END_CAMERA_ROTATE_LEFT`.
pub const MSG_META_END_CAMERA_ROTATE_LEFT: MessageType = MessageType(122);
/// `GameMessage::MSG_META_BEGIN_CAMERA_ROTATE_RIGHT`.
pub const MSG_META_BEGIN_CAMERA_ROTATE_RIGHT: MessageType = MessageType(123);
/// `GameMessage::MSG_META_END_CAMERA_ROTATE_RIGHT`.
pub const MSG_META_END_CAMERA_ROTATE_RIGHT: MessageType = MessageType(124);
/// `GameMessage::MSG_META_BEGIN_CAMERA_ZOOM_IN`.
pub const MSG_META_BEGIN_CAMERA_ZOOM_IN: MessageType = MessageType(125);
/// `GameMessage::MSG_META_END_CAMERA_ZOOM_IN`.
pub const MSG_META_END_CAMERA_ZOOM_IN: MessageType = MessageType(126);
/// `GameMessage::MSG_META_BEGIN_CAMERA_ZOOM_OUT`.
pub const MSG_META_BEGIN_CAMERA_ZOOM_OUT: MessageType = MessageType(127);
/// `GameMessage::MSG_META_END_CAMERA_ZOOM_OUT`.
pub const MSG_META_END_CAMERA_ZOOM_OUT: MessageType = MessageType(128);
/// `GameMessage::MSG_META_CAMERA_RESET`.
pub const MSG_META_CAMERA_RESET: MessageType = MessageType(129);
/// `GameMessage::MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE`.
pub const MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE: MessageType = MessageType(130);
/// `GameMessage::MSG_META_TOGGLE_FAST_FORWARD_REPLAY`.
pub const MSG_META_TOGGLE_FAST_FORWARD_REPLAY: MessageType = MessageType(131);
/// `GameMessage::MSG_META_DEMO_INSTANT_QUIT`.
pub const MSG_META_DEMO_INSTANT_QUIT: MessageType = MessageType(132);
/// `GameMessage::MSG_META_PLACE_BEACON`.
pub const MSG_META_PLACE_BEACON: MessageType = MessageType(133);
/// `GameMessage::MSG_META_REMOVE_BEACON`.
pub const MSG_META_REMOVE_BEACON: MessageType = MessageType(134);
/// `GameMessage::MSG_END_META_MESSAGES`.
pub const MSG_END_META_MESSAGES: MessageType = MessageType(135);
/// `GameMessage::MSG_MOUSEOVER_DRAWABLE_HINT`.
pub const MSG_MOUSEOVER_DRAWABLE_HINT: MessageType = MessageType(136);
/// `GameMessage::MSG_MOUSEOVER_LOCATION_HINT`.
pub const MSG_MOUSEOVER_LOCATION_HINT: MessageType = MessageType(137);
/// `GameMessage::MSG_VALID_GUICOMMAND_HINT`.
pub const MSG_VALID_GUICOMMAND_HINT: MessageType = MessageType(138);
/// `GameMessage::MSG_INVALID_GUICOMMAND_HINT`.
pub const MSG_INVALID_GUICOMMAND_HINT: MessageType = MessageType(139);
/// `GameMessage::MSG_AREA_SELECTION_HINT`.
pub const MSG_AREA_SELECTION_HINT: MessageType = MessageType(140);
/// `GameMessage::MSG_DO_ATTACK_OBJECT_HINT`.
pub const MSG_DO_ATTACK_OBJECT_HINT: MessageType = MessageType(141);
/// `GameMessage::MSG_IMPOSSIBLE_ATTACK_HINT`.
pub const MSG_IMPOSSIBLE_ATTACK_HINT: MessageType = MessageType(142);
/// `GameMessage::MSG_DO_FORCE_ATTACK_OBJECT_HINT`.
pub const MSG_DO_FORCE_ATTACK_OBJECT_HINT: MessageType = MessageType(143);
/// `GameMessage::MSG_DO_FORCE_ATTACK_GROUND_HINT`.
pub const MSG_DO_FORCE_ATTACK_GROUND_HINT: MessageType = MessageType(144);
/// `GameMessage::MSG_GET_REPAIRED_HINT`.
pub const MSG_GET_REPAIRED_HINT: MessageType = MessageType(145);
/// `GameMessage::MSG_GET_HEALED_HINT`.
pub const MSG_GET_HEALED_HINT: MessageType = MessageType(146);
/// `GameMessage::MSG_DO_REPAIR_HINT`.
pub const MSG_DO_REPAIR_HINT: MessageType = MessageType(147);
/// `GameMessage::MSG_RESUME_CONSTRUCTION_HINT`.
pub const MSG_RESUME_CONSTRUCTION_HINT: MessageType = MessageType(148);
/// `GameMessage::MSG_ENTER_HINT`.
pub const MSG_ENTER_HINT: MessageType = MessageType(149);
/// `GameMessage::MSG_DOCK_HINT`.
pub const MSG_DOCK_HINT: MessageType = MessageType(150);
/// `GameMessage::MSG_DO_MOVETO_HINT`.
pub const MSG_DO_MOVETO_HINT: MessageType = MessageType(151);
/// `GameMessage::MSG_DO_ATTACKMOVETO_HINT`.
pub const MSG_DO_ATTACKMOVETO_HINT: MessageType = MessageType(152);
/// `GameMessage::MSG_ADD_WAYPOINT_HINT`.
pub const MSG_ADD_WAYPOINT_HINT: MessageType = MessageType(153);
/// `GameMessage::MSG_HIJACK_HINT`.
pub const MSG_HIJACK_HINT: MessageType = MessageType(154);
/// `GameMessage::MSG_SABOTAGE_HINT`.
pub const MSG_SABOTAGE_HINT: MessageType = MessageType(155);
/// `GameMessage::MSG_FIREBOMB_HINT`.
pub const MSG_FIREBOMB_HINT: MessageType = MessageType(156);
/// `GameMessage::MSG_CONVERT_TO_CARBOMB_HINT`.
pub const MSG_CONVERT_TO_CARBOMB_HINT: MessageType = MessageType(157);
/// `GameMessage::MSG_CAPTUREBUILDING_HINT`.
pub const MSG_CAPTUREBUILDING_HINT: MessageType = MessageType(158);
/// `GameMessage::MSG_SNIPE_VEHICLE_HINT`.
pub const MSG_SNIPE_VEHICLE_HINT: MessageType = MessageType(159);
/// `GameMessage::MSG_DEFECTOR_HINT`.
pub const MSG_DEFECTOR_HINT: MessageType = MessageType(160);
/// `GameMessage::MSG_SET_RALLY_POINT_HINT`.
pub const MSG_SET_RALLY_POINT_HINT: MessageType = MessageType(161);
/// `GameMessage::MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT`.
pub const MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT: MessageType = MessageType(162);
/// `GameMessage::MSG_DO_SALVAGE_HINT`.
pub const MSG_DO_SALVAGE_HINT: MessageType = MessageType(163);
/// `GameMessage::MSG_DO_INVALID_HINT`.
pub const MSG_DO_INVALID_HINT: MessageType = MessageType(164);
/// `GameMessage::MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT`.
pub const MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT: MessageType = MessageType(165);
/// `GameMessage::MSG_HACK_HINT`.
pub const MSG_HACK_HINT: MessageType = MessageType(166);
/// `GameMessage::MSG_BEGIN_NETWORK_MESSAGES`.
pub const MSG_BEGIN_NETWORK_MESSAGES: MessageType = MessageType(1000);
/// `GameMessage::MSG_CREATE_SELECTED_GROUP`.
pub const MSG_CREATE_SELECTED_GROUP: MessageType = MessageType(1001);
/// `GameMessage::MSG_CREATE_SELECTED_GROUP_NO_SOUND`.
pub const MSG_CREATE_SELECTED_GROUP_NO_SOUND: MessageType = MessageType(1002);
/// `GameMessage::MSG_DESTROY_SELECTED_GROUP`.
pub const MSG_DESTROY_SELECTED_GROUP: MessageType = MessageType(1003);
/// `GameMessage::MSG_REMOVE_FROM_SELECTED_GROUP`.
pub const MSG_REMOVE_FROM_SELECTED_GROUP: MessageType = MessageType(1004);
/// `GameMessage::MSG_SELECTED_GROUP_COMMAND`.
pub const MSG_SELECTED_GROUP_COMMAND: MessageType = MessageType(1005);
/// `GameMessage::MSG_CREATE_TEAM0`.
pub const MSG_CREATE_TEAM0: MessageType = MessageType(1006);
/// `GameMessage::MSG_CREATE_TEAM1`.
pub const MSG_CREATE_TEAM1: MessageType = MessageType(1007);
/// `GameMessage::MSG_CREATE_TEAM2`.
pub const MSG_CREATE_TEAM2: MessageType = MessageType(1008);
/// `GameMessage::MSG_CREATE_TEAM3`.
pub const MSG_CREATE_TEAM3: MessageType = MessageType(1009);
/// `GameMessage::MSG_CREATE_TEAM4`.
pub const MSG_CREATE_TEAM4: MessageType = MessageType(1010);
/// `GameMessage::MSG_CREATE_TEAM5`.
pub const MSG_CREATE_TEAM5: MessageType = MessageType(1011);
/// `GameMessage::MSG_CREATE_TEAM6`.
pub const MSG_CREATE_TEAM6: MessageType = MessageType(1012);
/// `GameMessage::MSG_CREATE_TEAM7`.
pub const MSG_CREATE_TEAM7: MessageType = MessageType(1013);
/// `GameMessage::MSG_CREATE_TEAM8`.
pub const MSG_CREATE_TEAM8: MessageType = MessageType(1014);
/// `GameMessage::MSG_CREATE_TEAM9`.
pub const MSG_CREATE_TEAM9: MessageType = MessageType(1015);
/// `GameMessage::MSG_SELECT_TEAM0`.
pub const MSG_SELECT_TEAM0: MessageType = MessageType(1016);
/// `GameMessage::MSG_SELECT_TEAM1`.
pub const MSG_SELECT_TEAM1: MessageType = MessageType(1017);
/// `GameMessage::MSG_SELECT_TEAM2`.
pub const MSG_SELECT_TEAM2: MessageType = MessageType(1018);
/// `GameMessage::MSG_SELECT_TEAM3`.
pub const MSG_SELECT_TEAM3: MessageType = MessageType(1019);
/// `GameMessage::MSG_SELECT_TEAM4`.
pub const MSG_SELECT_TEAM4: MessageType = MessageType(1020);
/// `GameMessage::MSG_SELECT_TEAM5`.
pub const MSG_SELECT_TEAM5: MessageType = MessageType(1021);
/// `GameMessage::MSG_SELECT_TEAM6`.
pub const MSG_SELECT_TEAM6: MessageType = MessageType(1022);
/// `GameMessage::MSG_SELECT_TEAM7`.
pub const MSG_SELECT_TEAM7: MessageType = MessageType(1023);
/// `GameMessage::MSG_SELECT_TEAM8`.
pub const MSG_SELECT_TEAM8: MessageType = MessageType(1024);
/// `GameMessage::MSG_SELECT_TEAM9`.
pub const MSG_SELECT_TEAM9: MessageType = MessageType(1025);
/// `GameMessage::MSG_ADD_TEAM0`.
pub const MSG_ADD_TEAM0: MessageType = MessageType(1026);
/// `GameMessage::MSG_ADD_TEAM1`.
pub const MSG_ADD_TEAM1: MessageType = MessageType(1027);
/// `GameMessage::MSG_ADD_TEAM2`.
pub const MSG_ADD_TEAM2: MessageType = MessageType(1028);
/// `GameMessage::MSG_ADD_TEAM3`.
pub const MSG_ADD_TEAM3: MessageType = MessageType(1029);
/// `GameMessage::MSG_ADD_TEAM4`.
pub const MSG_ADD_TEAM4: MessageType = MessageType(1030);
/// `GameMessage::MSG_ADD_TEAM5`.
pub const MSG_ADD_TEAM5: MessageType = MessageType(1031);
/// `GameMessage::MSG_ADD_TEAM6`.
pub const MSG_ADD_TEAM6: MessageType = MessageType(1032);
/// `GameMessage::MSG_ADD_TEAM7`.
pub const MSG_ADD_TEAM7: MessageType = MessageType(1033);
/// `GameMessage::MSG_ADD_TEAM8`.
pub const MSG_ADD_TEAM8: MessageType = MessageType(1034);
/// `GameMessage::MSG_ADD_TEAM9`.
pub const MSG_ADD_TEAM9: MessageType = MessageType(1035);
/// `GameMessage::MSG_DO_ATTACKSQUAD`.
pub const MSG_DO_ATTACKSQUAD: MessageType = MessageType(1036);
/// `GameMessage::MSG_DO_WEAPON`.
pub const MSG_DO_WEAPON: MessageType = MessageType(1037);
/// `GameMessage::MSG_DO_WEAPON_AT_LOCATION`.
pub const MSG_DO_WEAPON_AT_LOCATION: MessageType = MessageType(1038);
/// `GameMessage::MSG_DO_WEAPON_AT_OBJECT`.
pub const MSG_DO_WEAPON_AT_OBJECT: MessageType = MessageType(1039);
/// `GameMessage::MSG_DO_SPECIAL_POWER`.
pub const MSG_DO_SPECIAL_POWER: MessageType = MessageType(1040);
/// `GameMessage::MSG_DO_SPECIAL_POWER_AT_LOCATION`.
pub const MSG_DO_SPECIAL_POWER_AT_LOCATION: MessageType = MessageType(1041);
/// `GameMessage::MSG_DO_SPECIAL_POWER_AT_OBJECT`.
pub const MSG_DO_SPECIAL_POWER_AT_OBJECT: MessageType = MessageType(1042);
/// `GameMessage::MSG_SET_RALLY_POINT`.
pub const MSG_SET_RALLY_POINT: MessageType = MessageType(1043);
/// `GameMessage::MSG_PURCHASE_SCIENCE`.
pub const MSG_PURCHASE_SCIENCE: MessageType = MessageType(1044);
/// `GameMessage::MSG_QUEUE_UPGRADE`.
pub const MSG_QUEUE_UPGRADE: MessageType = MessageType(1045);
/// `GameMessage::MSG_CANCEL_UPGRADE`.
pub const MSG_CANCEL_UPGRADE: MessageType = MessageType(1046);
/// `GameMessage::MSG_QUEUE_UNIT_CREATE`.
pub const MSG_QUEUE_UNIT_CREATE: MessageType = MessageType(1047);
/// `GameMessage::MSG_CANCEL_UNIT_CREATE`.
pub const MSG_CANCEL_UNIT_CREATE: MessageType = MessageType(1048);
/// `GameMessage::MSG_DOZER_CONSTRUCT`.
pub const MSG_DOZER_CONSTRUCT: MessageType = MessageType(1049);
/// `GameMessage::MSG_DOZER_CONSTRUCT_LINE`.
pub const MSG_DOZER_CONSTRUCT_LINE: MessageType = MessageType(1050);
/// `GameMessage::MSG_DOZER_CANCEL_CONSTRUCT`.
pub const MSG_DOZER_CANCEL_CONSTRUCT: MessageType = MessageType(1051);
/// `GameMessage::MSG_SELL`.
pub const MSG_SELL: MessageType = MessageType(1052);
/// `GameMessage::MSG_EXIT`.
pub const MSG_EXIT: MessageType = MessageType(1053);
/// `GameMessage::MSG_EVACUATE`.
pub const MSG_EVACUATE: MessageType = MessageType(1054);
/// `GameMessage::MSG_EXECUTE_RAILED_TRANSPORT`.
pub const MSG_EXECUTE_RAILED_TRANSPORT: MessageType = MessageType(1055);
/// `GameMessage::MSG_COMBATDROP_AT_LOCATION`.
pub const MSG_COMBATDROP_AT_LOCATION: MessageType = MessageType(1056);
/// `GameMessage::MSG_COMBATDROP_AT_OBJECT`.
pub const MSG_COMBATDROP_AT_OBJECT: MessageType = MessageType(1057);
/// `GameMessage::MSG_AREA_SELECTION`.
pub const MSG_AREA_SELECTION: MessageType = MessageType(1058);
/// `GameMessage::MSG_DO_ATTACK_OBJECT`.
pub const MSG_DO_ATTACK_OBJECT: MessageType = MessageType(1059);
/// `GameMessage::MSG_DO_FORCE_ATTACK_OBJECT`.
pub const MSG_DO_FORCE_ATTACK_OBJECT: MessageType = MessageType(1060);
/// `GameMessage::MSG_DO_FORCE_ATTACK_GROUND`.
pub const MSG_DO_FORCE_ATTACK_GROUND: MessageType = MessageType(1061);
/// `GameMessage::MSG_GET_REPAIRED`.
pub const MSG_GET_REPAIRED: MessageType = MessageType(1062);
/// `GameMessage::MSG_GET_HEALED`.
pub const MSG_GET_HEALED: MessageType = MessageType(1063);
/// `GameMessage::MSG_DO_REPAIR`.
pub const MSG_DO_REPAIR: MessageType = MessageType(1064);
/// `GameMessage::MSG_RESUME_CONSTRUCTION`.
pub const MSG_RESUME_CONSTRUCTION: MessageType = MessageType(1065);
/// `GameMessage::MSG_ENTER`.
pub const MSG_ENTER: MessageType = MessageType(1066);
/// `GameMessage::MSG_DOCK`.
pub const MSG_DOCK: MessageType = MessageType(1067);
/// `GameMessage::MSG_DO_MOVETO`.
pub const MSG_DO_MOVETO: MessageType = MessageType(1068);
/// `GameMessage::MSG_DO_ATTACKMOVETO`.
pub const MSG_DO_ATTACKMOVETO: MessageType = MessageType(1069);
/// `GameMessage::MSG_DO_FORCEMOVETO`.
pub const MSG_DO_FORCEMOVETO: MessageType = MessageType(1070);
/// `GameMessage::MSG_ADD_WAYPOINT`.
pub const MSG_ADD_WAYPOINT: MessageType = MessageType(1071);
/// `GameMessage::MSG_DO_GUARD_POSITION`.
pub const MSG_DO_GUARD_POSITION: MessageType = MessageType(1072);
/// `GameMessage::MSG_DO_GUARD_OBJECT`.
pub const MSG_DO_GUARD_OBJECT: MessageType = MessageType(1073);
/// `GameMessage::MSG_DO_STOP`.
pub const MSG_DO_STOP: MessageType = MessageType(1074);
/// `GameMessage::MSG_DO_SCATTER`.
pub const MSG_DO_SCATTER: MessageType = MessageType(1075);
/// `GameMessage::MSG_INTERNET_HACK`.
pub const MSG_INTERNET_HACK: MessageType = MessageType(1076);
/// `GameMessage::MSG_DO_CHEER`.
pub const MSG_DO_CHEER: MessageType = MessageType(1077);
/// `GameMessage::MSG_TOGGLE_OVERCHARGE`.
pub const MSG_TOGGLE_OVERCHARGE: MessageType = MessageType(1078);
/// `GameMessage::MSG_SWITCH_WEAPONS`.
pub const MSG_SWITCH_WEAPONS: MessageType = MessageType(1079);
/// `GameMessage::MSG_CONVERT_TO_CARBOMB`.
pub const MSG_CONVERT_TO_CARBOMB: MessageType = MessageType(1080);
/// `GameMessage::MSG_CAPTUREBUILDING`.
pub const MSG_CAPTUREBUILDING: MessageType = MessageType(1081);
/// `GameMessage::MSG_DISABLEVEHICLE_HACK`.
pub const MSG_DISABLEVEHICLE_HACK: MessageType = MessageType(1082);
/// `GameMessage::MSG_STEALCASH_HACK`.
pub const MSG_STEALCASH_HACK: MessageType = MessageType(1083);
/// `GameMessage::MSG_DISABLEBUILDING_HACK`.
pub const MSG_DISABLEBUILDING_HACK: MessageType = MessageType(1084);
/// `GameMessage::MSG_SNIPE_VEHICLE`.
pub const MSG_SNIPE_VEHICLE: MessageType = MessageType(1085);
/// `GameMessage::MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION`.
pub const MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION: MessageType = MessageType(1086);
/// `GameMessage::MSG_DO_SALVAGE`.
pub const MSG_DO_SALVAGE: MessageType = MessageType(1087);
/// `GameMessage::MSG_CLEAR_INGAME_POPUP_MESSAGE`.
pub const MSG_CLEAR_INGAME_POPUP_MESSAGE: MessageType = MessageType(1088);
/// `GameMessage::MSG_PLACE_BEACON`.
pub const MSG_PLACE_BEACON: MessageType = MessageType(1089);
/// `GameMessage::MSG_REMOVE_BEACON`.
pub const MSG_REMOVE_BEACON: MessageType = MessageType(1090);
/// `GameMessage::MSG_SET_BEACON_TEXT`.
pub const MSG_SET_BEACON_TEXT: MessageType = MessageType(1091);
/// `GameMessage::MSG_SET_REPLAY_CAMERA`.
pub const MSG_SET_REPLAY_CAMERA: MessageType = MessageType(1092);
/// `GameMessage::MSG_SELF_DESTRUCT`.
pub const MSG_SELF_DESTRUCT: MessageType = MessageType(1093);
/// `GameMessage::MSG_CREATE_FORMATION`.
pub const MSG_CREATE_FORMATION: MessageType = MessageType(1094);
/// `GameMessage::MSG_LOGIC_CRC`.
pub const MSG_LOGIC_CRC: MessageType = MessageType(1095);
/// `GameMessage::MSG_SET_MINE_CLEARING_DETAIL`.
pub const MSG_SET_MINE_CLEARING_DETAIL: MessageType = MessageType(1096);
/// `GameMessage::MSG_ENABLE_RETALIATION_MODE`.
pub const MSG_ENABLE_RETALIATION_MODE: MessageType = MessageType(1097);
/// `GameMessage::MSG_BEGIN_DEBUG_NETWORK_MESSAGES`.
pub const MSG_BEGIN_DEBUG_NETWORK_MESSAGES: MessageType = MessageType(1900);
/// `GameMessage::MSG_END_NETWORK_MESSAGES`.
pub const MSG_END_NETWORK_MESSAGES: MessageType = MessageType(1999);
/// `GameMessage::MSG_TIMESTAMP`.
pub const MSG_TIMESTAMP: MessageType = MessageType(2000);
/// `GameMessage::MSG_OBJECT_CREATED`.
pub const MSG_OBJECT_CREATED: MessageType = MessageType(2001);
/// `GameMessage::MSG_OBJECT_DESTROYED`.
pub const MSG_OBJECT_DESTROYED: MessageType = MessageType(2002);
/// `GameMessage::MSG_OBJECT_POSITION`.
pub const MSG_OBJECT_POSITION: MessageType = MessageType(2003);
/// `GameMessage::MSG_OBJECT_ORIENTATION`.
pub const MSG_OBJECT_ORIENTATION: MessageType = MessageType(2004);
/// `GameMessage::MSG_OBJECT_JOINED_TEAM`.
pub const MSG_OBJECT_JOINED_TEAM: MessageType = MessageType(2005);
/// `GameMessage::MSG_COUNT`.
pub const MSG_COUNT: MessageType = MessageType(2006);

/// Every name of the enumeration with its value, in declaration order.
pub const MESSAGE_TYPES: [(&str, MessageType); 274] = [
    ("MSG_INVALID", MSG_INVALID),
    ("MSG_FRAME_TICK", MSG_FRAME_TICK),
    ("MSG_RAW_MOUSE_BEGIN", MSG_RAW_MOUSE_BEGIN),
    ("MSG_RAW_MOUSE_POSITION", MSG_RAW_MOUSE_POSITION),
    ("MSG_RAW_MOUSE_LEFT_BUTTON_DOWN", MSG_RAW_MOUSE_LEFT_BUTTON_DOWN),
    ("MSG_RAW_MOUSE_LEFT_DOUBLE_CLICK", MSG_RAW_MOUSE_LEFT_DOUBLE_CLICK),
    ("MSG_RAW_MOUSE_LEFT_BUTTON_UP", MSG_RAW_MOUSE_LEFT_BUTTON_UP),
    ("MSG_RAW_MOUSE_LEFT_CLICK", MSG_RAW_MOUSE_LEFT_CLICK),
    ("MSG_RAW_MOUSE_LEFT_DRAG", MSG_RAW_MOUSE_LEFT_DRAG),
    ("MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN", MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN),
    ("MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK", MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK),
    ("MSG_RAW_MOUSE_MIDDLE_BUTTON_UP", MSG_RAW_MOUSE_MIDDLE_BUTTON_UP),
    ("MSG_RAW_MOUSE_MIDDLE_DRAG", MSG_RAW_MOUSE_MIDDLE_DRAG),
    ("MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN", MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN),
    ("MSG_RAW_MOUSE_RIGHT_DOUBLE_CLICK", MSG_RAW_MOUSE_RIGHT_DOUBLE_CLICK),
    ("MSG_RAW_MOUSE_RIGHT_BUTTON_UP", MSG_RAW_MOUSE_RIGHT_BUTTON_UP),
    ("MSG_RAW_MOUSE_RIGHT_DRAG", MSG_RAW_MOUSE_RIGHT_DRAG),
    ("MSG_RAW_MOUSE_WHEEL", MSG_RAW_MOUSE_WHEEL),
    ("MSG_RAW_MOUSE_END", MSG_RAW_MOUSE_END),
    ("MSG_RAW_KEY_DOWN", MSG_RAW_KEY_DOWN),
    ("MSG_RAW_KEY_UP", MSG_RAW_KEY_UP),
    ("MSG_MOUSE_LEFT_CLICK", MSG_MOUSE_LEFT_CLICK),
    ("MSG_MOUSE_LEFT_DOUBLE_CLICK", MSG_MOUSE_LEFT_DOUBLE_CLICK),
    ("MSG_MOUSE_MIDDLE_CLICK", MSG_MOUSE_MIDDLE_CLICK),
    ("MSG_MOUSE_MIDDLE_DOUBLE_CLICK", MSG_MOUSE_MIDDLE_DOUBLE_CLICK),
    ("MSG_MOUSE_RIGHT_CLICK", MSG_MOUSE_RIGHT_CLICK),
    ("MSG_MOUSE_RIGHT_DOUBLE_CLICK", MSG_MOUSE_RIGHT_DOUBLE_CLICK),
    ("MSG_CLEAR_GAME_DATA", MSG_CLEAR_GAME_DATA),
    ("MSG_NEW_GAME", MSG_NEW_GAME),
    ("MSG_BEGIN_META_MESSAGES", MSG_BEGIN_META_MESSAGES),
    ("MSG_META_SAVE_VIEW1", MSG_META_SAVE_VIEW1),
    ("MSG_META_SAVE_VIEW2", MSG_META_SAVE_VIEW2),
    ("MSG_META_SAVE_VIEW3", MSG_META_SAVE_VIEW3),
    ("MSG_META_SAVE_VIEW4", MSG_META_SAVE_VIEW4),
    ("MSG_META_SAVE_VIEW5", MSG_META_SAVE_VIEW5),
    ("MSG_META_SAVE_VIEW6", MSG_META_SAVE_VIEW6),
    ("MSG_META_SAVE_VIEW7", MSG_META_SAVE_VIEW7),
    ("MSG_META_SAVE_VIEW8", MSG_META_SAVE_VIEW8),
    ("MSG_META_VIEW_VIEW1", MSG_META_VIEW_VIEW1),
    ("MSG_META_VIEW_VIEW2", MSG_META_VIEW_VIEW2),
    ("MSG_META_VIEW_VIEW3", MSG_META_VIEW_VIEW3),
    ("MSG_META_VIEW_VIEW4", MSG_META_VIEW_VIEW4),
    ("MSG_META_VIEW_VIEW5", MSG_META_VIEW_VIEW5),
    ("MSG_META_VIEW_VIEW6", MSG_META_VIEW_VIEW6),
    ("MSG_META_VIEW_VIEW7", MSG_META_VIEW_VIEW7),
    ("MSG_META_VIEW_VIEW8", MSG_META_VIEW_VIEW8),
    ("MSG_META_CREATE_TEAM0", MSG_META_CREATE_TEAM0),
    ("MSG_META_CREATE_TEAM1", MSG_META_CREATE_TEAM1),
    ("MSG_META_CREATE_TEAM2", MSG_META_CREATE_TEAM2),
    ("MSG_META_CREATE_TEAM3", MSG_META_CREATE_TEAM3),
    ("MSG_META_CREATE_TEAM4", MSG_META_CREATE_TEAM4),
    ("MSG_META_CREATE_TEAM5", MSG_META_CREATE_TEAM5),
    ("MSG_META_CREATE_TEAM6", MSG_META_CREATE_TEAM6),
    ("MSG_META_CREATE_TEAM7", MSG_META_CREATE_TEAM7),
    ("MSG_META_CREATE_TEAM8", MSG_META_CREATE_TEAM8),
    ("MSG_META_CREATE_TEAM9", MSG_META_CREATE_TEAM9),
    ("MSG_META_SELECT_TEAM0", MSG_META_SELECT_TEAM0),
    ("MSG_META_SELECT_TEAM1", MSG_META_SELECT_TEAM1),
    ("MSG_META_SELECT_TEAM2", MSG_META_SELECT_TEAM2),
    ("MSG_META_SELECT_TEAM3", MSG_META_SELECT_TEAM3),
    ("MSG_META_SELECT_TEAM4", MSG_META_SELECT_TEAM4),
    ("MSG_META_SELECT_TEAM5", MSG_META_SELECT_TEAM5),
    ("MSG_META_SELECT_TEAM6", MSG_META_SELECT_TEAM6),
    ("MSG_META_SELECT_TEAM7", MSG_META_SELECT_TEAM7),
    ("MSG_META_SELECT_TEAM8", MSG_META_SELECT_TEAM8),
    ("MSG_META_SELECT_TEAM9", MSG_META_SELECT_TEAM9),
    ("MSG_META_ADD_TEAM0", MSG_META_ADD_TEAM0),
    ("MSG_META_ADD_TEAM1", MSG_META_ADD_TEAM1),
    ("MSG_META_ADD_TEAM2", MSG_META_ADD_TEAM2),
    ("MSG_META_ADD_TEAM3", MSG_META_ADD_TEAM3),
    ("MSG_META_ADD_TEAM4", MSG_META_ADD_TEAM4),
    ("MSG_META_ADD_TEAM5", MSG_META_ADD_TEAM5),
    ("MSG_META_ADD_TEAM6", MSG_META_ADD_TEAM6),
    ("MSG_META_ADD_TEAM7", MSG_META_ADD_TEAM7),
    ("MSG_META_ADD_TEAM8", MSG_META_ADD_TEAM8),
    ("MSG_META_ADD_TEAM9", MSG_META_ADD_TEAM9),
    ("MSG_META_VIEW_TEAM0", MSG_META_VIEW_TEAM0),
    ("MSG_META_VIEW_TEAM1", MSG_META_VIEW_TEAM1),
    ("MSG_META_VIEW_TEAM2", MSG_META_VIEW_TEAM2),
    ("MSG_META_VIEW_TEAM3", MSG_META_VIEW_TEAM3),
    ("MSG_META_VIEW_TEAM4", MSG_META_VIEW_TEAM4),
    ("MSG_META_VIEW_TEAM5", MSG_META_VIEW_TEAM5),
    ("MSG_META_VIEW_TEAM6", MSG_META_VIEW_TEAM6),
    ("MSG_META_VIEW_TEAM7", MSG_META_VIEW_TEAM7),
    ("MSG_META_VIEW_TEAM8", MSG_META_VIEW_TEAM8),
    ("MSG_META_VIEW_TEAM9", MSG_META_VIEW_TEAM9),
    ("MSG_META_SELECT_MATCHING_UNITS", MSG_META_SELECT_MATCHING_UNITS),
    ("MSG_META_SELECT_NEXT_UNIT", MSG_META_SELECT_NEXT_UNIT),
    ("MSG_META_SELECT_PREV_UNIT", MSG_META_SELECT_PREV_UNIT),
    ("MSG_META_SELECT_NEXT_WORKER", MSG_META_SELECT_NEXT_WORKER),
    ("MSG_META_SELECT_PREV_WORKER", MSG_META_SELECT_PREV_WORKER),
    ("MSG_META_VIEW_COMMAND_CENTER", MSG_META_VIEW_COMMAND_CENTER),
    ("MSG_META_VIEW_LAST_RADAR_EVENT", MSG_META_VIEW_LAST_RADAR_EVENT),
    ("MSG_META_SELECT_HERO", MSG_META_SELECT_HERO),
    ("MSG_META_SELECT_ALL", MSG_META_SELECT_ALL),
    ("MSG_META_SELECT_ALL_AIRCRAFT", MSG_META_SELECT_ALL_AIRCRAFT),
    ("MSG_META_SCATTER", MSG_META_SCATTER),
    ("MSG_META_STOP", MSG_META_STOP),
    ("MSG_META_DEPLOY", MSG_META_DEPLOY),
    ("MSG_META_CREATE_FORMATION", MSG_META_CREATE_FORMATION),
    ("MSG_META_FOLLOW", MSG_META_FOLLOW),
    ("MSG_META_CHAT_PLAYERS", MSG_META_CHAT_PLAYERS),
    ("MSG_META_CHAT_ALLIES", MSG_META_CHAT_ALLIES),
    ("MSG_META_CHAT_EVERYONE", MSG_META_CHAT_EVERYONE),
    ("MSG_META_DIPLOMACY", MSG_META_DIPLOMACY),
    ("MSG_META_OPTIONS", MSG_META_OPTIONS),
    ("MSG_META_TOGGLE_LOWER_DETAILS", MSG_META_TOGGLE_LOWER_DETAILS),
    ("MSG_META_TOGGLE_CONTROL_BAR", MSG_META_TOGGLE_CONTROL_BAR),
    ("MSG_META_BEGIN_PATH_BUILD", MSG_META_BEGIN_PATH_BUILD),
    ("MSG_META_END_PATH_BUILD", MSG_META_END_PATH_BUILD),
    ("MSG_META_BEGIN_FORCEATTACK", MSG_META_BEGIN_FORCEATTACK),
    ("MSG_META_END_FORCEATTACK", MSG_META_END_FORCEATTACK),
    ("MSG_META_BEGIN_FORCEMOVE", MSG_META_BEGIN_FORCEMOVE),
    ("MSG_META_END_FORCEMOVE", MSG_META_END_FORCEMOVE),
    ("MSG_META_BEGIN_WAYPOINTS", MSG_META_BEGIN_WAYPOINTS),
    ("MSG_META_END_WAYPOINTS", MSG_META_END_WAYPOINTS),
    ("MSG_META_BEGIN_PREFER_SELECTION", MSG_META_BEGIN_PREFER_SELECTION),
    ("MSG_META_END_PREFER_SELECTION", MSG_META_END_PREFER_SELECTION),
    ("MSG_META_TAKE_SCREENSHOT", MSG_META_TAKE_SCREENSHOT),
    ("MSG_META_ALL_CHEER", MSG_META_ALL_CHEER),
    ("MSG_META_TOGGLE_ATTACKMOVE", MSG_META_TOGGLE_ATTACKMOVE),
    ("MSG_META_BEGIN_CAMERA_ROTATE_LEFT", MSG_META_BEGIN_CAMERA_ROTATE_LEFT),
    ("MSG_META_END_CAMERA_ROTATE_LEFT", MSG_META_END_CAMERA_ROTATE_LEFT),
    ("MSG_META_BEGIN_CAMERA_ROTATE_RIGHT", MSG_META_BEGIN_CAMERA_ROTATE_RIGHT),
    ("MSG_META_END_CAMERA_ROTATE_RIGHT", MSG_META_END_CAMERA_ROTATE_RIGHT),
    ("MSG_META_BEGIN_CAMERA_ZOOM_IN", MSG_META_BEGIN_CAMERA_ZOOM_IN),
    ("MSG_META_END_CAMERA_ZOOM_IN", MSG_META_END_CAMERA_ZOOM_IN),
    ("MSG_META_BEGIN_CAMERA_ZOOM_OUT", MSG_META_BEGIN_CAMERA_ZOOM_OUT),
    ("MSG_META_END_CAMERA_ZOOM_OUT", MSG_META_END_CAMERA_ZOOM_OUT),
    ("MSG_META_CAMERA_RESET", MSG_META_CAMERA_RESET),
    ("MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE", MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE),
    ("MSG_META_TOGGLE_FAST_FORWARD_REPLAY", MSG_META_TOGGLE_FAST_FORWARD_REPLAY),
    ("MSG_META_DEMO_INSTANT_QUIT", MSG_META_DEMO_INSTANT_QUIT),
    ("MSG_META_PLACE_BEACON", MSG_META_PLACE_BEACON),
    ("MSG_META_REMOVE_BEACON", MSG_META_REMOVE_BEACON),
    ("MSG_END_META_MESSAGES", MSG_END_META_MESSAGES),
    ("MSG_MOUSEOVER_DRAWABLE_HINT", MSG_MOUSEOVER_DRAWABLE_HINT),
    ("MSG_MOUSEOVER_LOCATION_HINT", MSG_MOUSEOVER_LOCATION_HINT),
    ("MSG_VALID_GUICOMMAND_HINT", MSG_VALID_GUICOMMAND_HINT),
    ("MSG_INVALID_GUICOMMAND_HINT", MSG_INVALID_GUICOMMAND_HINT),
    ("MSG_AREA_SELECTION_HINT", MSG_AREA_SELECTION_HINT),
    ("MSG_DO_ATTACK_OBJECT_HINT", MSG_DO_ATTACK_OBJECT_HINT),
    ("MSG_IMPOSSIBLE_ATTACK_HINT", MSG_IMPOSSIBLE_ATTACK_HINT),
    ("MSG_DO_FORCE_ATTACK_OBJECT_HINT", MSG_DO_FORCE_ATTACK_OBJECT_HINT),
    ("MSG_DO_FORCE_ATTACK_GROUND_HINT", MSG_DO_FORCE_ATTACK_GROUND_HINT),
    ("MSG_GET_REPAIRED_HINT", MSG_GET_REPAIRED_HINT),
    ("MSG_GET_HEALED_HINT", MSG_GET_HEALED_HINT),
    ("MSG_DO_REPAIR_HINT", MSG_DO_REPAIR_HINT),
    ("MSG_RESUME_CONSTRUCTION_HINT", MSG_RESUME_CONSTRUCTION_HINT),
    ("MSG_ENTER_HINT", MSG_ENTER_HINT),
    ("MSG_DOCK_HINT", MSG_DOCK_HINT),
    ("MSG_DO_MOVETO_HINT", MSG_DO_MOVETO_HINT),
    ("MSG_DO_ATTACKMOVETO_HINT", MSG_DO_ATTACKMOVETO_HINT),
    ("MSG_ADD_WAYPOINT_HINT", MSG_ADD_WAYPOINT_HINT),
    ("MSG_HIJACK_HINT", MSG_HIJACK_HINT),
    ("MSG_SABOTAGE_HINT", MSG_SABOTAGE_HINT),
    ("MSG_FIREBOMB_HINT", MSG_FIREBOMB_HINT),
    ("MSG_CONVERT_TO_CARBOMB_HINT", MSG_CONVERT_TO_CARBOMB_HINT),
    ("MSG_CAPTUREBUILDING_HINT", MSG_CAPTUREBUILDING_HINT),
    ("MSG_SNIPE_VEHICLE_HINT", MSG_SNIPE_VEHICLE_HINT),
    ("MSG_DEFECTOR_HINT", MSG_DEFECTOR_HINT),
    ("MSG_SET_RALLY_POINT_HINT", MSG_SET_RALLY_POINT_HINT),
    ("MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT", MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT),
    ("MSG_DO_SALVAGE_HINT", MSG_DO_SALVAGE_HINT),
    ("MSG_DO_INVALID_HINT", MSG_DO_INVALID_HINT),
    ("MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT", MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT),
    ("MSG_HACK_HINT", MSG_HACK_HINT),
    ("MSG_BEGIN_NETWORK_MESSAGES", MSG_BEGIN_NETWORK_MESSAGES),
    ("MSG_CREATE_SELECTED_GROUP", MSG_CREATE_SELECTED_GROUP),
    ("MSG_CREATE_SELECTED_GROUP_NO_SOUND", MSG_CREATE_SELECTED_GROUP_NO_SOUND),
    ("MSG_DESTROY_SELECTED_GROUP", MSG_DESTROY_SELECTED_GROUP),
    ("MSG_REMOVE_FROM_SELECTED_GROUP", MSG_REMOVE_FROM_SELECTED_GROUP),
    ("MSG_SELECTED_GROUP_COMMAND", MSG_SELECTED_GROUP_COMMAND),
    ("MSG_CREATE_TEAM0", MSG_CREATE_TEAM0),
    ("MSG_CREATE_TEAM1", MSG_CREATE_TEAM1),
    ("MSG_CREATE_TEAM2", MSG_CREATE_TEAM2),
    ("MSG_CREATE_TEAM3", MSG_CREATE_TEAM3),
    ("MSG_CREATE_TEAM4", MSG_CREATE_TEAM4),
    ("MSG_CREATE_TEAM5", MSG_CREATE_TEAM5),
    ("MSG_CREATE_TEAM6", MSG_CREATE_TEAM6),
    ("MSG_CREATE_TEAM7", MSG_CREATE_TEAM7),
    ("MSG_CREATE_TEAM8", MSG_CREATE_TEAM8),
    ("MSG_CREATE_TEAM9", MSG_CREATE_TEAM9),
    ("MSG_SELECT_TEAM0", MSG_SELECT_TEAM0),
    ("MSG_SELECT_TEAM1", MSG_SELECT_TEAM1),
    ("MSG_SELECT_TEAM2", MSG_SELECT_TEAM2),
    ("MSG_SELECT_TEAM3", MSG_SELECT_TEAM3),
    ("MSG_SELECT_TEAM4", MSG_SELECT_TEAM4),
    ("MSG_SELECT_TEAM5", MSG_SELECT_TEAM5),
    ("MSG_SELECT_TEAM6", MSG_SELECT_TEAM6),
    ("MSG_SELECT_TEAM7", MSG_SELECT_TEAM7),
    ("MSG_SELECT_TEAM8", MSG_SELECT_TEAM8),
    ("MSG_SELECT_TEAM9", MSG_SELECT_TEAM9),
    ("MSG_ADD_TEAM0", MSG_ADD_TEAM0),
    ("MSG_ADD_TEAM1", MSG_ADD_TEAM1),
    ("MSG_ADD_TEAM2", MSG_ADD_TEAM2),
    ("MSG_ADD_TEAM3", MSG_ADD_TEAM3),
    ("MSG_ADD_TEAM4", MSG_ADD_TEAM4),
    ("MSG_ADD_TEAM5", MSG_ADD_TEAM5),
    ("MSG_ADD_TEAM6", MSG_ADD_TEAM6),
    ("MSG_ADD_TEAM7", MSG_ADD_TEAM7),
    ("MSG_ADD_TEAM8", MSG_ADD_TEAM8),
    ("MSG_ADD_TEAM9", MSG_ADD_TEAM9),
    ("MSG_DO_ATTACKSQUAD", MSG_DO_ATTACKSQUAD),
    ("MSG_DO_WEAPON", MSG_DO_WEAPON),
    ("MSG_DO_WEAPON_AT_LOCATION", MSG_DO_WEAPON_AT_LOCATION),
    ("MSG_DO_WEAPON_AT_OBJECT", MSG_DO_WEAPON_AT_OBJECT),
    ("MSG_DO_SPECIAL_POWER", MSG_DO_SPECIAL_POWER),
    ("MSG_DO_SPECIAL_POWER_AT_LOCATION", MSG_DO_SPECIAL_POWER_AT_LOCATION),
    ("MSG_DO_SPECIAL_POWER_AT_OBJECT", MSG_DO_SPECIAL_POWER_AT_OBJECT),
    ("MSG_SET_RALLY_POINT", MSG_SET_RALLY_POINT),
    ("MSG_PURCHASE_SCIENCE", MSG_PURCHASE_SCIENCE),
    ("MSG_QUEUE_UPGRADE", MSG_QUEUE_UPGRADE),
    ("MSG_CANCEL_UPGRADE", MSG_CANCEL_UPGRADE),
    ("MSG_QUEUE_UNIT_CREATE", MSG_QUEUE_UNIT_CREATE),
    ("MSG_CANCEL_UNIT_CREATE", MSG_CANCEL_UNIT_CREATE),
    ("MSG_DOZER_CONSTRUCT", MSG_DOZER_CONSTRUCT),
    ("MSG_DOZER_CONSTRUCT_LINE", MSG_DOZER_CONSTRUCT_LINE),
    ("MSG_DOZER_CANCEL_CONSTRUCT", MSG_DOZER_CANCEL_CONSTRUCT),
    ("MSG_SELL", MSG_SELL),
    ("MSG_EXIT", MSG_EXIT),
    ("MSG_EVACUATE", MSG_EVACUATE),
    ("MSG_EXECUTE_RAILED_TRANSPORT", MSG_EXECUTE_RAILED_TRANSPORT),
    ("MSG_COMBATDROP_AT_LOCATION", MSG_COMBATDROP_AT_LOCATION),
    ("MSG_COMBATDROP_AT_OBJECT", MSG_COMBATDROP_AT_OBJECT),
    ("MSG_AREA_SELECTION", MSG_AREA_SELECTION),
    ("MSG_DO_ATTACK_OBJECT", MSG_DO_ATTACK_OBJECT),
    ("MSG_DO_FORCE_ATTACK_OBJECT", MSG_DO_FORCE_ATTACK_OBJECT),
    ("MSG_DO_FORCE_ATTACK_GROUND", MSG_DO_FORCE_ATTACK_GROUND),
    ("MSG_GET_REPAIRED", MSG_GET_REPAIRED),
    ("MSG_GET_HEALED", MSG_GET_HEALED),
    ("MSG_DO_REPAIR", MSG_DO_REPAIR),
    ("MSG_RESUME_CONSTRUCTION", MSG_RESUME_CONSTRUCTION),
    ("MSG_ENTER", MSG_ENTER),
    ("MSG_DOCK", MSG_DOCK),
    ("MSG_DO_MOVETO", MSG_DO_MOVETO),
    ("MSG_DO_ATTACKMOVETO", MSG_DO_ATTACKMOVETO),
    ("MSG_DO_FORCEMOVETO", MSG_DO_FORCEMOVETO),
    ("MSG_ADD_WAYPOINT", MSG_ADD_WAYPOINT),
    ("MSG_DO_GUARD_POSITION", MSG_DO_GUARD_POSITION),
    ("MSG_DO_GUARD_OBJECT", MSG_DO_GUARD_OBJECT),
    ("MSG_DO_STOP", MSG_DO_STOP),
    ("MSG_DO_SCATTER", MSG_DO_SCATTER),
    ("MSG_INTERNET_HACK", MSG_INTERNET_HACK),
    ("MSG_DO_CHEER", MSG_DO_CHEER),
    ("MSG_TOGGLE_OVERCHARGE", MSG_TOGGLE_OVERCHARGE),
    ("MSG_SWITCH_WEAPONS", MSG_SWITCH_WEAPONS),
    ("MSG_CONVERT_TO_CARBOMB", MSG_CONVERT_TO_CARBOMB),
    ("MSG_CAPTUREBUILDING", MSG_CAPTUREBUILDING),
    ("MSG_DISABLEVEHICLE_HACK", MSG_DISABLEVEHICLE_HACK),
    ("MSG_STEALCASH_HACK", MSG_STEALCASH_HACK),
    ("MSG_DISABLEBUILDING_HACK", MSG_DISABLEBUILDING_HACK),
    ("MSG_SNIPE_VEHICLE", MSG_SNIPE_VEHICLE),
    ("MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION", MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION),
    ("MSG_DO_SALVAGE", MSG_DO_SALVAGE),
    ("MSG_CLEAR_INGAME_POPUP_MESSAGE", MSG_CLEAR_INGAME_POPUP_MESSAGE),
    ("MSG_PLACE_BEACON", MSG_PLACE_BEACON),
    ("MSG_REMOVE_BEACON", MSG_REMOVE_BEACON),
    ("MSG_SET_BEACON_TEXT", MSG_SET_BEACON_TEXT),
    ("MSG_SET_REPLAY_CAMERA", MSG_SET_REPLAY_CAMERA),
    ("MSG_SELF_DESTRUCT", MSG_SELF_DESTRUCT),
    ("MSG_CREATE_FORMATION", MSG_CREATE_FORMATION),
    ("MSG_LOGIC_CRC", MSG_LOGIC_CRC),
    ("MSG_SET_MINE_CLEARING_DETAIL", MSG_SET_MINE_CLEARING_DETAIL),
    ("MSG_ENABLE_RETALIATION_MODE", MSG_ENABLE_RETALIATION_MODE),
    ("MSG_BEGIN_DEBUG_NETWORK_MESSAGES", MSG_BEGIN_DEBUG_NETWORK_MESSAGES),
    ("MSG_END_NETWORK_MESSAGES", MSG_END_NETWORK_MESSAGES),
    ("MSG_TIMESTAMP", MSG_TIMESTAMP),
    ("MSG_OBJECT_CREATED", MSG_OBJECT_CREATED),
    ("MSG_OBJECT_DESTROYED", MSG_OBJECT_DESTROYED),
    ("MSG_OBJECT_POSITION", MSG_OBJECT_POSITION),
    ("MSG_OBJECT_ORIENTATION", MSG_OBJECT_ORIENTATION),
    ("MSG_OBJECT_JOINED_TEAM", MSG_OBJECT_JOINED_TEAM),
    ("MSG_COUNT", MSG_COUNT),
];

/// The five values `getCommandTypeAsAsciiString` has no `CHECK_IF` for (the release build's list names the other 269):
/// they read as `"UnknownMessage"`.
pub const NOT_NAMED_BY_COMMAND_STRING: [MessageType; 5] = [
    MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION,
    MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT,
    MSG_IMPOSSIBLE_ATTACK_HINT,
    MSG_BEGIN_DEBUG_NETWORK_MESSAGES,
    MSG_COUNT,
];
