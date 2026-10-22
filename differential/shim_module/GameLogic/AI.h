/* Stand-in for GameEngine/Include/GameLogic/AI.h, Module build only (differential tests, p1w12-sonnet, ReplaceObjectUpgrade): TheAI->pathfinder() and
** the two calls that module makes on it (answered by differential/l9_replace_object.inc). Renamed ModTestAI and ModTestPathfinder by
** module_prelude.h. */
#ifndef SHIM_MODULE_AI_H
#define SHIM_MODULE_AI_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"   /* PathfindLayerEnum (p1w13-sonnet) */
class Object;
class Pathfinder
{
public:
	void addObjectToPathfindMap(Object *obj);
	void removeObjectFromPathfindMap(Object *obj);
	/* --- p1w13-sonnet --- (batch R: OpenContain) */
	void removeUnitFromPathfindMap(Object *obj);
	void updatePos(Object *obj, const Coord3D *newPos);
	void updateGoal(Object *obj, const Coord3D *newGoal, PathfindLayerEnum layer);
	Bool adjustToPossibleDestination(Object *obj, const class LocomotorSet *locomotorSet, Coord3D *dest);
	/* --- end p1w13-sonnet --- */
};
/* --- p1w12-sonnet --- (update batch N: CheckpointUpdate) the two queries it makes; defined by differential/l9_checkpoint_n.inc */
class AttackPriorityInfo;
class PartitionFilter;
/* --- end p1w12-sonnet --- */
class AI
{
public:
	/* --- p1w13-sonnet --- (AI.h lines 253-261, verbatim; update batch O: EnemyNearUpdate) */
	enum
	{
		CAN_SEE														=	1 << 0,
		CAN_ATTACK												= 1 << 1,
		IGNORE_INSIGNIFICANT_BUILDINGS		= 1 << 2,
		ATTACK_BUILDINGS									= 1 << 3,
		WITHIN_ATTACK_RANGE								= 1 << 4,
		UNFOGGED													= 1 << 5
	};
	/* --- end p1w13-sonnet --- */
	/* --- p1w12-sonnet --- */
	Object *findClosestEnemy( const Object *me, Real range, UnsignedInt qualifiers, const AttackPriorityInfo *info = 0, PartitionFilter *optionalFilter = 0 );
	Object *findClosestAlly( const Object *me, Real range, UnsignedInt qualifiers );
	/* --- end p1w12-sonnet --- */
	Pathfinder *pathfinder() { return &m_pathfinder; }
	Pathfinder m_pathfinder;
	/* --- p1w13-sonnet --- (batch P: ActiveBody reads three fields of the AI data) */
	struct AIData { Bool m_enableRepulsors; Real m_maxRetaliateDistance; Real m_retaliateFriendsRadius; };
	const AIData *getAiData() const { return &m_aiData; }
	AIData m_aiData;
	/* --- end p1w13-sonnet --- */
};
extern AI *TheAI;
#endif
