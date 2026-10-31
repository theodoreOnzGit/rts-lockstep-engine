/* Differential-test wrapper for GameLogic::startNewGame (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp, cut verbatim by build.rs into OUT_DIR/glnew_gen:
**   game_logic_new_game_cut.inc holds isInSinglePlayerGame; placeObjectAtPosition to populateRandomStartPosition (batch
**   L3a's helpers); selectObject; isProgressComplete, lastHeardFrom and testTimeOut (batch L1's); and updateLoadProgress
**   to the end of loadMapINI (updateLoadProgress, deleteLoadScreen, startNewGame, findAndSelectCommandCenter,
**   loadMapINI). One edit, in startNewGame: `UnicodeString(L"Observer")` (a wide literal, 16-bit on MSVC and 32-bit on
**   g++) is written `observerDisplayName()`, which builds the same string from 16-bit characters.
** Compiled inside namespace gln on stand-ins that log every call that acts, in the format the test's Rust host logs:
** the global data, the game state, the campaign manager, the network, LAN and GameSpy, the recorder, the map cache and
** file system, INI and the game text, the display, the terrain logic, the game engine, the sides list (each side's and
** team's dictionary logged whole), the player templates and the name keys, the multiplayer settings, the team factory
** and player list, the script engine and the multiplayer scripts file, the radar, in-game UI, victory conditions,
** partition manager, ghost object manager, map objects and their templates, the thing factory, objects, drawables,
** the pathfinder, the terrain visual, the LOD manager, the control bar, the tactical view, the transition handler,
** the window manager, the shell, the stats collector, the GameSpy buddy queue and info. The games' options are the
** GameInfo build's real SkirmishGameInfo (one object per source) and the random numbers the RandomValue build's.
**
** Entry point (extern "C", tests/differential_game_logic_new_game.rs):
**   ref_glnew_run(cfg, slots, out, cap)  runs startNewGame once and writes the log */
#define TheGameInfo GiTestTheGameInfo
#define TheSkirmishGameInfo GiTestTheSkirmishGameInfo
#define TheChallengeGameInfo GiTestTheChallengeGameInfo
#define GameSlot GiTestGameSlot
#define GameInfo GiTestGameInfo
#define SkirmishGameInfo GiTestSkirmishGameInfo
#define GameInfoToAsciiString GiTestGameInfoToAsciiString
#define ParseAsciiStringToGameInfo GiTestParseAsciiStringToGameInfo

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <float.h>
#include <math.h>
#include <string>
#include <vector>
#include <map>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
class INI;   /* Money.h declares parseMoneyAmount(INI *, ...) */
#define protected public
#define private public
#include "GameNetwork/GameInfo.h"
#undef protected
#undef private

extern "C" void ref_ginfo_configure(const unsigned *cfg);
extern void InitGameLogicRandom(UnsignedInt seed);
extern UnsignedInt GetGameLogicRandomSeed(void);

/* the GameInfo build leaves these out (SkirmishGameOptionsMenu.cpp line 78, ChallengeLoadScreen's); TheGameInfo is
** GameInfo.cpp's */
SkirmishGameInfo *TheSkirmishGameInfo = NULL;
SkirmishGameInfo *TheChallengeGameInfo = NULL;

#ifndef _MAX_PATH
#define _MAX_PATH 260
#endif
#ifndef CRCDEBUG_LOG
#define CRCDEBUG_LOG(x)
#endif
#ifndef DUMPCOORD3D
#define DUMPCOORD3D(x)
#endif
#ifndef CRCGEN_LOG
#define CRCGEN_LOG(x)
#endif

