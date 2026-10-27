/* Stand-ins for the pathfinder build (differential tests, p1w13-sonnet, layer 10; differential/wrap_pathfind.cpp): what AIPathfind.cpp's
** pieces under test reach outside the pathfinder. Each is renamed by PreRTS.h. The definitions are the wrapper's.
**   Object        its id and its layer (Path::computePointOnPath reads obj->getLayer()); an obstacle's body and kind (batch P2)
**   TerrainLogic  getLayerForDestination (Path::updateLastNode)
**   AI            pathfinder(): the Pathfinder under test (a block of its size while only its pieces are compiled)
**   GameLogic     the frame; findObjectByID (batch P3: the wall's pieces)
**   Bridge        (batch P3) a bridge's bounds, ends and object, which points and cells are on it, its height
**   GeometryInfo  (batch P3) a wall piece's 2-D bounds, radii and shape; (P4) a footprint's shape, smallness, bounding circle
** Batch P4 (the Pathfinder's map, footprints and units' cells): the objects' kinds, template (fence), mobility, height, layers,
** AI (AIUpdateInterface: ground movement, goal and position cells), unmanned state and damage (DamageInfo); the terrain's cliffs,
** water, extent and bridge interactions; the AI's data (TAiData: the wall height); the game logic's object list. */
#ifndef SHIM_PATHFIND_WORLD_H
#define SHIM_PATHFIND_WORLD_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"
/* batch P2: the body PathfindCell::setTypeAsObstacle asks for its damage state (rubble or not) */
/* batch P3: a wall piece's geometry (its 2-D bounds the position plus and minus the major radius) */
/* (Geometry.h's GeometryType values, which this build does not include) */
enum { GEOMETRY_SPHERE = 0, GEOMETRY_CYLINDER = 1, GEOMETRY_BOX = 2 };
class GeometryInfo
{
public:
	void get2DBounds(const Coord3D &pos, Real angle, Region2D &b) const
	{
		b.lo.x = pos.x - m_major; b.lo.y = pos.y - m_major; b.hi.x = pos.x + m_major; b.hi.y = pos.y + m_major;
	}
	Real getMajorRadius() const { return m_major; }
	Real getMinorRadius() const { return m_minor; }
	int getGeomType() const { return m_sphere ? GEOMETRY_SPHERE : m_type; }
	/* batch P4 */
	Bool getIsSmall() const { return m_small; }
	Real getBoundingCircleRadius() const { return m_boundingRadius; }
	GeometryInfo() : m_major(0), m_minor(0), m_sphere(false), m_type(GEOMETRY_BOX), m_small(false), m_boundingRadius(0) {}
	Real m_major, m_minor;
	bool m_sphere;
	int m_type;
	bool m_small;
	Real m_boundingRadius;
};
/* batch P4: Damage.h's values the wall's splat uses (DAMAGE_FALLING, DEATH_SPLATTED, HUGE_DAMAGE_AMOUNT), and the info */
enum { DAMAGE_FALLING = 18 };
enum { DEATH_SPLATTED = 11 };
const Real HUGE_DAMAGE_AMOUNT = 999999.0f;
class DamageInfo
{
public:
	DamageInfo() { in.m_damageType = 0; in.m_deathType = 0; in.m_sourceID = INVALID_ID; in.m_amount = 0; }
	struct { int m_damageType; int m_deathType; ObjectID m_sourceID; Real m_amount; } in;
};
/* batch P4: a template's fence */
class ThingTemplate
{
public:
	Real getFenceWidth() const { return m_fenceWidth; }
	Real getFenceXOffset() const { return m_fenceXOffset; }
	Real m_fenceWidth, m_fenceXOffset;
};
/* batch P4: a unit's AI */
class AIUpdateInterface
{
public:
	Bool isDoingGroundMovement() const { return m_ground; }
	Bool isAircraftThatAdjustsDestination() const { return m_adjusts; }
	const ICoord2D *getPathfindGoalCell() const { return &m_goal; }
	void setPathfindGoalCell(const ICoord2D &c) { m_goal = c; }
	const ICoord2D *getCurPathfindCell() const { return &m_cur; }
	void setCurPathfindCell(const ICoord2D &c) { m_cur = c; }
	bool m_ground, m_adjusts;
	ICoord2D m_goal, m_cur;
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
	Object() : m_id(INVALID_ID), m_layer(LAYER_GROUND), m_rubble(false), m_seeThrough(false)
	{
		m_body.m_state = BODY_PRISTINE;
		m_pos.x = m_pos.y = m_pos.z = 0;
		m_angle = 0;
		memset(m_kinds, 0, sizeof m_kinds);
		m_template.m_fenceWidth = m_template.m_fenceXOffset = 0;
		m_mobile = m_unmanned = m_hasAI = false;
		m_heightAboveTerrain = 0;
		m_destLayer = LAYER_GROUND;
		m_ai.m_ground = m_ai.m_adjusts = false;
		m_ai.m_goal.x = m_ai.m_goal.y = m_ai.m_cur.x = m_ai.m_cur.y = -1;
		m_next = NULL;
	}
	ObjectID getID() const { return m_id; }
	PathfindLayerEnum getLayer() const { return m_layer; }
	/* batch P2: an obstacle's body (rubble or pristine) and KINDOF_CAN_SEE_THROUGH_STRUCTURE */
	BodyModuleInterface *getBodyModule() const { m_body.m_state = m_rubble ? BODY_RUBBLE : BODY_PRISTINE; return &m_body; }
	Bool isKindOf(KindOfType t) const { return m_kinds[t] || (t == KINDOF_CAN_SEE_THROUGH_STRUCTURE && m_seeThrough); }
	/* batch P4 */
	const ThingTemplate *getTemplate() const { return &m_template; }
	Bool isMobile() const { return m_mobile; }
	Real getHeightAboveTerrain() const { return m_heightAboveTerrain; }
	void setLayer(PathfindLayerEnum l) { m_layer = l; }
	PathfindLayerEnum getDestinationLayer() const { return m_destLayer; }
	void setDestinationLayer(PathfindLayerEnum l) { m_destLayer = l; }
	Bool isDisabledByType(DisabledType t) const { return t == DISABLED_UNMANNED && m_unmanned; }
	AIUpdateInterface *getAIUpdateInterface() { return m_hasAI ? &m_ai : NULL; }
	void attemptDamage(DamageInfo *info);
	Object *getNextObject() { return m_next; }
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
	/* batch P4 */
	bool m_kinds[KINDOF_COUNT];
	ThingTemplate m_template;
	bool m_mobile, m_unmanned, m_hasAI;
	Real m_heightAboveTerrain;
	PathfindLayerEnum m_destLayer;
	AIUpdateInterface m_ai;
	Object *m_next;
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
/* batch P4: the AI's data (the wall height) */
class TAiData { public: Real m_wallHeight; };
class AI { public: Pathfinder *pathfinder(); const TAiData *getAiData(); };
extern AI *TheAI;
class TerrainLogic
{
public:
	PathfindLayerEnum getLayerForDestination(const Coord3D *pos);
	/* batch P3: the ground's height under a bridge */
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D *normal = NULL, Bool clip = true);
	/* batch P4: the map's cells and extent, the bridges' interactions */
	Bool isUnderwater(Real x, Real y, Real *waterZ = NULL, Real *terrainZ = NULL);
	Bool isCliffCell(Real x, Real y) const;
	void getMaximumPathfindExtent(Region3D *extent) const;
	Bool objectInteractsWithBridgeLayer(Object *obj, Int layer, Bool considerBridgeHealth = true) const;
	Bool objectInteractsWithBridgeEnd(Object *obj, Int layer) const;
};
extern TerrainLogic *TheTerrainLogic;
class GameLogic
{
public:
	UnsignedInt getFrame() { return m_frame; }
	/* batch P3: the wall's pieces */
	Object *findObjectByID(ObjectID id);
	/* batch P4: the object list */
	Object *getFirstObject();
	UnsignedInt m_frame;
};
extern GameLogic *TheGameLogic;
#endif
