/* Stand-in for GameEngine/Include/GameLogic/Module/StealthUpdate.h, Module build only (differential tests, p1w13-sonnet, batch R): the one call OpenContain
** makes on the stealth update of a contained object (differential/l9_open_contain_r.inc logs it). */
#ifndef SHIM_MODULE_STEALTHUPDATE_H
#define SHIM_MODULE_STEALTHUPDATE_H
#include "Lib/BaseType.h"
class StealthUpdate
{
public:
	void markAsDetected( UnsignedInt numFrames = 0 );
	Object *m_owner = 0;
};
#endif
