/* The driver of the Team differential (p1w9-sonnet): the definitions of the stand-ins of team_standins_decl.h, the
** program reader, the operations, the state dump, and the entry point ref_team_run. See tests/differential_team.rs
** for the program format (the writer is its generator; this file is the reader) and for the method. Included by
** differential/wrap_team.cpp after the cut Team.cpp. */
#ifndef TEAM_DRIVER_H
#define TEAM_DRIVER_H

/* Common/GameState.h's SaveCode (verbatim from upstream, GameState.h lines 129-138) and the counter the Xfer archive's
** GameState keeps (differential/wrap_xfer.cpp: XferLoad::xferSnapshot calls addPostProcessSnapshot, which counts) */
class GameState { public: AsciiString realMapPathToPortableMapPath(const AsciiString&) const; AsciiString portableMapPathToRealMapPath(const AsciiString&) const; void addPostProcessSnapshot(Snapshot*); int m_postProcessCount; };
extern GameState* TheGameState;

/* ---- globals ------------------------------------------------------------------------------------------------ */
static ThingFactory g_thingFactory; ThingFactory* TheThingFactory = &g_thingFactory;
static GameLogic g_gameLogic; GameLogic* TheGameLogic = &g_gameLogic;
static TerrainLogic g_terrain; TerrainLogic* TheTerrainLogic = &g_terrain;
static ScriptEngine g_scriptEngine; ScriptEngine* TheScriptEngine = &g_scriptEngine;
static PartitionManager g_partition; PartitionManager* ThePartitionManager = &g_partition;
static PlayerList g_playerList; PlayerList* ThePlayerList = &g_playerList;
static AIGroup g_groups[3];
static PolygonTrigger g_triggers[2];
static unsigned g_setTeamCalls = 0;
static const unsigned HANG_LIMIT = 10000;
struct HangException {};

static int tid(const Team* t) { return t ? (int)t->getID() : 0; }

/* ---- the stand-ins' members ------------------------------------------------------------------------------------- */
static void applyReaction(Object* o, unsigned bit)
{
	if (o->m_reactions & bit)
	{
		Player* n = ThePlayerList->getNeutralPlayer();
		o->setTeam(n->getDefaultTeam());
	}
}
void ContainModuleInterface::removeAllContained(Bool) { Rec("remove-all-contained %d", (int)m_owner->m_id); applyReaction(m_owner, 4); }
void AIGroup::add(Object* obj) { Rec("group.add %d %d", m_id, (int)obj->m_id); }
Script* Script::duplicate() const
{
	Script* s = new Script(*this);
	s->m_serial = ++g_scriptEngine.m_nextSerial;
	s->m_calls = 0;
	s->m_action.m_serial = s->m_serial;
	Rec("script.dup %d %d", m_serial, s->m_serial);
	return s;
}
void ScriptEngine::friend_executeAction(ScriptAction* action, Team* team) { Rec("exec %d team=%d", action->m_serial, tid(team)); }
Bool ScriptEngine::evaluateConditions(Script* script, Team* team, Player* player)
{
	bool r = (script->m_pattern >> (script->m_calls & 31)) & 1;
	script->m_calls++;
	Rec("eval %d team=%d player=%d -> %d", script->m_serial, tid(team), player ? player->m_index : -1, r ? 1 : 0);
	return r;
}
void ScriptEngine::runScript(const AsciiString& name, Team* team) { Rec("run %s team=%d", name.str(), tid(team)); }
void ScriptEngine::notifyOfTeamDestruction(Team* team) { Rec("notify-destroy team=%d", tid(team)); }

void Player::addTeamToList(TeamPrototype* proto) { Rec("p.add %d proto=%u", m_index, (unsigned)proto->getID()); m_protos.push_back(proto); }
void Player::removeTeamFromList(TeamPrototype* proto)
{
	Rec("p.remove %d proto=%u", m_index, (unsigned)proto->getID());
	std::vector<TeamPrototype*>::iterator it = std::find(m_protos.begin(), m_protos.end(), proto);
	if (it != m_protos.end()) m_protos.erase(it);
}
void Player::preTeamDestroy(const Team* team)
{
	Rec("p.pre %d team=%d", m_index, tid(team));
	/* a player's default team that is deleted reads as none afterwards (the real game resets the player first) */
	if (m_defaultTeam == team) m_defaultTeam = NULL;
}
Relationship Player::getRelationship(const Team* that) const
{
	return (Relationship)((m_relSalt + 5u * (unsigned)m_index + 11u * (unsigned)(that ? that->getID() : 0)) % 3u);
}
void PlayerList::teamAboutToBeDeleted(Team* team) { Rec("pl.about team=%d", tid(team)); }

Object* PartitionManager::getClosestObject(const Object* obj, Real maxDist, DistanceCalculationType dc, PartitionFilter** filters)
{
	Rec("closest %d range=%08x", (int)obj->m_id, FB(maxDist));
	return obj->m_enemyNear ? const_cast<Object*>(obj) : NULL;
}
Object* GameLogic::findObjectByID(ObjectID id) const
{
	for (size_t i = 0; i < m_objs.size(); ++i) if (m_objs[i]->m_id == id) return m_objs[i];
	return NULL;
}
void GameLogic::destroyObject(Object* obj) { Rec("destroy %d", (int)obj->m_id); applyReaction(obj, 2); }

