/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h (layer 8), p1l9-updates build only (differential/wrap_l9updates.cpp):
** differential/shim_module/GameLogic/GameLogic.h with the same members in the same order, plus the three calls the l9 update
** modules make that the module build does not need: destroyObject and getHulkMaxLifetimeOverride (defined in
** wrap_l9updates.cpp). No data member is added. (p1w13-sonnet, batch R26: the module build's wake hook is here too, at the same place, so
** that the one inline friend_awakenUpdateModule the two builds share reads the same layout.) */
#ifndef SHIM_UPDATES_GAMELOGIC_H
#define SHIM_UPDATES_GAMELOGIC_H
#include "Lib/BaseType.h"
class Object;
class UpdateModule;
class GameLogic
{
public:
	UnsignedInt getFrame() const { return m_frame; }
	void friend_awakenUpdateModule(Object *obj, UpdateModule *u, UnsignedInt whenToWakeUp)
	{
		if (m_awakenHook) m_awakenHook(obj, u, whenToWakeUp);   /* p1w13-sonnet, batch R26: a driver that applies each call */
		m_awakenCalls++;
		m_awakenObject = obj;
		m_awakenModule = u;
		m_awakenFrame = whenToWakeUp;
	}
	/* p1l9-updates: defined in wrap_l9updates.cpp */
	void destroyObject(Object *obj);
	Int getHulkMaxLifetimeOverride() const;
	UnsignedInt m_frame = 0;
	int m_awakenCalls = 0;
	Object *m_awakenObject = 0;
	UpdateModule *m_awakenModule = 0;
	UnsignedInt m_awakenFrame = 0;
	void (*m_awakenHook)( Object *obj, UpdateModule *u, UnsignedInt whenToWakeUp ) = 0;   /* batch R26 (see above) */
};
extern GameLogic *TheGameLogic;
#endif
