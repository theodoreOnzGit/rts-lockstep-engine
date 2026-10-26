/* Stand-ins for the pathfinder build (differential tests, p1w13-sonnet, layer 10; differential/wrap_pathfind.cpp): what AIPathfind.cpp's
** pieces under test reach outside the pathfinder. Each is renamed by PreRTS.h. The definitions are the wrapper's.
**   Object        its id and its layer (Path::computePointOnPath reads obj->getLayer()); an obstacle's body and kind (batch P2)
**   TerrainLogic  getLayerForDestination (Path::updateLastNode)
**   AI            pathfinder(): the Pathfinder under test (a block of its size while only its pieces are compiled)
**   GameLogic     the frame */
#ifndef SHIM_PATHFIND_WORLD_H
#define SHIM_PATHFIND_WORLD_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"
/* batch P2: the body PathfindCell::setTypeAsObstacle asks for its damage state (rubble or not) */
class BodyModuleInterface
{
public:
	BodyDamageType getDamageState() const { return m_state; }
	BodyDamageType m_state;
};
class Object
{
public:
	Object() : m_id(INVALID_ID), m_layer(LAYER_GROUND), m_rubble(false), m_seeThrough(false) { m_body.m_state = BODY_PRISTINE; }
	ObjectID getID() const { return m_id; }
	PathfindLayerEnum getLayer() const { return m_layer; }
	/* batch P2: an obstacle's body (rubble or pristine) and KINDOF_CAN_SEE_THROUGH_STRUCTURE */
	BodyModuleInterface *getBodyModule() const { m_body.m_state = m_rubble ? BODY_RUBBLE : BODY_PRISTINE; return &m_body; }
	Bool isKindOf(KindOfType t) const { return t == KINDOF_CAN_SEE_THROUGH_STRUCTURE && m_seeThrough; }
	ObjectID m_id;
	PathfindLayerEnum m_layer;
	bool m_rubble, m_seeThrough;
	mutable BodyModuleInterface m_body;
};
class Pathfinder;
class AI { public: Pathfinder *pathfinder(); };
extern AI *TheAI;
class TerrainLogic { public: PathfindLayerEnum getLayerForDestination(const Coord3D *pos); };
extern TerrainLogic *TheTerrainLogic;
class GameLogic { public: UnsignedInt getFrame() { return m_frame; } UnsignedInt m_frame; };
extern GameLogic *TheGameLogic;
#endif