void Object::setTeam(Team* newTeam)
{
	Rec("setteam obj=%d team=%d", (int)m_id, tid(newTeam));
	if (++g_setTeamCalls > HANG_LIMIT) throw HangException();
	if (m_team == newTeam) return;
	if (m_team && m_team->isInList_TeamMemberList(this)) m_team->removeFrom_TeamMemberList(this);
	m_team = newTeam;
	if (m_team)
	{
		if (!m_team->isInList_TeamMemberList(this)) m_team->prependTo_TeamMemberList(this);
	}
}
void Object::healCompletely() { Rec("heal %d", (int)m_id); }
void Object::kill() { Rec("kill %d", (int)m_id); applyReaction(this, 1); }
void Object::attemptDamage(DamageInfo* info)
{
	Rec("damage %d amt=%08x type=%d death=%d src=%d", (int)m_id, FB(info->in.m_amount), (int)info->in.m_damageType, (int)info->in.m_deathType, (int)info->in.m_sourceID);
	applyReaction(this, 8);
}
void Object::handlePartitionCellMaintenance() { Rec("pcm %d", (int)m_id); }

/* ---- the program ---------------------------------------------------------------------------------------------- */
struct Reader
{
	const unsigned* w; int n; int p;
	unsigned u() { return p < n ? w[p++] : 0; }
	int i() { return (int)u(); }
	bool b() { return u() != 0; }
	float f() { unsigned v = u(); float x; memcpy(&x, &v, 4); return x; }
	std::string s() { unsigned len = u(); std::string r; for (unsigned k = 0; k < len; ++k) r += (char)u(); return r; }
	Coord3D c() { Coord3D v; v.x = f(); v.y = f(); v.z = f(); return v; }
};

struct DictEntry { int type; std::string key; unsigned bits; std::string str; };
struct DictSpec { std::vector<DictEntry> e; };
static std::vector<DictSpec> g_dicts;
static std::vector<ThingTemplate*> g_tpls;
static std::vector<Player*> g_players;
static std::vector<Object*> g_objs;
static std::vector<Waypoint*> g_ways;
static std::vector<Script*> g_scripts;

static std::string g_tmpPath;
static std::vector<unsigned char> g_saved;           /* the last save of the factory */
static std::map<int, int> g_savedObjTeam;            /* object id -> team id at the time of that save */
static bool g_haveSave = false;

static void buildDict(const DictSpec& s, Dict& d)
{
	for (size_t i = 0; i < s.e.size(); ++i)
	{
		const DictEntry& e = s.e[i];
		NameKeyType k = TheNameKeyGenerator->nameToKey(e.key.c_str());
		float f;
		switch (e.type)
		{
		case 0: d.setBool(k, e.bits != 0); break;
		case 1: d.setInt(k, (Int)e.bits); break;
		case 2: memcpy(&f, &e.bits, 4); d.setReal(k, f); break;
		default: d.setAsciiString(k, AsciiString(e.str.c_str())); break;
		}
	}
}

static std::vector<unsigned char> readFileBytes(const char* path)
{
	std::vector<unsigned char> v;
	FILE* f = fopen(path, "rb");
	if (!f) return v;
	unsigned char buf[4096];
	size_t n;
	while ((n = fread(buf, 1, sizeof buf, f)) > 0) v.insert(v.end(), buf, buf + n);
	fclose(f);
	return v;
}
static void writeFileBytes(const char* path, const std::vector<unsigned char>& v)
{
	FILE* f = fopen(path, "wb");
	if (v.size()) fwrite(&v[0], 1, v.size(), f);
	fclose(f);
}
static unsigned hashBytes(const std::vector<unsigned char>& v)
{
	unsigned h = 2166136261u;
	for (size_t i = 0; i < v.size(); ++i) { h ^= v[i]; h *= 16777619u; }
	return h;
}

/* ---- the selectors: the alive prototypes and teams, found as the Rust finds them (through the lists) ------------- */
static std::vector<TeamPrototype*> aliveProtos()
{
	std::vector<TeamPrototype*> v;
	for (size_t i = 0; i < g_players.size(); ++i)
		for (size_t k = 0; k < g_players[i]->m_protos.size(); ++k)
			if (std::find(v.begin(), v.end(), g_players[i]->m_protos[k]) == v.end()) v.push_back(g_players[i]->m_protos[k]);
	std::stable_sort(v.begin(), v.end(), [](TeamPrototype* a, TeamPrototype* b) { return a->getID() < b->getID(); });
	return v;
}
static std::vector<Team*> aliveTeams()
{
	std::vector<Team*> v;
	std::vector<TeamPrototype*> ps = aliveProtos();
	for (size_t i = 0; i < ps.size(); ++i)
		for (Team* t = ps[i]->getFirstItemIn_TeamInstanceList(); t; t = t->dlink_next_TeamInstanceList()) v.push_back(t);
	std::stable_sort(v.begin(), v.end(), [](Team* a, Team* b) { return a->getID() < b->getID(); });
	return v;
}
static Team* pickTeam(unsigned k) { std::vector<Team*> v = aliveTeams(); return v.empty() ? NULL : v[k % v.size()]; }
static TeamPrototype* pickProto(unsigned k) { std::vector<TeamPrototype*> v = aliveProtos(); return v.empty() ? NULL : v[k % v.size()]; }
static Object* pickObj(unsigned k) { return g_objs[k % g_objs.size()]; }

