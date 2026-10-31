/* Differential-test wrapper for startNewGame's helpers (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp, cut verbatim by build.rs into
**   OUT_DIR/glstart_gen/game_logic_start_cut.inc: from placeObjectAtPosition to the end of populateRandomStartPosition
**   (placeObjectAtPosition, placeNetworkBuildingsForPlayer, GameLogic::getLoadScreen, handleNameChange,
**   checkForDuplicateColors, populateRandomSideAndColor, populateRandomStartPosition).
** Compiled inside namespace gls on stand-ins that log what acts: the thing factory and its objects (templates by name,
** each with a placement angle, mobile or not, a bounding radius), players and their default teams, the pathfinder,
** the partition manager (findPositionAround answers from the test's choice), waypoints, the terrain's height, the
** player templates (starting building and units, old faction), the challenge generals, the multiplayer settings'
** colour count, the map cache (one map: its player count and start waypoints), the campaign manager and the load
** screens. The game is the GameInfo build's real SkirmishGameInfo (wrap_game_info.cpp, under its names, configured
** by ref_ginfo_configure) and the random numbers the RandomValue build's real game-logic generator.
**
** Entry point (extern "C", tests/differential_game_logic_start.rs):
**   ref_glstart_run(cfg, slots, ops, n, out, cap)  runs n operations of six words and writes the log */
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

#ifndef CRCDEBUG_LOG
#define CRCDEBUG_LOG(x)
#endif
#ifndef DUMPCOORD3D
#define DUMPCOORD3D(x)
#endif

