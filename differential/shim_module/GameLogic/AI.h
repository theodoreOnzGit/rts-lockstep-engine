/* Stand-in for GameEngine/Include/GameLogic/AI.h, Module build only (differential tests, p1w12-sonnet, ReplaceObjectUpgrade): TheAI->pathfinder() and
** the two calls that module makes on it (answered by differential/l9_replace_object.inc). Renamed ModTestAI and ModTestPathfinder by
** module_prelude.h. */
#ifndef SHIM_MODULE_AI_H
#define SHIM_MODULE_AI_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"   /* PathfindLayerEnum (p1w13-sonnet) */
class Object;
/* --- p1w13-sonnet --- (batch R2: TransportContain) */
class Locomotor;
enum AttitudeType { AI_SLEEP = -2, AI_PASSIVE=-1, AI_NORMAL=0, AI_ALERT=1, AI_AGGRESSIVE=2, AI_INVALID=3 };		///< AI "attitude" behavior modifiers
/* --- end p1w13-sonnet --- */
/* --- p1w13-sonnet --- (batch R8: ParachuteContain reads a cell's type; AIPathfind.h lines 257-266 and 439-440, verbatim) */
class PathfindCell
{
public:
	enum CellType
	{
		CELL_CLEAR		= 0x00,									///< clear, unobstructed ground
		CELL_WATER		= 0x01,									///< water area
		CELL_CLIFF		= 0x02,									///< steep altitude change
		CELL_RUBBLE		= 0x03,									///< Cell is occupied by rubble.
		CELL_OBSTACLE	= 0x04,									///< Occupied by a structure
		CELL_BRIDGE_IMPASSABLE = 0x05,				///< Piece of a bridge that is impassable.
		CELL_IMPASSABLE = 0x06								///< Just plain impassable except for aircraft.
	};
	CellType getType() const { return m_type; }
	CellType m_type;
};
#define PATHFIND_CELL_SIZE		10
#define PATHFIND_CELL_SIZE_F	10.0f
/* --- end p1w13-sonnet --- */
class Pathfinder
{
public:
	void addObjectToPathfindMap(Object *obj);
	void removeObjectFromPathfindMap(Object *obj);
	/* --- p1w13-sonnet --- (batch R: OpenContain) */
	void removeUnitFromPathfindMap(Object *obj);
	PathfindCell *getCell( PathfindLayerEnum layer, Int x, Int y );   /* batch R8 */
	void updatePos(Object *obj, const Coord3D *newPos);
	void updateGoal(Object *obj, const Coord3D *newGoal, PathfindLayerEnum layer);
	Bool adjustToPossibleDestination(Object *obj, const class LocomotorSet *locomotorSet, Coord3D *dest);
	Bool adjustDestination(Object *obj, const class LocomotorSet *locomotorSet, Coord3D *dest, const Coord3D *groupDest = NULL);   /* batch R18: the production exits (differential/l9_exits_r18.inc) */
	void snapPosition(Object *obj, Coord3D *pos);   /* batch R18 */
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	Bool validMovementTerrain( PathfindLayerEnum layer, const Locomotor* locomotor, const Coord3D *pos );
	void createAWallFromMyFootprint( Object *obj );   /* batch R34: TensileFormationUpdate (lines, differential/l9_tensile_r34.inc) */
	void removeWallFromMyFootprint( Object *obj );
	/* --- end p1w13-sonnet --- */
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
	struct AIData { Bool m_enableRepulsors; Real m_maxRetaliateDistance; Real m_retaliateFriendsRadius;
		Real m_attackPriorityDistanceModifier;   /* batch R69: CommandButtonHuntUpdate */ };
	const AIData *getAiData() const { return &m_aiData; }
	AIData m_aiData;
	/* --- end p1w13-sonnet --- */
};
extern AI *TheAI;
#endif