/* ---- the dump --------------------------------------------------------------------------------------------------- */
static std::string idList(const std::vector<int>& v)
{
	std::string s; char b[32];
	for (size_t i = 0; i < v.size(); ++i) { sprintf(b, "%s%d", i ? "," : "", v[i]); s += b; }
	return s.empty() ? "-" : s;
}
static std::string relMapStr(const std::map<int, Relationship>& m)
{
	std::string s; char b[48];
	for (std::map<int, Relationship>::const_iterator it = m.begin(); it != m.end(); ++it) { sprintf(b, "%s%d:%d", s.empty() ? "" : ",", it->first, (int)it->second); s += b; }
	return s.empty() ? "-" : s;
}
static void dumpState()
{
	Rec("D frame=%u tid=%u pid=%u nprotos=%u", (unsigned)TheGameLogic->m_frame, (unsigned)TheTeamFactory->m_uniqueTeamID, (unsigned)TheTeamFactory->m_uniqueTeamPrototypeID,
		(unsigned)TheTeamFactory->m_prototypes.size());
	for (TeamFactory::TeamPrototypeMap::const_iterator it = TheTeamFactory->m_prototypes.begin(); it != TheTeamFactory->m_prototypes.end(); ++it)
		Rec("D map nk=%u proto=%u", (unsigned)it->first, (unsigned)it->second->getID());
	std::vector<TeamPrototype*> ps = aliveProtos();
	for (size_t i = 0; i < ps.size(); ++i)
	{
		TeamPrototype* p = ps[i];
		std::vector<int> insts;
		for (Team* t = p->getFirstItemIn_TeamInstanceList(); t; t = t->dlink_next_TeamInstanceList()) insts.push_back((int)t->getID());
		std::string gens; char b[64];
		for (int k = 0; k < MAX_GENERIC_SCRIPTS; ++k) { sprintf(b, "%s%d", k ? "," : "", p->m_genericScriptsToRun[k] ? p->m_genericScriptsToRun[k]->m_serial : 0); gens += b; }
		const TeamTemplateInfo& t = p->m_teamTemplate;
		std::string units;
		for (int k = 0; k < t.m_numUnitsInfo; ++k) { sprintf(b, "%s%d/%d/%s", k ? "," : "", t.m_unitsInfo[k].minUnits, t.m_unitsInfo[k].maxUnits, t.m_unitsInfo[k].unitThingName.str()); units += b; }
		Rec("D proto id=%u name=%s owner=%d flags=%d fals=%d cond=%d retr=%d gen=%s head=%d insts=%s attack=%s",
			(unsigned)p->getID(), p->m_name.str(), p->m_owningPlayer ? p->m_owningPlayer->m_index : -1, p->m_flags, p->m_productionConditionAlwaysFalse ? 1 : 0,
			p->m_productionConditionScript ? p->m_productionConditionScript->m_serial : 0, p->m_retrievedGenericScripts ? 1 : 0, gens.c_str(),
			tid(p->getFirstItemIn_TeamInstanceList()), idList(insts).c_str(), p->m_attackPriorityName.str());
		Rec("D tmpl id=%u units=%s n=%d home=%08x,%08x,%08x has=%d idle=%d dthr=%08x rec=%d bd=%d pd=%d ar=%d tr=%d av=%d ac=%d maxi=%d prio=%d psi=%d pfd=%d att=%d",
			(unsigned)p->getID(), units.empty() ? "-" : units.c_str(), t.m_numUnitsInfo, FB(t.m_homeLocation.x), FB(t.m_homeLocation.y), FB(t.m_homeLocation.z),
			t.m_hasHomeLocation ? 1 : 0, t.m_initialIdleFrames, FB(t.m_destroyedThreshold), t.m_isAIRecruitable ? 1 : 0, t.m_isBaseDefense ? 1 : 0,
			t.m_isPerimeterDefense ? 1 : 0, t.m_automaticallyReinforce ? 1 : 0, t.m_transportsReturn ? 1 : 0, t.m_avoidThreats ? 1 : 0, t.m_attackCommonTarget ? 1 : 0,
			t.m_maxInstances, t.m_productionPriority, t.m_productionPrioritySuccessIncrease, t.m_productionPriorityFailureDecrease, (int)t.m_initialTeamAttitude);
		Rec("D tmpl2 id=%u create=%s idlescr=%s sight=%s clear=%s unitdead=%s dead=%s trans=%s wp=%s full=%d exit=%d vet=%d cond=%s exec=%d gs=%s",
			(unsigned)p->getID(), t.m_scriptOnCreate.str(), t.m_scriptOnIdle.str(), t.m_scriptOnEnemySighted.str(), t.m_scriptOnAllClear.str(), t.m_scriptOnUnitDestroyed.str(),
			t.m_scriptOnDestroyed.str(), t.m_transportUnitType.str(), t.m_startReinforceWaypoint.str(), t.m_teamStartsFull ? 1 : 0, t.m_transportsExit ? 1 : 0, (int)t.m_veterancy,
			t.m_productionCondition.str(), t.m_executeActions ? 1 : 0, [&]() { std::string g; for (int k = 0; k < MAX_GENERIC_SCRIPTS; ++k) { g += t.m_teamGenericScripts[k].str(); g += ";"; } return g; }().c_str());
	}
	std::vector<Team*> ts = aliveTeams();
	for (size_t i = 0; i < ts.size(); ++i)
	{
		Team* t = ts[i];
		std::vector<int> mem;
		for (Object* o = t->getFirstItemIn_TeamMemberList(); o; o = o->dlink_next_TeamMemberList()) mem.push_back((int)o->m_id);
		char gen[40]; for (int k = 0; k < MAX_GENERIC_SCRIPTS; ++k) gen[k] = t->m_shouldAttemptGenericScript[k] ? '1' : '0'; gen[MAX_GENERIC_SCRIPTS] = 0;
		std::map<int, Relationship> tr, pr;
		for (TeamRelationMapType::const_iterator it = t->m_teamRelations->m_map.begin(); it != t->m_teamRelations->m_map.end(); ++it) tr[(int)it->first] = it->second;
		for (PlayerRelationMapType::const_iterator it = t->m_playerRelations->m_map.begin(); it != t->m_playerRelations->m_map.end(); ++it) pr[(int)it->first] = it->second;
		std::vector<int> xm; for (std::list<ObjectID>::const_iterator it = t->m_xferMemberIDList.begin(); it != t->m_xferMemberIDList.end(); ++it) xm.push_back((int)*it);
		Rec("D team id=%u proto=%u head=%d members=%s state=%s eoe=%d act=%d cre=%d ces=%d see=%d psee=%d widle=%d dth=%d cur=%d way=%u gen=%s rs=%d r=%d tgt=%d trel=%s prel=%s xm=%s",
			(unsigned)t->getID(), (unsigned)t->m_proto->getID(), (t->getFirstItemIn_TeamMemberList() ? (int)t->getFirstItemIn_TeamMemberList()->m_id : 0), idList(mem).c_str(), t->m_state.str(), t->m_enteredOrExited ? 1 : 0, t->m_active ? 1 : 0, t->m_created ? 1 : 0,
			t->m_checkEnemySighted ? 1 : 0, t->m_seeEnemy ? 1 : 0, t->m_prevSeeEnemy ? 1 : 0, t->m_wasIdle ? 1 : 0, t->m_destroyThreshold, t->m_curUnits,
			t->m_currentWaypoint ? (unsigned)t->m_currentWaypoint->getID() : 0u, gen, t->m_isRecruitablitySet ? 1 : 0, t->m_isRecruitable ? 1 : 0, (int)t->m_commonAttackTarget,
			relMapStr(tr).c_str(), relMapStr(pr).c_str(), idList(xm).c_str());
	}
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		Object* o = g_objs[i];
		Rec("D obj id=%d team=%d next=%d prev=%d", (int)o->m_id, tid(o->m_team), o->m_dlink_TeamMemberList.m_next ? (int)o->m_dlink_TeamMemberList.m_next->m_id : 0,
			o->m_dlink_TeamMemberList.m_prev ? (int)o->m_dlink_TeamMemberList.m_prev->m_id : 0);
	}
	for (size_t i = 0; i < g_players.size(); ++i)
	{
		std::vector<int> pr;
		for (size_t k = 0; k < g_players[i]->m_protos.size(); ++k) pr.push_back((int)g_players[i]->m_protos[k]->getID());
		Rec("D player %d default=%d protos=%s", g_players[i]->m_index, tid(g_players[i]->m_defaultTeam), idList(pr).c_str());
	}
}