namespace gls {

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

unsigned g_cfg[16];
enum { GAME_SINGLE_PLAYER, GAME_LAN, GAME_SKIRMISH, GAME_REPLAY, GAME_SHELL, GAME_INTERNET, GAME_NONE };
enum { KINDOF_IMMOBILE = 1 };
enum PathfindLayerEnum { LAYER_INVALID = 0, LAYER_GROUND = 1 };
enum { MAX_MP_STARTING_UNITS = 10 };   /* PlayerTemplate.h line 70 */
inline Real sqr(Real x) { return x * x; }

/* ---- templates, objects, players ---- */
const char *g_thingNames[6] = { "ConYardA", "ConYardB", "Tank", "Ranger", "AmericaTankCrusader", "AmericaVehicleHumvee" };
class ThingTemplate { public: Int m_index; Real getPlacementViewAngle() const { return 0.25f * (Real)m_index; } };
ThingTemplate g_things[6];
class CreateModuleInterface { public: Int m_obj; void onBuildComplete() { logf("build complete %d", (int)m_obj); } };
class BehaviorModule { public: CreateModuleInterface m_create; CreateModuleInterface *getCreate() { return &m_create; } };
class LocomotorSet {};
class AIUpdateInterface { public: LocomotorSet m_set; LocomotorSet &getLocomotorSet() { return m_set; } };
class GeometryInfo { public: Real m_radius; Real getBoundingSphereRadius() const { return m_radius; } };
class Object
{
public:
	Int m_id; const ThingTemplate *m_template; BehaviorModule m_module; BehaviorModule *m_modules[2]; AIUpdateInterface m_ai;
	GeometryInfo m_geom;
	const ThingTemplate *getTemplate() const { return m_template; }
	void setOrientation(Real a) { logf("object %d angle %.6f", (int)m_id, a); }
	void setPosition(const Coord3D *p) { logf("object %d at %.4f %.4f %.4f", (int)m_id, p->x, p->y, p->z); }
	BehaviorModule **getBehaviorModules() { return m_modules; }
	/* tanks and rangers move */
	AIUpdateInterface *getAIUpdateInterface() { return (m_template->m_index == 2 || m_template->m_index == 3) ? &m_ai : NULL; }
	Bool isKindOf(Int k) const { return k == KINDOF_IMMOBILE && m_template->m_index == 3 && (g_cfg[9] & 1); }
	const GeometryInfo &getGeometryInfo() const { return m_geom; }
};
Object g_objs[64];
Int g_nextObj;
class Team { public: Int m_owner; void setActive() { logf("team %d active", (int)m_owner); } };
Team g_teams[8];
class Player
{
public:
	Int m_index;
	Team *getDefaultTeam() { return (g_cfg[10] >> m_index & 1) ? NULL : &g_teams[m_index]; }
	void onStructureCreated(Object *, Object *o) { logf("player %d structure %d", (int)m_index, (int)o->m_id); }
	void onStructureConstructionComplete(Object *, Object *o, Bool b) { logf("player %d complete %d %d", (int)m_index, (int)o->m_id, (int)b); }
	void onUnitCreated(Object *, Object *o) { logf("player %d unit %d", (int)m_index, (int)o->m_id); }
	UnicodeString getPlayerDisplayName() { return UnicodeString(); }
};
Player g_players[8];
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
		/* the stand-in refuses a template the test marks (cfg[11]'s bits) */
		if ((g_cfg[11] >> tt->m_index) & 1) { logf("newobject refused %d", (int)tt->m_index); return NULL; }
		Object *o = &g_objs[g_nextObj % 64];
		o->m_id = 100 + g_nextObj++;
		o->m_template = tt;
		o->m_module.m_create.m_obj = o->m_id;
		o->m_modules[0] = &o->m_module; o->m_modules[1] = NULL;
		o->m_geom.m_radius = 10.0f + 3.5f * (Real)tt->m_index;
		logf("new object %d template %d team %d", (int)o->m_id, (int)tt->m_index, team ? (int)team->m_owner : -1);
		return o;
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
		if (g_cfg[12] & 1) { logf("adjust %d none", (int)o->m_id); return FALSE; }
		pos->x += 2.5f; pos->y -= 1.25f;
		logf("adjust %d", (int)o->m_id);
		return TRUE;
	}
	void updateGoal(Object *o, const Coord3D *p, PathfindLayerEnum l) { logf("goal %d %.4f %.4f %d", (int)o->m_id, p->x, p->y, (int)l); }
};
Pathfinder g_pf;
class AI { public: Pathfinder *pathfinder() { return &g_pf; } };
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
		Bool found = ((g_cfg[13] >> (m_calls++ % 32)) & 1) == 0;
		logf("find around %.4f %.4f %.4f r %.6f %.6f -> %d", pos->x, pos->y, pos->z, o->minRadius, o->maxRadius, (int)found);
		if (found) { result->x = pos->x + o->minRadius; result->y = pos->y + o->maxRadius; result->z = pos->z; }
		return found;
	}
};
PartitionManager g_part;
PartitionManager *ThePartitionManager = &g_part;
class TerrainLogic { public: Real getGroundHeight(Real x, Real y) { return x * 0.125f - y * 0.0625f; } };
TerrainLogic g_terrain;
TerrainLogic *TheTerrainLogic = &g_terrain;
class Waypoint { public: Coord3D m_loc; const Coord3D *getLocation() const { return &m_loc; } };
Waypoint g_waypoints[16];   /* Player_1..8_Start, then Player_1..8_Rally */
Waypoint *findNamedWaypoint(AsciiString name)
{
	for (int k = 1; k <= 8; ++k) {
		char s[32], r[32];
		snprintf(s, sizeof s, "Player_%d_Start", k);
		snprintf(r, sizeof r, "Player_%d_Rally", k);
		if (name == s && (g_cfg[14] >> (k - 1) & 1)) return &g_waypoints[k - 1];
		if (name == r && (g_cfg[14] >> (k + 7) & 1)) return &g_waypoints[k + 7];
	}
	return NULL;
}

/* ---- templates, generals, colours, the map cache ---- */
class PlayerTemplate
{
public:
	Int m_index; AsciiString m_building, m_name; AsciiString m_units[MAX_MP_STARTING_UNITS]; Bool m_old;
	const AsciiString &getStartingBuilding() const { return m_building; }
	const AsciiString &getStartingUnit(Int i) const { return m_units[i]; }
	Bool isOldFaction() const { return m_old; }
	const AsciiString &getName() const { return m_name; }
	UnicodeString getDisplayName() const { return UnicodeString(); }
};
PlayerTemplate g_pt[12];
class PlayerTemplateStore
{
public:
	Int getPlayerTemplateCount() { return (Int)(g_cfg[1] % 13); }
	/* a template the test marks (cfg[2]'s bits) is NULL */
	const PlayerTemplate *getNthPlayerTemplate(Int i) { return (i >= 0 && i < 12 && !((g_cfg[2] >> i) & 1)) ? &g_pt[i] : NULL; }
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
		for (int i = 0; i < 12; ++i) if (name == g_pt[i].m_name && ((g_cfg[3] >> i) & 1)) return &g_generals[i];
		return NULL;
	}
};
ChallengeGenerals g_cg;
ChallengeGenerals *TheChallengeGenerals = &g_cg;
class MultiplayerSettings { public: Int getNumColors() { return (Int)(g_cfg[4] % 10); } };
MultiplayerSettings g_mps;
MultiplayerSettings *TheMultiplayerSettings = &g_mps;
struct AsciiLess { bool operator()(const AsciiString &a, const AsciiString &b) const { return strcmp(a.str(), b.str()) < 0; } };
class WaypointMap : public std::map<AsciiString, Coord3D, AsciiLess> {};
class MapMetaData { public: Int m_numPlayers; WaypointMap m_waypoints; };
MapMetaData g_md;
class MapCache
{
public:
	/* MapUtil.cpp lines 1084-1091 (the cache holds one map, "maps\\one\\one.map") */
	const MapMetaData *findMap(AsciiString mapName)
	{
		mapName.toLower();
		if (mapName == "maps\\one\\one.map" && (g_cfg[5] & 1)) return &g_md;
		return NULL;
	}
};
MapCache g_mc;
MapCache *TheMapCache = &g_mc;
class MapObject
{
public:
	AsciiString m_name; const ThingTemplate *m_tt;
	const AsciiString &getName() const { return m_name; }
	void setName(const AsciiString &n) { m_name = n; }
	void setThingTemplate(const ThingTemplate *t) { m_tt = t; }
};