namespace gln {

std::string g_log;
void logf(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
void logf(const char *fmt, ...)
{
	static char buf[1 << 16];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	g_log += buf;
	g_log += '\n';
}

unsigned g_cfg[40];
#define FLAG(n) ((g_cfg[1] >> (n)) & 1)

enum { GAME_SINGLE_PLAYER, GAME_LAN, GAME_SKIRMISH, GAME_REPLAY, GAME_SHELL, GAME_INTERNET, GAME_NONE };
enum RecorderModeType { RECORDERMODETYPE_RECORD, RECORDERMODETYPE_PLAYBACK, RECORDERMODETYPE_NONE };
enum { KINDOF_IMMOBILE = 1, KINDOF_WALK_ON_TOP_OF_WALL, KINDOF_SHRUBBERY, KINDOF_OPTIMIZED_TREE, KINDOF_PROP,
	KINDOF_CLEARED_BY_BUILD, KINDOF_CAN_CAST_REFLECTIONS, KINDOF_COMMANDCENTER };
enum { FLAG_BRIDGE_FLAGS = 1, FLAG_ROAD_FLAGS = 2, FLAG_DRAWS_IN_MIRROR = 4 };
enum DrawableStatus { DRAWABLE_STATUS_NONE = 0, DRAWABLE_STATUS_DRAWS_IN_MIRROR = 0x40 };
enum PathfindLayerEnum { LAYER_INVALID = 0, LAYER_GROUND = 1 };
enum { MAX_MP_STARTING_UNITS = 10 };
enum { VICTORY_NOBUILDINGS = 1 };
enum PlayerType { PLAYER_HUMAN, PLAYER_COMPUTER };
enum Relationship { ENEMIES = 0, NEUTRAL, ALLIES };
enum { ALLOW_ENEMIES = 0x04 };
enum TimeOfDay { TIME_OF_DAY_INVALID = 0, TIME_OF_DAY_FIRST = 1, TIME_OF_DAY_MORNING = TIME_OF_DAY_FIRST, TIME_OF_DAY_AFTERNOON,
	TIME_OF_DAY_EVENING, TIME_OF_DAY_NIGHT, TIME_OF_DAY_COUNT };
enum StaticGameLODLevel { STATIC_GAME_LOD_UNKNOWN = -1, STATIC_GAME_LOD_LOW, STATIC_GAME_LOD_MEDIUM, STATIC_GAME_LOD_HIGH, STATIC_GAME_LOD_CUSTOM };
enum INILoadType { INI_LOAD_INVALID, INI_LOAD_OVERWRITE, INI_LOAD_CREATE_OVERRIDES, INI_LOAD_MULTIFILE };
enum GPEnum { GP_PLAYING = 3 };
inline Real sqr(Real x) { return x * x; }

void setFPMode() { logf("fpmode"); }
unsigned g_time;
UnsignedInt timeGetTime() { unsigned t = g_time; g_time += g_cfg[23]; logf("time %d", (int)t); return t; }
void Sleep(UnsignedInt ms) { logf("sleep %u", ms); }
/* GameCommon.cpp lines 51-65 */
Real normalizeAngle(Real angle)
{
	if( isnan(angle) )
		return 0;
	while (angle > PI)
		angle -= 2*PI;
	while (angle <= -PI)
		angle += 2*PI;
	return angle;
}

/* ---- name keys ---- */
typedef Int NameKeyType;
class NameKeyGenerator
{
public:
	std::vector<std::string> m_names;
	NameKeyType nameToKey(const char *n) { for (size_t i = 0; i < m_names.size(); ++i) if (m_names[i] == n) return (NameKeyType)i; m_names.push_back(n); return (NameKeyType)(m_names.size() - 1); }
	NameKeyType nameToKey(const AsciiString &n) { return nameToKey(n.str()); }
	AsciiString keyToName(NameKeyType k) { return (k >= 0 && (size_t)k < m_names.size()) ? AsciiString(m_names[k].c_str()) : AsciiString(); }
};
NameKeyGenerator g_nkg;
NameKeyGenerator *TheNameKeyGenerator = &g_nkg;
#define NAMEKEY(x) TheNameKeyGenerator->nameToKey(x)
#define KEYNAME(x) TheNameKeyGenerator->keyToName(x)
enum { TheKey_playerName = 1, TheKey_playerIsHuman, TheKey_playerDisplayName, TheKey_playerFaction, TheKey_playerIsPreorder,
	TheKey_playerAllies, TheKey_playerEnemies, TheKey_playerColor, TheKey_playerNightColor, TheKey_multiplayerStartIndex,
	TheKey_multiplayerIsLocal, TheKey_playerIsSkirmish, TheKey_skirmishDifficulty, TheKey_teamName, TheKey_teamOwner,
	TheKey_teamIsSingleton, TheKey_originalOwner, TheKey_InitialCameraPosition = 1000 };

/* ---- dictionaries: what startNewGame sets, logged whole by the sides list ---- */
class Dict
{
public:
	std::map<int, std::string> m_s; std::map<int, int> m_i; std::map<int, std::basic_string<WideChar> > m_u; Int m_index;
	void clear() { m_s.clear(); m_i.clear(); m_u.clear(); }
	void setAsciiString(int k, const AsciiString &v) { m_s[k] = v.str(); }
	void setBool(int k, Bool v) { m_i[k] = v ? 1 : 0; }
	void setInt(int k, Int v) { m_i[k] = v; }
	void setUnicodeString(int k, const UnicodeString &v) { m_u[k] = v.str(); }
	AsciiString getAsciiString(int k) const { std::map<int, std::string>::const_iterator it = m_s.find(k); return it == m_s.end() ? AsciiString() : AsciiString(it->second.c_str()); }
	std::string s(int k) const { std::map<int, std::string>::const_iterator it = m_s.find(k); return it == m_s.end() ? std::string("none") : "[" + it->second + "]"; }
	std::string i(int k) const { std::map<int, int>::const_iterator it = m_i.find(k); char b[16]; if (it == m_i.end()) return "none"; snprintf(b, sizeof b, "%d", it->second); return b; }
	std::string u(int k) const
	{
		std::map<int, std::basic_string<WideChar> >::const_iterator it = m_u.find(k);
		if (it == m_u.end()) return "none";
		std::string r = "[";
		char b[8];
		for (size_t n = 0; n < it->second.size(); ++n) { snprintf(b, sizeof b, "%04x", (unsigned)it->second[n]); r += b; }
		return r + "]";
	}
};
UnicodeString observerDisplayName() { const char *o = "Observer"; std::basic_string<WideChar> w; for (; *o; ++o) w += (WideChar)*o; UnicodeString u; u.set(w.c_str()); return u; }

/* ---- templates, objects, drawables ---- */
const char *g_thingNames[6] = { "ConYardA", "ConYardB", "Tank", "Ranger", "AmericaTankCrusader", "AmericaVehicleHumvee" };
class ThingTemplate
{
public:
	Int m_index;
	Real getPlacementViewAngle() const { return 0.25f * (Real)m_index; }
	/* cfg[28]: bridges; cfg[29]: four kind bits per template (walk on wall, shrubbery, optimized tree, prop);
	** cfg[30]: (cleared by build, reflections, fence width 0) */
	Bool isBridge() const { return (g_cfg[28] >> m_index) & 1; }
	Bool isKindOf(Int k) const
	{
		switch (k) {
		case KINDOF_WALK_ON_TOP_OF_WALL: return (g_cfg[29] >> (4 * m_index)) & 1;
		case KINDOF_SHRUBBERY: return (g_cfg[29] >> (4 * m_index + 1)) & 1;
		case KINDOF_OPTIMIZED_TREE: return (g_cfg[29] >> (4 * m_index + 2)) & 1;
		case KINDOF_PROP: return (g_cfg[29] >> (4 * m_index + 3)) & 1;
		case KINDOF_CLEARED_BY_BUILD: return (g_cfg[30] >> (3 * m_index)) & 1;
		case KINDOF_CAN_CAST_REFLECTIONS: return (g_cfg[30] >> (3 * m_index + 1)) & 1;
		default: return FALSE;
		}
	}
	Real getFenceWidth() const { return ((g_cfg[30] >> (3 * m_index + 2)) & 1) ? 0.0f : 12.5f; }
};
ThingTemplate g_things[6];
class CreateModuleInterface { public: Int m_obj; void onBuildComplete() { logf("build complete %d", (int)m_obj); } };
class BehaviorModule { public: CreateModuleInterface m_create; CreateModuleInterface *getCreate() { return &m_create; } };
class LocomotorSet {};
class AIUpdateInterface { public: LocomotorSet m_set; LocomotorSet &getLocomotorSet() { return m_set; } };
class GeometryInfo { public: Real m_radius; Real getBoundingSphereRadius() const { return m_radius; } };
class Drawable
{
public:
	Int m_id; Drawable *m_next;
	void setOrientation(Real a) { logf("drawable %d angle %.6f", (int)m_id, a); }
	void setPosition(const Coord3D *p) { logf("drawable %d at %.4f %.4f %.4f", (int)m_id, p->x, p->y, p->z); }
	void setDrawableStatus(DrawableStatus s) { logf("drawable %d status %d", (int)m_id, (int)s); }
	void onLevelStart() { logf("drawable %d level start", (int)m_id); }
	Drawable *getNextDrawable() { return m_next; }
};
Drawable g_drawables[80];
Int g_nextDrawable;
class Player;
class Object
{
public:
	Int m_id; const ThingTemplate *m_template; BehaviorModule m_module; BehaviorModule *m_modules[2]; AIUpdateInterface m_ai;
	GeometryInfo m_geom; Drawable *m_drawable; Player *m_controller; Bool m_cc;
	const ThingTemplate *getTemplate() const { return m_template; }
	void setOrientation(Real a) { logf("object %d angle %.6f", (int)m_id, a); }
	void setPosition(const Coord3D *p) { logf("object %d at %.4f %.4f %.4f", (int)m_id, p->x, p->y, p->z); }
	BehaviorModule **getBehaviorModules() { return m_modules; }
	AIUpdateInterface *getAIUpdateInterface() { return (m_template && (m_template->m_index == 2 || m_template->m_index == 3)) ? &m_ai : NULL; }
	Bool isKindOf(Int k) const
	{
		if (k == KINDOF_IMMOBILE) return m_template && m_template->m_index == 3 && (g_cfg[31] & 1);
		if (k == KINDOF_COMMANDCENTER) return m_cc;
		return m_template ? m_template->isKindOf(k) : FALSE;
	}
	const GeometryInfo &getGeometryInfo() const { return m_geom; }
	Drawable *getDrawable() { return m_drawable; }
	void updateObjValuesFromMapProperties(Dict *d) { logf("map values %d %d", (int)m_id, (int)d->m_index); }
	void setLayer(PathfindLayerEnum l) { logf("layer %d %d", (int)m_id, (int)l); }
	Player *getControllingPlayer() { return m_controller; }
	Bool isLocallyControlled() const;
	Bool isMassSelectable() const { return TRUE; }
};
Object g_objs[128];
Int g_nextObj;
class Team { public: Int m_owner; void setActive() { logf("team %d active", (int)m_owner); } };
Team g_teams[16];
class AIGroup
{
public:
	UnsignedInt m_id;
	void add(Object *o) { logf("group %u add %d", m_id, (int)o->m_id); }
};
AIGroup g_groups[64];
UnsignedInt g_nextGroup;
class Player
{
public:
	Int m_index; std::vector<Object *> m_objects;
	Team *getDefaultTeam() { return &g_teams[m_index]; }
	void onStructureCreated(Object *, Object *o) { logf("player %d structure %d", (int)m_index, (int)o->m_id); }
	void onStructureConstructionComplete(Object *, Object *o, Bool b) { logf("player %d complete %d %d", (int)m_index, (int)o->m_id, (int)b); }
	void onUnitCreated(Object *, Object *o) { logf("player %d unit %d", (int)m_index, (int)o->m_id); }
	UnicodeString getPlayerDisplayName() { return UnicodeString(); }
	Int getPlayerIndex() const { return m_index; }
	UnsignedShort getPlayerMask() const { return (UnsignedShort)(1 << m_index); }
	PlayerType getPlayerType() const { return ((g_cfg[17] >> m_index) & 1) ? PLAYER_HUMAN : PLAYER_COMPUTER; }
	void addSkillPoints(Int p) { logf("skill %d %d", (int)m_index, (int)p); }
	void setPlayerRelationship(Player *other, Relationship r) { logf("relationship %d %d %d", (int)m_index, (int)other->m_index, (int)r); }
	Bool isPlayerActive() const { return (g_cfg[18] >> m_index) & 1; }
	void iterateObjects(void (*f)(Object *, void *), void *u) { for (size_t i = 0; i < m_objects.size(); ++i) f(m_objects[i], u); }
	void setCurrentlySelectedAIGroup(AIGroup *g) { if (g) logf("player %d select group %u", (int)m_index, g->m_id); else logf("player %d select none", (int)m_index); }
	void addAIGroupToCurrentSelection(AIGroup *g) { logf("player %d add group %u", (int)m_index, g->m_id); }
};
/* players 0-7 "player0".."player7", 8 "ReplayObserver", 9 "ThePlayer", 10 "PlyrCivilian", 11 the neutral player */
Player g_players[12];
Player *g_localPlayer;
Bool Object::isLocallyControlled() const { return m_controller == g_localPlayer; }

const char *playerNameOf(int i)
{
	static char b[16];
	if (i < 8) { snprintf(b, sizeof b, "player%d", i); return b; }
	return i == 8 ? "ReplayObserver" : i == 9 ? "ThePlayer" : i == 10 ? "PlyrCivilian" : "Neutral";
}

class PlayerList
{
public:
	Player *getNthPlayer(Int i) { return (i >= 0 && i < 12 && ((g_cfg[16] >> i) & 1)) ? &g_players[i] : NULL; }
	Int getPlayerCount() { return 12; }
	Player *getNeutralPlayer() { return FLAG(22) ? &g_players[11] : NULL; }
	Player *getLocalPlayer() { return g_localPlayer; }
	void setLocalPlayer(Player *p) { if (p) logf("local player %d", (int)p->m_index); else logf("local player null"); g_localPlayer = p ? p : getNeutralPlayer(); }
	Player *findPlayerWithNameKey(NameKeyType k)
	{
		AsciiString n = KEYNAME(k);
		/* cfg[31] bit 3: player 9 is not named "ThePlayer" */
		for (int i = 0; i < 11; ++i) if (n == playerNameOf(i) && !(i == 9 && (g_cfg[31] & 8))) return getNthPlayer(i);
		return NULL;
	}
	/* PlayerList.cpp lines 294-308: the team named so, else the neutral player's default team */
	Team *validateTeam(AsciiString owner)
	{
		for (int i = 0; i < 8; ++i) {
			char b[24];
			snprintf(b, sizeof b, "teamplayer%d", i);
			if (owner == b) return &g_teams[i];
		}
		return getNeutralPlayer()->getDefaultTeam();
	}
	UnsignedShort getPlayersWithRelationship(Int, UnsignedInt) { return (UnsignedShort)g_cfg[20]; }
	/* PlayerList.cpp lines 374-393 */
	Player *getEachPlayerFromMask( PlayerMaskType& maskToAdjust )
	{
		Player *player = NULL;
		Int i;
		for( i = 0; i < 16; i++ )
		{
			player = getNthPlayer( i );
			if ( player && BitTest(player->getPlayerMask(), maskToAdjust ))
			{
				maskToAdjust &= (~player->getPlayerMask());
				return player;
			}
		}
		maskToAdjust = 0;
		return NULL;
	}
	void newGame() { logf("playerlist newgame"); }
	void newMap() { logf("playerlist newmap"); }
};
PlayerList g_playerList;
PlayerList *ThePlayerList = &g_playerList;

class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &name)
	{
		for (int i = 0; i < 6; ++i) if (name == g_thingNames[i]) return &g_things[i];
		return NULL;
	}
	Object *newObject(const ThingTemplate *tt, Team *team)
	{
		if ((g_cfg[25] >> tt->m_index) & 1) { logf("newobject refused %d", (int)tt->m_index); return NULL; }
		Object *o = &g_objs[g_nextObj % 128];
		o->m_id = 100 + g_nextObj++;
		o->m_template = tt;
		o->m_module.m_create.m_obj = o->m_id;
		o->m_modules[0] = &o->m_module; o->m_modules[1] = NULL;
		o->m_geom.m_radius = 10.0f + 3.5f * (Real)tt->m_index;
		o->m_drawable = (tt->m_index % 2) ? &g_drawables[g_nextDrawable++ % 80] : NULL;
		if (o->m_drawable) o->m_drawable->m_id = 500 + (Int)(o->m_drawable - g_drawables);
		o->m_controller = NULL; o->m_cc = FALSE;
		logf("new object %d template %d team %d", (int)o->m_id, (int)tt->m_index, team ? (int)team->m_owner : -1);
		return o;
	}
	Drawable *newDrawable(const ThingTemplate *tt, DrawableStatus)
	{
		Drawable *d = &g_drawables[g_nextDrawable++ % 80];
		d->m_id = 500 + (Int)(d - g_drawables);
		logf("new drawable %d template %d", (int)d->m_id, (int)tt->m_index);
		return d;
	}
};
ThingFactory g_tf;
ThingFactory *TheThingFactory = &g_tf;
class Pathfinder
{
public:
	void addObjectToPathfindMap(Object *o) { logf("pathfind map %d", (int)o->m_id); }
	Bool adjustDestination(Object *o, const LocomotorSet &, Coord3D *pos)
	{
		if (g_cfg[31] & 2) { logf("adjust %d none", (int)o->m_id); return FALSE; }
		pos->x += 2.5f; pos->y -= 1.25f;
		logf("adjust %d", (int)o->m_id);
		return TRUE;
	}
	void updateGoal(Object *o, const Coord3D *p, PathfindLayerEnum l) { logf("goal %d %.4f %.4f %d", (int)o->m_id, p->x, p->y, (int)l); }
	void addWallPiece(Object *o) { logf("wall piece %d", (int)o->m_id); }
	void newMap() { logf("pathfinder newmap"); }
};
Pathfinder g_pf;
class AI
{
public:
	Pathfinder *pathfinder() { return &g_pf; }
	AIGroup *createGroup() { AIGroup *g = &g_groups[g_nextGroup % 64]; g->m_id = g_nextGroup++; logf("create group %u", g->m_id); return g; }
	void destroyGroup(AIGroup *g) { logf("destroy group %u", g->m_id); }
};
AI g_ai;
AI *TheAI = &g_ai;
struct FindPositionOptions { Real minRadius, maxRadius; FindPositionOptions() : minRadius(0.0f), maxRadius(0.0f) {} };
class PartitionManager
{
public:
	unsigned m_calls;
	void update() { logf("partition update"); }
	Bool findPositionAround(const Coord3D *pos, const FindPositionOptions *o, Coord3D *result)
	{
		Bool found = ((g_cfg[26] >> (m_calls++ % 32)) & 1) == 0;
		logf("find around %.4f %.4f %.4f r %.6f %.6f -> %d", pos->x, pos->y, pos->z, o->minRadius, o->maxRadius, (int)found);
		if (found) { result->x = pos->x + o->minRadius; result->y = pos->y + o->maxRadius; result->z = pos->z; }
		return found;
	}
	void init() { logf("partition init"); }
	void refreshShroudForLocalPlayer() { logf("partition shroud"); }
	void revealMapForPlayerPermanently(Int i) { logf("reveal permanently %d", (int)i); }
	void revealMapForPlayer(Int i) { logf("reveal %d", (int)i); }
};
PartitionManager g_part;
PartitionManager *ThePartitionManager = &g_part;
class Waypoint { public: Coord3D m_loc; const Coord3D *getLocation() const { return &m_loc; } };
Waypoint g_waypoints[16];
Waypoint *findNamedWaypoint(AsciiString name)
{
	for (int k = 1; k <= 8; ++k) {
		char s[32], r[32];
		snprintf(s, sizeof s, "Player_%d_Start", k);
		snprintf(r, sizeof r, "Player_%d_Rally", k);
		if (name == s && (g_cfg[24] >> (k - 1) & 1)) return &g_waypoints[k - 1];
		if (name == r && (g_cfg[24] >> (k + 7) & 1)) return &g_waypoints[k + 7];
	}
	return NULL;
}
class TerrainLogic
{
public:
	Real getGroundHeight(Real x, Real y) { return x * 0.125f - y * 0.0625f; }
	void loadMap(AsciiString name, Bool b) { logf("loadmap [%s] %d", name.str(), (int)b); }
	void newMap(Bool b) { logf("terrain newmap %d", (int)b); }
	void getExtent(Region3D *r) { r->lo.x = -10.0f; r->lo.y = 5.0f; r->lo.z = 0; r->hi.x = 1270.5f; r->hi.y = 990.25f; r->hi.z = 100; }
	void addLandmarkBridgeToLogic(Object *o) { logf("landmark bridge %d", (int)o->m_id); }
	PathfindLayerEnum getLayerForDestination(const Coord3D *p) { return p->x > 500.0f ? (PathfindLayerEnum)2 : LAYER_GROUND; }
};
TerrainLogic g_terrain;
TerrainLogic *TheTerrainLogic = &g_terrain;

