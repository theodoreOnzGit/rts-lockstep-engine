/* Stand-ins for the pathfinder build (differential tests, p1w13-sonnet, layer 10; differential/wrap_pathfind.cpp): what AIPathfind.cpp's
** pieces under test reach outside the pathfinder. Each is renamed by PreRTS.h. The definitions are the wrapper's.
**   Object        its id and its layer (Path::computePointOnPath reads obj->getLayer())
**   TerrainLogic  getLayerForDestination (Path::updateLastNode)
**   AI            pathfinder(): the Pathfinder under test (a block of its size while only its pieces are compiled)
**   GameLogic     the frame */
#ifndef SHIM_PATHFIND_WORLD_H
#define SHIM_PATHFIND_WORLD_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"
class Object
{
public:
	ObjectID getID() const { return m_id; }
	PathfindLayerEnum getLayer() const { return m_layer; }
	ObjectID m_id;
	PathfindLayerEnum m_layer;
};
class Pathfinder;
class AI { public: Pathfinder *pathfinder(); };
extern AI *TheAI;
class TerrainLogic { public: PathfindLayerEnum getLayerForDestination(const Coord3D *pos); };
extern TerrainLogic *TheTerrainLogic;
class GameLogic { public: UnsignedInt getFrame() { return m_frame; } UnsignedInt m_frame; };
extern GameLogic *TheGameLogic;
#endif
