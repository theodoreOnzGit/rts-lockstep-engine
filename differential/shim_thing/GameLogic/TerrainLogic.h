/* Stand-in for GameEngine/Include/GameLogic/TerrainLogic.h (layer 6, not
** translated), Thing tests only: the three members Thing.cpp calls, with
** upstream's signatures. The definitions are in wrap_thing.cpp, which
** renames the class and the singleton (TheTerrainLogic). */
#ifndef SHIM_THING_TERRAINLOGIC_H
#define SHIM_THING_TERRAINLOGIC_H
#include "Lib/BaseType.h"
#include "Common/KindOf.h"
#include "WWMath/Matrix3D.h"
class TerrainLogic
{
public:
	Real getGroundHeight(Real x, Real y, Coord3D* normal = NULL) const;
	Int alignOnTerrain(Real angle, const Coord3D& pos, Bool stickToGround, Matrix3D& mtx);
	Bool isUnderwater(Real x, Real y, Real *waterZ = NULL, Real *terrainZ = NULL);
};
extern TerrainLogic *TheTerrainLogic;
#endif