/* ---- player templates, generals, colours, the map cache ---- */
class PlayerTemplate
{
public:
	Int m_index; AsciiString m_building, m_name; AsciiString m_units[MAX_MP_STARTING_UNITS]; Bool m_old;
	const AsciiString &getStartingBuilding() const { return m_building; }
	const AsciiString &getStartingUnit(Int i) const { return m_units[i]; }
	Bool isOldFaction() const { return m_old; }
	const AsciiString &getName() const { return m_name; }
	NameKeyType getNameKey() const { return NAMEKEY(m_name); }
	UnicodeString getDisplayName() const { return UnicodeString(); }
};
PlayerTemplate g_pt[12];
class PlayerTemplateStore
{
public:
	Int getPlayerTemplateCount() { return (Int)(g_cfg[9] % 13); }
	const PlayerTemplate *getNthPlayerTemplate(Int i) { return (i >= 0 && i < getPlayerTemplateCount() && !((g_cfg[10] >> i) & 1)) ? &g_pt[i] : NULL; }
	const PlayerTemplate *findPlayerTemplate(NameKeyType k)
	{
		for (Int i = 0; i < getPlayerTemplateCount(); ++i) if (getNthPlayerTemplate(i) && g_pt[i].getNameKey() == k) return &g_pt[i];
		return NULL;
	}
};
PlayerTemplateStore g_pts;
PlayerTemplateStore *ThePlayerTemplateStore = &g_pts;
class GeneralPersona { public: Bool m_enabled; Bool isStartingEnabled() const { return m_enabled; } };
GeneralPersona g_generals[12];
class ChallengeGenerals
{
public:
	const GeneralPersona *getGeneralByTemplateName(const AsciiString &name)
	{
		for (int i = 0; i < 12; ++i) if (name == g_pt[i].m_name && ((g_cfg[13] >> i) & 1)) return &g_generals[i];
		return NULL;
	}
};
ChallengeGenerals g_cg;
ChallengeGenerals *TheChallengeGenerals = &g_cg;
class MultiplayerColorDefinition { public: Int m_c, m_n; Int getColor() const { return m_c; } Int getNightColor() const { return m_n; } };
MultiplayerColorDefinition g_colors[10];
class MultiplayerSettings
{
public:
	Int getNumColors() { return (Int)(g_cfg[8] % 10); }
	MultiplayerColorDefinition *getColor(Int i) { return (i >= 0 && i < getNumColors()) ? &g_colors[i] : NULL; }
	Bool isShroudInMultiplayer() { return FLAG(13); }
};
MultiplayerSettings g_mps;
MultiplayerSettings *TheMultiplayerSettings = &g_mps;
struct AsciiLess { bool operator()(const AsciiString &a, const AsciiString &b) const { return strcmp(a.str(), b.str()) < 0; } };
class WaypointMap : public std::map<AsciiString, Coord3D, AsciiLess> {};
class MapMetaData { public: Int m_numPlayers; WaypointMap m_waypoints; };
MapMetaData g_md;
class MapCache
{
public:
	const MapMetaData *findMap(AsciiString mapName)
	{
		mapName.toLower();
		if (mapName == "maps\\one\\one.map") return &g_md;
		return NULL;
	}
};
MapCache g_mc;
MapCache *TheMapCache = NULL;
class MapObject
{
public:
	AsciiString m_name; const ThingTemplate *m_tt; Int m_flags; Coord3D m_loc; Real m_angle; Dict m_props; MapObject *m_next;
	static MapObject *s_first;
	const AsciiString &getName() const { return m_name; }
	void setName(const AsciiString &n) { m_name = n; }
	void setThingTemplate(const ThingTemplate *t) { m_tt = t; }
	const ThingTemplate *getThingTemplate() const { return m_tt; }
	Bool getFlag(Int f) const { return (m_flags & f) != 0; }
	const Coord3D *getLocation() const { return &m_loc; }
	Real getAngle() const { return m_angle; }
	Dict *getProperties() { return &m_props; }
	MapObject *getNext() { return m_next; }
	static MapObject *getFirstMapObject() { return s_first; }
};
MapObject *MapObject::s_first = NULL;
MapObject g_mapObjects[8];

