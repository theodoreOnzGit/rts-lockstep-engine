/* Differential-test wrapper for ScriptEngine.cpp (upstream commit 0a05454d), p1w13-sonnet, layer 11 batch E2:
**   GeneralsMD/Code/GameEngine/Include/GameLogic/ScriptEngine.h              (unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp (SE_SOURCE: build.rs's copy, see there)
** on the stand-ins of differential/shim_engine/se_world.h, all inside namespace sew. Entry point (extern "C",
** tests/differential_script_engine.rs): ref_engine_run.
**
** The configuration g_cfg (256 words): 0 flags (bit 0: a current campaign, bit 1: it is a challenge); 4 players (bits 0-3, at
** most 8) and the local player (bits 4-7); 5 sides (bits 0-3, at most 8); 7 seed; 11 templates (bits 0-2: count - 1); 12
** waypoints (bit k: wp<k>); 16 + 2p player p (bit 0 human, 1-2 difficulty % 3, 4 active, 5 skirmish AI, 8-11 start index, 12-15
** enemy, 16-19 default team; 15: none); 32 + k prototype k (bit 0 singleton, 1-3 max instances, 31 exists); 40 + t team t (bit 0
** active, 1 exists, 4-7 controller, 8-10 prototype); 64 + o object id o + 1 (bit 0 exists, 1 AI, 4-7 player, 8-11 name number,
** 12 named, 16-18 template).
**
** The strings a program names are numbers (seStr): 0-99 the special names, 100 + k "script<k>", 200 + k "team<k>", 300 + k
** "unit<k>", 400 + k "player<k>", 500 + k "c<k>", 600 + k "tmpl<k>", 700 + k "wp<k>", 800 + k "list<k>", 900 + k the perimeter
** areas, 1000 + k "snd<k>".
**
** The scripts (one word each): for each of the 8 sides: has a list (0/1); its scripts (count, then each); its groups (count, then
** each: name, active, subroutine, scripts). A script: name, flags (bit 0 active, 1 one shot, 2 subroutine, 3 easy, 4 normal, 5
** hard), delay, or-terms (count; each: conditions (count; each: type, parameters)), actions (count; each: type, parameters), false
** actions. Parameters: count, then each: type, int, real (bits), string. */
#include <string>
#include <vector>
#include <list>
#include <map>
#include <utility>
#include <algorithm>
#include <cstdio>
#include <cstring>
#include <cstdarg>
#include <cstdlib>
#include <cmath>
#include <new>
#include <cstddef>

