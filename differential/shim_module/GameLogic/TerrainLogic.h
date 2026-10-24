/* Stand-in for GameEngine/Include/GameLogic/TerrainLogic.h, Module build only (differential tests, p1w12-sonnet, update batch N): the one call OCLUpdate
** makes, answered by differential/l9_ocl_update_n.inc. Renamed ModTestTerrainLogic by module_prelude.h. */
#ifndef SHIM_MODULE_TERRAINLOGIC_H
#define SHIM_MODULE_TERRAINLOGIC_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"   /* PathfindLayerEnum (p1w13-sonnet) */
class TerrainLogic
{
public:
	Coord3D findClosestEdgePoint( const Coord3D *closestTo ) const;
	Coord3D findFarthestEdgePoint( const Coord3D *farthestFrom ) const;   /* p1w13-sonnet, batch R22: OCLSpecialPower (differential/l9_ocl_power_r22.inc) */
	/* --- p1w13-sonnet --- (update batch O: HeightDieUpdate; differential/l9_height_die_o.inc) */
	Real getGroundHeight( Real x, Real y, Coord3D* normal = NULL ) const;
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D* normal = NULL, Bool clip = true) const;
	PathfindLayerEnum getHighestLayerForDestination(const Coord3D *pos, Bool onlyHealthyBridges = false);
	Bool isUnderwater( Real x, Real y, Real *waterZ = NULL, Real *terrainZ = NULL );   /* update batch O: FloatUpdate */
	Bool isCliffCell( Real x, Real y ) const;   /* batch R41: GenerateMinefieldBehavior (differential/l9_gen_minefield_r41.inc) */
	PathfindLayerEnum getLayerForDestination(const Coord3D *pos);   /* batch R: OpenContain */
	/* --- end p1w13-sonnet --- */
};
extern TerrainLogic *TheTerrainLogic;
#endif
