/* Stand-ins for the pathfinder build (differential tests, p1w13-sonnet, layer 10; differential/wrap_pathfind.cpp): what AIPathfind.cpp's
** pieces under test reach outside the pathfinder. Each is renamed by PreRTS.h. The definitions are the wrapper's.
**   Object        its id and its layer (Path::computePointOnPath reads obj->getLayer()); an obstacle's body and kind (batch P2)
**   TerrainLogic  getLayerForDestination (Path::updateLastNode)
**   AI            pathfinder(): the Pathfinder under test (a block of its size while only its pieces are compiled)
**   GameLogic     the frame; findObjectByID (batch P3: the wall's pieces)
**   Bridge        (batch P3) a bridge's bounds, ends and object, which points and cells are on it, its height
**   GeometryInfo  (batch P3) a wall piece's 2-D bounds, radii and shape */
#ifndef SHIM_PATHFIND_WORLD_H
#define SHIM_PATHFIND_WORLD_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"
/* batch P2: the body PathfindCell::setTypeAsObstacle asks for its damage state (rubble or not) */
/* batch P3: a wall piece's geometry (its 2-D bounds the position plus and minus the major radius) */
/* (Geometry.h's GeometryType values, which this build does not include) */
enum { GEOMETRY_SPHERE = 0, GEOMETRY_BOX = 2 };
class GeometryInfo
{
public:
	void get2DBounds(const Coord3D &pos, Real angle, Region2D &b) const
	{
		b.lo.x = pos.x - m_major; b.lo.y = pos.y - m_major; b.hi.x = pos.x + m_major; b.hi.y = pos.y + m_major;
	}
	Real getMajorRadius() const { return m_major; }
	Real getMinorRadius() const { return m_minor; }
	int getGeomType() const { return m_sphere ? GEOMETRY_SPHERE : GEOMETRY_BOX; }
	Real m_major, m_minor;
	bool m_sphere;
};
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
	/* batch P3: a wall piece's place and geometry */
	const Coord3D *getPosition() const { return &m_pos; }
	Real getOrientation() const { return m_angle; }
	const GeometryInfo &getGeometryInfo() const { return m_geom; }
	ObjectID m_id;
	PathfindLayerEnum m_layer;
	bool m_rubble, m_seeThrough;
	mutable BodyModuleInterface m_body;
	Coord3D m_pos;
	Real m_angle;
	GeometryInfo m_geom;
};
/* batch P3: a bridge, its answers the wrapper's */
class BridgeInfo { public: Coord3D from, to; ObjectID bridgeObjectID; };
class Bridge
{
public:
	const Region2D *getBounds() const { return &m_bounds; }
	void getBridgeInfo(BridgeInfo *p) { *p = m_info; }
	const BridgeInfo *peekBridgeInfo() const { return &m_info; }
	Bool isPointOnBridge(const Coord3D *p);
	Bool isCellOnEnd(const Region2D *c);
	Bool isCellOnSide(const Region2D *c);
	Bool isCellEntryPoint(const Region2D *c);
	Real getBridgeHeight(const Coord3D *p, Coord3D *normal);
	Region2D m_bounds;
	BridgeInfo m_info;
	unsigned m_index;
};
class Pathfinder;
class AI { public: Pathfinder *pathfinder(); };
extern AI *TheAI;
class TerrainLogic
{
public:
	PathfindLayerEnum getLayerForDestination(const Coord3D *pos);
	/* batch P3: the ground's height under a bridge */
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D *normal = NULL, Bool clip = true);
};
extern TerrainLogic *TheTerrainLogic;
class GameLogic
{
public:
	UnsignedInt getFrame() { return m_frame; }
	/* batch P3: the wall's pieces */
	Object *findObjectByID(ObjectID id);
	UnsignedInt m_frame;
};
extern GameLogic *TheGameLogic;
#endif