namespace sew {
#define private public
#define protected public
#include SE_SOURCE
#undef protected
#undef private

/* the debugger and particle editor helpers ScriptEngine.cpp declares static and defines after line 9355 (not compiled): with
** their DLLs never loaded each does nothing (see build.rs) */
static void _appendMessage(const AsciiString &, Bool, Bool) {}
static void _adjustVariable(const AsciiString &, Int, Bool) {}
static void _updateFrameNumber(void) {}
static void _updateCurrentParticleCap(void) {}

unsigned g_cfg[SE_CFG];
unsigned g_frame;
unsigned g_rng;
std::vector<std::string> g_log;
void seLog(const char *fmt, ...)
{
	va_list ap, ap2;
	va_start(ap, fmt);
	va_copy(ap2, ap);
	int n = vsnprintf(NULL, 0, fmt, ap);
	va_end(ap);
	std::string buf((size_t)n + 1, '\0');
	vsnprintf(&buf[0], buf.size(), fmt, ap2);
	va_end(ap2);
	buf.resize((size_t)n);
	g_log.push_back(buf);
}
AsciiString AsciiString::TheEmptyString;
NameKeyGenerator g_keys; NameKeyGenerator *TheNameKeyGenerator = &g_keys;
Player g_players[SE_PLAYERS];
PlayerList g_playerList; PlayerList *ThePlayerList = &g_playerList;
Team g_teams[SE_TEAMS];
TeamPrototype g_protos[SE_PROTOS];
TeamFactory g_teamFactory; TeamFactory *TheTeamFactory = &g_teamFactory;
Object g_objs[SE_OBJS];
Bool g_dead[SE_OBJS];
GameLogic g_logic; GameLogic *TheGameLogic = &g_logic;
ThingTemplate g_templates[SE_TEMPLATES];
ThingFactory g_factory; ThingFactory *TheThingFactory = &g_factory;
Waypoint g_waypoints[8];
PolygonTrigger g_trigger;
TerrainLogic g_terrain; TerrainLogic *TheTerrainLogic = &g_terrain;
PartitionManager g_partition; PartitionManager *ThePartitionManager = &g_partition;
AI g_ai; AI *TheAI = &g_ai;
AIGroup g_groups[64];
Int g_nextGroup;
GameAudio g_audio; GameAudio *TheAudio = &g_audio;
MessageStream g_messages; MessageStream *TheMessageStream = &g_messages;
CampaignManager g_campaign; CampaignManager *TheCampaignManager = &g_campaign;
GameEngine g_gameEngine; GameEngine *TheGameEngine = &g_gameEngine;
GlobalData g_global; GlobalData *TheGlobalData = &g_global; GlobalData *TheWritableGlobalData = &g_global;
ScriptActions g_actions; ScriptActions *TheScriptActions = &g_actions;
ScriptConditions g_conditions; ScriptConditions *TheScriptConditions = &g_conditions;
SidesInfo g_sides[8];
SidesList g_sidesList; SidesList *TheSidesList = &g_sidesList;

static const char *const kSpecial[] = { "", "<none>", THIS_TEAM, THIS_OBJECT, THIS_PLAYER, LOCAL_PLAYER, THE_PLAYER, TEAM_THE_PLAYER,
	THIS_PLAYER_ENEMY, MY_INNER_PERIMETER, MY_OUTER_PERIMETER, ENEMY_INNER_PERIMETER, ENEMY_OUTER_PERIMETER, "USA Team is Building0",
	"USA Team is Building3", "GLA Inf Team is Building15", "China Air Team Is Building7", "USA Team is Building" };
enum { NSPECIAL = sizeof(kSpecial) / sizeof(kSpecial[0]) };
AsciiString seStr(unsigned id)
{
	unsigned c = id / 100, k = id % 100;
	switch (c) {
		case 0: return AsciiString(k < NSPECIAL ? kSpecial[k] : "");
		case 1: return seName("script", k);
		case 2: return seName("team", k);
		case 3: return seName("unit", k);
		case 4: return seName("player", k);
		case 5: return seName("c", k);
		case 6: return seName("tmpl", k);
		case 7: return seName("wp", k);
		case 8: return seName("list", k);
		case 9: return k < 50 ? seName(INNER_PERIMETER, k) : seName(OUTER_PERIMETER, k - 50);
		default: return seName("snd", k);
	}
}

Int GameLogicRandomValue(Int lo, Int hi)
{
	g_rng = g_rng * 1664525u + 1013904223u;
	unsigned span = (unsigned)hi - (unsigned)lo + 1u;
	Int v = hi < lo || span == 0 ? lo : (Int)((unsigned)lo + (g_rng >> 8) % span);
	seLog("rand %d %d %d", lo, hi, v);
	return v;
}

Int PlayerList::getPlayerCount() const { int n = (int)(g_cfg[4] & 15); return n > SE_PLAYERS ? SE_PLAYERS : n; }
Player *PlayerList::getNthPlayer(Int i) { return i >= 0 && i < getPlayerCount() ? &g_players[i] : NULL; }
Player *PlayerList::getLocalPlayer() { return getNthPlayer((Int)(g_cfg[4] >> 4 & 15)); }
Player *PlayerList::findPlayerWithNameKey(NameKeyType key) { for (int i = 0; i < getPlayerCount(); ++i) if (g_players[i].getPlayerNameKey() == key) return &g_players[i]; return NULL; }
Player *Player::getCurrentEnemy() { return g_playerList.getNthPlayer((Int)(w() >> 12 & 15)); }
static Team *seTeam(int t) { return t >= 0 && t < SE_TEAMS && (g_cfg[40 + t] >> 1 & 1) ? &g_teams[t] : NULL; }
Team *Player::getDefaultTeam() { return seTeam((int)(w() >> 16 & 15)); }
AsciiString Team::getName() const { return seName("team", (int)(w() >> 8 & 7)); }
Player *Team::getControllingPlayer() const { return g_playerList.getNthPlayer((Int)(w() >> 4 & 15)); }
std::vector<Team *> TeamPrototype::instances() const
{
	std::vector<Team *> v;
	for (int t = 0; t < SE_TEAMS; ++t) if (seTeam(t) && (int)(g_cfg[40 + t] >> 8 & 7) == m_index) v.push_back(&g_teams[t]);
	return v;
}
Team *TeamFactory::findTeamByID(TeamID id) { return seTeam(id - 1); }
TeamPrototype *TeamFactory::findTeamPrototype(const AsciiString &name)
{
	int k = seNum(name.str(), "team");
	return k >= 0 && k < SE_PROTOS && (g_cfg[32 + k] >> 31 & 1) ? &g_protos[k] : NULL;
}
AIGroup *AI::createGroup()
{
	AIGroup *g = &g_groups[g_nextGroup % 64];
	g->m_id = ++g_nextGroup; g->m_team = 0;
	seLog("ai creategroup %d", g->m_id);
	return g;
}
static int seNumTemplates() { return (int)(g_cfg[11] & 7) + 1; }
const ThingTemplate *ThingTemplate::getFinalOverride() const { return &g_templates[(m_index * 5 + 1) % seNumTemplates()]; }
const ThingTemplate *ThingTemplate::friend_getNextTemplate() const { return m_index + 1 < seNumTemplates() ? &g_templates[m_index + 1] : NULL; }
const ThingTemplate *ThingFactory::findTemplate(const AsciiString &name) { int k = seNum(name.str(), "tmpl"); return k >= 0 && k < seNumTemplates() ? &g_templates[k] : NULL; }
const ThingTemplate *ThingFactory::firstTemplate() { return &g_templates[0]; }
static Object *seObj(int o) { return o >= 0 && o < SE_OBJS && (g_cfg[64 + o] & 1) && !g_dead[o] ? &g_objs[o] : NULL; }
const ThingTemplate *Object::getTemplate() const { return &g_templates[(int)(w() >> 16 & 7) % seNumTemplates()]; }
Player *Object::getControllingPlayer() const { return g_playerList.getNthPlayer((Int)(w() >> 4 & 15)); }
Object *Object::getNextObject() { for (int o = m_index + 1; o < SE_OBJS; ++o) if (seObj(o)) return &g_objs[o]; return NULL; }
Object *GameLogic::findObjectByID(ObjectID id) { return seObj(id - 1); }
Object *GameLogic::getFirstObject() { for (int o = 0; o < SE_OBJS; ++o) if (seObj(o)) return &g_objs[o]; return NULL; }
Waypoint *TerrainLogic::getWaypointByName(const AsciiString &name)
{
	int k = seNum(name.str(), "wp");
	if (k < 0 || k >= 8 || !(g_cfg[12] >> k & 1)) return NULL;
	g_waypoints[k].m_pos.x = (Real)(seHash(SE_SEED, 700, k) % 1000) * 0.5f;
	g_waypoints[k].m_pos.y = (Real)(seHash(SE_SEED, 701, k) % 1000) * 0.5f;
	g_waypoints[k].m_pos.z = 0.0f;
	return &g_waypoints[k];
}
PolygonTrigger *TerrainLogic::getTriggerAreaByName(const AsciiString &name)
{
	unsigned h = seStrHash(name.str()) ^ SE_SEED;
	if (h % 3 == 0) return NULL;
	g_trigger.m_id = (Int)(h & 0xffff);
	return &g_trigger;
}
Int SidesList::getNumSides() const { int n = (int)(g_cfg[5] & 15); return n > 8 ? 8 : n; }
SidesInfo *SidesList::getSideInfo(Int i) { return &g_sides[i]; }

/* ---- the stand-in actions and conditions ---- */
static std::string seParms(Parameter *const *parms, Int n)
{
	std::string s;
	char b[64];
	for (int i = 0; i < n; ++i) { snprintf(b, sizeof(b), " %d:%s", parms[i]->m_int, parms[i]->m_string.str()); s += b; }
	return s;
}
static AsciiString pStr(Parameter *p) { return p ? p->getString() : AsciiString(); }
static Int pInt(Parameter *p) { return p ? p->getInt() : 0; }
void ScriptActions::executeAction(ScriptAction *a)
{
	seLog("act %d%s", a->getActionType(), seParms(a->m_parms, a->m_numParms).c_str());
	ScriptEngine *e = TheScriptEngine;
	switch (a->getActionType()) {
		case ScriptAction::UNIT_EXECUTE_SEQUENTIAL_SCRIPT:
		case ScriptAction::UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING:
		case ScriptAction::TEAM_EXECUTE_SEQUENTIAL_SCRIPT:
		case ScriptAction::TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING: {
			Bool team = a->getActionType() == ScriptAction::TEAM_EXECUTE_SEQUENTIAL_SCRIPT || a->getActionType() == ScriptAction::TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING;
			Bool looping = a->getActionType() == ScriptAction::UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING || a->getActionType() == ScriptAction::TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING;
			Script *s = const_cast<Script *>(e->findScriptByName(pStr(a->getParameter(1))));
			if (!s) break;
			SequentialScript seq;
			if (team) { Team *t = e->getTeamNamed(pStr(a->getParameter(0))); if (!t) break; seq.m_teamToExecOn = t; }
			else { Object *o = e->getUnitNamed(pStr(a->getParameter(0))); if (!o) break; seq.m_objectID = o->getID(); }
			seq.m_scriptToExecuteSequentially = s;
			seq.m_timesToLoop = looping ? pInt(a->getParameter(2)) : 0;
			e->appendSequentialScript(&seq);
			break;
		}
		case ScriptAction::UNIT_STOP_SEQUENTIAL_SCRIPT: { Object *o = e->getUnitNamed(pStr(a->getParameter(0))); if (o) e->removeAllSequentialScripts(o); break; }
		case ScriptAction::TEAM_STOP_SEQUENTIAL_SCRIPT: { Team *t = e->getTeamNamed(pStr(a->getParameter(0))); if (t) e->removeAllSequentialScripts(t); break; }
		case ScriptAction::UNIT_GUARD_FOR_FRAMECOUNT: { Object *o = e->getUnitNamed(pStr(a->getParameter(0))); if (o) e->setSequentialTimer(o, pInt(a->getParameter(1))); break; }
		case ScriptAction::TEAM_GUARD_FOR_FRAMECOUNT: { Team *t = e->getTeamNamed(pStr(a->getParameter(0))); if (t) e->setSequentialTimer(t, pInt(a->getParameter(1))); break; }
		case ScriptAction::NAMED_DELETE: {
			Object *o = e->getUnitNamed(pStr(a->getParameter(0)));
			if (o) { e->notifyOfObjectDestruction(o); g_dead[o->m_index] = true; seLog("obj %d deleted", o->getID()); }
			break;
		}
		default: break;
	}
}
Bool ScriptConditions::evaluateCondition(Condition *c)
{
	Bool v = (seHash(SE_SEED, (unsigned)c->m_type * 7u + (unsigned)c->m_customData, g_frame) & 1) != 0;
	c->m_customData++;
	seLog("cond %d %d", c->m_type, (int)v);
	return v;
}
Bool ScriptConditions::evaluateSkirmishCommandButtonIsReady(Player *, Parameter *p1, Parameter *p2, Bool all)
{
	Bool v = (seHash(SE_SEED, 800 + (unsigned)pInt(p1) + (unsigned)pInt(p2), g_frame) & 1) != 0;
	seLog("buttonready %s %s %d %d", pStr(p1).str(), pStr(p2).str(), (int)all, (int)v);
	return v;
}
Bool ScriptConditions::evaluateTeamIsContained(Parameter *p0, Bool all)
{
	Bool v = (seHash(SE_SEED, 900 + (unsigned)pInt(p0), g_frame) & 1) != 0;
	seLog("contained %s %d %d", pStr(p0).str(), (int)all, (int)v);
	return v;
}

/* ---- the scripts from their words ---- */
struct Rd { const unsigned *w; int n, at; unsigned u() { return at < n ? w[at++] : 0; } };
static Parameter *readParm(Rd &r)
{
	Parameter *p = new Parameter;
	p->m_type = (Int)r.u(); p->m_int = (Int)r.u();
	unsigned rb = r.u(); memcpy(&p->m_real, &rb, 4);
	p->m_string = seStr(r.u());
	return p;
}
template <class T> static void readParms(Rd &r, T *x)
{
	x->m_numParms = (Int)(r.u() % (MAX_PARMS + 1));
	for (int i = 0; i < MAX_PARMS; ++i) x->m_parms[i] = i < x->m_numParms ? readParm(r) : NULL;
}
static ScriptAction *readActions(Rd &r)
{
	unsigned n = r.u() % 64;
	ScriptAction *head = NULL, **tail = &head;
	for (unsigned i = 0; i < n; ++i) { ScriptAction *a = new ScriptAction((Int)r.u()); readParms(r, a); *tail = a; tail = &a->m_next; }
	return head;
}
static Script *readScript(Rd &r)
{
	Script *s = new Script;
	s->m_name = seStr(r.u());
	unsigned f = r.u();
	s->m_active = f & 1; s->m_oneShot = f >> 1 & 1; s->m_subroutine = f >> 2 & 1; s->m_easy = f >> 3 & 1; s->m_normal = f >> 4 & 1; s->m_hard = f >> 5 & 1;
	s->m_delay = (Int)r.u(); s->m_frameToEvaluate = 0; s->m_curTime = 0;
	unsigned nOr = r.u() % 64;
	OrCondition **ot = &s->m_or; *ot = NULL;
	for (unsigned i = 0; i < nOr; ++i) {
		OrCondition *o = new OrCondition; o->m_next = NULL; o->m_first = NULL;
		unsigned nc = r.u() % 64;
		Condition **ct = &o->m_first;
		for (unsigned j = 0; j < nc; ++j) { Condition *c = new Condition; c->m_type = (Int)r.u(); c->m_customData = 0; c->m_next = NULL; readParms(r, c); *ct = c; ct = &c->m_next; }
		*ot = o; ot = &o->m_next;
	}
	s->m_action = readActions(r);
	s->m_falseAction = readActions(r);
	s->m_next = NULL;
	return s;
}
static Script *readScripts(Rd &r)
{
	unsigned n = r.u() % 64;
	Script *head = NULL, **tail = &head;
	for (unsigned i = 0; i < n; ++i) { Script *s = readScript(r); *tail = s; tail = &s->m_next; }
	return head;
}
static void readSides(const unsigned *w, int n)
{
	Rd r = { w, n, 0 };
	for (int i = 0; i < 8; ++i) {
		g_sides[i].m_list = NULL;
		if (!(r.u() & 1)) continue;
		ScriptList *l = new ScriptList;
		l->m_first = readScripts(r);
		unsigned ng = r.u() % 64;
		ScriptGroup **gt = &l->m_groups; *gt = NULL;
		for (unsigned k = 0; k < ng; ++k) {
			ScriptGroup *g = new ScriptGroup;
			g->m_name = seStr(r.u()); g->m_active = r.u() & 1; g->m_subroutine = r.u() & 1; g->m_first = readScripts(r); g->m_next = NULL;
			*gt = g; gt = &g->m_next;
		}
		g_sides[i].m_list = l;
	}
}

/* ---- the dump of the engine's state and the scripts' changing fields ---- */
static std::string g_dump;
static void dp(const char *fmt, ...)
{
	char buf[512];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof(buf), fmt, ap);
	va_end(ap);
	g_dump += buf;
}
static int tid(Team *t) { return t ? t->getID() : 0; }
static int oid(Object *o) { return o ? o->getID() : 0; }
static int pid(Player *p) { return p ? p->getPlayerIndex() + 1 : 0; }
static void dumpParms(Parameter *const *parms, Int n) { for (int i = 0; i < n; ++i) dp(" %d", parms[i]->m_int); }
static void dumpScript(Script *s)
{
	dp(" [%s %d %u %s %08x", s->m_name.str(), (int)s->m_active, s->m_frameToEvaluate, s->m_conditionTeamName.str(), fb(s->m_curTime));
	for (OrCondition *o = s->m_or; o; o = o->m_next) for (Condition *c = o->m_first; c; c = c->m_next) { dp(" c%d", c->m_customData); dumpParms(c->m_parms, c->m_numParms); }
	for (ScriptAction *a = s->m_action; a; a = a->m_next) { dp(" a"); dumpParms(a->m_parms, a->m_numParms); }
	for (ScriptAction *a = s->m_falseAction; a; a = a->m_next) { dp(" f"); dumpParms(a->m_parms, a->m_numParms); }
	dp("]");
}
static void dumpPairs(const char *what, ListAsciiStringObjectID *l)
{
	for (int p = 0; p < MAX_PLAYER_COUNT; ++p) for (ListAsciiStringObjectIDIt it = l[p].begin(); it != l[p].end(); ++it) dp(" %s%d:%s:%d", what, p, it->first.str(), it->second);
}
static void dumpState(ScriptEngine *e)
{
	g_dump.clear();
	dp("state c%d", e->m_numCounters);
	for (int i = 0; i < e->m_numCounters; ++i) dp(" %s=%d%s", e->m_counters[i].name.str(), e->m_counters[i].value, e->m_counters[i].isCountdownTimer ? "t" : "");
	dp(" f%d", e->m_numFlags);
	for (int i = 0; i < e->m_numFlags; ++i) dp(" %s=%d", e->m_flags[i].name.str(), (int)e->m_flags[i].value);
	dp(" p%d", e->m_numAttackInfo);
	for (int i = 0; i < e->m_numAttackInfo; ++i) {
		AttackPriorityInfo &a = e->m_attackPriorityInfo[i];
		dp(" %s/%d", a.m_name.str(), a.m_defaultPriority);
		if (a.m_priorityMap) for (AttackPriorityMap::iterator it = a.m_priorityMap->begin(); it != a.m_priorityMap->end(); ++it) dp(",%d:%d", it->first->m_index, it->second);
	}
	dp(" timers %d %d who %d %d %d %d %d first %d", e->m_endGameTimer, e->m_closeWindowTimer, tid(e->m_callingTeam), oid(e->m_callingObject), tid(e->m_conditionTeam), oid(e->m_conditionObject), pid(e->m_currentPlayer), (int)e->m_firstUpdate);
	dp(" fade %d %08x %08x %08x %d %d %d %d", (int)e->m_fade, fb(e->m_minFade), fb(e->m_maxFade), fb(e->m_curFadeValue), e->m_curFadeFrame, e->m_fadeFramesIncrease, e->m_fadeFramesHold, e->m_fadeFramesDecrease);
	BreezeInfo &b = e->m_breezeInfo;
	dp(" breeze %08x %08x %08x %08x %08x %08x %d %d", fb(b.m_direction), fb(b.m_directionVec.x), fb(b.m_directionVec.y), fb(b.m_intensity), fb(b.m_lean), fb(b.m_randomness), b.m_breezePeriod, b.m_breezeVersion);
	dp(" track %s", e->m_currentTrackName.str());
	dp(" changed %u diff %d freeze %d bonus %d victim %d mpdefeat %d", e->m_frameObjectCountChanged, (int)e->m_gameDifficulty, (int)e->m_freezeByScript, (int)e->m_objectsShouldReceiveDifficultyBonus, (int)e->m_ChooseVictimAlwaysUsesNormal, (int)e->m_shownMPLocalDefeatWindow);
	dp(" names");
	for (VecNamedRequestsIt it = e->m_namedObjects.begin(); it != e->m_namedObjects.end(); ++it) dp(" %s:%d", it->first.str(), oid(it->second));
	dp(" notes");
	for (ListAsciiStringIt it = e->m_completedVideo.begin(); it != e->m_completedVideo.end(); ++it) dp(" v:%s", it->str());
	for (ListAsciiStringUINTIt it = e->m_testingSpeech.begin(); it != e->m_testingSpeech.end(); ++it) dp(" s:%s:%u", it->first.str(), it->second);
	for (ListAsciiStringUINTIt it = e->m_testingAudio.begin(); it != e->m_testingAudio.end(); ++it) dp(" a:%s:%u", it->first.str(), it->second);
	for (ListAsciiStringIt it = e->m_uiInteractions.begin(); it != e->m_uiInteractions.end(); ++it) dp(" u:%s", it->str());
	dumpPairs("t", e->m_triggeredSpecialPowers); dumpPairs("m", e->m_midwaySpecialPowers); dumpPairs("d", e->m_finishedSpecialPowers); dumpPairs("g", e->m_completedUpgrades);
	for (int p = 0; p < MAX_PLAYER_COUNT; ++p) for (size_t k = 0; k < e->m_acquiredSciences[p].size(); ++k) dp(" sci%d:%d", p, e->m_acquiredSciences[p][k]);
	for (ListAsciiStringCoord3DIt it = e->m_toppleDirections.begin(); it != e->m_toppleDirections.end(); ++it) dp(" top:%s:%08x:%08x:%08x", it->first.str(), fb(it->second.x), fb(it->second.y), fb(it->second.z));
	for (VecNamedRevealIt it = e->m_namedReveals.begin(); it != e->m_namedReveals.end(); ++it) dp(" rv:%s:%s:%08x:%s", it->m_revealName.str(), it->m_waypointName.str(), fb(it->m_radiusToReveal), it->m_playerName.str());
	for (AllObjectTypesIt it = e->m_allObjectTypeLists.begin(); it != e->m_allObjectTypeLists.end(); ++it) {
		dp(" ot:%s", (*it)->m_name.str());
		for (size_t k = 0; k < (*it)->m_types.size(); ++k) dp(",%s", (*it)->m_types[k].str());
	}
	for (int p = 0; p < MAX_PLAYER_COUNT; ++p) for (ObjectTypeCount::iterator it = e->m_objectCounts[p].begin(); it != e->m_objectCounts[p].end(); ++it) dp(" n%d:%s:%d", p, it->first.str(), it->second);
	dp(" seq");
	for (size_t i = 0; i < e->m_sequentialScripts.size(); ++i) {
		dp(" {");
		for (SequentialScript *s = e->m_sequentialScripts[i]; s; s = s->m_nextScriptInSequence)
			dp(" %d/%d/%s/%d/%d/%d/%d", tid(s->m_teamToExecOn), s->m_objectID, s->m_scriptToExecuteSequentially->m_name.str(), s->m_currentInstruction, s->m_timesToLoop, s->m_framesToWait, (int)s->m_dontAdvanceInstruction);
		dp(" }");
	}
	dp(" scripts");
	for (int i = 0; i < 8; ++i) {
		ScriptList *l = g_sides[i].m_list;
		if (!l) continue;
		dp(" side%d", i);
		for (Script *s = l->m_first; s; s = s->m_next) dumpScript(s);
		for (ScriptGroup *g = l->m_groups; g; g = g->m_next) { dp(" g%s:%d", g->m_name.str(), (int)g->m_active); for (Script *s = g->m_first; s; s = s->m_next) dumpScript(s); }
	}
	g_log.push_back(g_dump);
}