/* ---- the kinds and statuses a program can name (index -> real enumerator) ---------------------------------------- */
static const KindOfType k_kindPool[8] = { KINDOF_STRUCTURE, KINDOF_PROJECTILE, KINDOF_MINE, KINDOF_INERT, KINDOF_TECH_BUILDING, KINDOF_AIRCRAFT, KINDOF_INFANTRY, KINDOF_VEHICLE };
static const ObjectStatusTypes k_statusPool[4] = { OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_UNDER_CONSTRUCTION };
static KindOfMaskType kindMask(unsigned bits)
{
	KindOfMaskType m;
	for (int k = 0; k < 8; ++k) if (bits & (1u << k)) m.set(k_kindPool[k]);
	return m;
}

static void iterCb(Object* o, void* ud) { Rec("iter %d", (int)o->m_id); }

/* ---- the world of a program --------------------------------------------------------------------------------------- */
static void clearWorld()
{
	g_lines = NULL;
	if (TheTeamFactory) { try { delete TheTeamFactory; } catch (...) {} TheTeamFactory = NULL; }
	for (size_t i = 0; i < g_objs.size(); ++i) { g_objs[i]->m_team = NULL; }
	for (size_t i = 0; i < g_objs.size(); ++i) delete g_objs[i];
	g_objs.clear();
	for (size_t i = 0; i < g_players.size(); ++i) delete g_players[i];
	g_players.clear();
	for (size_t i = 0; i < g_tpls.size(); ++i) delete g_tpls[i];
	g_tpls.clear();
	for (size_t i = 0; i < g_ways.size(); ++i) delete g_ways[i];
	g_ways.clear();
	for (size_t i = 0; i < g_scriptEngine.m_scripts.size(); ++i) delete g_scriptEngine.m_scripts[i];
	g_scriptEngine.m_scripts.clear();
	g_dicts.clear();
	g_thingFactory.m_templates.clear();
	g_gameLogic.m_objs.clear();
	g_terrain.m_ways.clear();
	g_playerList.m_count = 0;
	g_playerList.m_byKey.clear();
	g_haveSave = false;
	g_saved.clear();
	g_savedObjTeam.clear();
}

static void resetKeys()
{
	/* the well-known keys cache their key in the StaticNameKey; a fresh generator needs them looked up afresh */
#define X(n) TheKey_##n.m_key = NAMEKEY_INVALID;
#include "well_known_keys_list.h"
#undef X
}

