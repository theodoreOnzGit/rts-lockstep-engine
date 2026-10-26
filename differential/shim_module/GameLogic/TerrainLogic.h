/* Stand-in for GameEngine/Include/GameLogic/TerrainLogic.h, Module build only (differential tests, p1w12-sonnet, update batch N): the one call OCLUpdate
** makes, answered by differential/l9_ocl_update_n.inc. Renamed ModTestTerrainLogic by module_prelude.h. */
#ifndef SHIM_MODULE_TERRAINLOGIC_H
#define SHIM_MODULE_TERRAINLOGIC_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"   /* PathfindLayerEnum (p1w13-sonnet) */
#include "Common/AsciiString.h"   /* (batch R70) */
#include "GameLogic/Module/BridgeBehavior.h"   /* BridgeTowerType (batch R77) */
/* --- p1w13-sonnet --- (batch R70: WaveGuideUpdate's waypoint path and bridges; differential/l9_wave_guide_r70.inc) TerrainLogic.h's Waypoint getters,
** verbatim, on the test's nodes; the bridge's info's two ends (the rest of BridgeInfo is unused) */
class Waypoint
{
public:
	enum {MAX_LINKS=8};
	Int getNumLinks(void) const {return m_numLinks; }
	Waypoint *getLink(Int ndx) const {if (ndx>=0 && ndx <= MAX_LINKS) return m_links[ndx]; return NULL; }
	const Coord3D *getLocation(void) const { return &m_location; }
	Int getID(void) const { return m_id; }
	Int m_id;
	Coord3D m_location;
	Waypoint *m_links[MAX_LINKS + 1];
	Int m_numLinks;
};
class BridgeInfo { public: Coord3D from, to;
	Coord3D fromLeft, fromRight, toLeft, toRight;   /* batch R77: BridgeBehavior's corners */ };
/* batch R77: what BridgeBehavior reads of a bridge (TerrainLogic.h lines 189, 200-201, verbatim), its template name and layer the test's; the
** object and tower ids it sets are lines (differential/l9_bridge_r77.inc) */
class Bridge { public: void getBridgeInfo(class BridgeInfo *pInfo) {*pInfo = m_bridgeInfo; } BridgeInfo m_bridgeInfo; Int m_cell;
	AsciiString getBridgeTemplateName( void ) { return m_templateName; }
	inline const BridgeInfo *peekBridgeInfo(void) const {return &m_bridgeInfo;}
	inline PathfindLayerEnum getLayer(void) const {return m_layer;}
	void setBridgeObjectID( ObjectID id );
	void setTowerObjectID( ObjectID id, BridgeTowerType which );
	AsciiString m_templateName; PathfindLayerEnum m_layer; };
/* --- end p1w13-sonnet --- */
class TerrainLogic
{
public:
	/* --- p1w13-sonnet --- (batch R70: the test's) */
	Waypoint *getWaypointByName( AsciiString name );
	Bridge *findBridgeAt(const Coord3D *pLoc) const;
	void deleteBridge( Bridge *bridge );
	/* --- end p1w13-sonnet --- */
	Coord3D findClosestEdgePoint( const Coord3D *closestTo ) const;
	Coord3D findFarthestEdgePoint( const Coord3D *farthestFrom ) const;   /* p1w13-sonnet, batch R22: OCLSpecialPower (differential/l9_ocl_power_r22.inc) */
	/* --- p1w13-sonnet --- (update batch O: HeightDieUpdate; differential/l9_height_die_o.inc) */
	Real getGroundHeight( Real x, Real y, Coord3D* normal = NULL ) const;
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D* normal = NULL, Bool clip = true) const;
	PathfindLayerEnum getHighestLayerForDestination(const Coord3D *pos, Bool onlyHealthyBridges = false);
	Bool isUnderwater( Real x, Real y, Real *waterZ = NULL, Real *terrainZ = NULL );   /* update batch O: FloatUpdate */
	Bool isCliffCell( Real x, Real y ) const;   /* batch R41: GenerateMinefieldBehavior (differential/l9_gen_minefield_r41.inc) */
	PathfindLayerEnum getLayerForDestination(const Coord3D *pos);   /* batch R: OpenContain */
	void getExtentIncludingBorder( struct Region3D *extent ) const;   /* batch R72: SpectreGunshipUpdate (the test's) */
	void updateBridgeDamageStates( void );   /* batch R77: BridgeBehavior (a line, differential/l9_bridge_r77.inc) */
	Waypoint *getWaypointByID( UnsignedInt id );   /* batch R75: ParticleUplinkCannonUpdate (the test's nodes, differential/l9_uplink_r75.inc) */
	/* --- end p1w13-sonnet --- */
};
extern TerrainLogic *TheTerrainLogic;
#endif