static Script *seScript(unsigned id) { return TheScriptEngine->findScript(seStr(id)); }
static ScriptEngine *g_engine;
static Bool g_stop;

/* One operation (three words: kind, a, b); the table is the test's. */
static void runOp(const unsigned *op)
{
	ScriptEngine *e = g_engine;
	unsigned a = op[1], b = op[2];
	switch (op[0]) {
		case 0: e->update(); break;
		case 1: g_frame += a % 7; break;
		case 2: g_cfg[a % SE_CFG] = b; break;
		case 3: e->runScript(seStr(a), seTeam((int)b - 1)); break;
		case 4: e->runObjectScript(seStr(a), seObj((int)b - 1)); break;
		case 5: {
			Int player = (Int)(b >> 8 & 15) + ((b >> 30 & 1) ? 16 : 0);
			AsciiString name = seStr(1000 + (b & 7));
			ObjectID src = (ObjectID)(b >> 16 & 3);
			switch (a % 8) {
				case 0: e->notifyOfCompletedVideo(name); break;
				case 1: e->notifyOfTriggeredSpecialPower(player, name, src); break;
				case 2: e->notifyOfMidwaySpecialPower(player, name, src); break;
				case 3: e->notifyOfCompletedSpecialPower(player, name, src); break;
				case 4: e->notifyOfCompletedUpgrade(player, name, src); break;
				case 5: e->notifyOfAcquiredScience(player, (ScienceType)(b & 7)); break;
				case 6: e->signalUIInteract(seStr(500 + (b & 7))); break;
				default: e->notifyOfObjectCreationOrDestruction(); break;
			}
			break;
		}
		case 6: {
			Int player = (Int)(b >> 8 & 15) - ((b >> 30 & 1) ? 3 : 0);
			AsciiString name = seStr(1000 + (b & 7));
			Bool rm = (b >> 4 & 1) != 0;
			ObjectID src = (ObjectID)(b >> 16 & 3);
			Bool v = false;
			switch (a % 10) {
				case 0: v = e->isVideoComplete(name, rm); break;
				case 1: v = e->isSpeechComplete(name, rm); break;
				case 2: v = e->isAudioComplete(name, rm); break;
				case 3: v = e->isSpecialPowerTriggered(player, name, rm, src); break;
				case 4: v = e->isSpecialPowerMidway(player, name, rm, src); break;
				case 5: v = e->isSpecialPowerComplete(player, name, rm, src); break;
				case 6: v = e->isUpgradeComplete(player, name, rm, src); break;
				case 7: v = e->isScienceAcquired(player, (ScienceType)(b & 7), rm); break;
				case 8: v = oid(e->getUnitNamed(seStr(b & 0xfff))) != 0; break;
				default: v = e->didUnitExist(seStr(b & 0xfff)); break;
			}
			seLog("query %u %d", a % 10, (int)v);
			break;
		}
		case 7: {
			Object *o = seObj((int)(b % 16));
			switch (a % 5) {
				case 0: e->addObjectToCache(o); break;
				case 1: if (o) e->removeObjectFromCache(o); break;
				case 2: e->transferObjectName(seStr(b >> 8 & 0xfff), o); break;
				case 3: if (o) e->notifyOfObjectDestruction(o); break;
				default: if (o) o->setName(seStr(b >> 8 & 0xfff)); break;
			}
			break;
		}
		case 8: {
			AsciiString name = seStr(800 + (a >> 4 & 3));
			switch (a % 4) {
				case 0: { Real r; unsigned rb = (b & 0xff) << 20 | 0x40000000u; memcpy(&r, &rb, 4); e->createNamedMapReveal(name, seStr(700 + (b >> 8 & 15)), r, seStr(b >> 12 & 0x3ff)); break; }
				case 1: e->doNamedMapReveal(name); break;
				case 2: e->undoNamedMapReveal(name); break;
				default: e->removeNamedMapReveal(name); break;
			}
			break;
		}
		case 9: {
			if (a % 2 == 0) {
				Coord3D d; d.x = (Real)(Int)(b & 15) - 7.0f; d.y = (Real)(Int)(b >> 4 & 15) - 7.0f; d.z = (Real)(Int)(b >> 8 & 3);
				e->setToppleDirection(seStr(300 + (a >> 4 & 15)), (b >> 12 & 1) ? NULL : &d);
			} else {
				Object *o = seObj((int)(b % 16));
				if (o) {
					Coord3D d; d.x = 1.0f; d.y = 2.0f; d.z = 0.5f;
					if (b >> 4 & 1) { Coord2D d2; d2.x = 3.0f; d2.y = -1.0f; e->adjustToppleDirection(o, &d2); seLog("topple2 %08x %08x", fb(d2.x), fb(d2.y)); }
					else { e->adjustToppleDirection(o, &d); seLog("topple %08x %08x %08x", fb(d.x), fb(d.y), fb(d.z)); }
				}
			}
			break;
		}
		case 10: e->doObjectTypeListMaintenance(seStr(800 + (a & 3)), seStr(600 + (b & 7)), (b >> 3 & 1) != 0); break;
		case 11: {
			AsciiString name = seStr(b & 0xfff);
			switch (a % 8) {
				case 0: seLog("teamnamed %d", tid(e->getTeamNamed(name))); break;
				case 1: seLog("playerfrom %d", pid(e->getPlayerFromAsciiString(name))); break;
				case 2: { PolygonTrigger *t = e->getQualifiedTriggerAreaByName(name); seLog("trigger %d", t ? t->m_id : -1); break; }
				case 3: seLog("enemy %d", pid(e->getSkirmishEnemyPlayer())); break;
				case 4: seLog("current %d", pid(e->getCurrentPlayer())); break;
				case 5: { const TCounter *c = e->getCounter(name); seLog("counter %d", c ? c->value : -99999); break; }
				case 6: { const AttackPriorityInfo *i = e->getAttackInfo(name); const ThingTemplate *t = g_factory.findTemplate(seStr(600 + (b >> 12 & 7))); seLog("attack %s %d", i->getName().str(), i->getPriority(t ? t->getFinalOverride() : NULL)); break; }
				default: { ObjectTypes *t = e->getObjectTypes(name); seLog("objecttypes %d", t ? t->getListSize() : -1); break; }
			}
			break;
		}
		case 12: e->reset(); break;
		case 13: e->newMap(); break;
		case 14: { Script *s = seScript(a); if (s) e->friend_executeAction((b & 1) ? s->getFalseAction() : s->getAction(), seTeam((int)(b >> 1) - 1)); break; }
		case 15: { Script *s = seScript(a); if (s) seLog("evaluate %d", (int)e->evaluateConditions(s, seTeam((int)(b & 15) - 1), g_playerList.getNthPlayer((Int)(b >> 4 & 15) - 1))); break; }
		case 16:
			switch (a % 10) {
				case 0: e->startEndGameTimer(); break;
				case 1: e->startQuickEndGameTimer(); break;
				case 2: e->startCloseWindowTimer(); break;
				case 3: e->doFreezeTime(); break;
				case 4: e->doUnfreezeTime(); break;
				case 5: e->setGlobalDifficulty((GameDifficulty)(b % 3)); break;
				case 6: e->clearTeamFlags(); break;
				case 7: e->clearFlag(seStr(b & 0xfff)); break;
				case 8: e->markMPLocalDefeatWindowShown(); break;
				default: e->setCurrentTrackName(seStr(1000 + (b & 7))); break;
			}
			break;
		case 17:
			if (a & 1) e->setObjectCount((Int)(b & 7), seStr(600 + (b >> 4 & 7)), (Int)(b >> 8 & 0xff) - 100);
			else seLog("objectcount %d", e->getObjectCount((Int)(b & 7), seStr(600 + (b >> 4 & 7))));
			break;
		case 18: {
			Script *s = seScript(a);
			if (!s) break;
			SequentialScript seq;
			if (b & 1) seq.m_teamToExecOn = seTeam((int)(b >> 1 & 7)); else seq.m_objectID = (ObjectID)(b >> 1 & 15) + 1;
			seq.m_scriptToExecuteSequentially = s;
			seq.m_timesToLoop = (Int)(b >> 8 & 3) - 1;
			seq.m_framesToWait = (Int)(b >> 12 & 3) - 1;
			e->appendSequentialScript(&seq);
			break;
		}
		case 19:
			switch (a % 5) {
				case 0: { Team *t = seTeam((int)(b & 7)); if (t) e->notifyOfTeamDestruction(t); break; }
				case 1: { Object *o = seObj((int)(b & 15)); if (o) e->removeAllSequentialScripts(o); break; }
				case 2: e->setSequentialTimer(seObj((int)(b & 15)), (Int)(b >> 8 & 7) - 1); break;
				case 3: e->setSequentialTimer(seTeam((int)(b & 7)), (Int)(b >> 8 & 7) - 1); break;
				default: e->createNamedCache(); break;
			}
			break;
		case 21: {
			Xfer x(XFER_SAVE);
			e->xfer(&x);
			std::string h;
			for (size_t i = 0; i < x.m_buf.size(); ++i) { char c[3]; snprintf(c, sizeof(c), "%02x", x.m_buf[i]); h += c; }
			seLog("save %s", h.c_str());
			break;
		}
		case 22: {
			Xfer save(XFER_SAVE);
			e->xfer(&save);
			Xfer load(XFER_LOAD);
			load.m_buf = save.m_buf;
			if ((a & 1) && !load.m_buf.empty()) load.m_buf[b % load.m_buf.size()] ^= (unsigned char)((b >> 8) | 1);
			if (a & 2) load.m_buf.resize(load.m_buf.size() * (b & 7) / 8);
			e->reset();
			try { e->xfer(&load); }
			catch (...) { seLog("load failed"); g_stop = true; break; }
			e->loadPostProcess();
			break;
		}
		default: { Script *s = seScript(a); if (s) e->executeScript(s); break; }
	}
}

} /* namespace sew */

