/* Stand-in for GameEngine/Include/Common/Radar.h, Module build only (differential tests, p1w13-sonnet): the calls the crates make, answered by
** differential/l9_sabotage_r13.inc (batch R13: tryInfiltrationEvent) and differential/l9_crates_r13d.inc (batch R13d: removeObject, addObject). */
#ifndef SHIM_MODULE_RADAR_H
#define SHIM_MODULE_RADAR_H
class Object;
struct Coord3D;
/* batch R24 (OverchargeBehavior): Radar.h lines 64-80, verbatim apart from the comments */
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
class Radar
{
public:
	void tryInfiltrationEvent(const Object *obj);
	void removeObject(Object *obj);   /* batch R13d */
	void addObject(Object *obj);   /* batch R13d */
	void createEvent( const Coord3D *world, RadarEventType type, float secondsToLive = 4.0f );   /* batch R24: differential/l9_overcharge_r24.inc */
	Bool tryEvent( RadarEventType event, const Coord3D *pos );   /* batch R27: differential/l9_stealth_detector_r27.inc (a line; the test's answer) */
	void refreshTerrain( class TerrainLogic *terrain );   /* batch R70: a line (differential/l9_wave_guide_r70.inc) */
};
extern Radar *TheRadar;
#endif