static void setupWorld(Reader& r)
{
	int np = r.i();
	for (int i = 0; i < np; ++i)
	{
		Player* p = new Player;
		p->m_index = i; p->m_type = (PlayerType)r.i(); p->m_diff = (GameDifficulty)r.i(); p->m_relSalt = r.u(); p->m_defaultTeam = NULL;
		int bt = r.i();
		char b[32]; sprintf(b, bt >= 0 ? "Tpl%d" : "NoSuchTemplate", bt);
		p->m_tmpl.m_beacon = AsciiString(b);
		g_players.push_back(p);
		g_playerList.m_players[i] = p;
		sprintf(b, "Player%d", i);
		g_playerList.m_byKey[(unsigned)TheNameKeyGenerator->nameToKey(b)] = i;
	}
	g_playerList.m_count = np;

	int nt = r.i();
	for (int i = 0; i < nt; ++i)
	{
		ThingTemplate* t = new ThingTemplate;
		t->m_idx = i;
		int nk = r.i();
		for (int k = 0; k < nk; ++k) t->m_kinds.set(k_kindPool[r.u() % 8]);
		t->m_buildFacility = r.b(); t->m_equiv = r.i();
		char b[32]; sprintf(b, "Tpl%d", r.i());
		t->m_name = AsciiString(b);
		int nv = r.i();
		for (int k = 0; k < nv; ++k) { int v = r.i(); if (v >= 8) sprintf(b, "tpl%d", v - 8); else sprintf(b, "Tpl%d", v); t->m_variations.push_back(AsciiString(b)); }
		g_tpls.push_back(t);
		g_thingFactory.m_templates.push_back(t);
	}

	int no = r.i();
	for (int i = 0; i < no; ++i)
	{
		Object* o = new Object;
		o->m_id = (ObjectID)r.u();
		o->m_tpl = g_tpls[r.u() % g_tpls.size()];
		o->m_player = g_players[r.u() % g_players.size()];
		o->m_team = NULL;
		int ns = r.i();
		for (int k = 0; k < ns; ++k) o->m_status.set(k_statusPool[r.u() % 4]);
		o->m_dead = r.b(); o->m_destroyed = r.b(); o->m_contained = r.b(); o->m_held = r.b(); o->m_enemyNear = r.b();
		o->m_hasAI = r.b(); o->m_ai.m_owner = o; o->m_ai.m_idle = r.b(); o->m_ai.m_recruitable = r.b(); o->m_ai.m_loco.m_valid = r.u();
		int cc = r.i(); o->m_hasContain = cc >= 0; o->m_contain.m_owner = o; o->m_contain.m_count = cc;
		o->m_pos = r.c(); o->m_vision = r.f();
		o->m_didEnter = r.u(); o->m_didExit = r.u(); o->m_inside = r.u(); o->m_reactions = r.u();
		o->m_nextObj = NULL;
		if (!g_objs.empty()) g_objs.back()->m_nextObj = o;
		g_objs.push_back(o);
		g_gameLogic.m_objs.push_back(o);
	}

	int ns = r.i();
	g_scriptEngine.m_nextSerial = ns;
	for (int i = 0; i < ns; ++i)
	{
		Script* s = new Script;
		s->m_serial = i + 1;
		char b[32]; sprintf(b, "Scr%d", i);
		s->m_name = AsciiString(b);
		s->m_frameToEvaluate = r.u(); s->m_delay = r.i(); s->m_easy = r.b(); s->m_normal = r.b(); s->m_hard = r.b(); s->m_oneShot = r.b();
		s->m_pattern = r.u(); s->m_calls = 0; s->m_action.m_serial = s->m_serial;
		g_scriptEngine.m_scripts.push_back(s);
	}

	int nw = r.i();
	for (int i = 0; i < nw; ++i)
	{
		Waypoint* w = new Waypoint;
		char b[32]; sprintf(b, "Way%d", r.i());
		w->m_name = AsciiString(b);
		w->m_id = r.u(); w->m_loc = r.c(); w->m_next = NULL;
		if (!g_ways.empty()) g_ways.back()->m_next = w;
		g_ways.push_back(w);
		g_terrain.m_ways.push_back(w);
	}

	int nd = r.i();
	for (int i = 0; i < nd; ++i)
	{
		DictSpec spec;
		int ne = r.i();
		for (int k = 0; k < ne; ++k)
		{
			DictEntry e; e.type = r.i(); e.key = r.s(); e.bits = 0;
			if (e.type == 3) e.str = r.s(); else e.bits = r.u();
			spec.e.push_back(e);
		}
		g_dicts.push_back(spec);
	}
	g_gameLogic.m_frame = r.u();
}

static AsciiString teamName(unsigned id) { char b[32]; sprintf(b, "Team%u", id); return AsciiString(b); }
static std::string countsStr(const int* c, int n) { std::string s; char b[16]; for (int i = 0; i < n; ++i) { sprintf(b, "%s%d", i ? "," : "", c[i]); s += b; } return s; }