/* ---- the load screens and the campaign ---- */
class LoadScreen { public: const char *m_kind; virtual ~LoadScreen() {} };
class ShellGameLoadScreen : public LoadScreen { public: ShellGameLoadScreen() { m_kind = "shell"; } };
class SinglePlayerLoadScreen : public LoadScreen { public: SinglePlayerLoadScreen() { m_kind = "singleplayer"; } };
class ChallengeLoadScreen : public LoadScreen { public: ChallengeLoadScreen() { m_kind = "challenge"; } };
class MultiPlayerLoadScreen : public LoadScreen { public: MultiPlayerLoadScreen() { m_kind = "multiplayer"; } };
class GameSpyLoadScreen : public LoadScreen { public: GameSpyLoadScreen() { m_kind = "gamespy"; } };
class Campaign { public: Bool m_isChallengeCampaign; };
Campaign g_campaign;
Bool g_hasCampaign;
class CampaignManager { public: Campaign *getCurrentCampaign() { return g_hasCampaign ? &g_campaign : NULL; } };
CampaignManager g_cm;
CampaignManager *TheCampaignManager = &g_cm;
class GameLogic { public: Int m_gameMode; LoadScreen *getLoadScreen(Bool loadingSaveGame); };
GameLogic g_logic;

/* ---- the upstream code ---- */
#include "game_logic_start_cut.inc"

/* ---- the test ---- */
GameInfo *g_game;

void dumpSlots()
{
	std::string s;
	char t[64];
	for (int i = 0; i < MAX_SLOTS; ++i) {
		const GameSlot *sl = g_game->getConstSlot(i);
		snprintf(t, sizeof t, " %d:%d:%d:%d:%d", (int)sl->getState(), (int)sl->getColor(), (int)sl->getStartPos(), (int)sl->getPlayerTemplate(), (int)sl->getTeamNumber());
		s += t;
	}
	logf("slots%s seed %u", s.c_str(), GetGameLogicRandomSeed());
}

void run_op(const unsigned *w)
{
	unsigned a = w[1], b = w[2], c = w[3];
	GameInfo *game = (w[5] & 1) ? NULL : g_game;
	switch (w[0]) {
	case 0: checkForDuplicateColors(game); logf("duplicates"); dumpSlots(); break;
	case 1: populateRandomSideAndColor(game); logf("sides"); dumpSlots(); break;
	case 2: populateRandomStartPosition(game); logf("starts"); dumpSlots(); break;
	case 3: {
		static const char *names[4] = { "AmericaTankLeopard", "AmericaVehicleHumVee", "Tank", "AmericaTankleopard" };
		MapObject m;
		m.m_name = AsciiString(names[a % 4]);
		m.m_tt = &g_things[2];
		handleNameChange(&m);
		logf("renamed [%s] %d", m.m_name.str(), m.m_tt ? (int)m.m_tt->m_index : -1);
		break;
	}
	case 4: {
		GameSlot *slot = g_game->getSlot((Int)(a % MAX_SLOTS));
		placeNetworkBuildingsForPlayer((Int)(a % MAX_SLOTS), slot, &g_players[a % MAX_SLOTS], &g_pt[b % 12]);
		logf("placed");
		break;
	}
	case 5: {
		g_logic.m_gameMode = (Int)(a % 8);
		g_hasCampaign = (Bool)(b & 1);
		g_campaign.m_isChallengeCampaign = (Bool)(b >> 1 & 1);
		LoadScreen *l = g_logic.getLoadScreen((Bool)(c & 1));
		logf("loadscreen %s", l ? l->m_kind : "none");
		delete l;
		break;
	}
	case 6: dumpSlots(); break;
	default: logf("op?"); break;
	}
}

} // namespace gls