/* Run one program: the configuration (256 words), the scripts' words, and n operations; the log (lines joined by '\n', a state
** line after each operation) into out. Returns the length needed. */
extern "C" int ref_engine_run(const unsigned *cfg, const unsigned *scripts, int nscripts, const unsigned *ops, int n, char *out, int cap)
{
	using namespace sew;
	memcpy(g_cfg, cfg, sizeof(g_cfg));
	g_frame = 0; g_rng = cfg[7]; g_nextGroup = 0;
	g_log.clear();
	memset(g_dead, 0, sizeof(g_dead));
	for (int i = 0; i < SE_PLAYERS; ++i) g_players[i].m_index = i;
	for (int i = 0; i < SE_TEAMS; ++i) g_teams[i].m_index = i;
	for (int i = 0; i < SE_PROTOS; ++i) g_protos[i].m_index = i;
	for (int i = 0; i < SE_TEMPLATES; ++i) g_templates[i].m_index = i;
	for (int o = 0; o < SE_OBJS; ++o) {
		g_objs[o].m_index = o; g_objs[o].m_hasColor = (seHash(cfg[7], 100, o) & 1) != 0; g_objs[o].m_color = (Color)(seHash(cfg[7], 101, o) & 0xffffff);
		g_objs[o].m_name = (cfg[64 + o] >> 12 & 1) ? seName("unit", (int)(cfg[64 + o] >> 8 & 15)) : AsciiString();
	}
	g_global.m_framesPerSecondLimit = 30;
	readSides(scripts, nscripts);
	void *mem = calloc(1, sizeof(ScriptEngine));
	g_engine = new (mem) ScriptEngine;
	TheScriptEngine = g_engine;
	g_engine->init();
	dumpState(g_engine);
	g_stop = false;
	for (int i = 0; i < n; ++i) { runOp(ops + 3 * i); if (g_stop) break; dumpState(g_engine); }
	std::string s;
	for (size_t i = 0; i < g_log.size(); ++i) { s += g_log[i]; s += '\n'; }
	if ((int)s.size() <= cap) memcpy(out, s.data(), s.size());
	/* the engine and the scripts are not freed: the memory is the test process's, and the destructor's reset() would log */
	return (int)s.size();
}
