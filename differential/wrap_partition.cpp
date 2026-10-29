/* Differential-test wrapper for PartitionManager (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SimpleObjectIterator.cpp
**   GeneralsMD/Code/GameEngine/Source/GameClient/Line2D.cpp
** (#included below; PartitionManager.cpp through a copy that build.rs writes with ONE mechanical
** edit, and PartitionManager.h through a copy with two, see build.rs, p1w6-partition; its other
** headers are this build's, see differential/shim_partition).
**
** The edits, all in OUT_DIR/partition_gen and each asserted to match once:
**  - PartitionManager.h, PartitionData::wasSeenByAnyPlayers: `for (Int i=0; ...) ...; return i;`
**    uses the loop variable after the loop (MSVC 6 leaked it): `Int i;` moves before the loop.
**  - PartitionManager.h: the eight `friend void hLine...(...)` declarations are removed. MSVC made a
**    friend declaration visible to ordinary lookup; clang does not, and the functions are `static`
**    at the end of the .cpp. Their friendship only gave them access to protected members, which
**    `#define protected public` (below, around the header) gives them anyway.
**  - PartitionManager.cpp: eight `static void hLine...(Int, Int, Int, void *);` declarations are
**    added after `static int cellValueProc(...);`, so the uses above the definitions find them.
**
** What is real here: PartitionManager.cpp and .h (all of it: the grid, the cell and object
** records, the contact list, the rasterisers, the shroud, the threat/value maps, the closest-object
** search, the filters, findPositionAround, crc/xfer); SimpleObjectIterator.cpp (renamed
** PMSimpleObjectIterator, because its sort asks the Object for a build cost); Line2D.cpp
** (PointInsideArea2D); the real DiscreteCircle (upstream_gameengine), GeometryInfo (upstream_geometry),
** XferCRC (upstream_xfer), BitFlags (KindOfMaskType, ObjectStatusMaskType), WWMath Vector3, BaseType.h.
**
** What is a stand-in (differential/shim_partition/pm_world.h declares them; the definitions are
** below): Object, Player, Team, PlayerList, ThingTemplate, the modules (contain, stealth, body, AI
** update), Squad, ActionManager, CommandButton, PolygonTrigger, TerrainLogic, AI/Pathfinder,
** Display, Radar, GhostObject(Manager), GameLogic, GlobalData. They answer from the scenario the
** test installs, by the rules the Rust `ScenarioHost` (src/logic/world/partition/scenario.rs)
** implements, and LOG every call that crosses the cut. Two logs per op: the object callbacks
** (cell change, collide, ghost creation) and the client calls (Display/Radar shroud writes, ghost
** snapshot calls).
**
** Entry point (extern "C", tests/differential_partition.rs, which documents the encodings):
** ref_partition_run installs a scenario, runs a program of operations on one PartitionManager, and
** writes, per operation, its results and the two logs. */
#include "PreRTS.h"

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdint.h>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <queue>
#include <algorithm>

#include "Common/Xfer.h"
#include "Common/XferCRC.h"
#include "pm_world.h"

/* PartitionManager.cpp's own members and its friends are protected/private; the wrapper reads the
** records to dump them (and the friend functions lose their friendship, see above). */
#define private public
#define protected public
#define SimpleObjectIterator PMSimpleObjectIterator
#define ObjectIterator PMObjectIterator
#include "GameLogic/PartitionManager.h"
#undef private
#undef protected
#include "PartitionManager_decl.cpp"
#define private public
#define protected public
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SimpleObjectIterator.cpp"
#undef private
#undef protected
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameClient/Line2D.cpp"
/* batch A26: AI.cpp's queries, verbatim (build.rs: from PartitionFilterLiveMapEnemies to the end of
** getAdjustedVisionRangeForObject), over pm_world.h's AI, TAiData, Weapon, AttackPriorityInfo and ScriptEngine */
#include "ai_queries.inc"

/* ------------------------------------------------------------------------- */
/* The scenario                                                               */
/* ------------------------------------------------------------------------- */

namespace {

const int NO_ID = -1;

/* the object flags of scenario.rs `sflag` */
enum
{
	SF_DEAD = 1 << 0,
	SF_OFF_MAP = 1 << 1,
	SF_AIRBORNE = 1 << 2,
	SF_STRUCTURE = 1 << 3,
	SF_NON_FACTION = 1 << 4,
	SF_CAN_ATTACK = 1 << 5,
	SF_UNMANNED = 1 << 6,
	SF_WANTS_GHOST = 1 << 7,
	SF_KILL_ON_COLLIDE = 1 << 8,
	SF_SQUAD1 = 1 << 9
};

/* scenario.rs `mix` */
unsigned mix(unsigned a, unsigned b, unsigned c)
{
	unsigned h = a * 0x9E3779B1u ^ b * 0x85EBCA6Bu ^ c * 0xC2B2AE35u;
	h ^= h >> 15;
	h = h * 0x2C1B3C6Du;
	h ^= h >> 12;
	return h;
}

struct SObj
{
	int id;
	Coord3D pos;
	Real orient;
	GeometryInfo geom;
	KindOfMaskType kind;
	ObjectStatusMaskType status;
	unsigned team;
	int player;
	unsigned flags;
	int cost;
	int tmpl;
	bool hasContain;
	int firstDetected;	/* 0 none, 1 no, 2 yes */
	int stealthKind;	/* 0 none, 1 not disguised, 2 disguised */
	Coord3D dir;
	Real vision;
	bool hasLooked;
	Real lookX, lookY;
	Object obj;
	Object member;	/* the first contained item */
	ContainModuleInterface contain;
	StealthUpdate stealth;
	BodyModuleInterface body;
	AIUpdateInterface ai;
	Object::XMatrix xm;
	ThingTemplate *tmplPtr;
	Weapon weapons[WEAPONSLOT_COUNT];	/* batch A26 */
	SObj() : geom(GEOMETRY_SPHERE, false, 1.0f, 1.0f, 1.0f), tmplPtr(NULL) {}
};

struct LogWord
{
	std::vector<unsigned> w;
};

struct Scn
{
	UnsignedInt frame;
	Int playerCount;
	Int local;
	bool hasTerrain;
	unsigned rng;
	int nextGhost;
	Real cellSize;
	UnsignedInt unlook;
	Region3D extent;
	Relationship rel[16][16];
	std::vector<SObj *> objs;
	std::map<int, SObj *> byId;
	Player players[16];
	Player noPlayer;
	Team defaultTeams[16];
	std::map<unsigned, Team *> teams;
	std::map<int, ThingTemplate *> templates;
	std::map<int, GhostObject *> ghosts;
	std::vector<unsigned> hostLog;
	std::vector<unsigned> clientLog;
	std::vector<unsigned> radarLog;
	PMGlobals globals;
	GameLogic logic;
	TerrainLogic terrain;
	AI ai;
	Display display;
	Radar radar;
	ActionManager actions;
	GhostObjectManager ghostMgr;
	PlayerList playerList;
	Squad squad1, squad2;
	PathfindCell pcell;
	PartitionManager *pm;
	ShroudStatusStoreRestore store;
	Scn() : frame(0), playerCount(0), local(-1), hasTerrain(false), rng(0), nextGhost(0), cellSize(40), unlook(30), pm(NULL) {}
};

Scn *g = NULL;

void logHost(std::initializer_list<unsigned> w)
{
	g->hostLog.insert(g->hostLog.end(), w.begin(), w.end());
}

unsigned bitsOf(float f)
{
	unsigned u;
	memcpy(&u, &f, 4);
	return u;
}

SObj *objOf(const Object *o)
{
	return g->objs[o->m_idx];
}

Object *objectById(int id)
{
	std::map<int, SObj *>::iterator it = g->byId.find(id);
	return it == g->byId.end() ? NULL : &it->second->obj;
}

Real terrainHeight(Real x, Real y)
{
	Real a = x * 0.5f;
	Real b = y * 0.25f;
	return (a * a + b * b) * 0.001f - 3.0f;
}

Team *teamFor(unsigned id, int owner)
{
	std::map<unsigned, Team *>::iterator it = g->teams.find(id);
	if (it != g->teams.end()) return it->second;
	Team *t = new Team;
	t->m_owner = owner;
	g->teams[id] = t;
	return t;
}

}	/* namespace */