static void teamOp(Team* t, unsigned sub, const unsigned* a, int nPlayers)
{
	Dict dummy;
	switch (sub)
	{
	case 0: Rec("ret targetable %d", t->getTargetableCount()); break;
	case 1: { Team* that = a[0] ? pickTeam(a[0] - 1) : NULL; Rec("ret rel %d", (int)t->getRelationship(that)); break; }
	case 2: { Object* o = a[0] ? pickObj(a[0] - 1) : NULL; t->setTeamTargetObject(o); break; }
	case 3: { Object* o = t->getTeamTargetObject(); Rec("ret target %d", o ? (int)o->m_id : 0); break; }
	case 4: t->setOverrideTeamRelationship((TeamID)a[0], (Relationship)(a[1] % 3)); break;
	case 5: Rec("ret rmteamrel %d", t->removeOverrideTeamRelationship((TeamID)a[0]) ? 1 : 0); break;
	case 6: t->setOverridePlayerRelationship((Int)(int)a[0], (Relationship)(a[1] % 3)); break;
	case 7: Rec("ret rmplayerrel %d", t->removeOverridePlayerRelationship((Int)(int)a[0]) ? 1 : 0); break;
	case 8: {
		int n = a[0] % 5; const ThingTemplate* things[4]; int counts[4] = { 0, 0, 0, 0 };
		for (int k = 0; k < n; ++k) things[k] = a[1 + k] ? g_tpls[(a[1 + k] - 1) % g_tpls.size()] : NULL;
		t->countObjectsByThingTemplate(n, things, a[5] != 0, counts, a[6] != 0);
		Rec("ret count %s", countsStr(counts, n).c_str());
		break; }
	case 9: Rec("ret buildings %d", t->countBuildings()); break;
	case 10: Rec("ret objects %d", t->countObjects(kindMask(a[0]), kindMask(a[1]))); break;
	case 11: t->healAllObjects(); break;
	case 12: t->iterateObjects(iterCb, NULL); break;
	case 13: Rec("ret anybuildings %d", t->hasAnyBuildings() ? 1 : 0); break;
	case 14: Rec("ret anybuildingskind %d", t->hasAnyBuildings(kindMask(a[0])) ? 1 : 0); break;
	case 15: Rec("ret anyunits %d", t->hasAnyUnits() ? 1 : 0); break;
	case 16: Rec("ret anyobjects %d", t->hasAnyObjects() ? 1 : 0); break;
	case 17: Rec("ret idle %d", t->isIdle() ? 1 : 0); break;
	case 18: t->updateState(); break;
	case 19: t->notifyTeamOfObjectDeath(); break;
	case 20: {
		PolygonTrigger* trg = &g_triggers[a[1] % 2]; unsigned which = a[2] % 4; bool r = false;
		switch (a[0] % 7)
		{
		case 0: r = t->didAllEnter(trg, which); break;
		case 1: r = t->didPartialEnter(trg, which); break;
		case 2: r = t->didPartialExit(trg, which); break;
		case 3: r = t->didAllExit(trg, which); break;
		case 4: r = t->allInside(trg, which); break;
		case 5: r = t->noneInside(trg, which); break;
		default: r = t->someInsideSomeOutside(trg, which); break;
		}
		Rec("ret trigger %u %d", a[0] % 7, r ? 1 : 0);
		break; }
	case 21: { const Coord3D* p = t->getEstimateTeamPosition(); if (p) Rec("ret pos %08x,%08x,%08x", FB(p->x), FB(p->y), FB(p->z)); else Rec("ret pos null"); break; }
	case 22: t->deleteTeam(a[0] != 0); break;
	case 23: { Team* other = a[0] ? pickTeam(a[0] - 1) : NULL; t->transferUnitsTo(other); break; }
	case 24: {
		Coord3D home; memcpy(&home.x, &a[1], 4); memcpy(&home.y, &a[2], 4); memcpy(&home.z, &a[3], 4); float md; memcpy(&md, &a[4], 4);
		Object* o = t->tryToRecruit(g_tpls[a[0] % g_tpls.size()], &home, md);
		Rec("ret recruit %d", o ? (int)o->m_id : 0);
		break; }
	case 25: t->evacuateTeam(); break;
	case 26: t->killTeam(); break;
	case 27: { float amt; memcpy(&amt, &a[0], 4); Rec("ret damage %d", t->damageTeamMembers(amt) ? 1 : 0); break; }
	case 28: { Coord3D d; memcpy(&d.x, &a[0], 4); memcpy(&d.y, &a[1], 4); memcpy(&d.z, &a[2], 4); t->moveTeamTo(d); break; }
	case 29: Rec("ret buildfacility %d", t->hasAnyBuildFacility() ? 1 : 0); break;
	case 30: t->updateGenericScripts(); break;
	case 31: t->setActive(); break;
	case 32: { t->setState(teamName(a[0])); Rec("ret state %s", t->getState().str()); break; }
	case 33: t->setRecruitable(a[0] != 0); break;
	case 34: t->setEnteredExited(); break;
	case 35: { t->setCurrentWaypoint(a[0] && !g_ways.empty() ? g_ways[(a[0] - 1) % g_ways.size()] : NULL); const Waypoint* w = t->getCurrentWaypoint(); Rec("ret waypoint %u", w ? (unsigned)w->getID() : 0u); break; }
	case 36: t->setAttackPriorityName(teamName(a[0])); break;
	case 37: {
		Player* p = t->getControllingPlayer();
		Rec("ret getters name=%s player=%d active=%d created=%d eoe=%d proto=%u", t->getName().str(), p ? p->m_index : -1, t->isActive() ? 1 : 0, t->isCreated() ? 1 : 0,
			t->didEnterOrExit() ? 1 : 0, (unsigned)t->getPrototype()->getID());
		break; }
	case 38: t->setControllingPlayer(a[0] ? g_players[(a[0] - 1) % nPlayers] : NULL); break;
	case 39: { AIGroup* g = a[0] ? &g_groups[(a[0] - 1) % 3] : NULL; t->getTeamAsAIGroup(g); break; }
	default: Rec("ret nop"); break;
	}
}

static void protoOp(TeamPrototype* p, unsigned sub, const unsigned* a, int nPlayers)
{
	switch (sub)
	{
	case 0: { Player* pl = p->getControllingPlayer(); Rec("ret owner %d", pl ? pl->m_index : -1); break; }
	case 1: { Team* t = p->findTeamByID((TeamID)a[0]); Rec("ret findteam %d", tid(t)); break; }
	case 2: p->setControllingPlayer(a[0] ? g_players[(a[0] - 1) % nPlayers] : NULL); break;
	case 3: {
		int n = a[0] % 5; const ThingTemplate* things[4]; int counts[4] = { 0, 0, 0, 0 };
		for (int k = 0; k < n; ++k) things[k] = a[1 + k] ? g_tpls[(a[1 + k] - 1) % g_tpls.size()] : NULL;
		p->countObjectsByThingTemplate(n, things, a[5] != 0, counts, a[6] != 0);
		Rec("ret count %s", countsStr(counts, n).c_str());
		break; }
	case 4: { Script* s = p->getGenericScript((Int)(a[0] % MAX_GENERIC_SCRIPTS)); Rec("ret generic %d", s ? s->m_serial : 0); break; }
	case 5: p->increaseAIPriorityForSuccess(); Rec("ret prio %d", p->m_teamTemplate.m_productionPriority); break;
	case 6: p->decreaseAIPriorityForFailure(); Rec("ret prio %d", p->m_teamTemplate.m_productionPriority); break;
	case 7: Rec("ret buildings %d", p->countBuildings()); break;
	case 8: Rec("ret objects %d", p->countObjects(kindMask(a[0]), kindMask(a[1]))); break;
	case 9: p->healAllObjects(); break;
	case 10: p->iterateObjects(iterCb, NULL); break;
	case 11: Rec("ret instances %d", p->countTeamInstances()); break;
	case 12: Rec("ret anybuildings %d", p->hasAnyBuildings() ? 1 : 0); break;
	case 13: Rec("ret anybuildingskind %d", p->hasAnyBuildings(kindMask(a[0])) ? 1 : 0); break;
	case 14: Rec("ret anyunits %d", p->hasAnyUnits() ? 1 : 0); break;
	case 15: Rec("ret anyobjects %d", p->hasAnyObjects() ? 1 : 0); break;
	case 16: Rec("ret buildfacility %d", p->hasAnyBuildFacility() ? 1 : 0); break;
	case 17: { float amt; memcpy(&amt, &a[0], 4); p->damageTeamMembers(amt); break; }
	case 18: { Coord3D d; memcpy(&d.x, &a[0], 4); memcpy(&d.y, &a[1], 4); memcpy(&d.z, &a[2], 4); p->moveTeamTo(d); break; }
	case 19: p->updateState(); break;
	case 20: Rec("ret production %d", p->evaluateProductionCondition() ? 1 : 0); break;
	case 21: Rec("ret getters id=%u name=%s singleton=%d maxi=%d", (unsigned)p->getID(), p->getName().str(), p->getIsSingleton() ? 1 : 0, p->getTemplateInfo()->m_maxInstances); break;
	case 22: { Team* t = a[0] ? pickTeam(a[0] - 1) : NULL; p->teamAboutToBeDeleted(t); break; }
	case 23: p->setAttackPriorityName(teamName(a[0])); Rec("ret attack %s", p->getAttackPriorityName().str()); break;
	default: Rec("ret nop"); break;
	}
}