/* ---- the rest of the engine ---- */
class LoadScreen
{
public:
	const char *m_kind;
	virtual ~LoadScreen() { logf("loadscreen %s deleted", m_kind); }
	void init(GameInfo *g) { logf("loadscreen %s init %d", m_kind, g ? 1 : 0); }
	void update(Int p) { logf("loadscreen %s update %d", m_kind, (int)p); }
};
class ShellGameLoadScreen : public LoadScreen { public: ShellGameLoadScreen() { m_kind = "shell"; } };
class SinglePlayerLoadScreen : public LoadScreen { public: SinglePlayerLoadScreen() { m_kind = "singleplayer"; } };
class ChallengeLoadScreen : public LoadScreen { public: ChallengeLoadScreen() { m_kind = "challenge"; } };
class MultiPlayerLoadScreen : public LoadScreen { public: MultiPlayerLoadScreen() { m_kind = "multiplayer"; } };
class GameSpyLoadScreen : public LoadScreen { public: GameSpyLoadScreen() { m_kind = "gamespy"; } };
class Campaign { public: Bool m_isChallengeCampaign; };
Campaign g_campaign;
class CampaignManager
{
public:
	Campaign *getCurrentCampaign() { return FLAG(3) ? &g_campaign : NULL; }
	void SetVictorious(Bool b) { logf("campaign victorious %d", (int)b); }
};
CampaignManager g_cm;
CampaignManager *TheCampaignManager = &g_cm;
class WindowLayout { public: void destroyWindows() { logf("background destroyed"); } void deleteInstance() {} };
WindowLayout g_background;
struct GlobalData
{
	AsciiString m_mapName; Bool m_loadScreenRender, m_TiVOFastMode, m_useTrees, m_useShadowVolumes, m_preloadAssets, m_preloadEverything;
	TimeOfDay m_timeOfDay; Int m_playStats;
};
GlobalData g_gd;
GlobalData *TheGlobalData = &g_gd;
GlobalData *TheWritableGlobalData = &g_gd;
struct SaveGameInfo { AsciiString pristineMapName; };
class GameState
{
public:
	SaveGameInfo m_info;
	void setPristineMapName(const AsciiString &n) { m_info.pristineMapName = n; logf("pristine [%s]", n.str()); }
	SaveGameInfo *getSaveGameInfo() { return &m_info; }
	Bool isInSaveDirectory(const AsciiString &p) const { return strncmp(p.str(), "Save\\", 5) == 0; }
};
GameState g_gs;
GameState *TheGameState = &g_gs;
class LANAPI { public: GameInfo *m_game; GameInfo *GetMyGame() { return m_game; } };
LANAPI g_lan;
LANAPI *TheLAN = NULL;
class Network
{
public:
	void loadProgressComplete() { logf("network loadcomplete"); }
	void liteupdate() { logf("network liteupdate"); }
};
Network g_net;
Network *TheNetwork = NULL;
/* TheGameSpyGame is a GameSpyStagingRoom (a GameInfo with a name) */
class GameSpyStagingRoom : public SkirmishGameInfo { public: UnicodeString getGameName() { return UnicodeString(); } };
GameSpyStagingRoom *TheGameSpyGame = NULL;
class RecorderClass
{
public:
	GameInfo *m_game;
	RecorderModeType getMode() { return FLAG(8) ? RECORDERMODETYPE_PLAYBACK : RECORDERMODETYPE_RECORD; }
	GameInfo *getGameInfo() { return m_game; }
	Bool isMultiplayer() { return FLAG(9); }
	void initControls() { logf("recorder initcontrols"); }
	Int getGameMode() { return (Int)(g_cfg[4] % 7); }
};
RecorderClass g_rec;
RecorderClass *TheRecorder = NULL;
class FileSystem
{
public:
	Bool doesFileExist(const char *p)
	{
		Bool e = (strstr(p, "map.ini") && (g_cfg[5] & 1)) || (strstr(p, "solo.ini") && (g_cfg[5] & 2)) || (strstr(p, "map.str") && (g_cfg[5] & 4));
		logf("exists [%s] %d", p, (int)e);
		return e;
	}
};
FileSystem g_fs;
FileSystem *TheFileSystem = &g_fs;
class INI { public: void load(AsciiString p, INILoadType t, void *) { logf("ini [%s] %d", p.str(), (int)t); } };
class GameTextInterface
{
public:
	void initMapStringFile(const char *p) { logf("mapstr [%s]", p); }
	UnicodeString fetch(const char *) { return UnicodeString(); }
};
GameTextInterface g_text;
GameTextInterface *TheGameText = &g_text;
class Display
{
public:
	void doSmartAssetPurgeAndPreload(const char *p) { logf("assets [%s]", p); }
	void draw() { logf("display draw"); }
	Bool isMoviePlaying() { return FALSE; }
};
Display g_display;
Display *TheDisplay = NULL;
class GameEngine { public: Bool isMultiplayerSession() { return FLAG(12); } };
GameEngine g_engine;
GameEngine *TheGameEngine = &g_engine;
class ScriptList;
class SidesInfo { public: ScriptList *getScriptList(); };
class SidesList
{
public:
	SidesInfo m_side0;
	void prepareForMP_or_Skirmish() { logf("sides prepare"); }
	void addSide(Dict *d)
	{
		logf("side name=%s human=%s display=%s faction=%s preorder=%s allies=%s enemies=%s color=%s night=%s start=%s local=%s skirmish=%s difficulty=%s",
			d->s(TheKey_playerName).c_str(), d->i(TheKey_playerIsHuman).c_str(), d->u(TheKey_playerDisplayName).c_str(),
			d->s(TheKey_playerFaction).c_str(), d->i(TheKey_playerIsPreorder).c_str(), d->s(TheKey_playerAllies).c_str(),
			d->s(TheKey_playerEnemies).c_str(), d->i(TheKey_playerColor).c_str(), d->i(TheKey_playerNightColor).c_str(),
			d->i(TheKey_multiplayerStartIndex).c_str(), d->i(TheKey_multiplayerIsLocal).c_str(), d->i(TheKey_playerIsSkirmish).c_str(),
			d->i(TheKey_skirmishDifficulty).c_str());
	}
	void addTeam(Dict *d) { logf("team name=%s owner=%s singleton=%s", d->s(TheKey_teamName).c_str(), d->s(TheKey_teamOwner).c_str(), d->i(TheKey_teamIsSingleton).c_str()); }
	void validateSides() { logf("sides validate"); }
	SidesInfo *getSideInfo(Int) { return &m_side0; }
};
SidesList g_sides;
SidesList *TheSidesList = &g_sides;
/* the multiplayer scripts file: cfg[7] & 3: 0 does not open, 1 the parse fails, else read: (cfg[7] >> 2) % 4 lists
** whose first holds (cfg[7] >> 4) % 4 scripts */
class Script
{
public:
	Int m_n; Script *m_next;
	Script *duplicate() { return this; }
	Script *getNext() { return m_next; }
};
Script g_scripts[4];
class ScriptList
{
public:
	Script *m_first;
	Script *getScript() { return m_first; }
	void addScript(Script *s, Int at) { logf("mpscripts add %d at %d", (int)s->m_n, (int)at); }
	void deleteInstance() {}
	static Bool ParseScriptsDataChunk(void *, void *, void *) { return TRUE; }
	static Int getReadScripts(ScriptList **lists)
	{
		Int count = (Int)((g_cfg[7] >> 2) % 4);
		static ScriptList l[4];
		Int n = (Int)((g_cfg[7] >> 4) % 4);
		for (Int i = 0; i < n; ++i) { g_scripts[i].m_n = i; g_scripts[i].m_next = i + 1 < n ? &g_scripts[i + 1] : NULL; }
		for (Int i = 0; i < count; ++i) { l[i].m_first = i == 0 && n ? &g_scripts[0] : NULL; lists[i] = &l[i]; }
		return count;
	}
};
ScriptList g_side0Scripts;
ScriptList *SidesInfo::getScriptList() { return &g_side0Scripts; }
class ChunkInputStream {};
class CachedFileInputStream : public ChunkInputStream
{
public:
	Bool open(AsciiString p) { Bool ok = (g_cfg[7] & 3) != 0; logf("mpscripts open [%s] %d", p.str(), (int)ok); return ok; }
};
class DataChunkInput
{
public:
	DataChunkInput(ChunkInputStream *) {}
	void registerParser(AsciiString, AsciiString, Bool (*)(void *, void *, void *)) {}
	Bool parse(void *) { Bool ok = (g_cfg[7] & 3) != 1; logf("mpscripts parse %d", (int)ok); return ok; }
};
class TeamFactory { public: void reset() { logf("teamfactory reset"); } };
TeamFactory g_teamFactory;
TeamFactory *TheTeamFactory = &g_teamFactory;
class ScriptEngine { public: void newMap() { logf("script newmap"); } };
ScriptEngine g_se;
ScriptEngine *TheScriptEngine = &g_se;
class Radar
{
public:
	void newMap(TerrainLogic *) { logf("radar newmap"); }
	void refreshTerrain(TerrainLogic *) { logf("radar refresh"); }
	void forceOn(Bool b) { logf("radar forceon %d", (int)b); }
};
Radar g_radar;
Radar *TheRadar = &g_radar;
class InGameUI
{
public:
	void setClientQuiet(Bool b) { logf("ui quiet %d", (int)b); }
	void message(UnicodeString) { logf("ui message"); }
	void selectDrawable(Drawable *d) { logf("ui select %d", (int)d->m_id); }
};
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;
class VictoryConditions
{
public:
	void cachePlayerPtrs() { logf("victory cache"); }
	void setVictoryConditions(Int c) { logf("victory set %d", (int)c); }
};
VictoryConditions g_vc;
VictoryConditions *TheVictoryConditions = &g_vc;
class GhostObjectManager
{
public:
	void setLocalPlayerIndex(Int i) { logf("ghost local %d", (int)i); }
	void reset() { logf("ghost reset"); }
};
GhostObjectManager g_ghost;
GhostObjectManager *TheGhostObjectManager = &g_ghost;
class TerrainVisual { public: void addProp(const ThingTemplate *t, const Coord3D *p, Real a) { logf("prop %d %.4f %.4f %.4f %.6f", (int)t->m_index, p->x, p->y, p->z, a); } };
TerrainVisual g_tv;
TerrainVisual *TheTerrainVisual = &g_tv;
class GameLODManager { public: StaticGameLODLevel getStaticLODLevel() { Int l = (Int)(g_cfg[21] % 3); return l == 0 ? STATIC_GAME_LOD_HIGH : l == 1 ? STATIC_GAME_LOD_CUSTOM : STATIC_GAME_LOD_LOW; } };
GameLODManager g_lod;
GameLODManager *TheGameLODManager = &g_lod;
class GameClient
{
public:
	void destroyDrawable(Drawable *d) { logf("drawable %d destroyed", (int)d->m_id); }
	void preloadAssets(TimeOfDay t) { logf("preload %d", (int)t); }
	Drawable *getDrawableList() { return g_cfg[32] % 4 ? &g_drawables[70] : NULL; }
};
GameClient g_client;
GameClient *TheGameClient = &g_client;
class ControlBar
{
public:
	void hideCommunicator(Bool b) { logf("communicator %d", (int)b); }
	void setControlBarSchemeByPlayer(Player *) { logf("controlbar scheme"); }
	void initSpecialPowershortcutBar(Player *) { logf("controlbar specialpower"); }
};
ControlBar g_controlBar;
ControlBar *TheControlBar = &g_controlBar;
void HideControlBar() { logf("controlbar hide"); }
void ShowControlBar(Bool b) { logf("controlbar show %d", (int)b); }
class View
{
public:
	void setAngleAndPitchToDefault() { logf("view angle default"); }
	void setZoomToDefault() { logf("view zoom default"); }
	void lookAt(const Coord3D *p) { logf("lookat %.4f %.4f %.4f", p->x, p->y, p->z); }
	void initHeightForMap() { logf("view initheight"); }
	void setOkToAdjustHeight(Bool b) { logf("view adjust height %d", (int)b); }
};
View g_view;
View *TheTacticalView = &g_view;
class Mouse { public: void setVisibility(Bool b) { logf("mouse visible %d", (int)b); } };
Mouse g_mouse;
Mouse *TheMouse = &g_mouse;
class TransitionHandler
{
public:
	unsigned m_calls;
	void setGroup(AsciiString g) { logf("transition [%s]", g.str()); }
	Bool isFinished() { Bool f = m_calls++ >= g_cfg[22] % 4; logf("transition finished %d", (int)f); return f; }
};
TransitionHandler g_th;
TransitionHandler *TheTransitionHandler = &g_th;
class WindowManager { public: void update() { logf("windows update"); } };
WindowManager g_wm;
WindowManager *TheWindowManager = &g_wm;
class WindowLayoutShell { public: void hide(Bool b) { logf("shell top hide %d", (int)b); } void bringForward() { logf("shell top forward"); } };
WindowLayoutShell g_shellTop;
class Shell
{
public:
	Int getScreenCount() { return (Int)(g_cfg[33] % 2); }
	void push(AsciiString n) { logf("shell push [%s]", n.str()); }
	WindowLayoutShell *top() { return &g_shellTop; }
};
Shell g_shell;
Shell *TheShell = &g_shell;
class StatsCollector { public: void reset() { logf("stats reset"); } };
StatsCollector g_stats;
StatsCollector *TheStatsCollector = NULL;
struct BuddyRequest
{
	enum { BUDDYREQUEST_SETSTATUS = 7 };
	Int buddyRequestType;
	struct { struct { Int status; char statusString[64]; char locationString[256]; } status; } arg;
};
class GameSpyBuddyMessageQueue { public: void addRequest(const BuddyRequest &r) { logf("buddy %d %d [%s] [%s]", (int)r.buddyRequestType, (int)r.arg.status.status, r.arg.status.statusString, r.arg.status.locationString); } };
GameSpyBuddyMessageQueue g_buddy;
GameSpyBuddyMessageQueue *TheGameSpyBuddyMessageQueue = NULL;
std::string WideCharStringToMultiByte(const WideChar *) { return std::string("lobby"); }
class GameSpyInfo { public: void updateAdditionalGameSpyDisconnections(Int n) { logf("gamespy disconnections %d", (int)n); } };
GameSpyInfo g_gsi;
GameSpyInfo *TheGameSpyInfo = NULL;

