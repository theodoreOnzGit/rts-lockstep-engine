/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h (layer 8), Module
** build only (differential tests, p1w3-modules): the frame counter and the
** one call UpdateModule::setWakeFrame makes, which records its arguments.
** The class is renamed by the wrapper's macros (module_prelude.h). */
#ifndef SHIM_MODULE_GAMELOGIC_H
#define SHIM_MODULE_GAMELOGIC_H
#include "Lib/BaseType.h"
class Object;
class UpdateModule;
class GameLogic
{
public:
	UnsignedInt getFrame() const { return m_frame; }
	void friend_awakenUpdateModule(Object *obj, UpdateModule *u, UnsignedInt whenToWakeUp)
	{
		m_awakenCalls++;
		m_awakenObject = obj;
		m_awakenModule = u;
		m_awakenFrame = whenToWakeUp;
	}
	UnsignedInt m_frame = 0;
	int m_awakenCalls = 0;
	Object *m_awakenObject = 0;
	UpdateModule *m_awakenModule = 0;
	UnsignedInt m_awakenFrame = 0;
};
extern GameLogic *TheGameLogic;
#endif
