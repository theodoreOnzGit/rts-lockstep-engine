/* Stand-in for Common/Radar.h in the sides/scripts build: only RadarEventType
** (Radar.h lines 64-84, the enumerators Parameter::getUiText names). */
#ifndef SHIM_SIDES_RADAR_H
#define SHIM_SIDES_RADAR_H
#include "GameClient/View.h" /* Scripts.cpp reaches View::SHAKE_COUNT through the real Radar.h chain */
enum RadarEventType
{
	RADAR_EVENT_INVALID = 0,
	RADAR_EVENT_CONSTRUCTION,
	RADAR_EVENT_UPGRADE,
	RADAR_EVENT_UNDER_ATTACK,
	RADAR_EVENT_INFORMATION,
	RADAR_EVENT_BEACON_PULSE,
	RADAR_EVENT_INFILTRATION,
	RADAR_EVENT_BATTLE_PLAN,
	RADAR_EVENT_STEALTH_DISCOVERED,
	RADAR_EVENT_STEALTH_NEUTRALIZED,
	RADAR_EVENT_FAKE,
	RADAR_EVENT_NUM_EVENTS
};
#endif