/* ---- GameLogic ---- */
class GameLogic
{
public:
	Int m_gameMode, m_rankLevelLimit, m_scriptHulkMaxLifetimeOverride, m_rankPointsToAddAtGameStart;
	Bool m_startNewGame, m_showBehindBuildingMarkers, m_drawIconUI, m_showDynamicLOD, m_forceGameStartByTimeOut, m_loadingMap;
	UnsignedShort m_superweaponRestriction;
	UnsignedInt m_frame;
	Real m_width, m_height;
	LoadScreen *m_loadScreen;
	WindowLayout *m_background;
	Bool m_progressComplete[MAX_SLOTS];
	Int m_progressCompleteTimeout[MAX_SLOTS];
	enum { PROGRESS_COMPLETE_TIMEOUT = 60000 };
	void setLoadingMap(Bool b) { m_loadingMap = b; logf("loadingmap %d", (int)b); }
	void setDefaults(Bool b) { logf("setdefaults %d", (int)b); }
	void setWidth(Real w) { m_width = w; logf("width %.4f", w); }
	void setHeight(Real h) { m_height = h; logf("height %.4f", h); }
	Bool isInMultiplayerGame() { return ((m_gameMode == GAME_LAN) || (m_gameMode == GAME_INTERNET)); }   /* GameLogic.h lines 403-407 */
	Bool isInReplayGame() { return (m_gameMode == GAME_REPLAY); }
	Bool isInInternetGame() { return (m_gameMode == GAME_INTERNET); }
	Bool isInSinglePlayerGame();
	LoadScreen *getLoadScreen(Bool loadingSaveGame);
	void updateLoadProgress(Int progress);
	void deleteLoadScreen();
	void startNewGame(Bool loadingSaveGame);
	void loadMapINI(AsciiString mapName);
	void selectObject(Object *obj, Bool createNewSelection, PlayerMaskType playerMask, Bool affectClient = FALSE);
	Bool isProgressComplete();
	void lastHeardFrom(Int playerId);
	void testTimeOut();
};
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
static void findAndSelectCommandCenter(Object *obj, void* alreadyFound);   /* GameLogic.cpp line 146 */

