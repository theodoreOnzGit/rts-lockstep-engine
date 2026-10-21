/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h (layer 8), Module
** build only (differential tests, p1w3-modules): the frame counter and the
** one call UpdateModule::setWakeFrame makes, which records its arguments.
** The class is renamed by the wrapper's macros (module_prelude.h). */
#ifndef SHIM_MODULE_GAMELOGIC_H
#define SHIM_MODULE_GAMELOGIC_H
#include "Lib/BaseType.h"
class Object;
class UpdateModule;
#include "Common/GameType.h"
void modhelp_destroy(Object *obj);
Object *modhelp_find(ObjectID id);
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
	/* --- p1w10-sonnet --- (the layer-9 die batch: DestroyDie and UpgradeDie; both answered by l9_helpers.inc) */
	void destroyObject(Object *obj) { modhelp_destroy(obj); }   /* inline hooks: shim_updates' GameLogic has its own, out of line, destroyObject */
	Object *findObjectByID(ObjectID id) { return modhelp_find(id); }
	/* --- end p1w10-sonnet --- */
	UnsignedInt m_frame = 0;
	int m_awakenCalls = 0;
	Object *m_awakenObject = 0;
	UpdateModule *m_awakenModule = 0;
	UnsignedInt m_awakenFrame = 0;
};
extern GameLogic *TheGameLogic;
#endif