/* slots: eight of five words (state, colour, start position, template, team); cfg: [0] the seed, [1] the template
** count, [2] NULL templates, [3] templates with a general, [4] the colour count, [5] bit 0 the map is cached, bit 1
** old factions only, [6] the map's player count, [7] general enabled bits, [8] templates with a starting building,
** [9] bit 0 rangers immobile, [10] players with no default team, [11] templates newObject refuses, [12] bit 0 the
** pathfinder adjusts nothing, [13] findPositionAround's failures, [14] waypoints present, [15] old faction bits */
extern "C" int ref_glstart_run(const unsigned *cfg, const unsigned *slots, const unsigned *ops, int n, char *out, int cap)
{
	using namespace gls;
	g_log.clear();
	memcpy(g_cfg, cfg, sizeof g_cfg);
	unsigned ginfo_cfg[9] = { 0, 8, 12, 1000, 0, 0, 0, 0, 0 };
	ref_ginfo_configure(ginfo_cfg);
	InitGameLogicRandom(g_cfg[0]);
	g_nextObj = 0;
	g_part.m_calls = 0;
	for (int i = 0; i < 6; ++i) g_things[i].m_index = i;
	for (int i = 0; i < 8; ++i) { g_players[i].m_index = i; g_teams[i].m_owner = i; }
	for (int i = 0; i < 12; ++i) {
		char nb[32];
		g_pt[i].m_index = i;
		snprintf(nb, sizeof nb, "Faction%d", i);
		g_pt[i].m_name = AsciiString(nb);
		g_pt[i].m_building = ((g_cfg[8] >> i) & 1) ? AsciiString(g_thingNames[i % 2]) : AsciiString();
		for (int u = 0; u < MAX_MP_STARTING_UNITS; ++u)
			g_pt[i].m_units[u] = ((i + u) % 4 == 0) ? AsciiString(g_thingNames[2 + (u % 2)]) : ((i + u) % 7 == 0 && i >= 10 ? AsciiString("NoSuchThing") : AsciiString());
		g_pt[i].m_old = (Bool)((g_cfg[15] >> i) & 1);
		g_generals[i].m_enabled = (Bool)((g_cfg[7] >> i) & 1);
	}
	for (int k = 0; k < 16; ++k) {
		g_waypoints[k].m_loc.x = 100.0f * (Real)(k % 8) + 7.0f * (Real)(k / 8);
		g_waypoints[k].m_loc.y = 37.5f * (Real)((k * 5) % 8);
		g_waypoints[k].m_loc.z = 0;
	}
	g_md.m_numPlayers = (Int)(g_cfg[6] % 10) - 1;
	g_md.m_waypoints.clear();
	for (int k = 0; k < 8; ++k) {
		if (!((g_cfg[14] >> k) & 1)) continue;
		char s[32];
		snprintf(s, sizeof s, "Player_%d_Start", k + 1);
		g_md.m_waypoints[AsciiString(s)] = g_waypoints[k].m_loc;
	}
	void *mem = calloc(1, sizeof(SkirmishGameInfo));
	g_game = new (mem) SkirmishGameInfo;
	g_game->m_oldFactionsOnly = (Bool)((g_cfg[5] >> 1) & 1);
	g_game->m_mapName = AsciiString((g_cfg[5] & 4) ? "Maps\\One\\One.map" : ((g_cfg[5] & 8) ? "Maps\\Two\\Two.map" : "maps\\one\\one.map"));
	for (int i = 0; i < MAX_SLOTS; ++i) {
		GameSlot *s = g_game->getSlot(i);
		const unsigned *v = slots + 5 * i;
		s->m_state = (SlotState)(v[0] % 6);
		s->m_color = (Int)v[1];
		s->m_startPos = (Int)v[2];
		s->m_playerTemplate = (Int)v[3];
		s->m_teamNumber = (Int)v[4];
	}
	for (int i = 0; i < n; ++i)
		run_op(ops + 6 * i);
	g_game->~GameInfo();
	free(mem);
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}