/* ---- the upstream code ---- */
#include "game_logic_new_game_cut.inc"

/* ---- the test ---- */
SkirmishGameInfo *g_games[5];   /* LAN, GameSpy, recorder, skirmish, challenge */
void *g_gameMem[5];

void dumpEnd()
{
	const char *src = "none";
	for (int i = 0; i < 5; ++i) if (TheGameInfo && TheGameInfo == g_games[i]) src = i == 0 ? "lan" : i == 1 ? "gamespy" : i == 2 ? "recorder" : i == 3 ? "skirmish" : "challenge";
	logf("end frame %u ranklimit %d render %d tivo %d markers %d icon %d lod %d hulk %d superweapon %u pending %d loadingmap %d gameinfo %s skirmishinfo %d loadscreen %d force %d width %.4f height %.4f",
		g_logic.m_frame, (int)g_logic.m_rankLevelLimit, (int)g_gd.m_loadScreenRender, (int)g_gd.m_TiVOFastMode,
		(int)g_logic.m_showBehindBuildingMarkers, (int)g_logic.m_drawIconUI, (int)g_logic.m_showDynamicLOD,
		(int)g_logic.m_scriptHulkMaxLifetimeOverride, (unsigned)g_logic.m_superweaponRestriction, (int)g_logic.m_startNewGame,
		(int)g_logic.m_loadingMap, src, TheSkirmishGameInfo ? 1 : 0, g_logic.m_loadScreen ? 1 : 0,
		(int)g_logic.m_forceGameStartByTimeOut, g_logic.m_width, g_logic.m_height);
	std::string pc;
	for (int i = 0; i < MAX_SLOTS; ++i) { char t[24]; snprintf(t, sizeof t, " %d:%d", (int)g_logic.m_progressComplete[i], (int)g_logic.m_progressCompleteTimeout[i]); pc += t; }
	logf("progress%s", pc.c_str());
	for (int k = 0; k < 5; ++k) {
		if (!g_games[k]) continue;
		std::string s;
		for (int i = 0; i < MAX_SLOTS; ++i) {
			const GameSlot *sl = g_games[k]->getConstSlot(i);
			char t[64];
			snprintf(t, sizeof t, " %d:%d:%d:%d:%d", (int)sl->getState(), (int)sl->getColor(), (int)sl->getStartPos(), (int)sl->getPlayerTemplate(), (int)sl->getTeamNumber());
			s += t;
		}
		logf("game %d slots%s", k, s.c_str());
	}
	logf("seed %u mapobjects %s %s", GetGameLogicRandomSeed(), g_mapObjects[0].m_name.str(), g_mapObjects[0].m_tt ? "t" : "-");
}

