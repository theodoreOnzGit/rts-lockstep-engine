/* Stand-in for GameEngine/Include/GameLogic/AI.h, Module build only (differential tests, p1w12-sonnet, ReplaceObjectUpgrade): TheAI->pathfinder() and
** the two calls that module makes on it (answered by differential/l9_replace_object.inc). Renamed ModTestAI and ModTestPathfinder by
** module_prelude.h. */
#ifndef SHIM_MODULE_AI_H
#define SHIM_MODULE_AI_H
#include "Lib/BaseType.h"
class Object;
class Pathfinder
{
public:
	void addObjectToPathfindMap(Object *obj);
	void removeObjectFromPathfindMap(Object *obj);
};
/* --- p1w12-sonnet --- (update batch N: CheckpointUpdate) the two queries it makes; defined by differential/l9_checkpoint_n.inc */
class AttackPriorityInfo;
class PartitionFilter;
/* --- end p1w12-sonnet --- */
class AI
{
public:
	/* --- p1w12-sonnet --- */
	Object *findClosestEnemy( const Object *me, Real range, UnsignedInt qualifiers, const AttackPriorityInfo *info = 0, PartitionFilter *optionalFilter = 0 );
	Object *findClosestAlly( const Object *me, Real range, UnsignedInt qualifiers );
	/* --- end p1w12-sonnet --- */
	Pathfinder *pathfinder() { return &m_pathfinder; }
	Pathfinder m_pathfinder;
};
extern AI *TheAI;
#endif
