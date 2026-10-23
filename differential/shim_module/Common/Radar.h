/* Stand-in for GameEngine/Include/Common/Radar.h, Module build only (differential tests, p1w13-sonnet, batch R13: the sabotage crates): the one call
** they make, answered by differential/l9_crates_f.inc. */
#ifndef SHIM_MODULE_RADAR_H
#define SHIM_MODULE_RADAR_H
class Object;
class Radar
{
public:
	void tryInfiltrationEvent(const Object *obj);
};
extern Radar *TheRadar;
#endif
