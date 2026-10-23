/* Stand-in for GameEngine/Include/Common/Radar.h, Module build only (differential tests, p1w13-sonnet): the calls the crates make, answered by
** differential/l9_sabotage_r13.inc (batch R13: tryInfiltrationEvent) and differential/l9_crates_r13d.inc (batch R13d: removeObject, addObject). */
#ifndef SHIM_MODULE_RADAR_H
#define SHIM_MODULE_RADAR_H
class Object;
class Radar
{
public:
	void tryInfiltrationEvent(const Object *obj);
	void removeObject(Object *obj);   /* batch R13d */
	void addObject(Object *obj);   /* batch R13d */
};
extern Radar *TheRadar;
#endif