static void objOp(Object* o, unsigned sub, const unsigned* a)
{
	switch (sub)
	{
	case 0: o->m_dead = a[0] != 0; break;
	case 1: o->m_destroyed = a[0] != 0; break;
	case 2: o->m_status.set(k_statusPool[a[0] % 4], a[1] != 0 ? 1 : 0); break;
	case 3: o->m_enemyNear = a[0] != 0; break;
	case 4: o->m_didEnter = a[0]; o->m_didExit = a[1]; o->m_inside = a[2]; break;
	case 5: o->m_hasContain = (int)a[0] >= 0; o->m_contain.m_count = (int)a[0]; break;
	case 6: o->m_held = a[0] != 0; break;
	case 7: o->m_contained = a[0] != 0; break;
	case 8: o->m_ai.m_idle = a[0] != 0; o->m_ai.m_recruitable = a[1] != 0; o->m_ai.m_loco.m_valid = a[2]; o->m_hasAI = a[3] != 0; break;
	case 9: memcpy(&o->m_pos.x, &a[0], 4); memcpy(&o->m_pos.y, &a[1], 4); memcpy(&o->m_pos.z, &a[2], 4); break;
	case 10: o->m_reactions = a[0]; break;
	case 11: memcpy(&o->m_vision, &a[0], 4); break;
	default: break;
	}
}

static std::string throwName(int kind) { return kind == 0 ? "badarg" : "invaliddata"; }