int run(const unsigned *cfg, const unsigned *slots, char *out, int cap)
{
	g_log.clear();
	memcpy(g_cfg, cfg, sizeof g_cfg);
	unsigned ginfo_cfg[9] = { 0, 8, 12, 1000, 0, 0, 0, 0, 0 };
	ref_ginfo_configure(ginfo_cfg);
	InitGameLogicRandom(g_cfg[0]);
	g_nkg.m_names.clear();
	g_nkg.m_names.push_back("");
	g_nextObj = 0; g_nextDrawable = 0; g_nextGroup = 100; g_part.m_calls = 0; g_th.m_calls = 0; g_time = g_cfg[34];
	for (int i = 0; i < 6; ++i) g_things[i].m_index = i;
	for (int i = 0; i < 12; ++i) { g_players[i].m_index = i; g_players[i].m_objects.clear(); g_teams[i].m_owner = i; }
	g_localPlayer = FLAG(23) ? &g_players[g_cfg[35] % 11] : NULL;
	for (int i = 0; i < 8; ++i) {
		/* each player owns two objects; either may be a command center (cfg[19]: bit i the second, bit i + 8 the first) */
		for (int k = 0; k < 2; ++k) {
			Object *o = &g_objs[112 + i * 2 + k];
			o->m_id = 200 + i * 2 + k; o->m_template = NULL; o->m_controller = (g_cfg[36] >> i) & 1 ? NULL : &g_players[i];
			o->m_cc = (g_cfg[19] >> (i + 8 * (1 - k))) & 1; o->m_drawable = NULL;
			g_players[i].m_objects.push_back(o);
		}
	}
	for (int i = 0; i < 12; ++i) {
		char nb[32];
		g_pt[i].m_index = i;
		snprintf(nb, sizeof nb, "Faction%d", i);
		g_pt[i].m_name = AsciiString(i == (int)(g_cfg[12] % 16) ? "FactionObserver" : nb);
		g_pt[i].m_building = ((g_cfg[11] >> i) & 1) ? AsciiString(g_thingNames[i % 2]) : AsciiString();
		for (int u = 0; u < MAX_MP_STARTING_UNITS; ++u) g_pt[i].m_units[u] = ((i + u) % 4 == 0) ? AsciiString(g_thingNames[2 + (u % 2)]) : AsciiString();
		g_pt[i].m_old = (Bool)((g_cfg[15] >> i) & 1);
		g_generals[i].m_enabled = (Bool)((g_cfg[14] >> i) & 1);
	}
	for (int i = 0; i < 10; ++i) { g_colors[i].m_c = 0x110000 * (i + 1); g_colors[i].m_n = 0x001100 * (i + 1); }
	for (int k = 0; k < 16; ++k) {
		g_waypoints[k].m_loc.x = 100.0f * (Real)(k % 8) + 7.0f * (Real)(k / 8);
		g_waypoints[k].m_loc.y = 37.5f * (Real)((k * 5) % 8);
		g_waypoints[k].m_loc.z = 0;
	}
	g_md.m_numPlayers = 8;
	g_md.m_waypoints.clear();
	for (int k = 0; k < 8; ++k) {
		if (!((g_cfg[24] >> k) & 1)) continue;
		char s[32];
		snprintf(s, sizeof s, "Player_%d_Start", k + 1);
		g_md.m_waypoints[AsciiString(s)] = g_waypoints[k].m_loc;
	}
	/* map objects: cfg[27] holds four bits per object (a template index 0-5, or none) */
	Int nobj = (Int)(g_cfg[26 + 11] % 9);
	static const char *objNames[4] = { "AmericaTankLeopard", "Thing", "AmericaVehicleHumVee", "Rock" };
	for (Int i = 0; i < nobj; ++i) {
		MapObject &m = g_mapObjects[i];
		unsigned t = (g_cfg[27] >> (4 * i)) & 7;
		m.m_tt = t < 6 ? &g_things[t] : NULL;
		m.m_name = AsciiString(objNames[(g_cfg[38] >> (2 * i)) & 3]);
		m.m_flags = (Int)((g_cfg[39] >> (3 * i)) & 7);
		m.m_loc.x = 160.0f * (Real)i + 3.5f; m.m_loc.y = 90.0f * (Real)(i % 3); m.m_loc.z = 1.5f;
		m.m_angle = -7.0f + 2.75f * (Real)i;
		m.m_props.clear(); m.m_props.m_index = i;
		if ((g_cfg[38] >> (16 + i)) & 1) {
			char b[24];
			snprintf(b, sizeof b, "teamplayer%d", (int)(i % 8));
			m.m_props.setAsciiString(TheKey_originalOwner, AsciiString(b));
		}
		m.m_next = i + 1 < nobj ? &g_mapObjects[i + 1] : NULL;
	}
	MapObject::s_first = nobj ? &g_mapObjects[0] : NULL;
	if (!nobj) { g_mapObjects[0].m_name = AsciiString("none"); g_mapObjects[0].m_tt = NULL; }
	for (int i = 0; i < 4; ++i) { g_drawables[70 + i].m_id = 570 + i; g_drawables[70 + i].m_next = i + 1 < (int)(g_cfg[32] % 4) ? &g_drawables[71 + i] : NULL; }
	/* the global data and the engine's singletons */
	static const char *maps[4] = { "Maps\\One\\One.map", "Save\\00000001.map", "x.map", "Maps\\Two\\Two.map" };
	g_gd.m_mapName = AsciiString(maps[g_cfg[6] % 4]);
	g_gd.m_loadScreenRender = FALSE; g_gd.m_TiVOFastMode = TRUE; g_gd.m_useTrees = FLAG(14); g_gd.m_useShadowVolumes = FLAG(15);
	g_gd.m_preloadAssets = FLAG(16); g_gd.m_preloadEverything = FLAG(17); g_gd.m_timeOfDay = TIME_OF_DAY_AFTERNOON;
	g_gd.m_playStats = FLAG(19) ? 1 : 0;
	g_gs.m_info.pristineMapName = AsciiString("Maps\\Pristine\\Pristine.map");
	g_campaign.m_isChallengeCampaign = FLAG(4);
	TheNetwork = FLAG(5) ? &g_net : NULL;
	TheLAN = FLAG(6) ? &g_lan : NULL;
	TheRecorder = FLAG(7) ? &g_rec : NULL;
	TheMapCache = &g_mc;   /* populateRandomStartPosition dereferences it unchecked */
	TheDisplay = &g_display;   /* the transition loop draws it unchecked */
	TheStatsCollector = FLAG(18) ? &g_stats : NULL;
	TheGameSpyBuddyMessageQueue = FLAG(20) ? &g_buddy : NULL;
	TheGameSpyInfo = FLAG(21) ? &g_gsi : NULL;
	TheGameText = FLAG(25) ? &g_text : NULL;
	/* the games' options */
	for (int k = 0; k < 5; ++k) {
		g_games[k] = NULL;
		if (!((g_cfg[3] >> k) & 1)) continue;
		g_gameMem[k] = calloc(1, sizeof(GameSpyStagingRoom));
		SkirmishGameInfo *g = k == 1 ? new (g_gameMem[k]) GameSpyStagingRoom : new (g_gameMem[k]) SkirmishGameInfo;
		g->m_mapName = AsciiString("maps\\one\\one.map");
		g->m_superweaponRestriction = (UnsignedShort)(k + 3);
		g->m_inGame = (Bool)((g_cfg[31] >> 2) & 1);
		g->m_localIP = 77;
		for (int i = 0; i < MAX_SLOTS; ++i) {
			GameSlot *s = g->getSlot(i);
			const unsigned *v = slots + 5 * i;
			s->m_state = (SlotState)(v[0] % 6);
			s->m_color = (Int)v[1];
			s->m_startPos = (Int)v[2];
			s->m_playerTemplate = (Int)v[3];
			s->m_teamNumber = (Int)v[4];
			WideChar nm[3] = { (WideChar)('A' + i % 3), (WideChar)('a' + i), 0 };
			s->m_name.set(nm);
			s->m_IP = (i == (int)(g_cfg[35] % 8)) ? 77 : 0;
		}
		g_games[k] = g;
	}
	g_lan.m_game = g_games[0];
	TheGameSpyGame = static_cast<GameSpyStagingRoom *>(g_games[1]);
	g_rec.m_game = g_games[2];
	TheSkirmishGameInfo = g_games[3];
	TheChallengeGameInfo = g_games[4];
	TheGameInfo = NULL;
	/* GameLogic */
	g_logic.m_gameMode = (Int)(g_cfg[2] % 7);
	g_logic.m_rankLevelLimit = 7; g_logic.m_scriptHulkMaxLifetimeOverride = 5; g_logic.m_rankPointsToAddAtGameStart = (Int)(g_cfg[16] >> 16);
	g_logic.m_startNewGame = FLAG(1); g_logic.m_showBehindBuildingMarkers = FALSE; g_logic.m_drawIconUI = FALSE; g_logic.m_showDynamicLOD = FALSE;
	g_logic.m_forceGameStartByTimeOut = FALSE; g_logic.m_loadingMap = FALSE; g_logic.m_superweaponRestriction = 9; g_logic.m_frame = 55;
	g_logic.m_width = 0; g_logic.m_height = 0;
	g_logic.m_loadScreen = NULL;
	g_logic.m_background = FLAG(2) ? &g_background : NULL;
	for (int i = 0; i < MAX_SLOTS; ++i) { g_logic.m_progressComplete[i] = FALSE; g_logic.m_progressCompleteTimeout[i] = 0; }
	g_logic.startNewGame((Bool)FLAG(0));
	/* startNewGame may have deleted TheSkirmishGameInfo (a single-player game without options) */
	if (g_games[3] && TheSkirmishGameInfo == NULL) g_games[3] = NULL;
	logf("started");
	dumpEnd();
	g_logic.m_loadScreen = NULL;   /* left to leak: its destructor logs */
	for (int k = 0; k < 5; ++k) {
		if (!g_games[k]) continue;
		if (TheSkirmishGameInfo == g_games[k]) TheSkirmishGameInfo = NULL;
		g_games[k]->~SkirmishGameInfo();
		free(g_gameMem[k]);
		g_games[k] = NULL;
	}
	TheSkirmishGameInfo = NULL; TheChallengeGameInfo = NULL; TheGameInfo = NULL;
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}

} // namespace gln

extern "C" int ref_glnew_run(const unsigned *cfg, const unsigned *slots, char *out, int cap)
{
	return gln::run(cfg, slots, out, cap);
}
