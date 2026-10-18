/* Stand-in for GameEngine/Include/GameLogic/PartitionManager.h, ThingTemplate
** build only (differential tests, p1w4-thingtemplate): newObject registers
** the object with it. (Renamed Tt* by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_PARTITIONMANAGER_H
#define SHIM_TT_PARTITIONMANAGER_H
#include "GameLogic/Object.h"
class PartitionManager
{
public:
	void registerObject(Object *obj) { m_registered = obj; }
	Object *m_registered;
};
extern PartitionManager *ThePartitionManager;
#endif