/* one operation; returns false for an operation the program cannot give (no team to pick: written as a skip) */
static void doOp(unsigned op, Reader& r)
{
	int np = (int)g_players.size();
	switch (op)
	{
	case 0: { unsigned n = r.u(); g_gameLogic.m_frame += n; break; }
	case 1: {
		unsigned nameId = r.u(), ownerSel = r.u(); bool singleton = r.b(); unsigned dictIdx = r.u();
		char ob[32]; if ((int)(ownerSel % (np + 1)) < np) sprintf(ob, "Player%d", (int)(ownerSel % (np + 1))); else strcpy(ob, "Nobody");
		Dict d; if (!g_dicts.empty()) buildDict(g_dicts[dictIdx % g_dicts.size()], d);
		TheTeamFactory->initTeam(teamName(nameId), AsciiString(ob), singleton, &d);
		break; }
	case 2: {
		unsigned n = r.u() % 4; SidesList sides;
		for (unsigned k = 0; k < n; ++k) { unsigned di = r.u(); TeamsInfo* ti = new TeamsInfo; if (!g_dicts.empty()) buildDict(g_dicts[di % g_dicts.size()], ti->m_dict); sides.m_teams.push_back(ti); }
		TheTeamFactory->initFromSides(&sides);
		for (size_t k = 0; k < sides.m_teams.size(); ++k) delete sides.m_teams[k];
		break; }
	case 3: TheTeamFactory->reset(); break;
	case 4: TheTeamFactory->init(); break;
	case 5: TheTeamFactory->clear(); break;
	case 6: { unsigned id = r.u(); Team* t = TheTeamFactory->createInactiveTeam(teamName(id)); Rec("ret team %d", tid(t)); break; }
	case 7: { unsigned id = r.u(); Team* t = TheTeamFactory->createTeam(teamName(id)); Rec("ret team %d", tid(t)); break; }
	case 8: { unsigned sel = r.u(); TeamPrototype* p = sel ? pickProto(sel - 1) : NULL; Team* t = TheTeamFactory->createTeamOnPrototype(p); Rec("ret team %d", tid(t)); break; }
	case 9: { unsigned id = r.u(); Team* t = TheTeamFactory->findTeam(teamName(id)); Rec("ret team %d", tid(t)); break; }
	case 10: { unsigned id = r.u(); TeamPrototype* p = TheTeamFactory->findTeamPrototype(teamName(id)); Rec("ret proto %d", p ? (int)p->getID() : 0); break; }
	case 11: { unsigned raw = r.u(); Team* t = TheTeamFactory->findTeamByID((TeamID)raw); Rec("ret team %d", tid(t)); break; }
	case 12: { unsigned raw = r.u(); TeamPrototype* p = TheTeamFactory->findTeamPrototypeByID((TeamPrototypeID)raw); Rec("ret proto %d", p ? (int)p->getID() : 0); break; }
	case 13: { unsigned ok = r.u(), ts = r.u(); Team* t = ts ? pickTeam(ts - 1) : NULL; if (ts && !t) { Rec("skip"); break; } pickObj(ok)->setTeam(t); break; }
	case 14: { unsigned pk = r.u(), ts = r.u(); Team* t = ts ? pickTeam(ts - 1) : NULL; if (ts && !t) { Rec("skip"); break; } g_players[pk % np]->m_defaultTeam = t; break; }
	case 15: {
		unsigned tk = r.u(), sub = r.u(), a[8]; for (int k = 0; k < 8; ++k) a[k] = r.u();
		Team* t = pickTeam(tk);
		if (!t) { Rec("skip"); break; }
		teamOp(t, sub, a, np);
		break; }
	case 16: {
		unsigned pk = r.u(), sub = r.u(), a[8]; for (int k = 0; k < 8; ++k) a[k] = r.u();
		TeamPrototype* p = pickProto(pk);
		if (!p) { Rec("skip"); break; }
		protoOp(p, sub, a, np);
		break; }
	case 17: { unsigned ok = r.u(), sub = r.u(), a[8]; for (int k = 0; k < 8; ++k) a[k] = r.u(); objOp(pickObj(ok), sub, a); break; }
	case 18: {
		unsigned sub = r.u();
		if (sub == 0)
		{
			{ XferCRC xc; xc.open(AsciiString("t")); xc.xferSnapshot(TheTeamFactory); xc.close(); Rec("ret crc %08x", xc.getCRC()); }
			XferSave xs; xs.open(AsciiString(g_tmpPath.c_str())); xs.xferSnapshot(TheTeamFactory); xs.close();
			g_saved = readFileBytes(g_tmpPath.c_str());
			g_haveSave = true;
			g_savedObjTeam.clear();
			for (size_t i = 0; i < g_objs.size(); ++i) g_savedObjTeam[(int)g_objs[i]->m_id] = tid(g_objs[i]->m_team);
			Rec("ret xfer-save len=%u hash=%08x", (unsigned)g_saved.size(), hashBytes(g_saved));
		}
		else if (!g_haveSave) Rec("skip");
		else
		{
			writeFileBytes(g_tmpPath.c_str(), g_saved);
			TheGameState->m_postProcessCount = 0;
			XferLoad xl; xl.open(AsciiString(g_tmpPath.c_str()));
			try { xl.xferSnapshot(TheTeamFactory); } catch (...) { try { xl.close(); } catch (...) {} throw; }
			xl.close();
			Rec("ret xfer-load pp=%d", TheGameState->m_postProcessCount);
			/* the objects' own xfer: each puts itself on the team it was saved on */
			for (size_t i = 0; i < g_objs.size(); ++i)
			{
				int saved = g_savedObjTeam[(int)g_objs[i]->m_id];
				g_objs[i]->setTeam(saved ? TheTeamFactory->findTeamByID((TeamID)saved) : NULL);
			}
			/* post process: the teams (in id order), then the factory */
			std::vector<Team*> ts = aliveTeams();
			for (size_t i = 0; i < ts.size(); ++i) ts[i]->loadPostProcess();
			TheTeamFactory->loadPostProcess();
		}
		break; }
	case 19: { unsigned ts = r.u(); Team* t = ts ? pickTeam(ts - 1) : NULL; if (ts && !t) { Rec("skip"); break; } TheTeamFactory->teamAboutToBeDeleted(t); break; }
	default: Rec("ret nop"); break;
	}
}

/* The Team differential (tests/differential_team.rs): runs a program (the words) against the real Team.cpp on the
** stand-ins, and writes the trace lines, each ended by '\n'. Returns the size of the text (written only if it fits
** `cap`). tmpPath is a file the XferSave/XferLoad operations use. */
extern "C" int ref_team_run(const unsigned* words, int n, const char* tmpPath, char* out, int cap)
{
	static NameKeyGenerator* gen = NULL;
	if (!gen) gen = new NameKeyGenerator;
	gen->reset();
	TheNameKeyGenerator = gen;
	resetKeys();
	g_tmpPath = tmpPath;
	for (int i = 0; i < 3; ++i) g_groups[i].m_id = i + 1;
	for (int i = 0; i < 2; ++i) g_triggers[i].m_id = i;
	std::vector<std::string> lines;
	g_lines = &lines;
	g_setTeamCalls = 0;
	TheTeamFactory = new TeamFactory;
	TheTeamFactory->init();
	Reader r; r.w = words; r.n = n; r.p = 0;
	setupWorld(r);
	int nProbe = r.i();
	std::vector<std::string> probes;
	for (int i = 0; i < nProbe; ++i) probes.push_back(r.s());
	int nOps = r.i();
	bool threw = false;
	for (int k = 0; k < nOps && !threw; ++k)
	{
		g_setTeamCalls = 0;
		unsigned op = r.u();
		try { doOp(op, r); }
		catch (ErrorCode) { Rec("threw badarg"); }
		catch (SaveCode) { Rec("threw invaliddata"); threw = true; }
		catch (XferStatus s) { Rec("threw xfer %d", (int)s); threw = true; }
		catch (HangException) { Rec("threw hang"); threw = true; }
		if (!threw) dumpState();
	}
	if (!threw) for (size_t i = 0; i < probes.size(); ++i) Rec("K %s %u", probes[i].c_str(), (unsigned)TheNameKeyGenerator->nameToKey(probes[i].c_str()));
	std::string all;
	for (size_t i = 0; i < lines.size(); ++i) { all += lines[i]; all += '\n'; }
	clearWorld();
	if ((int)all.size() <= cap) memcpy(out, all.data(), all.size());
	return (int)all.size();
}

#endif