/* ------------------------------------------------------------------------- */
/* The stand-ins' members                                                      */
/* ------------------------------------------------------------------------- */

PMGlobals *PMTheGlobalData = NULL;
GameLogic *TheGameLogic = NULL;
PlayerList *ThePlayerList = NULL;
ActionManager *TheActionManager = NULL;
TerrainLogic *TheTerrainLogic = NULL;
AI *TheAI = NULL;
ScriptEngine *TheScriptEngine = NULL;
Display *TheDisplay = NULL;
Radar *TheRadar = NULL;
GhostObjectManager *TheGhostObjectManager = NULL;

Real PMGameLogicRandomValueReal(Real lo, Real hi)
{
	g->rng = g->rng * 1664525u + 1013904223u;
	return lo + (hi - lo) * ((float)(g->rng >> 8) / 16777216.0f);
}

UnsignedInt GameLogic::getFrame() const { return g->frame; }

Relationship Team::getRelationship(const Team *that) const
{
	if (m_owner >= 0 && that->m_owner >= 0 && m_owner < 16 && that->m_owner < 16) return g->rel[m_owner][that->m_owner];
	return NEUTRAL;
}
Relationship Player::getRelationship(const Team *that) const
{
	if (that->m_owner >= 0 && that->m_owner < 16 && m_index >= 0 && m_index < 16) return g->rel[m_index][that->m_owner];
	return NEUTRAL;
}
const Team *Player::getDefaultTeam() const { return &g->defaultTeams[m_index]; }
PlayerType Player::getPlayerType() const { return (m_index & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN; }

Int PlayerList::getPlayerCount() const { return g->playerCount; }
Player *PlayerList::getNthPlayer(Int i) const { return (i >= 0 && i < 16) ? &g->players[i] : NULL; }
Player *PlayerList::getLocalPlayer() const { return g->local >= 0 ? &g->players[g->local] : &g->noPlayer; }
/* PlayerList.cpp lines 397-438, with the same logic */
PlayerMaskType PlayerList::getPlayersWithRelationship(Int srcPlayerIndex, UnsignedInt allowedRelationships)
{
	PlayerMaskType retVal = 0;
	if (allowedRelationships == 0) return retVal;
	Player *srcPlayer = getNthPlayer(srcPlayerIndex);
	if (!srcPlayer) return retVal;
	if (BitTest(allowedRelationships, ALLOW_SAME_PLAYER)) BitSet(retVal, srcPlayer->getPlayerMask());
	for (Int i = 0; i < getPlayerCount(); ++i)
	{
		Player *player = getNthPlayer(i);
		if (!player) continue;
		if (player == srcPlayer) continue;
		switch (srcPlayer->getRelationship(player->getDefaultTeam()))
		{
			case ENEMIES: if (BitTest(allowedRelationships, ALLOW_ENEMIES)) BitSet(retVal, player->getPlayerMask()); break;
			case ALLIES: if (BitTest(allowedRelationships, ALLOW_ALLIES)) BitSet(retVal, player->getPlayerMask()); break;
			case NEUTRAL: if (BitTest(allowedRelationships, ALLOW_NEUTRAL)) BitSet(retVal, player->getPlayerMask()); break;
		}
	}
	return retVal;
}

Int ModuleInfo::getCount() const { return 1; }
AsciiString ModuleInfo::getNthName(Int) const { return AsciiString(m_flavour == 0 ? "W3DDefaultDraw" : "W3DModelDraw"); }

Bool Squad::isOnSquad(const Object *obj) const
{
	return this == &g->squad1 && (objOf(obj)->flags & SF_SQUAD1) != 0;
}

const Player *ContainModuleInterface::getApparentControllingPlayer(const Player *) const
{
	return (m_apparent >= 0 && m_apparent < 16) ? &g->players[m_apparent] : NULL;
}

Bool CommandButton::isValidToUseOn(const Object *source, const Object *target, const Object *, CommandSourceType cs) const
{
	unsigned t = (unsigned)target->getID();
	unsigned rot = (t << 9) | (t >> 23);
	return mix((unsigned)m_key, (unsigned)source->getID() ^ rot, 27 + (unsigned)cs) % 2 == 0;
}
Bool ActionManager::canPlayerGarrison(const Player *player, const Object *target, CommandSourceType cs)
{
	return mix((unsigned)(player ? player->m_index : -1), (unsigned)target->getID(), 26 + (unsigned)cs) % 2 == 0;
}
Bool ActionManager::canEnterObject(const Object *obj, const Object *objectToEnter, CommandSourceType cs, CanEnterType)
{
	return mix((unsigned)obj->getID(), (unsigned)objectToEnter->getID(), 23 + (unsigned)cs) % 3 == 0;
}
Bool ActionManager::canHijackVehicle(const Object *hijacker, const Object *target, CommandSourceType cs)
{
	return mix((unsigned)hijacker->getID(), (unsigned)target->getID(), 24 + (unsigned)cs) % 3 == 0;
}

Bool PolygonTrigger::pointInTrigger(ICoord3D &point) const
{
	long long dx = (long long)point.x - 150;
	long long dy = (long long)point.y - 150;
	long long r = 40 + 10 * (long long)m_key;
	return dx * dx + dy * dy <= r * r;
}

void TerrainLogic::getExtent(Region3D *extent) const { *extent = g->extent; }
Real TerrainLogic::getGroundHeight(Real x, Real y, Coord3D *) const { return terrainHeight(x, y); }
PathfindLayerEnum TerrainLogic::getHighestLayerForDestination(const Coord3D *pos) const
{
	return (PathfindLayerEnum)(1 + (int)(mix((unsigned)(int)pos->x, (unsigned)(int)pos->y, 11) % 3));
}
Real TerrainLogic::getLayerHeight(Real x, Real y, PathfindLayerEnum layer) const { return terrainHeight(x, y) + (Real)(int)layer * 0.5f; }
Bool TerrainLogic::isCliffCell(Real x, Real y) const { return mix((unsigned)(int)x, (unsigned)(int)y, 12) % 11 == 0; }
Bool TerrainLogic::isUnderwater(Real x, Real y, Real *, Real *) const { return terrainHeight(x, y) < -2.0f; }
void TerrainLogic::getMaximumPathfindExtent(Region3D *extent) const
{
	if (!g->hasTerrain) { memset(extent, 0, sizeof(*extent)); return; }
	extent->lo.x = g->extent.lo.x + 10.0f;
	extent->lo.y = g->extent.lo.y + 10.0f;
	extent->lo.z = g->extent.lo.z;
	extent->hi.x = g->extent.hi.x - 10.0f;
	extent->hi.y = g->extent.hi.y - 10.0f;
	extent->hi.z = g->extent.hi.z;
}
Bool TerrainLogic::isClearLineOfSight(const Coord3D &pos, const Coord3D &posOther) const
{
	for (int i = 0; i <= 16; ++i)
	{
		Real t = (Real)i / 16.0f;
		Real x = pos.x + t * (posOther.x - pos.x);
		Real y = pos.y + t * (posOther.y - pos.y);
		Real z = pos.z + t * (posOther.z - pos.z);
		if (terrainHeight(x, y) > z + 0.5f) return false;
	}
	return true;
}

PathfindCell *Pathfinder::getCell(PathfindLayerEnum layer, Int x, Int y)
{
	if (x < 0 || y < 0) return NULL;
	unsigned m = mix((unsigned)x, (unsigned)y, 13 + (unsigned)layer) % 9;
	g->pcell.m_type = (m == 0) ? PathfindCell::CELL_IMPASSABLE : (m <= 2 ? PathfindCell::CELL_WATER : PathfindCell::CELL_CLEAR);
	return &g->pcell;
}
Bool Pathfinder::clientSafeQuickDoesPathExist(const LocomotorSet &set, const Coord3D *, const Coord3D *to)
{
	return mix((unsigned)set.m_source, (unsigned)(int)to->x, (unsigned)(int)to->y) % 4 != 0;
}
Bool Pathfinder::isViewBlockedByObstacle(const Object *obj, const Object *objOther)
{
	return mix((unsigned)obj->getID(), (unsigned)objOther->getID(), 21) % 5 == 0;
}

void Display::setShroudLevel(Int x, Int y, CellShroudStatus s)
{
	g->clientLog.push_back(4);
	g->clientLog.push_back((unsigned)x);
	g->clientLog.push_back((unsigned)y);
	g->clientLog.push_back((unsigned)s);
}
void Display::clearShroud() { g->clientLog.push_back(5); }
void Radar::setShroudLevel(Int x, Int y, CellShroudStatus s)
{
	g->radarLog.push_back(4);
	g->radarLog.push_back((unsigned)x);
	g->radarLog.push_back((unsigned)y);
	g->radarLog.push_back((unsigned)s);
}
void Radar::clearShroud() { g->radarLog.push_back(5); }

void GhostObject::snapShot(int p) { g->clientLog.push_back(6); g->clientLog.push_back((unsigned)m_handle); g->clientLog.push_back((unsigned)p); }
void GhostObject::freeSnapShot(int p) { g->clientLog.push_back(7); g->clientLog.push_back((unsigned)m_handle); g->clientLog.push_back((unsigned)p); }
void GhostObject::updateParentObject(Object *, PartitionData *mod)
{
	g->clientLog.push_back(9);
	g->clientLog.push_back((unsigned)m_handle);
	m_partition = mod;
}
GhostObject *GhostObjectManager::addGhostObject(Object *object, PartitionData *pd)
{
	SObj *s = objOf(object);
	GhostObject *gh = new GhostObject;
	gh->m_handle = ++g->nextGhost;
	gh->m_type = s->geom.getGeomType();
	gh->m_small = s->geom.getIsSmall();
	gh->m_major = s->geom.getMajorRadius();
	gh->m_minor = s->geom.getMinorRadius();
	gh->m_pos = s->pos;
	gh->m_angle = s->orient;
	gh->m_partition = pd;
	g->ghosts[gh->m_handle] = gh;
	logHost({3u, (unsigned)s->id});
	return gh;
}
void GhostObjectManager::removeGhostObject(GhostObject *mod)
{
	if (mod)
	{
		g->clientLog.push_back(8);
		g->clientLog.push_back((unsigned)mod->m_handle);
	}
}

/* ---- the object */
ObjectID Object::getID() const { return (ObjectID)objOf(this)->id; }
const Coord3D *Object::getPosition() const { return &objOf(this)->pos; }
Real Object::getOrientation() const { return objOf(this)->orient; }
const GeometryInfo &Object::getGeometryInfo() const { return objOf(this)->geom; }
const ThingTemplate *Object::getTemplate() const { return objOf(this)->tmplPtr; }
Bool Object::isKindOf(KindOfType t) const { return objOf(this)->kind.test(t); }
Bool Object::isKindOfMulti(const KindOfMaskType &s, const KindOfMaskType &c) const { return objOf(this)->kind.testSetAndClear(s, c); }
ObjectStatusMaskType Object::getStatusBits() const { return objOf(this)->status; }
Bool Object::testStatus(ObjectStatusTypes bit) const { return objOf(this)->status.test(bit); }
Bool Object::isDestroyed() const { return objOf(this)->status.test(OBJECT_STATUS_DESTROYED); }
Bool Object::isEffectivelyDead() const { return (objOf(this)->flags & SF_DEAD) != 0; }
Bool Object::isOffMap() const { return (objOf(this)->flags & SF_OFF_MAP) != 0; }
Bool Object::isUsingAirborneLocomotor() const { return (objOf(this)->flags & SF_AIRBORNE) != 0; }
Bool Object::isStructure() const { return (objOf(this)->flags & SF_STRUCTURE) != 0; }
Bool Object::isNonFactionStructure() const { return (objOf(this)->flags & SF_NON_FACTION) != 0; }
Bool Object::isAbleToAttack() const { return (objOf(this)->flags & SF_CAN_ATTACK) != 0; }
Bool Object::isDisabledByType(Int) const { return (objOf(this)->flags & SF_UNMANNED) != 0; }
Player *Object::getControllingPlayer() const
{
	int p = objOf(this)->player;
	return (p >= 0 && p < 16) ? &g->players[p] : NULL;
}
const Team *Object::getTeam() const { return teamFor(objOf(this)->team, objOf(this)->player); }
Relationship Object::getRelationship(const Object *that) const
{
	int a = objOf(this)->player, b = objOf(that)->player;
	if (a >= 0 && b >= 0 && a < 16 && b < 16) return g->rel[a][b];
	return NEUTRAL;
}
const Coord3D *Object::getUnitDirectionVector2D() const { return &objOf(this)->dir; }
Vector3 Object::XMatrix::Get_X_Vector() const { return Vector3(x, y, z); }
const Object::XMatrix *Object::getTransformMatrix() const { return &objOf(this)->xm; }
ContainModuleInterface *Object::getContain() const { SObj *s = objOf(this); return s->hasContain ? &s->contain : NULL; }
StealthUpdate *Object::getStealth() const { SObj *s = objOf(this); return s->stealthKind ? &s->stealth : NULL; }
BodyModuleInterface *Object::getBodyModule() const { SObj *s = objOf(this); return (s->id % 4 != 0) ? &s->body : NULL; }
AIUpdateInterface *Object::getAIUpdateInterface() const { SObj *s = objOf(this); return (s->id % 5 != 0) ? &s->ai : NULL; }
/* batch A26: AI.cpp's questions, by scenario.rs's rules (`impl AiQueryHost for ScenarioHost`) */
const Weapon *Object::getWeaponInWeaponSlot(WeaponSlotType slot) const
{
	SObj *s = objOf(this);
	if (mix((unsigned)s->id, (unsigned)slot, 40) % 3 == 0) return NULL;	/* a third of the slots empty */
	s->weapons[slot].m_slot = slot;
	return &s->weapons[slot];
}
Bool Weapon::isWithinAttackRange(const Object *source, const Object *target) const
{
	return mix((unsigned)source->getID(), (unsigned)target->getID(), 41 + (unsigned)m_slot) % 2 == 0;
}
Int AttackPriorityInfo::getPriority(const ThingTemplate *tmpl) const { return (Int)(mix((unsigned)m_key, (unsigned)tmpl->m_key, 42) % 6); }
Real Object::getVisionRange() const { return objOf(this)->vision; }
Object *Object::getContainedBy() const { SObj *s = objOf(this); return (mix((unsigned)s->id, 0, 45) % 4 == 0) ? &s->obj : NULL; }
Real Object::getLargestWeaponRange() const { return objOf(this)->vision * 0.75f; }
UnsignedInt AIUpdateInterface::getMoodMatrixValue() const
{
	static const UnsignedInt moods[6] = { 0x100, 0x200, 0x400, 0x800, 0x1000, 0 };
	unsigned m = mix((unsigned)m_set.m_source, 0, 44);
	return (m & 1) | moods[(m >> 1) % 6];
}
CanAttackResult Object::getAbleToAttackSpecificObject(AbleToAttackType t, const Object *target, CommandSourceType cs) const
{
	switch (mix((unsigned)getID(), (unsigned)target->getID(), 22 + (unsigned)t * 7 + (unsigned)cs) % 4)
	{
		case 0: return ATTACKRESULT_POSSIBLE;
		case 1: return ATTACKRESULT_POSSIBLE_AFTER_MOVING;
		case 2: return ATTACKRESULT_NOT_POSSIBLE;
		default: return ATTACKRESULT_INVALID_SHOT;
	}
}
/* Object.cpp lines 1802-1813 */
ObjectShroudStatus Object::getShroudedStatus(Int playerIndex) const
{
	if (isKindOf(KINDOF_ALWAYS_VISIBLE)) return OBJECTSHROUD_CLEAR;
	if (m_partitionData) return m_partitionData->getShroudedStatus(playerIndex);
	return OBJECTSHROUD_CLEAR;
}
/* Object::onPartitionCellChange: unlook the old spot (queued), look at the new (scenario.rs) */
void Object::onPartitionCellChange()
{
	SObj *s = objOf(this);
	logHost({1u, (unsigned)s->id});
	if (s->vision > 0.0f && s->player >= 0 && s->player < 16)
	{
		PlayerMaskType mask = (PlayerMaskType)(1 << s->player);
		if (s->hasLooked) ThePartitionManager->queueUndoShroudReveal(s->lookX, s->lookY, s->vision, mask);
		ThePartitionManager->doShroudReveal(s->pos.x, s->pos.y, s->vision, mask);
		s->hasLooked = true;
		s->lookX = s->pos.x;
		s->lookY = s->pos.y;
	}
}
void Object::onCollide(Object *other, const Coord3D *loc, const Coord3D *normal)
{
	SObj *s = objOf(this);
	logHost({2u, (unsigned)s->id, (unsigned)other->getID(), bitsOf(loc->x), bitsOf(loc->y), bitsOf(loc->z), bitsOf(normal->x), bitsOf(normal->y), bitsOf(normal->z)});
	if (s->flags & SF_KILL_ON_COLLIDE)
	{
		s->status.set(OBJECT_STATUS_DESTROYED);
		ThePartitionManager->unRegisterObject(this);
	}
}

/* ------------------------------------------------------------------------- */
/* The program                                                                 */
/* ------------------------------------------------------------------------- */

namespace {

/* per object: ints (IO words) and floats (FO words), as tests/differential_partition.rs encodes them */
const int IO = 24;
const int FO = 10;
/* per filter: ints (type + 8 arguments) and floats */
const int FILTER_INTS = 9;
const int FILTER_FLOATS = 8;

struct Out
{
	unsigned *out;
	int cap;
	int n;
	bool full;
	void put(unsigned u) { if (n < cap) out[n++] = u; else full = true; }
	void putf(float f) { put(bitsOf(f)); }
};

/* FNV-1a over a word */
unsigned fold(unsigned h, unsigned w)
{
	h ^= w;
	h *= 16777619u;
	return h;
}

int cellIndexOf(PartitionManager *pm, const PartitionCell *c)
{
	return c ? (int)(c - pm->m_cells) : -1;
}

/* the whole observable state, as src/logic/world/partition/manager.rs `state_words` encodes it */
void dumpState(Out &o)
{
	PartitionManager *pm = ThePartitionManager;
	o.put(pm->m_updatedSinceLastReset ? 1 : 0);
	/* the module list, newest first */
	std::vector<PartitionData *> mods;
	for (PartitionData *m = pm->m_moduleList; m; m = m->m_next) mods.push_back(m);
	o.put((unsigned)mods.size());
	for (size_t i = 0; i < mods.size(); ++i) o.put(mods[i]->m_object ? (unsigned)mods[i]->m_object->getID() : 0u);
	/* the dirty list, head first */
	std::vector<PartitionData *> dirty;
	for (PartitionData *m = pm->m_dirtyModules; m; m = m->m_nextDirty) dirty.push_back(m);
	o.put((unsigned)dirty.size());
	for (size_t i = 0; i < dirty.size(); ++i) o.put(dirty[i]->m_object ? (unsigned)dirty[i]->m_object->getID() : 0u);
	/* each module */
	for (size_t i = 0; i < mods.size(); ++i)
	{
		PartitionData *m = mods[i];
		o.put(m->m_object ? (unsigned)m->m_object->getID() : 0u);
		o.put((unsigned)m->m_coiInUseCount);
		o.put((unsigned)m->m_coiArrayCount);
		for (int k = 0; k < m->m_coiInUseCount; ++k)
			o.put((unsigned)cellIndexOf(pm, m->m_coiArray[k].m_cell ? m->m_coiArray[k].m_cell : NULL));
		o.put((unsigned)m->m_dirtyStatus);
		for (int p = 0; p < MAX_PLAYER_COUNT; ++p) o.put((unsigned)m->m_shroudedness[p]);
		for (int p = 0; p < MAX_PLAYER_COUNT; ++p) o.put((unsigned)m->m_shroudednessPrevious[p]);
		unsigned seen = 0;
		for (int p = 0; p < MAX_PLAYER_COUNT; ++p) if (m->m_everSeenByPlayer[p]) seen |= 1u << p;
		o.put(seen);
		o.put((unsigned)cellIndexOf(pm, m->m_lastCell));
	}
	/* each cell */
	for (int c = 0; c < pm->m_totalCellCount; ++c)
	{
		PartitionCell *cell = &pm->m_cells[c];
		o.put((unsigned)cell->m_coiCount);
		unsigned count = 0;
		for (CellAndObjectIntersection *coi = cell->m_firstCoiInCell; coi; coi = coi->m_nextCoi) ++count;
		o.put(count);
		for (CellAndObjectIntersection *coi = cell->m_firstCoiInCell; coi; coi = coi->m_nextCoi)
			o.put(coi->m_module->m_object ? (unsigned)coi->m_module->m_object->getID() : 0u);
		unsigned hs = 2166136261u, ht = 2166136261u, hc = 2166136261u;
		for (int p = 0; p < MAX_PLAYER_COUNT; ++p)
		{
			hs = fold(hs, (unsigned)(unsigned short)cell->m_shroudLevel[p].m_currentShroud);
			hs = fold(hs, (unsigned)(unsigned short)cell->m_shroudLevel[p].m_activeShroudLevel);
			ht = fold(ht, (unsigned)cell->m_threatValue[p]);
			hc = fold(hc, (unsigned)cell->m_cashValue[p]);
		}
		o.put(hs);
		o.put(ht);
		o.put(hc);
	}
	/* the pending unlooks: count, then (x, y, radius, mask, due) */
	std::queue<SightingInfo *> q = pm->m_pendingUndoShroudReveals;
	o.put((unsigned)q.size());
	while (!q.empty())
	{
		SightingInfo *s = q.front();
		q.pop();
		o.putf(s->m_where.x); o.putf(s->m_where.y); o.putf(s->m_howFar);
		o.put((unsigned)s->m_forWhom);
		o.put(s->m_data);
	}
}

void readObjectBits(KindOfMaskType &kind, ObjectStatusMaskType &status, const int *io)
{
	for (int i = 0; i < KINDOF_COUNT; ++i)
	{
		unsigned w = (unsigned)io[8 + i / 32];
		if ((w >> (i % 32)) & 1u) kind.set(i);
	}
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i)
	{
		unsigned w = (unsigned)io[12 + i / 32];
		if ((w >> (i % 32)) & 1u) status.set(i);
	}
}

void clearScenario()
{
	if (g)
	{
		for (size_t i = 0; i < g->objs.size(); ++i) delete g->objs[i];
		for (std::map<unsigned, Team *>::iterator it = g->teams.begin(); it != g->teams.end(); ++it) delete it->second;
		for (std::map<int, GhostObject *>::iterator it = g->ghosts.begin(); it != g->ghosts.end(); ++it) delete it->second;
		delete g->pm;
		delete g;
	}
	g = NULL;
}

void installScenario(const int *ig, const float *fg, const unsigned char *rel, const int *io, const float *fo)
{
	clearScenario();
	g = new Scn;
	g->frame = (UnsignedInt)ig[0];
	g->unlook = (UnsignedInt)ig[1];
	g->playerCount = ig[2];
	g->local = ig[3];
	g->hasTerrain = ig[4] != 0;
	g->rng = (unsigned)ig[5];
	int nObjs = ig[6];
	g->cellSize = fg[0];
	g->extent.lo.x = fg[1]; g->extent.lo.y = fg[2]; g->extent.lo.z = fg[3];
	g->extent.hi.x = fg[4]; g->extent.hi.y = fg[5]; g->extent.hi.z = fg[6];
	for (int a = 0; a < 16; ++a)
		for (int b = 0; b < 16; ++b)
			g->rel[a][b] = (Relationship)rel[a * 16 + b];
	for (int p = 0; p < 16; ++p)
	{
		g->players[p].m_index = p;
		g->defaultTeams[p].m_owner = p;
	}
	g->noPlayer.m_index = -1;
	g->globals.m_partitionCellSize = g->cellSize;
	g->globals.m_unlookPersistDuration = g->unlook;
	PMTheGlobalData = &g->globals;
	TheGameLogic = &g->logic;
	ThePlayerList = &g->playerList;
	TheActionManager = &g->actions;
	TheTerrainLogic = g->hasTerrain ? &g->terrain : NULL;
	TheAI = &g->ai;
	static ScriptEngine scriptEngine;
	scriptEngine.m_default.m_key = 0;
	TheScriptEngine = &scriptEngine;
	TheDisplay = &g->display;
	TheRadar = &g->radar;
	TheGhostObjectManager = &g->ghostMgr;

	for (int k = 0; k < nObjs; ++k)
	{
		const int *oi = io + k * IO;
		const float *of = fo + k * FO;
		SObj *s = new SObj;
		s->id = oi[0];
		s->pos.x = of[0]; s->pos.y = of[1]; s->pos.z = of[2];
		s->orient = of[3];
		s->geom = GeometryInfo((GeometryType)oi[1], oi[2] != 0, of[4], of[5], of[6]);
		readObjectBits(s->kind, s->status, oi);
		s->team = (unsigned)oi[4];
		s->player = oi[3];
		s->flags = (unsigned)oi[5];
		s->cost = oi[6];
		s->tmpl = oi[7];
		s->hasContain = oi[14] != 0;
		s->firstDetected = oi[18];
		s->stealthKind = oi[20];
		s->dir.x = of[7]; s->dir.y = of[8]; s->dir.z = 0.0f;
		s->vision = of[9];
		s->hasLooked = false;
		s->lookX = s->lookY = 0;
		s->obj.m_idx = (int)g->objs.size();
		s->tmplPtr = new ThingTemplate;
		s->tmplPtr->m_key = s->tmpl;
		s->tmplPtr->m_cost = s->cost;
		s->tmplPtr->m_draw.m_flavour = (s->flags & SF_WANTS_GHOST) ? 1 : 0;
		s->xm.x = s->dir.x; s->xm.y = s->dir.y; s->xm.z = 0.25f;
		s->contain.m_garrisonable = oi[15] != 0;
		s->contain.m_count = oi[16];
		s->contain.m_stealthUnits = oi[17];
		s->contain.m_apparent = oi[19];
		s->stealth.m_disguised = (oi[20] == 2);
		s->stealth.m_idx = oi[21];
		s->body.m_last.in.m_sourceID = (ObjectID)(1 + (int)(mix((unsigned)s->id, 0, 25) % 40));
		s->ai.m_set.m_source = s->id;
		teamFor(s->team, s->player);
		g->objs.push_back(s);
		g->byId[s->id] = s;
		/* the first contained item: an object of its own, detected or not */
		if (s->hasContain && s->firstDetected != 0)
		{
			SObj *m = new SObj(*s);
			m->id = -s->id;
			m->status = ObjectStatusMaskType();
			if (s->firstDetected == 2) m->status.set(OBJECT_STATUS_DETECTED);
			m->obj.m_idx = (int)g->objs.size();
			g->objs.push_back(m);
			s->contain.m_items.push_back(&m->obj);
		}
	}
	g->pm = new PartitionManager;
	ThePartitionManager = g->pm;
	g->pm->init();
}

/* ---- the filters */

KindOfMaskType kindMask(const int *a)
{
	KindOfMaskType m;
	for (int i = 0; i < 3; ++i) if (a[i] >= 0) m.set(a[i]);
	return m;
}
ObjectStatusMaskType statusMask(const int *a)
{
	ObjectStatusMaskType m;
	for (int i = 0; i < 3; ++i) if (a[i] >= 0) m.set(a[i]);
	return m;
}
const Object *optObj(int id) { return id < 0 ? NULL : objectById(id); }
Object *optObjMut(int id) { return id < 0 ? NULL : objectById(id); }
Player *optPlayer(int p) { return (p >= 0 && p < 16) ? &g->players[p] : NULL; }

/* builds the NULL-terminated filter array of `n` filters starting at ip/fp, advancing them */
PartitionFilter **buildFilters(int n, const int *&ip, const float *&fp, std::vector<PartitionFilter *> &store)
{
	store.clear();
	for (int k = 0; k < n; ++k)
	{
		const int *a = ip + 1;
		const float *f = fp;
		PartitionFilter *pf = NULL;
		switch (ip[0])
		{
			case 0: pf = new PartitionFilterIsFlying(); break;
			case 1: {
				Coord3D pos; pos.x = f[0]; pos.y = f[1]; pos.z = f[2];
				GeometryInfo geom((GeometryType)a[0], a[1] != 0, f[4], f[5], f[6]);
				pf = new PartitionFilterWouldCollide(pos, geom, f[3], a[2] != 0);
				break; }
			case 2: pf = new PartitionFilterSamePlayer(optPlayer(a[0])); break;
			case 3: pf = new PartitionFilterRelationship(optObj(a[0]), a[1]); break;
			case 4: pf = new PartitionFilterAcceptOnTeam(teamFor((unsigned)a[0], -1)); break;
			case 5: pf = new PartitionFilterAcceptOnSquad(a[0] == 1 ? &g->squad1 : (a[0] == 2 ? &g->squad2 : NULL)); break;
			case 6: pf = new PartitionFilterLineOfSight(optObj(a[0])); break;
			case 7: pf = new PartitionFilterPossibleToAttack((AbleToAttackType)a[0], optObj(a[1]), (CommandSourceType)a[2]); break;
			case 8: pf = new PartitionFilterPossibleToEnter(optObj(a[0]), (CommandSourceType)a[1]); break;
			case 9: pf = new PartitionFilterPossibleToHijack(optObj(a[0]), (CommandSourceType)a[1]); break;
			case 10: pf = new PartitionFilterLastAttackedBy(optObjMut(a[0])); break;
			case 11: pf = new PartitionFilterAcceptByObjectStatus(statusMask(a), statusMask(a + 3)); break;
			case 12: pf = new PartitionFilterRejectByObjectStatus(statusMask(a), statusMask(a + 3)); break;
			case 13: pf = new PartitionFilterStealthedAndUndetected(optObj(a[0]), a[1] != 0); break;
			case 14: pf = new PartitionFilterAcceptByKindOf(kindMask(a), kindMask(a + 3)); break;
			case 15: pf = new PartitionFilterRejectByKindOf(kindMask(a), kindMask(a + 3)); break;
			case 16: pf = new PartitionFilterRejectBehind(optObjMut(a[0])); break;
			case 17: pf = new PartitionFilterAlive(); break;
			case 18: pf = new PartitionFilterSameMapStatus(optObj(a[0])); break;
			case 19: pf = new PartitionFilterOnMap(); break;
			case 20: pf = new PartitionFilterRejectBuildings(optObj(a[0])); break;
			case 21: pf = new PartitionFilterInsignificantBuildings(a[0] != 0, a[1] != 0); break;
			case 22: pf = new PartitionFilterFreeOfFog(a[0]); break;
			case 23: pf = new PartitionFilterRepulsor(optObj(a[0])); break;
			case 24: {
				Coord3D *area = new Coord3D[4];
				for (int i = 0; i < 4; ++i) { area[i].x = f[2 * i]; area[i].y = f[2 * i + 1]; area[i].z = 0; }
				pf = new PartitionFilterIrregularArea(area, a[0]);
				break; }
			case 25: { PolygonTrigger *t = new PolygonTrigger; t->m_key = a[0]; pf = new PartitionFilterPolygonTrigger(t); break; }
			case 26: pf = new PartitionFilterPlayer(optPlayer(a[0]), a[1] != 0); break;
			case 27: pf = new PartitionFilterPlayerAffiliation(optPlayer(a[0]), (UnsignedInt)a[1], a[2] != 0); break;
			case 28: { ThingTemplate *t = new ThingTemplate; t->m_key = a[0]; t->m_cost = 0; t->m_draw.m_flavour = 0; pf = new PartitionFilterThing(t, a[1] != 0); break; }
			case 29: pf = new PartitionFilterGarrisonable(a[0] != 0); break;
			case 30: pf = new PartitionFilterGarrisonableByPlayer(optPlayer(a[0]), a[1] != 0, (CommandSourceType)a[2]); break;
			case 31: pf = new PartitionFilterUnmannedObject(a[0] != 0); break;
			case 32: { CommandButton *b = new CommandButton; b->m_key = a[1]; pf = new PartitionFilterValidCommandButtonTarget(optObjMut(a[0]), b, a[2] != 0, (CommandSourceType)a[3]); break; }
		}
		store.push_back(pf);
		ip += FILTER_INTS;
		fp += FILTER_FLOATS;
	}
	store.push_back(NULL);
	return &store[0];
}

/* the logs, flushed after every operation */
bool flushLogs(Out &o)
{
	o.put((unsigned)g->hostLog.size());
	for (size_t i = 0; i < g->hostLog.size(); ++i) o.put(g->hostLog[i]);
	/* the client calls: Display's, then the ghost calls; Radar's must equal Display's */
	std::vector<unsigned> display;
	for (size_t i = 0; i < g->clientLog.size();)
	{
		unsigned t = g->clientLog[i];
		size_t len = (t == 4) ? 4 : (t == 5 ? 1 : (t == 6 || t == 7) ? 3 : 2);
		if (t == 4 || t == 5) display.insert(display.end(), g->clientLog.begin() + i, g->clientLog.begin() + i + len);
		i += len;
	}
	bool radarOk = (display == g->radarLog);
	o.put((unsigned)g->clientLog.size());
	for (size_t i = 0; i < g->clientLog.size(); ++i) o.put(g->clientLog[i]);
	g->hostLog.clear();
	g->clientLog.clear();
	g->radarLog.clear();
	return radarOk;
}

}	/* namespace */

