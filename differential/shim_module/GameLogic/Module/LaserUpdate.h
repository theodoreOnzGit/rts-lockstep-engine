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
	/* batch R75: ParticleUplinkCannonUpdate's beams (differential/l9_uplink_r75.inc: the calls lines, the radius the test's) */
	void setDecayFrames( UnsignedInt decayFrames );
	Real getCurrentLaserRadius() const;
	Int m_r75Drawable = 0;   /* the client drawable it is on (0: a laser object's, batch R59) */
};
#endif
