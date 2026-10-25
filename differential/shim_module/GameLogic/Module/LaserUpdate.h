/* Stand-in for GameEngine/Include/GameLogic/Module/LaserUpdate.h, Module build only (differential tests, p1w13-sonnet, batch R59):
** AssistedTargetingUpdate.cpp tells a laser object's LaserUpdate client module its ends; the call is a line (differential/l9_assisted_r59.inc). */
#ifndef SHIM_MODULE_LASERUPDATE_H
#define SHIM_MODULE_LASERUPDATE_H
#include "Common/AsciiString.h"
#include "Lib/BaseType.h"
class Object;
class LaserUpdate
{
public:
	void initLaser( const Object *parent, const Object *target, const Coord3D *startPos, const Coord3D *endPos, AsciiString parentBoneName, Int sizeDeltaFrames = 0 );
	Object *m_r59Owner = 0;   /* the laser object, for the line */
};
#endif