extern "C" {

/* ref_partition_run: installs a scenario and runs a program on a fresh PartitionManager.
**   ig[16], fg[8], rel[256] (a's relationship to b, 0 enemies / 1 neutral / 2 allies),
**   io[24 * n], fo[10 * n]: the scenario, see tests/differential_partition.rs
**   ops[], opf[]: the program: for each operation its code then its integer arguments (ops) and
**   its float arguments (opf), see the interpreter below
** Writes, per operation, its result words, then the host log and the client log (each a length
** and the words). Returns the number of words, -1 if they would not fit in cap, -3 for an unknown
** operation, -5 if Radar's calls differ from Display's. */
int ref_partition_run(const int *ig, const float *fg, const unsigned char *rel, const int *io, const float *fo, int nOps, const int *ops, const float *opf, unsigned *out, int cap)
{
	installScenario(ig, fg, rel, io, fo);
	Out o = { out, cap, 0, false };
	const int *ip = ops;
	const float *fp = opf;
	PartitionManager *pm = ThePartitionManager;
	std::vector<PartitionFilter *> filters;
	for (int opn = 0; opn < nOps; ++opn)
	{
		int code = *ip++;
		switch (code)
		{
			case 0: { int id = *ip++; Object *obj = objectById(id); pm->registerObject(obj); break; }
			case 1: { int id = *ip++; Object *obj = objectById(id); pm->unRegisterObject(obj); break; }
			case 2: { /* MOVE id, x y z orient: Object::setPosition + the partition data's makeDirty */
				int id = *ip++;
				SObj *s = g->byId[id];
				s->pos.x = fp[0]; s->pos.y = fp[1]; s->pos.z = fp[2]; s->orient = fp[3];
				fp += 4;
				if (s->obj.m_partitionData) s->obj.m_partitionData->makeDirty(true);
				break; }
			case 3: { int id = *ip++; int need = *ip++; SObj *s = g->byId[id]; if (s->obj.m_partitionData) s->obj.m_partitionData->makeDirty(need != 0); break; }
			case 4: pm->update(); break;
			case 5: g->frame = (UnsignedInt)*ip++; break;
			case 6: { /* CLOSEST_OBJ origin dc nf filters / maxdist */
				int origin = *ip++, dc = *ip++, nf = *ip++;
				PartitionFilter **fl = buildFilters(nf, ip, fp, filters);
				Real maxDist = *fp++;
				Real dist; Coord3D vec;
				Object *r = pm->getClosestObject(objectById(origin), maxDist, (DistanceCalculationType)dc, nf ? fl : NULL, &dist, &vec);
				o.put(r ? (unsigned)r->getID() : 0u); o.putf(dist);
				o.putf(r ? vec.x : 0.0f); o.putf(r ? vec.y : 0.0f); o.putf(r ? vec.z : 0.0f);
				break; }
			case 7: { /* CLOSEST_POS dc nf filters / x y z maxdist */
				int dc = *ip++, nf = *ip++;
				PartitionFilter **fl = buildFilters(nf, ip, fp, filters);
				Coord3D pos; pos.x = fp[0]; pos.y = fp[1]; pos.z = fp[2]; Real maxDist = fp[3]; fp += 4;
				Real dist; Coord3D vec;
				Object *r = pm->getClosestObject(&pos, maxDist, (DistanceCalculationType)dc, nf ? fl : NULL, &dist, &vec);
				o.put(r ? (unsigned)r->getID() : 0u); o.putf(dist);
				o.putf(r ? vec.x : 0.0f); o.putf(r ? vec.y : 0.0f); o.putf(r ? vec.z : 0.0f);
				break; }
			case 8: case 9: { /* RANGE_OBJ origin dc order nf filters / maxdist ; RANGE_POS dc order nf filters / x y z maxdist */
				int origin = 0;
				if (code == 8) origin = *ip++;
				int dc = *ip++, order = *ip++, nf = *ip++;
				PartitionFilter **fl = buildFilters(nf, ip, fp, filters);
				SimpleObjectIterator *it;
				if (code == 8)
				{
					Real maxDist = *fp++;
					it = pm->iterateObjectsInRange(objectById(origin), maxDist, (DistanceCalculationType)dc, nf ? fl : NULL, (IterOrderType)order);
				}
				else
				{
					Coord3D pos; pos.x = fp[0]; pos.y = fp[1]; pos.z = fp[2]; Real maxDist = fp[3]; fp += 4;
					it = pm->iterateObjectsInRange(&pos, maxDist, (DistanceCalculationType)dc, nf ? fl : NULL, (IterOrderType)order);
				}
				o.put((unsigned)it->getCount());
				Real num;
				for (Object *r = it->firstWithNumeric(&num); r; r = it->nextWithNumeric(&num)) { o.put((unsigned)r->getID()); o.putf(num); }
				it->deleteInstance();
				break; }
			case 10: { /* ALL_OBJECTS nf filters */
				int nf = *ip++;
				PartitionFilter **fl = buildFilters(nf, ip, fp, filters);
				SimpleObjectIterator *it = pm->iterateAllObjects(nf ? fl : NULL);
				o.put((unsigned)it->getCount());
				for (Object *r = it->first(); r; r = it->next()) o.put((unsigned)r->getID());
				it->deleteInstance();
				break; }
			case 11: { /* POTENTIAL_COLL gtype small use2d / x y z height major minor angle */
				int gtype = *ip++, small = *ip++, use2d = *ip++;
				Coord3D pos; pos.x = fp[0]; pos.y = fp[1]; pos.z = fp[2];
				GeometryInfo geom((GeometryType)gtype, small != 0, fp[3], fp[4], fp[5]);
				Real angle = fp[6]; fp += 7;
				SimpleObjectIterator *it = pm->iteratePotentialCollisions(&pos, geom, angle, use2d != 0);
				o.put((unsigned)it->getCount());
				for (Object *r = it->first(); r; r = it->next()) o.put((unsigned)r->getID());
				it->deleteInstance();
				break; }
			case 12: { /* DIST kind a b dc / x y z gx gy gz */
				int kind = *ip++, a = *ip++, b = *ip++, dc = *ip++;
				Coord3D pos; pos.x = fp[0]; pos.y = fp[1]; pos.z = fp[2];
				Coord3D goal; goal.x = fp[3]; goal.y = fp[4]; goal.z = fp[5]; fp += 6;
				Coord3D v; Real d = 0.0f;
				Object *oa = objectById(a);
				switch (kind)
				{
					case 0: pm->getVectorTo(oa, objectById(b), (DistanceCalculationType)dc, v); break;
					case 1: pm->getVectorTo(oa, &pos, (DistanceCalculationType)dc, v); break;
					case 2: d = pm->getDistanceSquared(oa, objectById(b), (DistanceCalculationType)dc, &v); break;
					case 3: d = pm->getDistanceSquared(oa, &pos, (DistanceCalculationType)dc, &v); break;
					case 4: d = pm->getGoalDistanceSquared(oa, &goal, objectById(b), (DistanceCalculationType)dc, &v); break;
					default: d = pm->getGoalDistanceSquared(oa, &goal, &pos, (DistanceCalculationType)dc, &v); break;
				}
				o.putf(d); o.putf(v.x); o.putf(v.y); o.putf(v.z);
				break; }
			case 13: { /* REL_ANGLE a b / x y z */
				int a = *ip++, b = *ip++;
				Coord3D pos; pos.x = fp[0]; pos.y = fp[1]; pos.z = fp[2]; fp += 3;
				Real r = (b >= 0) ? pm->getRelativeAngle2D(objectById(a), objectById(b)) : pm->getRelativeAngle2D(objectById(a), &pos);
				o.putf(r);
				break; }
			case 14: { int a = *ip++, b = *ip++; o.put(pm->isColliding(objectById(a), objectById(b)) ? 1u : 0u); break; }
			case 15: { /* GEOM_COLLIDES g1 g2 / x1 y1 z1 h1 maj1 min1 ang1 x2 ... */
				int g1 = *ip++, g2 = *ip++;
				Coord3D p1; p1.x = fp[0]; p1.y = fp[1]; p1.z = fp[2];
				GeometryInfo ge1((GeometryType)g1, false, fp[3], fp[4], fp[5]); Real a1 = fp[6];
				Coord3D p2; p2.x = fp[7]; p2.y = fp[8]; p2.z = fp[9];
				GeometryInfo ge2((GeometryType)g2, false, fp[10], fp[11], fp[12]); Real a2 = fp[13];
				fp += 14;
				o.put(pm->geomCollidesWithGeom(&p1, ge1, a1, &p2, ge2, a2) ? 1u : 0u);
				break; }
			case 16: { Real x = fp[0], y = fp[1]; fp += 2; o.putf(pm->getGroundOrStructureHeight(x, y)); break; }
			case 17: { /* LOS a b / ax ay az bx by bz */
				int a = *ip++, b = *ip++;
				Coord3D pa; pa.x = fp[0]; pa.y = fp[1]; pa.z = fp[2];
				Coord3D pb; pb.x = fp[3]; pb.y = fp[4]; pb.z = fp[5]; fp += 6;
				o.put(pm->isClearLineOfSightTerrain(optObj(a), pa, optObj(b), pb) ? 1u : 0u);
				break; }
			case 18: { /* EXTREMES / x0 y0 z0 x1 y1 z1 */
				Coord3D pa; pa.x = fp[0]; pa.y = fp[1]; pa.z = fp[2];
				Coord3D pb; pb.x = fp[3]; pb.y = fp[4]; pb.z = fp[5]; fp += 6;
				Real minZ, maxZ; Coord2D minPos, maxPos;
				minPos.x = minPos.y = maxPos.x = maxPos.y = 0.0f;
				Bool valid = pm->estimateTerrainExtremesAlongLine(pa, pb, &minZ, &maxZ, &minPos, &maxPos);
				o.put(valid ? 1u : 0u); o.putf(minZ); o.putf(maxZ);
				o.putf(minPos.x); o.putf(minPos.y); o.putf(maxPos.x); o.putf(maxPos.y);
				break; }
			case 19: { /* SHROUD kind mask value / x y r */
				int kind = *ip++, mask = *ip++, value = *ip++;
				Real x = fp[0], y = fp[1], r = fp[2]; fp += 3;
				PlayerMaskType m = (PlayerMaskType)mask;
				switch (kind)
				{
					case 0: pm->doShroudReveal(x, y, r, m); break;
					case 1: pm->undoShroudReveal(x, y, r, m); break;
					case 2: pm->doShroudCover(x, y, r, m); break;
					case 3: pm->undoShroudCover(x, y, r, m); break;
					case 4: pm->queueUndoShroudReveal(x, y, r, m); break;
					case 5: pm->doThreatAffect(x, y, r, (UnsignedInt)value, m); break;
					case 6: pm->undoThreatAffect(x, y, r, (UnsignedInt)value, m); break;
					case 7: pm->doValueAffect(x, y, r, (UnsignedInt)value, m); break;
					default: pm->undoValueAffect(x, y, r, (UnsignedInt)value, m); break;
				}
				break; }
			case 20: pm->processEntirePendingUndoShroudRevealQueue(); break;
			case 21: { /* CELL_STATUS kind player / x y */
				int kind = *ip++, player = *ip++;
				Coord3D loc; loc.x = fp[0]; loc.y = fp[1]; loc.z = 0.0f; fp += 2;
				o.put(kind == 0 ? (unsigned)pm->getShroudStatusForPlayer(player, &loc) : (unsigned)pm->getPropShroudStatusForPlayer(player, &loc));
				break; }
			case 22: { int id = *ip++, player = *ip++; o.put((unsigned)objectById(id)->getShroudedStatus(player)); break; }
			case 23: { /* MOST_VALUABLE player types valtype */
				int player = *ip++, types = *ip++, vt = *ip++;
				Coord3D loc; loc.x = -1.0f; loc.y = -1.0f; loc.z = -1.0f;
				pm->getMostValuableLocation(player, (UnsignedInt)types, (ValueOrThreat)vt, &loc);
				bool found = !(loc.x == -1.0f && loc.y == -1.0f && loc.z == -1.0f);
				o.put(found ? 1u : 0u); o.putf(found ? loc.x : 0.0f); o.putf(found ? loc.y : 0.0f); o.putf(found ? loc.z : 0.0f);
				break; }
			case 24: { /* NEAREST_GROUP player types valtype required greater / x y z */
				int player = *ip++, types = *ip++, vt = *ip++, required = *ip++, greater = *ip++;
				Coord3D src; src.x = fp[0]; src.y = fp[1]; src.z = fp[2]; fp += 3;
				Coord3D loc; loc.x = -1.0f; loc.y = -1.0f; loc.z = -1.0f;
				pm->getNearestGroupWithValue(player, (UnsignedInt)types, (ValueOrThreat)vt, &src, required, greater != 0, &loc);
				bool found = !(loc.x == -1.0f && loc.y == -1.0f && loc.z == -1.0f);
				o.put(found ? 1u : 0u); o.putf(found ? loc.x : 0.0f); o.putf(found ? loc.y : 0.0f); o.putf(found ? loc.z : 0.0f);
				break; }
			case 25: { /* MAP_OPS kind player */
				int kind = *ip++, player = *ip++;
				switch (kind)
				{
					case 0: pm->revealMapForPlayer(player); break;
					case 1: pm->revealMapForPlayerPermanently(player); break;
					case 2: pm->undoRevealMapForPlayerPermanently(player); break;
					case 3: pm->shroudMapForPlayer(player); break;
					default: pm->refreshShroudForLocalPlayer(); break;
				}
				break; }
			case 26: { /* STORE_RESTORE kind: 0 store fog, 1 store reveal, 2 restore fog, 3 restore reveal */
				int kind = *ip++;
				switch (kind)
				{
					case 0: pm->storeFoggedCells(g->store, true); break;
					case 1: pm->storeFoggedCells(g->store, false); break;
					case 2: pm->restoreFoggedCells(g->store, true); break;
					default: pm->restoreFoggedCells(g->store, false); break;
				}
				unsigned h = 2166136261u;
				h = fold(h, (unsigned)g->store.m_cellsWide);
				for (int p = 0; p < MAX_PLAYER_COUNT; ++p)
					for (size_t i = 0; i < g->store.m_foggedOrRevealed[p].size(); ++i) h = fold(h, g->store.m_foggedOrRevealed[p][i]);
				o.put(h);
				break; }
			case 27: { XferCRC xc; pm->crc(&xc); o.put(xc.getCRC()); break; }
			case 28: dumpState(o); break;
			case 29: { /* FIND_POSITION flags ignore src rel / cx cy cz minR maxR startAngle maxZDelta */
				FindPositionOptions opts;
				opts.flags = (FindPositionFlags)*ip++;
				opts.ignoreObject = optObj(*ip++);
				opts.sourceToPathToDest = optObj(*ip++);
				opts.relationshipObject = optObj(*ip++);
				Coord3D c; c.x = fp[0]; c.y = fp[1]; c.z = fp[2];
				opts.minRadius = fp[3]; opts.maxRadius = fp[4]; opts.startAngle = fp[5]; opts.maxZDelta = fp[6];
				fp += 7;
				Coord3D result; result.x = result.y = result.z = 0.0f;
				Bool found = pm->findPositionAround(&c, &opts, &result);
				o.put(found ? 1u : 0u); o.putf(found ? result.x : 0.0f); o.putf(found ? result.y : 0.0f); o.putf(found ? result.z : 0.0f);
				break; }
			case 30: { /* UNREGISTER_GHOST handle */
				int h = *ip++;
				std::map<int, GhostObject *>::iterator it = g->ghosts.find(h);
				if (it != g->ghosts.end()) pm->unRegisterGhostObject(it->second);
				break; }
			case 31: break;	/* NOP */
			/* batch A26: AI.cpp's queries */
			case 32: { /* AI_ENEMY me qualifiers info nf filters / range distMod */
				int me = *ip++; UnsignedInt qual = (UnsignedInt)*ip++; int info = *ip++; int nf = *ip++;
				PartitionFilter **fl = buildFilters(nf, ip, fp, filters);
				TheAI->m_aiData.m_attackPriorityDistanceModifier = fp[1];
				AttackPriorityInfo custom; custom.m_key = info;
				const AttackPriorityInfo *ai = info < 0 ? NULL : (info == 0 ? TheScriptEngine->getDefaultAttackInfo() : &custom);
				Object *r = TheAI->findClosestEnemy(objectById(me), fp[0], qual, ai, nf ? fl[0] : NULL);
				fp += 2;
				o.put(r ? (unsigned)r->getID() : 0u);
				break; }
			case 33: { int me = *ip++; UnsignedInt qual = (UnsignedInt)*ip++; Object *r = TheAI->findClosestAlly(objectById(me), *fp++, qual); o.put(r ? (unsigned)r->getID() : 0u); break; }
			case 34: { int me = *ip++; TheAI->m_aiData.m_enableRepulsors = *ip++ != 0; Object *r = TheAI->findClosestRepulsor(objectById(me), *fp++); o.put(r ? (unsigned)r->getID() : 0u); break; }
			case 35: { /* AI_VISION obj factors / guard inner AI, outer AI, inner human, outer human, alert, aggressive */
				int id = *ip++; int factors = *ip++;
				TAiData &d = TheAI->m_aiData;
				d.m_guardInnerModifierAI = fp[0]; d.m_guardOuterModifierAI = fp[1]; d.m_guardInnerModifierHuman = fp[2]; d.m_guardOuterModifierHuman = fp[3];
				d.m_alertRangeModifier = fp[4]; d.m_aggressiveRangeModifier = fp[5];
				fp += 6;
				o.putf(TheAI->getAdjustedVisionRangeForObject(objectById(id), factors));
				break; }
			default: return -3;
		}
		if (!flushLogs(o)) return -5;
		if (o.full) return -1;
	}
	int n = o.n;
	clearScenario();
	return n;
}

/* --- p1w13-sonnet --- */
/* PartitionManager::geomCollidesWithGeom (PartitionManager.cpp, unchanged) for the module build's stand-in manager (update batch O: SquishCollide). It reads
** no member of the manager, only the static collide procedures. */
int ref_partition_geom_collides(const Coord3D *p1, const GeometryInfo *g1, float a1, const Coord3D *p2, const GeometryInfo *g2, float a2)
{
	static PartitionManager *pm = new PartitionManager;
	return pm->geomCollidesWithGeom(p1, *g1, a1, p2, *g2, a2) ? 1 : 0;
}
/* --- end p1w13-sonnet --- */

}	/* extern "C" */
