/* The driver of the Team differential (p1w9-sonnet): the definitions of the stand-ins of team_standins_decl.h, the
** program reader, the operations, the state dump, and the entry point ref_team_run. See tests/differential_team.rs
** for the program format (the writer is its generator; this file is the reader) and for the method. Included by
** differential/wrap_team.cpp after the cut Team.cpp. */
#ifndef TEAM_DRIVER_H
#define TEAM_DRIVER_H

/* Common/GameState.h is the Xfer archive's shim (differential/shim/Common/GameState.h), renamed here: XferLoad::xferSnapshot calls
** addPostProcessSnapshot, which counts (the number of post-process requests a load made), and Xfer::xferMapName is not reached */
static GameState g_theGameState; GameState* TheGameState = &g_theGameState;
AsciiString GameState::realMapPathToPortableMapPath(const AsciiString&) const { abort(); }
AsciiString GameState::portableMapPathToRealMapPath(const AsciiString&) const { abort(); }
void GameState::addPostProcessSnapshot(Snapshot*) { ++m_postProcessCount; }

/* ---- globals ------------------------------------------------------------------------------------------------ */
static ThingFactory g_thingFactory; ThingFactory* TheThingFactory = &g_thingFactory;
static GameLogic g_gameLogic; GameLogic* TheGameLogic = &g_gameLogic;
static TerrainLogic g_terrain; TerrainLogic* TheTerrainLogic = &g_terrain;
static ScriptEngine g_scriptEngine; ScriptEngine* TheScriptEngine = &g_scriptEngine;
static PartitionManager g_partition; PartitionManager* ThePartitionManager = &g_partition;
static PlayerList g_playerList; PlayerList* ThePlayerList = &g_playerList;
static GlobalData g_globalData; GlobalData* TheGlobalData = &g_globalData;
static GameInfo g_gameInfo; GameInfo* TheGameInfo = NULL;
static MessageStream g_messageStream; MessageStream* TheMessageStream = &g_messageStream;
static AI g_ai; AI* TheAI = &g_ai;
static Eva g_eva; Eva* TheEva = &g_eva;
static ControlBar g_controlBar; ControlBar* TheControlBar = &g_controlBar;
static InGameUI g_inGameUI; InGameUI* TheInGameUI = &g_inGameUI;
static GameText g_gameText; GameText* TheGameText = &g_gameText;
static VictoryConditions g_victory; VictoryConditions* TheVictoryConditions = &g_victory;
static AudioManager g_audio; AudioManager* TheAudio = &g_audio;
static RankInfoStore g_rankStore; RankInfoStore* TheRankInfoStore = &g_rankStore;
static ScienceStore* g_scienceStorePtr = NULL; ScienceStore* TheScienceStore = NULL;
static AIGroup g_groups[3];
static UpgradeCenter g_upgradeCenter; UpgradeCenter* TheUpgradeCenter = &g_upgradeCenter;
void UpgradeCenter::init() {}
void UpgradeCenter::reset() {}
static std::vector<UpgradeTemplate*> g_upgrades;
static std::string g_tmpPath;
static std::vector<unsigned char> g_saved;
static bool g_haveSave = false;
static PolygonTrigger g_triggers[2];
static unsigned g_setTeamCalls = 0;
static const unsigned HANG_LIMIT = 10000;
struct HangException {};
static bool g_msgLive = false;
static GameMessage g_theMessage;

static int tid(const Team* t) { return t ? (int)t->getID() : 0; }
static int pidx(const Player* p) { return p ? (int)p->m_playerIndex : -1; }

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
	Rec("eval %d team=%d player=%d -> %d", script->m_serial, tid(team), player ? pidx(player) : -1, r ? 1 : 0);
	return r;
}
void ScriptEngine::runScript(const AsciiString& name, Team* team) { Rec("run %s team=%d", name.str(), tid(team)); }
void ScriptEngine::notifyOfTeamDestruction(Team* team) { Rec("notify-destroy team=%d", tid(team)); }
void ScriptEngine::notifyOfObjectCreationOrDestruction() { Rec("script.objchange"); }
void ScriptEngine::notifyOfAcquiredScience(Int playerIndex, ScienceType science) { Rec("script.science %d %d", playerIndex, (int)science); }

/* PlayerList::teamAboutToBeDeleted is PlayerList.cpp's, verbatim (player_fns.inc) */

Object* PartitionManager::getClosestObject(const Object* obj, Real maxDist, DistanceCalculationType dc, PartitionFilter** filters)
{
	Rec("closest %d range=%08x", (int)obj->m_id, FB(maxDist));
	return obj->m_enemyNear ? const_cast<Object*>(obj) : NULL;
}
Real PartitionManager::getDistanceSquared(const Object* a, const Object* b, DistanceCalculationType dc)
{
	Real dx = a->m_pos.x - b->m_pos.x, dy = a->m_pos.y - b->m_pos.y;
	return dx * dx + dy * dy;
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
void Object::setScriptStatus(ObjectScriptStatusBit bit, Bool set) { Rec("script-status %d %d %d", (int)m_id, (int)bit, set ? 1 : 0); }
void Object::setDisabled(DisabledType type) { Rec("disable %d %d", (int)m_id, (int)type); }
void Object::clearDisabled(DisabledType type) { Rec("enable %d %d", (int)m_id, (int)type); }
void SpecialPowerModuleInterface::onSpecialPowerCreation() { Rec("sp.created %d", m_serial); }
void SpecialPowerModuleInterface::setReadyFrame(UnsignedInt f) { Rec("sp.ready %d %u", m_serial, (unsigned)f); }

/* ---- the players' sub-objects and services ------------------------------------------------------------------------ */
void GameMessage::appendIntegerArgument(Int a) { Rec("msg.int %d", a); }
void GameMessage::appendBooleanArgument(Bool a) { Rec("msg.bool %d", a ? 1 : 0); }
GameMessage* MessageStream::appendMessage(GameMessage::Type t) { Rec("msg.append %d", (int)t); return &g_theMessage; }
void Eva::setShouldPlay(EvaMessage m) { Rec("eva %d", (int)m); }
void ControlBar::markUIDirty() { Rec("ui.dirty"); }
void ControlBar::onPlayerSciencePurchasePointsChanged(const Player* p) { Rec("ui.spp %d", pidx(p)); }
void ControlBar::onPlayerRankChanged(const Player* p) { Rec("ui.rank %d", pidx(p)); }
UnicodeString GameText::fetch(const char* label) { Rec("text %s", label); return strcmp(label, "GUI:AddCash") == 0 ? UnicodeString(L"+$%d") : UnicodeString(L"General"); }
Bool VictoryConditions::hasSinglePlayerBeenDefeated(const Player* p) { return m_defeated[pidx(p) & 15]; }
void AudioManager::addAudioEvent(const AudioEventRTS* e) { Rec("audio %d player=%d", (int)e->m_id, (int)e->m_playerIndex); }

AIPlayer::AIPlayer(Player* p) : m_player(p), m_skirmish(false), m_diff(DIFFICULTY_NORMAL), m_enemy(NULL), m_supplySafe(true), m_supplyAttacked(false), m_checkBridges(false), m_hasBase(false), m_superOK(false), m_construct(false)
{
	m_base.x = m_base.y = m_base.z = 0; m_constructAt = m_base;
	Rec("ai.new %d", pidx(p));
}
AIPlayer::~AIPlayer() { Rec("ai.delete %d", pidx(m_player)); }
void AIPlayer::update() { Rec("ai.update %d", pidx(m_player)); }
void AIPlayer::newMap() { Rec("ai.newmap %d", pidx(m_player)); }
void AIPlayer::setAIDifficulty(GameDifficulty d) { Rec("ai.diff %d %d", pidx(m_player), (int)d); m_diff = d; }
Bool AIPlayer::computeSuperweaponTarget(const SpecialPowerTemplate*, Coord3D* pos, Int playerNdx, Real radius) { Rec("ai.super %d %d %08x", pidx(m_player), playerNdx, FB(radius)); if (m_superOK) *pos = m_base; return m_superOK; }
Bool AIPlayer::checkBridges(Object* unit, Waypoint* way) { Rec("ai.bridges %d %d %u", pidx(m_player), unit ? (int)unit->m_id : 0, way ? (unsigned)way->m_id : 0u); return m_checkBridges; }
Bool AIPlayer::getBaseCenter(Coord3D* pos) { Rec("ai.base %d", pidx(m_player)); if (m_hasBase) *pos = m_base; return m_hasBase; }
void AIPlayer::repairStructure(ObjectID id) { Rec("ai.repair %d %d", pidx(m_player), (int)id); }
void AIPlayer::onUnitProduced(Object* factory, Object* unit) { Rec("ai.unit %d %d %d", pidx(m_player), factory ? (int)factory->m_id : 0, unit ? (int)unit->m_id : 0); }
void AIPlayer::onStructureProduced(Object* builder, Object* structure) { Rec("ai.structure %d %d %d", pidx(m_player), builder ? (int)builder->m_id : 0, structure ? (int)structure->m_id : 0); }
Bool AIPlayer::isSupplySourceSafe(Int minSupplies) { Rec("ai.safe %d %d", pidx(m_player), minSupplies); return m_supplySafe; }
Bool AIPlayer::isSupplySourceAttacked() { Rec("ai.attacked %d", pidx(m_player)); return m_supplyAttacked; }
void AIPlayer::setTeamDelaySeconds(Int delay) { Rec("ai.delay %d %d", pidx(m_player), delay); }
void AIPlayer::guardSupplyCenter(Team* team, Int minSupplies) { Rec("ai.guard %d %d %d", pidx(m_player), tid(team), minSupplies); }
void AIPlayer::aiPreTeamDestroy(const Team* team) { Rec("ai.preteam %d %d", pidx(m_player), tid(team)); }
void AIPlayer::selectSkillset(Int skillSet) { Rec("ai.skillset %d %d", pidx(m_player), skillSet); }
void AIPlayer::buildSpecificAITeam(TeamPrototype* proto, Bool priority) { Rec("ai.buildteam %d %u %d", pidx(m_player), proto ? (unsigned)proto->getID() : 0u, priority ? 1 : 0); }
void AIPlayer::buildAIBaseDefense(Bool flank) { Rec("ai.basedef %d %d", pidx(m_player), flank ? 1 : 0); }
void AIPlayer::buildAIBaseDefenseStructure(const AsciiString& name, Bool flank) { Rec("ai.basedefstruct %d %s %d", pidx(m_player), name.str(), flank ? 1 : 0); }
void AIPlayer::buildSpecificAIBuilding(const AsciiString& name) { Rec("ai.building %d %s", pidx(m_player), name.str()); }
void AIPlayer::buildBySupplies(Int cash, const AsciiString& name) { Rec("ai.bysupplies %d %d %s", pidx(m_player), cash, name.str()); }
void AIPlayer::buildSpecificBuildingNearestTeam(const AsciiString& name, const Team* team) { Rec("ai.nearest %d %s %d", pidx(m_player), name.str(), tid(team)); }
void AIPlayer::buildUpgrade(const AsciiString& name) { Rec("ai.upgrade %d %s", pidx(m_player), name.str()); }
void AIPlayer::recruitSpecificAITeam(TeamPrototype* proto, Real radius) { Rec("ai.recruit %d %u %08x", pidx(m_player), proto ? (unsigned)proto->getID() : 0u, FB(radius)); }
Bool AIPlayer::calcClosestConstructionZoneLocation(const ThingTemplate* t, Coord3D* loc) { Rec("ai.zone %d %s", pidx(m_player), t ? t->m_name.str() : "-"); if (m_construct) *loc = m_constructAt; return m_construct; }

/* the sub-objects of the player that are other units (verified by their own tests): each call that has an effect is a line */
ScoreKeeper::ScoreKeeper() { memset((void*)this, 0, 0); }
ScoreKeeper::~ScoreKeeper() {}
void ScoreKeeper::reset(Int playerIdx) { Rec("score.reset %d", playerIdx); }
void ScoreKeeper::addObjectBuilt(const Object* o) { Rec("score.built %d", o ? (int)o->m_id : 0); }
void ScoreKeeper::removeObjectBuilt(const Object* o) { Rec("score.unbuilt %d", o ? (int)o->m_id : 0); }
void ScoreKeeper::crc(Xfer*) {}
/* the score keeper is another unit (differential_score_keeper.rs): this stand-in saves a version and the two sums, as the Rust stand-in does */
void ScoreKeeper::xfer(Xfer* xfer)
{
	XferVersion currentVersion = 1; XferVersion version = currentVersion; xfer->xferVersion(&version, currentVersion);
	xfer->xferInt(&m_totalMoneySpent); xfer->xferInt(&m_totalMoneyEarned);
}
void ScoreKeeper::loadPostProcess() {}
AcademyStats::AcademyStats() {}
void AcademyStats::init(const Player* p) { Rec("academy.init %d", pidx(p)); m_generalsPointsSpent = 0; m_controlGroupsUsed = 0; }
void AcademyStats::update() { Rec("academy.update"); }
void AcademyStats::recordIncome() { Rec("academy.income"); }
void AcademyStats::crc(Xfer*) {}
void AcademyStats::xfer(Xfer*) {}
void AcademyStats::loadPostProcess() {}
void MissionStats::crc(Xfer*) {}
void MissionStats::xfer(Xfer*) {}
void MissionStats::loadPostProcess() {}

/* the science store: the answers come from the program (a table of costs, grantable flags and prerequisites) */
struct SciCfg { int cost; bool grantable; std::vector<int> prereqs; std::string name; };
static std::map<int, SciCfg> g_sciCfg;
/* the eight sciences of a program are name keys ("Sci0" ... "Sci7"), as ScienceStore makes them; -1 stays -1 */
static int g_sciKey[8];
static int sciKey(int i) { return (i >= 0 && i < 8) ? g_sciKey[i] : i; }
ScienceStore::~ScienceStore() {}
void ScienceStore::init() {}
void ScienceStore::reset() {}
Bool ScienceStore::isScienceGrantable(ScienceType st) const { std::map<int, SciCfg>::const_iterator it = g_sciCfg.find((int)st); return it != g_sciCfg.end() && it->second.grantable; }
Int ScienceStore::getSciencePurchaseCost(ScienceType st) const { std::map<int, SciCfg>::const_iterator it = g_sciCfg.find((int)st); return it == g_sciCfg.end() ? 0 : it->second.cost; }
Bool ScienceStore::playerHasPrereqsForScience(const Player* player, ScienceType st) const
{
	std::map<int, SciCfg>::const_iterator it = g_sciCfg.find((int)st);
	if (it == g_sciCfg.end()) return false;
	for (size_t i = 0; i < it->second.prereqs.size(); ++i) if (!player->hasScience((ScienceType)it->second.prereqs[i])) return false;
	return true;
}

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
static std::vector<Player*> g_allPlayers;
static std::vector<Object*> g_objs;
static std::vector<Waypoint*> g_ways;
static std::vector<Script*> g_scripts;
static std::vector<PlayerTemplate*> g_ptpls;
static std::vector<SpecialPowerTemplate*> g_sptpls;
static std::vector<RankInfo*> g_ranks;

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

/* ---- the selectors: the alive prototypes and teams, found as the Rust finds them (through the lists) ------------- */
static std::vector<TeamPrototype*> aliveProtos()
{
	std::vector<TeamPrototype*> v;
	for (size_t i = 0; i < g_players.size(); ++i)
		for (Player::PlayerTeamList::const_iterator it = g_players[i]->m_playerTeamPrototypes.begin(); it != g_players[i]->m_playerTeamPrototypes.end(); ++it)
			if (std::find(v.begin(), v.end(), *it) == v.end()) v.push_back(*it);
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
static Player* pickPlayer(unsigned k) { return g_players[k % g_players.size()]; }

/* ---- the dump --------------------------------------------------------------------------------------------------- */
static std::string idList(const std::vector<int>& v)
{
	std::string s; char b[32];
	for (size_t i = 0; i < v.size(); ++i) { sprintf(b, "%s%d", i ? "," : "", v[i]); s += b; }
	return s.empty() ? "-" : s;
}
template <class M> static std::string relMapStr(const M& m)
{
	std::map<int, int> o;
	for (typename M::const_iterator it = m.begin(); it != m.end(); ++it) o[(int)it->first] = (int)it->second;
	std::string s; char b[48];
	for (std::map<int, int>::const_iterator it = o.begin(); it != o.end(); ++it) { sprintf(b, "%s%d:%d", s.empty() ? "" : ",", it->first, it->second); s += b; }
	return s.empty() ? "-" : s;
}
static std::string uniStr(const UnicodeString& u)
{
	std::string s; const WideChar* c = u.str();
	for (; c && *c; ++c) s += (*c < 128) ? (char)*c : '?';
	return s.empty() ? "-" : s;
}
static std::string ascStr(const AsciiString& a) { return a.isEmpty() ? "-" : a.str(); }
template <class V> static std::string sciStr(const V& v)
{
	std::string s; char b[24];
	for (size_t i = 0; i < v.size(); ++i) { sprintf(b, "%s%d", i ? "," : "", (int)v[i]); s += b; }
	return s.empty() ? "-" : s;
}
static std::string pmapStr(const ProductionChangeMap& m)
{
	std::string s; char b[48];
	for (ProductionChangeMap::const_iterator it = m.begin(); it != m.end(); ++it) { sprintf(b, "%s%u:%08x", s.empty() ? "" : ",", (unsigned)it->first, FB(it->second)); s += b; }
	return s.empty() ? "-" : s;
}
static std::string vmapStr(const ProductionVeterancyMap& m)
{
	std::string s; char b[48];
	for (ProductionVeterancyMap::const_iterator it = m.begin(); it != m.end(); ++it) { sprintf(b, "%s%u:%d", s.empty() ? "" : ",", (unsigned)it->first, (int)it->second); s += b; }
	return s.empty() ? "-" : s;
}
static std::string kindBits(const KindOfMaskType& m)
{
	std::string s; char b[16];
	for (int k = 0; k < KINDOF_COUNT; ++k) if (m.test(k)) { sprintf(b, "%s%d", s.empty() ? "" : "+", k); s += b; }
	return s.empty() ? "-" : s;
}

typedef Player::PlayerTeamList PlayerTeamList;
static int tplIndexOf(const PlayerTemplate* t) { for (size_t i = 0; i < g_ptpls.size(); ++i) if (g_ptpls[i] == t) return (int)i; return -1; }
static void dumpPlayer(Player* p)
{
	char b[256];
	std::string build;
	for (BuildListInfo* e = p->m_pBuildList; e; e = e->m_nextBuildList)
	{
		sprintf(b, "%s%d/%s/%08x,%08x,%08x/%08x/%d/%d", build.empty() ? "" : ";", (int)e->m_objectID, ascStr(e->m_templateName).c_str(), FB(e->m_location.x), FB(e->m_location.y), FB(e->m_location.z), FB(e->m_angle), e->m_numRebuilds, e->m_priorityBuild ? 1 : 0);
		build += b;
		{ char c[512]; std::string gat; char g[16];
		  for (int j = 0; j < BuildListInfo::MAX_RESOURCE_GATHERERS; ++j) { sprintf(g, "%s%d", j ? "," : "", (int)e->m_resourceGatherers[j]); gat += g; }
		  sprintf(c, "/%s/%s/%08x/%08x/%d/%d%d%d%d%d%d%d/%u/%d,%d/%s", ascStr(e->m_buildingName).c_str(), ascStr(e->m_script).c_str(), FB(e->m_rallyPointOffset.x), FB(e->m_rallyPointOffset.y), (int)e->m_health, e->m_whiner ? 1 : 0, e->m_unsellable ? 1 : 0, e->m_repairable ? 1 : 0, e->m_automaticallyBuild ? 1 : 0, e->m_isInitiallyBuilt ? 1 : 0, e->m_underConstruction ? 1 : 0, e->m_isSupplyBuilding ? 1 : 0, (unsigned)e->m_objectTimestamp, (int)e->m_desiredGatherers, (int)e->m_currentGatherers, gat.c_str());
		  build += c; }
	}
	if (build.empty()) build = "-";
	std::vector<int> protos;
	for (PlayerTeamList::const_iterator it = p->m_playerTeamPrototypes.begin(); it != p->m_playerTeamPrototypes.end(); ++it) protos.push_back((int)(*it)->getID());
	std::string att; for (int k = 0; k < MAX_PLAYER_COUNT; ++k) att += p->m_attackedBy[k] ? '1' : '0';
	std::string kop; for (Player::KindOfPercentProductionChangeListIt it = p->m_kindOfPercentProductionChangeList.begin(); it != p->m_kindOfPercentProductionChangeList.end(); ++it)
	{ sprintf(b, "%s%s/%08x/%u", kop.empty() ? "" : ";", kindBits((*it)->m_kindOf).c_str(), FB((*it)->m_percent), (unsigned)(*it)->m_ref); kop += b; }
	if (kop.empty()) kop = "-";
	std::string spt; for (Player::SpecialPowerReadyTimerList::const_iterator it = p->m_specialPowerReadyTimerList.begin(); it != p->m_specialPowerReadyTimerList.end(); ++it)
	{ sprintf(b, "%s%u/%u", spt.empty() ? "" : ";", (unsigned)it->m_templateID, (unsigned)it->m_readyFrame); spt += b; }
	if (spt.empty()) spt = "-";
	int nsq = 0; for (int k = 0; k < NUM_HOTKEY_SQUADS; ++k) if (p->m_squads[k]) ++nsq;
	char ai[48]; if (p->m_ai) sprintf(ai, "%d/%d", p->m_ai->m_skirmish ? 1 : 0, (int)p->m_ai->m_diff); else strcpy(ai, "-");
	Rec("D player %d tpl=%d name=%s key=%d disp=%s side=%s base=%s type=%d money=%u/%d hc=%08x,%08x,%08x,%08x radar=%d,%d,%d plans=%d,%d,%d bonus=%d up=%d energy=%d,%d,%u,%d build=%s color=%08x,%08x pcost=%s ptime=%s pvet=%s ai=%s mp=%d rgm=%d tun=%d def=%d",
		p->m_playerIndex, tplIndexOf(p->m_playerTemplate), ascStr(p->m_playerName).c_str(), (int)p->m_playerNameKey, uniStr(p->m_playerDisplayName).c_str(),
		ascStr(p->m_side).c_str(), ascStr(p->m_baseSide).c_str(), (int)p->m_playerType, (unsigned)p->m_money.countMoney(), p->m_money.m_playerIndex,
		FB(p->m_handicap.m_handicaps[0][0]), FB(p->m_handicap.m_handicaps[0][1]), FB(p->m_handicap.m_handicaps[1][0]), FB(p->m_handicap.m_handicaps[1][1]),
		p->m_radarCount, p->m_disableProofRadarCount, p->m_radarDisabled ? 1 : 0, p->m_bombardBattlePlans, p->m_holdTheLineBattlePlans, p->m_searchAndDestroyBattlePlans,
		p->m_battlePlanBonuses ? 1 : 0, p->m_upgradeList ? 1 : 0,
		p->m_energy.getProduction(), p->m_energy.getConsumption(), (unsigned)p->m_energy.getPowerSabotagedTillFrame(), p->m_energy.m_owner ? p->m_energy.m_owner->m_playerIndex : -1,
		build.c_str(), (unsigned)p->m_color, (unsigned)p->m_nightColor, pmapStr(p->m_productionCostChanges).c_str(), pmapStr(p->m_productionTimeChanges).c_str(), vmapStr(p->m_productionVeterancyLevels).c_str(),
		ai, p->m_mpStartIndex, p->m_resourceGatheringManager ? 1 : 0, p->m_tunnelSystem ? 1 : 0, tid(p->m_defaultTeam));
	Rec("D player2 %d sci=%s dis=%s hid=%s rank=%d,%d,%d,%d,%d gen=%s protos=%s prel=%s trel=%s flags=%d%d%d%d%d%d%d%d mod=%08x att=%s attf=%u bounty=%08x kop=%s spt=%s squads=%d,%d gpts=%u",
		p->m_playerIndex, sciStr(p->m_sciences).c_str(), sciStr(p->m_sciencesDisabled).c_str(), sciStr(p->m_sciencesHidden).c_str(), p->m_rankLevel, p->m_skillPoints, p->m_sciencePurchasePoints, p->m_levelUp, p->m_levelDown,
		uniStr(p->m_generalName).c_str(), idList(protos).c_str(), relMapStr(p->m_playerRelations->m_map).c_str(), relMapStr(p->m_teamRelations->m_map).c_str(),
		p->m_canBuildUnits ? 1 : 0, p->m_canBuildBase ? 1 : 0, p->m_observer ? 1 : 0, p->m_isPreorder ? 1 : 0, p->m_listInScoreScreen ? 1 : 0, p->m_unitsShouldHunt ? 1 : 0, p->m_isPlayerDead ? 1 : 0, p->m_logicalRetaliationModeEnabled ? 1 : 0,
		FB(p->m_skillPointsModifier), att.c_str(), (unsigned)p->m_attackedFrame, FB(p->m_cashBountyPercent), kop.c_str(), spt.c_str(), nsq, p->m_currentSelection ? 1 : 0, (unsigned)p->m_academyStats.m_generalsPointsSpent);
}

static void dumpPlayer3(Player* p)
{
	const BattlePlanBonuses* b = p->m_battlePlanBonuses;
	if (b) Rec("D player3 %d spent=%d earned=%d bonus=%08x,%08x,%d,%d,%d,%s,%s", p->m_playerIndex, p->m_scoreKeeper.m_totalMoneySpent, p->m_scoreKeeper.m_totalMoneyEarned,
		FB(b->m_armorScalar), FB(b->m_sightRangeScalar), b->m_bombardment, b->m_holdTheLine, b->m_searchAndDestroy, kindBits(b->m_validKindOf).c_str(), kindBits(b->m_invalidKindOf).c_str());
	else Rec("D player3 %d spent=%d earned=%d bonus=-", p->m_playerIndex, p->m_scoreKeeper.m_totalMoneySpent, p->m_scoreKeeper.m_totalMoneyEarned);
}

static void dumpPlayer4(Player* p);
static void dumpState()
{
	Rec("D frame=%u tid=%u pid=%u nprotos=%u local=%d", (unsigned)TheGameLogic->m_frame, (unsigned)TheTeamFactory->m_uniqueTeamID, (unsigned)TheTeamFactory->m_uniqueTeamPrototypeID,
		(unsigned)TheTeamFactory->m_prototypes.size(), pidx(ThePlayerList->m_local));
	std::vector<TeamPrototype*> ps = aliveProtos();
	for (size_t i = 0; i < ps.size(); ++i)
	{
		TeamPrototype* p = ps[i];
		std::vector<int> insts;
		for (Team* t = p->getFirstItemIn_TeamInstanceList(); t; t = t->dlink_next_TeamInstanceList()) insts.push_back((int)t->getID());
		Rec("D proto id=%u name=%s owner=%d insts=%s", (unsigned)p->getID(), p->m_name.str(), p->m_owningPlayer ? p->m_owningPlayer->m_playerIndex : -1, idList(insts).c_str());
	}
	std::vector<Team*> ts = aliveTeams();
	for (size_t i = 0; i < ts.size(); ++i)
	{
		Team* t = ts[i];
		std::vector<int> mem;
		for (Object* o = t->getFirstItemIn_TeamMemberList(); o; o = o->dlink_next_TeamMemberList()) mem.push_back((int)o->m_id);
		Rec("D team id=%u proto=%u members=%s act=%d", (unsigned)t->getID(), (unsigned)t->m_proto->getID(), idList(mem).c_str(), t->m_active ? 1 : 0);
	}
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		Object* o = g_objs[i];
		Rec("D obj id=%d team=%d next=%d prev=%d", (int)o->m_id, tid(o->m_team), o->m_dlink_TeamMemberList.m_next ? (int)o->m_dlink_TeamMemberList.m_next->m_id : 0,
			o->m_dlink_TeamMemberList.m_prev ? (int)o->m_dlink_TeamMemberList.m_prev->m_id : 0);
	}
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		Object* o = g_objs[i];
		Rec("D obj2 %d health=%08x vision=%08x shroud=%08x wb=%08x", (int)o->m_id, FB(o->m_health), FB(o->m_vision), FB(o->m_shroud), o->m_wb);
	}
	for (size_t i = 0; i < g_players.size(); ++i) { dumpPlayer(g_players[i]); dumpPlayer4(g_players[i]); dumpPlayer3(g_players[i]); }
}

/* ---- slice 2b and 3: upgrades, squads, save and load ------------------------------------------------------------- */
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
/* a word that picks an object, or (a quarter of the time) an id no object has */
static ObjectID objIdOf(unsigned w) { return (w % 4 == 0) ? (ObjectID)(900 + (w / 4) % 7) : g_objs[(w / 4) % g_objs.size()]->m_id; }
static Object* realObj(unsigned w) { return g_objs[w % g_objs.size()]; }
static AIGroup* groupOf(unsigned w) { return (w % 4 == 0) ? NULL : &g_groups[w % 3]; }
static UpgradeMaskType upgradeMaskOf(const unsigned* a, int k)
{
	size_t n = g_upgrades.size();
	UpgradeMaskType m;
	if (a[k] % 3 != 0) m.set(g_upgrades[a[k + 1] % n]->getUpgradeMask());
	if (a[k] % 3 == 2) m.set(g_upgrades[a[k + 2] % n]->getUpgradeMask());
	return m;
}
static int upgradeIndex(const UpgradeTemplate* t) { for (size_t i = 0; i < g_upgrades.size(); ++i) if (g_upgrades[i] == t) return (int)i; return -1; }
static int listIndex(Player* p, Upgrade* u) { int i = 0; for (Upgrade* x = p->m_upgradeList; x; x = x->friend_getNext(), ++i) if (x == u) return i; return -1; }
static std::string maskStr(const UpgradeMaskType& m)
{
	std::string s; char b[16];
	for (int i = 0; i < 128; ++i) if (m.test(i)) { sprintf(b, "%s%d", s.empty() ? "" : ",", i); s += b; }
	return s.empty() ? "-" : s;
}
static std::string squadStr(Squad* q)
{
	std::vector<int> ids, cache;
	for (size_t i = 0; i < q->m_objectIDs.size(); ++i) ids.push_back((int)q->m_objectIDs[i]);
	for (size_t i = 0; i < q->m_objectsCached.size(); ++i) cache.push_back((int)q->m_objectsCached[i]->m_id);
	return idList(ids) + "/" + idList(cache);
}
static std::string idsOf(const VecObjectPtr& v)
{
	std::vector<int> ids;
	for (size_t i = 0; i < v.size(); ++i) ids.push_back((int)v[i]->m_id);
	return idList(ids);
}
static void dumpPlayer4(Player* p)
{
	std::string ups, squads;
	for (Upgrade* u = p->m_upgradeList; u; u = u->friend_getNext())
	{ char b[32]; sprintf(b, "%s%d:%d", ups.empty() ? "" : ";", upgradeIndex(u->getTemplate()), (int)u->getStatus()); ups += b; }
	if (ups.empty()) ups = "-";
	for (int k = 0; k < NUM_HOTKEY_SQUADS; ++k) if (p->m_squads[k]) squads += (squads.empty() ? "" : "|") + squadStr(p->m_squads[k]);
	if (squads.empty()) squads = "-";
	Rec("D player4 %d gused=%u up=%s inprog=%s comp=%s squads=%s sel=%s", p->m_playerIndex, (unsigned)p->m_academyStats.m_controlGroupsUsed, ups.c_str(), maskStr(p->m_upgradesInProgress).c_str(),
		maskStr(p->m_upgradesCompleted).c_str(), squads.c_str(), p->m_currentSelection ? squadStr(p->m_currentSelection).c_str() : "-");
}

/* ---- the kinds and statuses a program can name (index -> real enumerator) ---------------------------------------- */
static const KindOfType k_kindPool[15] = { KINDOF_STRUCTURE, KINDOF_PROJECTILE, KINDOF_MINE, KINDOF_INERT, KINDOF_TECH_BUILDING, KINDOF_AIRCRAFT, KINDOF_INFANTRY, KINDOF_VEHICLE,
	KINDOF_POWERED, KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_COMMANDCENTER, KINDOF_DISGUISER, KINDOF_FS_POWER, KINDOF_IGNORES_SELECT_ALL };
static const ObjectStatusTypes k_statusPool[5] = { OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_SOLD };
static KindOfMaskType kindMask(unsigned bits)
{
	KindOfMaskType m;
	for (int k = 0; k < 15; ++k) if (bits & (1u << k)) m.set(k_kindPool[k]);
	return m;
}

/* ---- slice 2: the object-facing services (each call that has an effect is a line, in the Rust host's words) ---- */
void Object::friend_adjustPowerForPlayer(Bool yes) { Rec("adjustpower %d %d", (int)m_id, yes ? 1 : 0); }
void AutoDepositUpdate::awardInitialCaptureBonus(Player* p) { Rec("capturebonus %d %d", (int)m_owner->m_id, pidx(p)); }
void InGameUI::addIdleWorker(Object* obj) { Rec("ui.idle+ %d", (int)obj->m_id); }
void InGameUI::removeIdleWorker(Object* obj, Int playerNumber) { Rec("ui.idle- %d %d", (int)obj->m_id, playerNumber); }
void InGameUI::addFloatingText(const UnicodeString& text, const Coord3D* pos, Color color) { Rec("ui.float %s %08x,%08x,%08x %08x", uniStr(text).c_str(), FB(pos->x), FB(pos->y), FB(pos->z), (unsigned)color); }
void Drawable::setIndicatorColor(Color c) { Rec("indicator %d %08x", (int)m_owner->m_id, (unsigned)c); }
void Object::leaveGroup() { Rec("leavegroup %d", (int)m_id); }
void Object::setVisionSpied(Bool setting, Int byWhom) { Rec("spied %d %d %d", (int)m_id, setting ? 1 : 0, byWhom); }
void Object::updateUpgradeModules() { Rec("upgrademods %d", (int)m_id); }
void Object::setWeaponBonusCondition(WeaponBonusConditionType c) { Rec("wb+ %d %d", (int)m_id, (int)c); m_wb |= 1u << (int)c; }
void Object::clearWeaponBonusCondition(WeaponBonusConditionType c) { Rec("wb- %d %d", (int)m_id, (int)c); m_wb &= ~(1u << (int)c); }
Real BodyModuleInterface::getMaxHealth() const { return m_owner->m_health; }
void BodyModuleInterface::setMaxHealth(Real maxHealth, MaxHealthChangeType) { Rec("health %d %08x", (int)m_owner->m_id, FB(maxHealth)); m_owner->m_health = maxHealth; }
void BodyModuleInterface::applyDamageScalar(Real scalar) { Rec("armor %d %08x", (int)m_owner->m_id, FB(scalar)); }
void SupplyTruckAIInterface::setForceWantingState(Bool v) { Rec("ai.force %d %d", (int)m_owner->m_id, v ? 1 : 0); }
SupplyTruckAIInterface* AIUpdateInterface::getSupplyTruckAIInterface() const { return m_owner->m_supplyTruck ? &m_owner->m_truck : NULL; }
void AIUpdateInterface::aiHunt(CommandSourceType source) { Rec("ai.hunt %d %d", (int)m_owner->m_id, (int)source); }
void AIUpdateInterface::aiIdle(CommandSourceType source) { Rec("ai.idle %d %d", (int)m_owner->m_id, (int)source); }
void AIUpdateInterface::aiEnter(Object* obj, CommandSourceType source) { Rec("ai.enter %d %d %d", (int)m_owner->m_id, (int)obj->m_id, (int)source); }
void AIUpdateInterface::aiEvacuate(Bool exposeStealthUnits, CommandSourceType source) { Rec("ai.evacuate %d %d %d", (int)m_owner->m_id, exposeStealthUnits ? 1 : 0, (int)source); }
void AIUpdateInterface::aiMoveToPosition(const Coord3D* pos, CommandSourceType source) { Rec("ai.move %d %08x,%08x,%08x %d", (int)m_owner->m_id, FB(pos->x), FB(pos->y), FB(pos->z), (int)source); }
void Pathfinder::removeObjectFromPathfindMap(Object* obj) { Rec("path.remove %d", (int)obj->m_id); }
void Pathfinder::addObjectToPathfindMap(Object* obj) { Rec("path.add %d", (int)obj->m_id); }
void PartitionManager::getMostValuableLocation(Int playerIndex, unsigned relationship, ValueOrThreat vot, Coord3D* pos) { Rec("partition.valuable %d", playerIndex); }
ObjectIterator* PartitionManager::iterateAllObjects()
{
	ObjectIterator* it = new ObjectIterator;
	it->m_objs = g_objs;
	return it;
}
ObjectIterator* PartitionManager::iterateObjectsInRange(const Coord3D* pos, Real maxDist, DistanceCalculationType dc, PartitionFilter** filters, IterOrderType order)
{
	/* the one use (Player::garrisonAllUnits): the structures, nearest first, from the centre in 3D; ties keep the object order */
	ObjectIterator* it = new ObjectIterator;
	std::vector<std::pair<Real, Object*> > v;
	PartitionFilterAcceptByKindOf* f = (PartitionFilterAcceptByKindOf*)filters[0];
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		Object* o = g_objs[i];
		if (!o->isKindOfMulti(f->m_set, f->m_clear)) continue;
		Real dx = o->m_pos.x - pos->x, dy = o->m_pos.y - pos->y, dz = o->m_pos.z - pos->z;
		v.push_back(std::make_pair(dx * dx + dy * dy + dz * dz, o));
	}
	std::stable_sort(v.begin(), v.end(), [](const std::pair<Real, Object*>& a, const std::pair<Real, Object*>& b) { return a.first < b.first; });
	for (size_t i = 0; i < v.size(); ++i) it->m_objs.push_back(v[i].second);
	return it;
}
void ContainModuleInterface::recalcApparentControllingPlayer() { Rec("recalc %d", (int)m_owner->m_id); }
Bool ActionManager::canEnterObject(const Object* obj, const Object* objectToEnter, CommandSourceType commandSource, CanEnterType modifierFlags) const
{
	Rec("canenter %d %d %d", (int)obj->m_id, (int)objectToEnter->m_id, (int)commandSource);
	return (obj->m_canEnter >> ((int)objectToEnter->m_id & 31)) & 1;
}
void BuildAssistant::sellObject(Object* obj) { Rec("sell %d", (int)obj->m_id); }
void Radar::removeObject(Object* obj) { Rec("radar- %d", (int)obj->m_id); }
void Radar::addObject(Object* obj) { Rec("radar+ %d", (int)obj->m_id); }
void GameClient::setTeamColor(Int red, Int green, Int blue) { Rec("teamcolor %d %d %d", red, green, blue); }
void ControlBar::setControlBarSchemeByPlayer(Player* p) { Rec("ui.scheme.player %d", pidx(p)); }
void ControlBar::setControlBarSchemeByPlayerTemplate(const PlayerTemplate* pt) { Rec("ui.scheme.template %d", tplIndexOf(pt)); }
const PlayerTemplate* PlayerTemplateStore::findPlayerTemplate(NameKeyType key) const { return key == TheNameKeyGenerator->nameToKey("FactionObserver") ? m_observer : NULL; }
ProductionUpdateInterface* ProductionUpdate::getProductionUpdateInterfaceFromObject(const Object* obj)
{
	if (obj->m_prodQueue < 0) return NULL;
	Object* o = const_cast<Object*>(obj);
	o->m_pui.m_queued = o->m_prodQueue;
	return &o->m_pui;
}
const ProductionPrerequisite* ThingTemplate::getNthPrereq(Int i) const
{
	static ProductionPrerequisite pp;
	pp.m_ok = (m_prereqMask >> (i & 31)) & 1;
	return &pp;
}
static BuildAssistant g_buildAssistant; BuildAssistant* TheBuildAssistant = &g_buildAssistant;
static Radar g_radar; Radar* TheRadar = &g_radar;
static GameClient g_gameClient; GameClient* TheGameClient = &g_gameClient;
static PlayerTemplateStore g_ptStore; PlayerTemplateStore* ThePlayerTemplateStore = &g_ptStore;
static ActionManager g_actionManager; ActionManager* TheActionManager = &g_actionManager;

static void iterCb(Object* o, void* ud) { Rec("iter %d", (int)o->m_id); }

/* ---- the world of a program --------------------------------------------------------------------------------------- */
static void clearWorld()
{
	g_lines = NULL;
	for (size_t i = 0; i < g_allPlayers.size(); ++i) g_allPlayers[i]->m_defaultTeam = NULL;
	if (TheTeamFactory) { try { delete TheTeamFactory; } catch (...) {} TheTeamFactory = NULL; }
	for (size_t i = 0; i < g_objs.size(); ++i) { g_objs[i]->m_team = NULL; }
	for (size_t i = 0; i < g_objs.size(); ++i) delete g_objs[i];
	g_objs.clear();
	g_playerList.m_count = 0;
	g_playerList.m_local = NULL;
	{ UpgradeTemplate* u = g_upgradeCenter.m_upgradeList; while (u) { UpgradeTemplate* nx = u->friend_getNext(); u->deleteInstance(); u = nx; } g_upgradeCenter.m_upgradeList = NULL; g_upgradeCenter.m_nextTemplateMaskBit = 0; }
	g_upgrades.clear();
	for (size_t i = 0; i < g_allPlayers.size(); ++i)
	{
		/* the player is not deleted through ~Player (slice 4): its AI is */
		if (g_allPlayers[i]->m_ai) { delete g_allPlayers[i]->m_ai; g_allPlayers[i]->m_ai = NULL; }
	}
	g_players.clear();
	g_allPlayers.clear();
	for (size_t i = 0; i < g_tpls.size(); ++i) delete g_tpls[i];
	g_tpls.clear();
	for (size_t i = 0; i < g_ptpls.size(); ++i) delete g_ptpls[i];
	g_ptpls.clear();
	for (size_t i = 0; i < g_sptpls.size(); ++i) delete g_sptpls[i];
	g_sptpls.clear();
	for (size_t i = 0; i < g_ranks.size(); ++i) delete g_ranks[i];
	g_ranks.clear();
	g_rankStore.m_ranks.clear();
	g_sciCfg.clear();
	for (size_t i = 0; i < g_ways.size(); ++i) delete g_ways[i];
	g_ways.clear();
	for (size_t i = 0; i < g_scriptEngine.m_scripts.size(); ++i) delete g_scriptEngine.m_scripts[i];
	g_scriptEngine.m_scripts.clear();
	g_dicts.clear();
	g_thingFactory.m_templates.clear();
	g_gameLogic.m_objs.clear();
	g_terrain.m_ways.clear();
	g_playerList.m_byKey.clear();
	TheGameInfo = NULL;
}

static void resetKeys()
{
	/* the well-known keys cache their key in the StaticNameKey; a fresh generator needs them looked up afresh */
#define X(n) TheKey_##n.m_key = NAMEKEY_INVALID;
#include "well_known_keys_list.h"
#undef X
}

/* the AI configuration of a player, read before the player is made (the AI is made later) */
struct AiCfg { bool safe, attacked, bridges, hasBase, superOK, construct; Coord3D base, at; int enemy; int diff; };
static std::vector<AiCfg> g_aiCfg;
static void applyAiCfg(Player* p)
{
	if (!p->m_ai) return;
	const AiCfg& c = g_aiCfg[p->m_playerIndex];
	p->m_ai->m_supplySafe = c.safe; p->m_ai->m_supplyAttacked = c.attacked; p->m_ai->m_checkBridges = c.bridges; p->m_ai->m_hasBase = c.hasBase; p->m_ai->m_base = c.base;
	p->m_ai->m_superOK = c.superOK; p->m_ai->m_construct = c.construct; p->m_ai->m_constructAt = c.at; p->m_ai->m_enemy = c.enemy >= 0 ? g_players[c.enemy % g_players.size()] : NULL; p->m_ai->m_diff = (GameDifficulty)c.diff;
}

static void readMoney(Reader& r, Money& m)
{
	unsigned amount = r.u(); int idx = r.i();
	m.init(); m.m_money = amount; m.setPlayerIndex(idx);
}

static void setupWorld2(Reader& r);
static void setupWorld(Reader& r)
{
	/* ---- the dictionaries (the handicaps of the player templates and the teams are read from them) ---- */
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
	/* ---- the services ---- */
	g_globalData.m_clientRetaliationModeEnabled = r.b(); g_globalData.m_baseValuePerSupplyBox = r.u();
	for (int a = 0; a < PLAYERTYPE_COUNT; ++a) for (int d = 0; d < DIFFICULTY_COUNT; ++d) g_globalData.m_soloPlayerHealthBonusForDifficulty[a][d] = r.f();
	readMoney(r, g_globalData.m_defaultStartingCash);
	g_globalData.m_timeOfDay = r.b() ? TIME_OF_DAY_NIGHT : TIME_OF_DAY_MORNING;
	if (r.b()) { readMoney(r, g_gameInfo.m_cash); TheGameInfo = &g_gameInfo; } else { Money skip; readMoney(r, skip); TheGameInfo = NULL; }
	g_ai.m_data.m_forceSkirmishAI = r.b();
	g_gameLogic.m_singlePlayer = r.b(); g_gameLogic.m_shellGame = r.b(); g_gameLogic.m_inUpdate = r.b(); g_gameLogic.m_rankLimit = r.i();
	g_inGameUI.m_quiet = r.b();
	g_scriptEngine.m_globalDifficulty = (GameDifficulty)(r.u() % 3);
	for (int k = 0; k < 16; ++k) g_victory.m_defeated[k] = r.b();
	int nr = r.i();
	for (int i = 0; i < nr; ++i)
	{
		RankInfo* ri = new RankInfo; ri->m_skillPointsNeeded = r.i(); ri->m_sciencePurchasePointsGranted = r.i();
		int ng = r.i(); for (int k = 0; k < ng; ++k) ri->m_sciencesGranted.push_back((ScienceType)sciKey(r.i()));
		g_ranks.push_back(ri); g_rankStore.m_ranks.push_back(ri);
	}
	int nsci = r.i();
	for (int i = 0; i < nsci; ++i)
	{
		int id = sciKey(r.i()); SciCfg c; c.cost = r.i(); c.grantable = r.b(); int np2 = r.i(); for (int k = 0; k < np2; ++k) c.prereqs.push_back(sciKey(r.i()));
		g_sciCfg[id] = c;
	}
	static ScienceStore sciStore;
	TheScienceStore = &sciStore;
	int nup = r.i();
	for (int i = 0; i < nup; ++i) { char b[16]; sprintf(b, "Upg%d", i); g_upgrades.push_back(TheUpgradeCenter->newUpgrade(AsciiString(b))); }
	int nsp = r.i();
	for (int i = 0; i < nsp; ++i) { SpecialPowerTemplate* t = new SpecialPowerTemplate; t->m_id = r.u(); t->m_reload = r.u(); t->m_shared = r.b(); g_sptpls.push_back(t); }
	int npt = r.i();
	for (int i = 0; i < npt; ++i)
	{
		PlayerTemplate* t = new PlayerTemplate;
		char b[40];
		sprintf(b, "Tpl%d", r.i()); t->m_beacon = AsciiString(b);
		sprintf(b, "Side%d", r.i()); t->m_side = AsciiString(b);
		sprintf(b, "Base%d", r.i()); t->m_baseSide = AsciiString(b);
		t->m_color.m_asInt = r.i();
		readMoney(r, t->m_money);
		/* the handicap: read from a dictionary of the program, as initFromDict does */
		{
			unsigned di = r.u();
			t->m_handicap.init();
			if (!g_dicts.empty()) { Dict d; buildDict(g_dicts[di % g_dicts.size()], d); t->m_handicap.readFromDict(&d); }
		}
		int nc = r.i(); for (int k = 0; k < nc; ++k) { sprintf(b, "Tpl%d", r.i()); NameKeyType key = TheNameKeyGenerator->nameToKey(b); t->m_cost[key] = r.f(); }
		nc = r.i(); for (int k = 0; k < nc; ++k) { sprintf(b, "Tpl%d", r.i()); NameKeyType key = TheNameKeyGenerator->nameToKey(b); t->m_time[key] = r.f(); }
		nc = r.i(); for (int k = 0; k < nc; ++k) { sprintf(b, "Tpl%d", r.i()); NameKeyType key = TheNameKeyGenerator->nameToKey(b); t->m_vet[key] = (VeterancyLevel)(r.u() % 4); }
		t->m_observer = r.b(); t->m_playable = r.b();
		nc = r.i(); for (int k = 0; k < nc; ++k) t->m_intrinsic.push_back((ScienceType)sciKey(r.i()));
		t->m_intrinsicSPP = r.i();
		g_ptpls.push_back(t);
	}
	int np = r.i();
	for (int i = 0; i < np; ++i)
	{
		AiCfg c; c.safe = r.b(); c.attacked = r.b(); c.bridges = r.b(); c.hasBase = r.b(); c.base = r.c(); c.superOK = r.b(); c.construct = r.b(); c.at = r.c(); c.enemy = r.i(); c.diff = (int)(r.u() % 3);
		g_aiCfg.push_back(c);
	}
	g_playerList.m_count = 0;
	for (int i = 0; i < 16; ++i) g_playerList.m_players[i] = NULL;
	g_playerList.m_local = NULL;
	/* PlayerList::PlayerList makes all MAX_PLAYER_COUNT players; the program's world uses the first np of them */
	for (int i = 0; i < MAX_PLAYER_COUNT; ++i)
	{
		void* mem = calloc(1, sizeof(Player));
		Player* p = new (mem) Player(i);
		g_allPlayers.push_back(p);
		if (i < np) g_players.push_back(p);
		g_playerList.m_players[i] = p;
		char b[32]; sprintf(b, "Player%d", i);
		g_playerList.m_byKey[(unsigned)TheNameKeyGenerator->nameToKey(b)] = i;
	}
	g_playerList.m_count = np;
	g_playerList.m_local = g_players[0];

	int nt = r.i();
	for (int i = 0; i < nt; ++i)
	{
		ThingTemplate* t = new ThingTemplate;
		t->m_idx = i;
		int nk = r.i();
		for (int k = 0; k < nk; ++k) t->m_kindof.set(k_kindPool[r.u() % 15]);
		t->m_buildFacility = r.b(); t->m_equiv = r.i();
		char b[32]; sprintf(b, "Tpl%d", r.i());
		t->m_name = AsciiString(b);
		int nv = r.i();
		for (int k = 0; k < nv; ++k) { int v = r.i(); if (v >= 8) sprintf(b, "tpl%d", v - 8); else sprintf(b, "Tpl%d", v); t->m_variations.push_back(AsciiString(b)); }
		for (int k = 0; k < 4; ++k) t->m_skill[k] = r.i();
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
		for (int k = 0; k < ns; ++k) o->m_status.set(k_statusPool[r.u() % 5]);
		o->m_dead = r.b(); o->m_destroyed = r.b(); o->m_contained = r.b(); o->m_held = r.b(); o->m_enemyNear = r.b();
		o->m_hasAI = r.b(); o->m_ai.m_owner = o; o->m_ai.m_idle = r.b(); o->m_ai.m_recruitable = r.b(); o->m_ai.m_loco.m_valid = r.u();
		int cc = r.i(); o->m_hasContain = cc >= 0; o->m_contain.m_owner = o; o->m_contain.m_count = cc;
		o->m_pos = r.c(); o->m_vision = r.f();
		o->m_didEnter = r.u(); o->m_didExit = r.u(); o->m_inside = r.u(); o->m_reactions = r.u();
		o->m_orientation = r.f(); o->m_vet = (VeterancyLevel)(r.u() % 4);
		int nm = r.i() % 4;
		for (int k = 0; k < 4; ++k) o->m_modules[k] = NULL;
		for (int k = 0; k < nm; ++k)
		{
			bool sp = r.b(); int serial = r.i(); int sci = r.i();
			o->m_spm[k].m_serial = serial; o->m_spm[k].m_required = (ScienceType)sciKey(sci);
			o->m_mod[k].m_sp = sp ? &o->m_spm[k] : NULL;
			o->m_modules[k] = &o->m_mod[k];
		}
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

	g_gameLogic.m_frame = r.u();
	setupWorld2(r);
}

/* The special powers a program can name (index -> real enumerator); 8 is SPECIAL_INVALID */
static const SpecialPowerType k_spPool[9] = { SPECIAL_PARTICLE_UPLINK_CANNON, SUPW_SPECIAL_PARTICLE_UPLINK_CANNON, LAZR_SPECIAL_PARTICLE_UPLINK_CANNON, SPECIAL_NEUTRON_MISSILE,
	NUKE_SPECIAL_NEUTRON_MISSILE, SUPW_SPECIAL_NEUTRON_MISSILE, SPECIAL_SCUD_STORM, SPECIAL_DAISY_CUTTER, SPECIAL_INVALID };

/* The second part of the header (slice 2): what the object-facing operations read. It follows the frame, so the first
** part of the program is the same as slice 1's. */
static bool g_hasObserver = false;
static unsigned g_observerIdx = 0;
static void setupWorld2(Reader& r)
{
	g_hasObserver = r.b(); g_observerIdx = r.u();
	g_ptStore.m_observer = (g_hasObserver && !g_ptpls.empty()) ? g_ptpls[g_observerIdx % g_ptpls.size()] : NULL;
	for (size_t i = 0; i < g_tpls.size(); ++i)
	{
		ThingTemplate* t = g_tpls[i];
		t->m_buildCost = r.i(); t->m_buildable = (BuildableStatus)(r.u() % 4); t->m_nPrereq = (Int)(r.u() % 4); t->m_prereqMask = r.u(); t->m_maxSimul = r.u() % 4;
		int lk = r.i(); char b[32];
		if (lk == 0) t->m_linkKey = NAMEKEY_INVALID; else { sprintf(b, "Link%d", lk); t->m_linkKey = TheNameKeyGenerator->nameToKey(b); }
		sprintf(b, "Side%u", r.u() % 4); t->m_owningSide = AsciiString(b);
	}
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		Object* o = g_objs[i];
		o->m_modulesReady = r.b(); o->m_hasAutoDeposit = r.b(); o->m_hasDrawable = r.b(); o->m_disabled = r.b(); o->m_factionStructure = r.b(); o->m_hasWeapon = r.b();
		o->m_weaponPercent = r.u() % 130; o->m_selectable = r.b(); o->m_supplyTruck = r.b(); o->m_disguisedPlayer = r.i(); o->m_nightColor = r.u(); o->m_indicatorColor = r.u();
		o->m_producer = (ObjectID)r.u(); o->m_health = r.f(); o->m_shroud = r.f(); o->m_contain.m_whoEntered = r.u(); o->m_canEnter = r.u(); o->m_prodQueue = r.i();
		o->m_adu.m_owner = o; o->m_drawable.m_owner = o; o->m_body.m_owner = o; o->m_truck.m_owner = o; o->m_stealth.m_player = o->m_disguisedPlayer; o->m_wb = 0;
		for (int k = 0; k < 4; ++k)
		{
			if (!o->m_modules[k]) continue;
			SpecialPowerModuleInterface& m = o->m_spm[k];
			m.m_type = (int)k_spPool[r.u() % 9]; m.m_scriptOnly = r.b(); m.m_ready = r.u(); m.m_percent = r.u() % 130; m.m_tmpl.m_shared = r.b();
		}
	}
}

static AsciiString teamName(unsigned id) { char b[32]; if (id >= 100) sprintf(b, "teamP%u", id - 100); else sprintf(b, "Team%u", id); return AsciiString(b); }
static AsciiString thingName(unsigned v) { char b[32]; if (v >= 8) sprintf(b, "tpl%u", v - 8); else sprintf(b, "Tpl%u", v); return AsciiString(b); }
static std::string countsStr(const int* c, int n) { std::string s; char b[16]; for (int i = 0; i < n; ++i) { sprintf(b, "%s%d", i ? "," : "", c[i]); s += b; } return s; }
static Coord3D coordOf(unsigned a, unsigned b2, unsigned c) { Coord3D d; memcpy(&d.x, &a, 4); memcpy(&d.y, &b2, 4); memcpy(&d.z, &c, 4); return d; }
static ScienceType sciOf(unsigned v) { return (ScienceType)sciKey((int)(v % 8)); }

static void objOp(Object* o, unsigned sub, const unsigned* a)
{
	switch (sub)
	{
	case 0: o->m_dead = a[0] != 0; break;
	case 1: o->m_destroyed = a[0] != 0; break;
	case 2: o->m_status.set(k_statusPool[a[0] % 5], a[1] != 0 ? 1 : 0); break;
	case 3: o->m_enemyNear = a[0] != 0; break;
	case 4: memcpy(&o->m_pos.x, &a[0], 4); memcpy(&o->m_pos.y, &a[1], 4); memcpy(&o->m_pos.z, &a[2], 4); break;
	case 5: o->m_reactions = a[0]; break;
	case 6: memcpy(&o->m_orientation, &a[0], 4); break;
	case 7: o->m_vet = (VeterancyLevel)(a[0] % 4); break;
	case 8: o->m_held = a[0] != 0; break;
	case 9: o->m_disabled = a[0] != 0; break;
	case 10: o->m_modulesReady = a[0] != 0; break;
	case 11: for (int k = 0; k < 4; ++k) if (o->m_modules[(k + a[0]) % 4]) { SpecialPowerModuleInterface& m = o->m_spm[(k + a[0]) % 4]; m.m_ready = a[1]; m.m_percent = a[2] % 130; m.m_scriptOnly = a[3] != 0; break; } break;
	case 12: o->m_hasWeapon = a[0] != 0; o->m_weaponPercent = a[1] % 130; o->m_factionStructure = a[2] != 0; break;
	case 13: o->m_prodQueue = (int)a[0] % 4 - 1; o->m_canEnter = a[1]; o->m_contain.m_whoEntered = a[2] % 2 ? (1u << (a[3] % 4)) : 0; break;
	case 14: memcpy(&o->m_health, &a[0], 4); o->m_disguisedPlayer = (int)(a[1] % 5) - 1; o->m_stealth.m_player = o->m_disguisedPlayer; o->m_supplyTruck = a[2] != 0; break;
	default: break;
	}
}

static std::string ptrStr(const void* p) { return p ? "1" : "0"; }

/* one Player operation; `a` are its eight words */
static void playerOp(Player* p, unsigned sub, const unsigned* a)
{
	int np = (int)g_players.size();
	switch (sub)
	{
	case 0: p->update(); break;
	case 1: p->newMap(); break;
	case 2: p->setPlayerType((PlayerType)(a[0] % 2), a[1] != 0); applyAiCfg(p); break;
	case 3: p->setDefaultTeam(); break;
	case 4: { char b[16]; sprintf(b, "P%u", a[0] % 4); p->m_playerName = AsciiString(b); break; }
	case 5: { Team* t = a[0] ? pickTeam(a[0] - 1) : NULL; Rec("ret rel %d", (int)p->getRelationship(t)); break; }
	case 6: { Player* o = a[0] ? g_players[(a[0] - 1) % np] : NULL; p->setPlayerRelationship(o, (Relationship)(a[1] % 3)); break; }
	case 7: { Player* o = a[0] ? g_players[(a[0] - 1) % np] : NULL; Rec("ret rmprel %d", p->removePlayerRelationship(o) ? 1 : 0); break; }
	case 8: { Team* t = a[0] ? pickTeam(a[0] - 1) : NULL; p->setTeamRelationship(t, (Relationship)(a[1] % 3)); break; }
	case 9: { Team* t = a[0] ? pickTeam(a[0] - 1) : NULL; Rec("ret rmtrel %d", p->removeTeamRelationship(t) ? 1 : 0); break; }
	case 10: {
		BuildListInfo* head = NULL; int n = a[0] % 4;
		for (int k = n - 1; k >= 0; --k) { BuildListInfo* e = new BuildListInfo; e->m_objectID = (ObjectID)(a[1] + k); e->m_templateName = thingName(a[2] + k); e->m_angle = (Real)k; e->m_numRebuilds = (Int)a[3] + k;
			e->m_health = (Int)(a[4] % 300); e->m_whiner = (a[4] & 1) != 0; e->m_unsellable = (a[4] & 2) != 0; e->m_repairable = (a[4] & 4) != 0; e->m_automaticallyBuild = (a[4] & 8) != 0;
			e->m_isInitiallyBuilt = (a[4] & 16) != 0; e->m_underConstruction = (a[4] & 32) != 0; e->m_isSupplyBuilding = (a[4] & 64) != 0;
			{ char b[16]; sprintf(b, "B%u", a[5] % 5); e->m_buildingName = AsciiString(b); sprintf(b, "S%u", (a[5] / 8) % 3); e->m_script = AsciiString(b); }
			e->m_rallyPointOffset.x = (Real)(a[6] % 50); e->m_rallyPointOffset.y = (Real)k; e->m_desiredGatherers = (Int)(a[7] % 6); e->m_currentGatherers = (Int)((a[7] / 8) % 6); e->m_objectTimestamp = a[7];
			for (int j = 0; j < BuildListInfo::MAX_RESOURCE_GATHERERS; ++j) e->m_resourceGatherers[j] = (ObjectID)(a[5] * 0x01010101u + (unsigned)j * a[6]);
			e->m_nextBuildList = head; head = e; }
		p->setBuildList(head); break; }
	case 11: p->addToBuildList(pickObj(a[0])); break;
	case 12: { Coord3D pos = coordOf(a[1], a[2], a[3]); float ang; memcpy(&ang, &a[4], 4); p->addToPriorityBuildList(thingName(a[0] % 16), &pos, ang); break; }
	case 13: { const PlayerTemplate* pt = (a[0] && !g_ptpls.empty()) ? g_ptpls[(a[0] - 1) % g_ptpls.size()] : NULL; p->init(pt); applyAiCfg(p); break; }
	case 14: p->m_energy.adjustPower((Int)a[0], a[1] != 0); break;
	case 15: p->m_energy.setPowerSabotagedTillFrame(a[0]); break;
	case 16: Rec("ret skirmish %d", p->isSkirmishAIPlayer() ? 1 : 0); break;
	case 17: { SpecialPowerTemplate* t = g_sptpls[a[0] % g_sptpls.size()]; Coord3D pos = coordOf(a[2], a[3], a[4]); float rad; memcpy(&rad, &a[1], 4);
		bool r = p->computeSuperweaponTarget(t, &pos, (Int)(a[5] % 4), rad); Rec("ret super %d %08x,%08x,%08x", r ? 1 : 0, FB(pos.x), FB(pos.y), FB(pos.z)); break; }
	case 18: { Player* e = p->getCurrentEnemy(); Rec("ret enemy %d", pidx(e)); break; }
	case 19: Rec("ret diff %d", (int)p->getPlayerDifficulty()); break;
	case 20: { Object* o = a[0] ? pickObj(a[0] - 1) : NULL; Waypoint* w = (a[1] && !g_ways.empty()) ? g_ways[(a[1] - 1) % g_ways.size()] : NULL; Rec("ret bridges %d", p->checkBridges(o, w) ? 1 : 0); break; }
	case 21: { Coord3D pos = coordOf(a[0], a[1], a[2]); bool r = p->getAiBaseCenter(&pos); Rec("ret base %d %08x,%08x,%08x", r ? 1 : 0, FB(pos.x), FB(pos.y), FB(pos.z)); break; }
	case 22: p->repairStructure((ObjectID)a[0]); break;
	case 23: { Object* f = a[0] ? pickObj(a[0] - 1) : NULL; Object* u = a[1] ? pickObj(a[1] - 1) : NULL; p->onUnitCreated(f, u); break; }
	case 24: Rec("ret safe %d", p->isSupplySourceSafe((Int)a[0]) ? 1 : 0); break;
	case 25: Rec("ret attacked %d", p->isSupplySourceAttacked() ? 1 : 0); break;
	case 26: p->setTeamDelaySeconds((Int)a[0]); break;
	case 27: { Team* t = a[0] ? pickTeam(a[0] - 1) : NULL; p->guardSupplyCenter(t, (Int)a[1]); break; }
	case 28: { Team* t = a[0] ? pickTeam(a[0] - 1) : NULL; p->preTeamDestroy(t); break; }
	case 29: { Object* b2 = a[0] ? pickObj(a[0] - 1) : NULL; Object* s = a[1] ? pickObj(a[1] - 1) : NULL; p->onStructureCreated(b2, s); break; }
	case 30: { Object* s = a[0] ? pickObj(a[0] - 1) : NULL; p->onStructureUndone(s); break; }
	case 31: { TeamPrototype* pr = pickProto(a[0]); if (!pr) { Rec("skip"); break; } p->addTeamToList(pr); break; }
	case 32: { TeamPrototype* pr = pickProto(a[0]); if (!pr) { Rec("skip"); break; } p->removeTeamFromList(pr); break; }
	case 33: p->healAllObjects(); break;
	case 34: p->iterateObjects(iterCb, NULL); break;
	case 35: {
		int n = a[0] % 5; const ThingTemplate* things[4]; int counts[4] = { 7, 7, 7, 7 };
		for (int k = 0; k < n; ++k) things[k] = a[1 + k] ? g_tpls[(a[1 + k] - 1) % g_tpls.size()] : NULL;
		p->countObjectsByThingTemplate(n, things, a[5] != 0, counts, a[6] != 0);
		Rec("ret count %s", countsStr(counts, n).c_str());
		break; }
	case 36: Rec("ret buildings %d", p->countBuildings()); break;
	case 37: Rec("ret objects %d", p->countObjects(kindMask(a[0]), kindMask(a[1]))); break;
	case 38: { Object* q = a[0] ? pickObj(a[0] - 1) : NULL; Object* c = p->findClosestByKindOf(q, kindMask(a[1]), kindMask(a[2])); Rec("ret closest %d", c ? (int)c->m_id : 0); break; }
	case 39: Rec("ret anybuildings %d", p->hasAnyBuildings() ? 1 : 0); break;
	case 40: Rec("ret anybuildingskind %d", p->hasAnyBuildings(kindMask(a[0])) ? 1 : 0); break;
	case 41: Rec("ret anyunits %d", p->hasAnyUnits() ? 1 : 0); break;
	case 42: Rec("ret anyobjects %d", p->hasAnyObjects() ? 1 : 0); break;
	case 43: Rec("ret buildfacility %d", p->hasAnyBuildFacility() ? 1 : 0); break;
	case 44: p->updateTeamStates(); break;
	case 45: Rec("ret local %d", p->isLocalPlayer() ? 1 : 0); break;
	case 46: p->setListInScoreScreen(a[0] != 0); Rec("ret score %d", p->getListInScoreScreen() ? 1 : 0); break;
	case 47: Rec("ret box %u", (unsigned)p->getSupplyBoxValue()); break;
	case 48: { AsciiString n = thingName(a[0] % 16); Rec("ret prod %08x %08x %d", FB(p->getProductionCostChangePercent(n)), FB(p->getProductionTimeChangePercent(n)), (int)p->getProductionVeterancyLevel(n)); break; }
	case 49: p->friend_setSkillset((Int)a[0]); break;
	case 50: p->setObjectsEnabled(thingName(a[0] % 16), a[1] != 0); break;
	case 51: p->setCanBuildUnits(a[0] != 0); p->setCanBuildBase(a[1] != 0); break;
	case 52: { const ThingTemplate* t = g_tpls[a[0] % g_tpls.size()]; Rec("ret allowed %d", p->allowedToBuild(t) ? 1 : 0); break; }
	case 53: { TeamPrototype* pr = a[0] ? pickProto(a[0] - 1) : NULL; p->buildSpecificTeam(pr); break; }
	case 54: p->buildBaseDefense(a[0] != 0); break;
	case 55: p->buildBaseDefenseStructure(thingName(a[0] % 16), a[1] != 0); break;
	case 56: p->buildSpecificBuilding(thingName(a[0] % 16)); break;
	case 57: p->buildBySupplies((Int)a[0], thingName(a[1] % 16)); break;
	case 58: { Team* t = a[1] ? pickTeam(a[1] - 1) : NULL; p->buildSpecificBuildingNearestTeam(thingName(a[0] % 16), t); break; }
	case 59: p->buildUpgrade(thingName(a[0] % 16)); break;
	case 60: { TeamPrototype* pr = a[0] ? pickProto(a[0] - 1) : NULL; float rad; memcpy(&rad, &a[1], 4); p->recruitSpecificTeam(pr, rad); break; }
	case 61: { const ThingTemplate* t = g_tpls[a[0] % g_tpls.size()]; Coord3D loc = coordOf(a[1], a[2], a[3]); bool r = p->calcClosestConstructionZoneLocation(t, &loc); Rec("ret zone %d %08x,%08x,%08x", r ? 1 : 0, FB(loc.x), FB(loc.y), FB(loc.z)); break; }
	case 62: p->addRadar(a[0] != 0); break;
	case 63: p->removeRadar(a[0] != 0); break;
	case 64: p->disableRadar(); break;
	case 65: p->enableRadar(); break;
	case 66: Rec("ret radar %d", p->hasRadar() ? 1 : 0); break;
	case 67: Rec("ret radarok %d", p->okToPlayRadarEdgeSound() ? 1 : 0); break;
	case 68: p->onPowerBrownOutChange(a[0] != 0); break;
	case 69: {
		SpecialPowerTemplate* t = g_sptpls[a[1] % g_sptpls.size()];
		switch (a[0] % 4)
		{
		case 0: p->resetOrStartSpecialPowerReadyFrame(t); break;
		case 1: p->expressSpecialPowerReadyFrame(t, a[2]); break;
		case 2: Rec("ret spready %u", (unsigned)p->getOrStartSpecialPowerReadyFrame(t)); break;
		default: p->addNewSharedSpecialPowerTimer(t, a[2]); break;
		}
		break; }
	case 70: {
		KindOfMaskType k = kindMask(a[1]); float pc; memcpy(&pc, &a[2], 4);
		switch (a[0] % 3)
		{
		case 0: p->addKindOfProductionCostChange(k, pc); break;
		case 1: p->removeKindOfProductionCostChange(k, pc); break;
		default: Rec("ret kopc %08x", FB(p->getProductionCostChangeBasedOnKindOf(k))); break;
		}
		break; }
	case 71: if (a[0] % 2 == 0) p->setAttackedBy((Int)(a[1] % 16)); else Rec("ret attackedby %d", p->getAttackedBy((Int)(a[1] % 16)) ? 1 : 0); break;
	case 72: Rec("ret status %d%d%d%d", p->isPlayerObserver() ? 1 : 0, p->isPlayerDead() ? 1 : 0, p->isPlayerActive() ? 1 : 0, p->isPlayableSide() ? 1 : 0); break;
	case 73: g_playerList.m_local = g_players[a[0] % np]; break;
	case 74: {
		ScienceType s = sciOf(a[1]);
		switch (a[0] % 10)
		{
		case 0: Rec("ret hasscience %d", p->hasScience(s) ? 1 : 0); break;
		case 1: p->setScienceAvailability(s, (ScienceAvailabilityType)(a[2] % 3)); break;
		case 2: Rec("ret purchase %d", p->attemptToPurchaseScience(s) ? 1 : 0); break;
		case 3: Rec("ret grant %d", p->grantScience(s) ? 1 : 0); break;
		case 4: Rec("ret capable %d", p->isCapableOfPurchasingScience(s) ? 1 : 0); break;
		case 5: Rec("ret disabledhidden %d%d", p->isScienceDisabled(s) ? 1 : 0, p->isScienceHidden(s) ? 1 : 0); break;
		case 6: Rec("ret prereqs %d", p->hasPrereqsForScience(s) ? 1 : 0); break;
		case 7: { static const char* names[] = { "Available", "disabled", "HIDDEN", "other" }; Rec("ret avail %d", (int)p->getScienceAvailabilityTypeFromString(AsciiString(names[a[2] % 4]))); break; }
		case 8: p->addSciencePurchasePoints((Int)a[2]); break;
		default: p->resetSciences(); break;
		}
		break; }
	case 75: {
		switch (a[0] % 5)
		{
		case 0: Rec("ret setrank %d", p->setRankLevel((Int)a[1] % 8) ? 1 : 0); break;
		case 1: Rec("ret addskill %d", p->addSkillPoints((Int)a[1] % 5000) ? 1 : 0); break;
		case 2: { Object* k = a[1] ? pickObj(a[1] - 1) : NULL; Object* v = a[2] ? pickObj(a[2] - 1) : NULL; Rec("ret killskill %d", p->addSkillPointsForKill(k, v) ? 1 : 0); break; }
		case 3: p->resetRank(); break;
		default: p->setSkillPointsModifier(a[1] % 3 == 0 ? 1.0f : (a[1] % 3 == 1 ? 0.5f : 2.25f)); break;
		}
		break; }
	case 76: p->setCashBounty(a[0] % 2 ? 0.25f : 0.0f); p->m_unitsShouldHunt = a[1] != 0; p->m_isPlayerDead = a[2] != 0; break;
	case 77: { Object* o = a[0] ? pickObj(a[0] - 1) : NULL; p->becomingTeamMember(o, a[1] != 0); break; }
	case 78: p->becomingLocalPlayer(a[0] != 0); break;
	case 79: {
		SpecialPowerType sp = k_spPool[a[1] % 9];
		const ThingTemplate* th = g_tpls[a[2] % g_tpls.size()];
		switch (a[0] % 7)
		{
		case 0: { Object* o = p->findNaturalCommandCenter(); Rec("ret ncc %d", o ? (int)o->m_id : 0); break; }
		case 1: { Object* o = p->findMostReadyShortcutSpecialPowerOfType(sp); Rec("ret mrsp %d", o ? (int)o->m_id : 0); break; }
		case 2: { UnsignedInt pct = 77; Object* o = p->findMostReadyShortcutWeaponForThing(th, pct); Rec("ret mrw %d %u", o ? (int)o->m_id : 0, (unsigned)pct); break; }
		case 3: { UnsignedInt pct = 77; Object* o = p->findMostReadyShortcutSpecialPowerForThing(th, pct); Rec("ret mrst %d %u", o ? (int)o->m_id : 0, (unsigned)pct); break; }
		case 4: { Object* o = p->findAnyExistingObjectWithThingTemplate(th); Rec("ret anyobj %d", o ? (int)o->m_id : 0); break; }
		case 5: Rec("ret anysp %d", p->hasAnyShortcutSpecialPower() ? 1 : 0); break;
		default: Rec("ret countsp %d", p->countReadyShortcutSpecialPowersOfType(sp)); break;
		}
		break; }
	case 80: { Object* b2 = a[0] ? pickObj(a[0] - 1) : NULL; p->onStructureConstructionComplete(b2, pickObj(a[1]), a[2] != 0); break; }
	case 81: p->setUnitsShouldHunt(a[0] != 0, (CommandSourceType)(a[1] % 4)); break;
	case 82: p->killPlayer(); break;
	case 83: { Player* that = pickPlayer(a[0]); if (!that->m_playerTemplate) { Rec("skip notemplate"); break; } p->transferAssetsFromThat(that); break; }
	case 84:
		switch (a[0] % 4)
		{
		case 0: p->garrisonAllUnits((CommandSourceType)(a[1] % 4)); break;
		case 1: p->ungarrisonAllUnits((CommandSourceType)(a[1] % 4)); break;
		case 2: p->setUnitsShouldIdleOrResume(a[1] != 0); break;
		default: p->sellEverythingUnderTheSun(); break;
		}
		break;
	case 85: { Object* k = a[0] ? pickObj(a[0] - 1) : NULL; Object* v = a[1] ? pickObj(a[1] - 1) : NULL; p->doBountyForKill(k, v); break; }
	case 86: {
		const ThingTemplate* t = (a[1] && a[0] % 3 == 1) ? g_tpls[(a[1] - 1) % g_tpls.size()] : g_tpls[a[1] % g_tpls.size()];
		if (a[0] % 3 == 1 && !a[1]) t = NULL;
		switch (a[0] % 3)
		{
		case 0: Rec("ret morethan %d", p->canBuildMoreOfType(t) ? 1 : 0); break;
		case 1: Rec("ret canbuild %d", p->canBuild(t) ? 1 : 0); break;
		default: Rec("ret afford %d", p->canAffordBuild(t) ? 1 : 0); break;
		}
		break; }
	case 87: p->friend_applyDifficultyBonusesForObject(pickObj(a[0]), a[1] != 0); break;
	case 88: {
		static const float scal[8] = { 1.0f, 0.5f, 2.0f, 0.0f, 0.7f, 1.0f, 1.3f, 3.0f };
		switch (a[0] % 6)
		{
		case 0: case 5: {
			BattlePlanBonuses* b2 = newInstance(BattlePlanBonuses);
			b2->m_armorScalar = scal[a[3] % 8]; b2->m_sightRangeScalar = scal[(a[3] >> 4) % 8];
			b2->m_bombardment = (Int)(a[4] % 4); b2->m_holdTheLine = (Int)((a[4] >> 4) % 4); b2->m_searchAndDestroy = (Int)((a[4] >> 8) % 4);
			b2->m_validKindOf = kindMask(a[5]); b2->m_invalidKindOf = kindMask(a[6]);
			if (a[0] % 6 == 0) p->changeBattlePlan((BattlePlanStatus)(a[1] % 4), (Int)(a[2] % 2 ? 1 : -1), b2); else p->applyBattlePlanBonusesForPlayerObjects(b2);
			Rec("bonus %08x %08x %d %d %d %s %s", FB(b2->m_armorScalar), FB(b2->m_sightRangeScalar), b2->m_bombardment, b2->m_holdTheLine, b2->m_searchAndDestroy, kindBits(b2->m_validKindOf).c_str(), kindBits(b2->m_invalidKindOf).c_str());
			delete b2;
			break; }
		case 1: { Object* o = a[1] ? pickObj(a[1] - 1) : NULL; Rec("ret qualifies %d", p->doesObjectQualifyForBattlePlan(o) ? 1 : 0); break; }
		case 2: Rec("ret plans %d %d", p->getBattlePlansActiveSpecific((BattlePlanStatus)(a[1] % 4)), p->getNumBattlePlansActive()); break;
		case 3: case 4:
			if (!p->m_battlePlanBonuses) { Rec("skip nobonus"); break; }
			if (a[0] % 6 == 3) p->applyBattlePlanBonusesForObject(pickObj(a[1])); else p->removeBattlePlanBonusesForObject(pickObj(a[1]));
			break;
		default: break;
		}
		break; }
	case 89: p->setUnitsVisionSpied(a[0] != 0, kindMask(a[1]), (PlayerIndex)(a[2] % np)); break;
	case 90: p->m_money.m_money = a[0] % 3 == 0 ? a[1] : (a[1] % 4000); break;
	case 91: {
		size_t n = g_upgrades.size();
		if (!n) { Rec("skip noupgrades"); break; }
		UpgradeTemplate* key = g_upgrades[a[1] % n];
		switch (a[0] % 8)
		{
		case 0: {
			static const UpgradeStatusType st[3] = { UPGRADE_STATUS_INVALID, UPGRADE_STATUS_IN_PRODUCTION, UPGRADE_STATUS_COMPLETE };
			Upgrade* u = p->addUpgrade(key, st[a[2] % 3]);
			Rec("ret addupg %d", listIndex(p, u));
			break; }
		case 1: Rec("ret findupg %d", listIndex(p, p->findUpgrade(key))); break;
		case 2: Rec("ret upgcomplete %d", p->hasUpgradeComplete(key) ? 1 : 0); break;
		case 3: Rec("ret upgmask %d", p->hasUpgradeComplete(upgradeMaskOf(a, 2)) ? 1 : 0); break;
		case 4: Rec("ret upginprod %d", p->hasUpgradeInProduction(key) ? 1 : 0); break;
		case 5: p->removeUpgrade(key); break;
		case 6: p->onUpgradeCompleted(key); break;
		default: {
			UpgradeMaskType m = upgradeMaskOf(a, 3);
			UpgradeMaskType& target = (a[2] % 2 == 0) ? p->m_upgradesInProgress : p->m_upgradesCompleted;
			if (a[6] % 3 == 0) target.clear(m); else target.set(m);
			break; }
		}
		break; }
	case 92: {
		int hk = (int)(a[1] % 13) - 1;
		int n = (int)(a[2] % 5);
		std::vector<ObjectID> ids;
		for (int i = 0; i < n; ++i) ids.push_back(objIdOf(a[3 + i % 4]));
		switch (a[0] % 12)
		{
		case 0: case 1: case 2: {
			GameMessage msg;
			for (size_t i = 0; i < ids.size(); ++i) { GameMessageArgument arg; arg.m_u.objectID = ids[i]; msg.m_args.push_back(arg); }
			if (a[0] % 12 == 0) p->processCreateTeamGameMessage(hk, &msg); else if (a[0] % 12 == 1) p->processSelectTeamGameMessage(hk, &msg); else p->processAddTeamGameMessage(hk, &msg);
			break; }
		case 3: p->getCurrentSelectionAsAIGroup(groupOf(a[2])); break;
		case 4: p->setCurrentlySelectedAIGroup(groupOf(a[2])); break;
		case 5: { Squad* q = p->getHotkeySquad(hk); Rec("ret hotkey %d", q ? q->getSizeOfGroup() : -1); break; }
		case 6: Rec("ret squadno %d", p->getSquadNumberForObject(realObj(a[2]))); break;
		case 7: p->removeObjectFromHotkeySquad(a[2] % 4 == 0 ? NULL : realObj(a[2])); break;
		case 8: p->addAIGroupToCurrentSelection(groupOf(a[2])); break;
		case 9: g_groups[a[1] % 3].m_ids = ids; break;
		default: {
			bool useSel = (a[2] & 4) != 0;
			Squad* q = useSel ? p->m_currentSelection : ((hk >= 0 && hk < NUM_HOTKEY_SQUADS) ? p->m_squads[hk] : NULL);
			if (!q) { Rec(useSel ? "skip nosel" : "skip nosquad"); break; }
			if (a[0] % 12 == 10)
			{
				switch (a[2] % 4)
				{
				case 0: Rec("ret squadall %s", idsOf(q->getAllObjects()).c_str()); break;
				case 1: Rec("ret squadlive %s", idsOf(q->getLiveObjects()).c_str()); break;
				default: Rec("ret squadsize %d %d", q->getSizeOfGroup(), q->isOnSquad(realObj(a[3])) ? 1 : 0); break;
				}
			}
			else if (a[3] % 2 == 0) q->squadFromTeam(a[4] ? pickTeam(a[4] - 1) : NULL, a[5] % 2 == 0);
			else q->squadFromAIGroup(groupOf(a[4]), a[5] % 2 == 0);
			break; }
		}
		break; }
	case 93: {
		if (a[0] % 3 == 0)
		{
			{ XferCRC xc; xc.open(AsciiString("p")); p->crc(&xc); xc.close(); Rec("ret pcrc %08x", xc.getCRC()); if (getenv("PCRC_DBG")) { fprintf(stderr, "CPP bonus=%d crc=%08x sp=%d spp=%d\n", p->m_battlePlanBonuses ? 1 : 0, xc.getCRC(), p->m_skillPoints, p->m_sciencePurchasePoints); if (p->m_battlePlanBonuses) { XferCRC x2; x2.open(AsciiString("q")); p->m_battlePlanBonuses->m_validKindOf.xfer(&x2); fprintf(stderr, " valid-only crc=%08x\n", x2.getCRC()); } } }
			XferSave xs; xs.open(AsciiString(g_tmpPath.c_str())); xs.xferSnapshot(p); xs.close();
			g_saved = readFileBytes(g_tmpPath.c_str());
			g_haveSave = true;
			Rec("ret psave len=%u hash=%08x", (unsigned)g_saved.size(), hashBytes(g_saved));
		}
		else if (!g_haveSave) Rec("skip");
		else
		{
			if (a[1] & 1) { p->m_kindOfPercentProductionChangeList.clear(); p->m_specialPowerReadyTimerList.clear(); }
			if (a[1] & 2)
			{
				for (int k = 0; k < NUM_HOTKEY_SQUADS; ++k) if (p->m_squads[k]) p->m_squads[k]->m_objectsCached.clear();
				if (p->m_currentSelection) p->m_currentSelection->m_objectsCached.clear();
			}
			writeFileBytes(g_tmpPath.c_str(), g_saved);
			TheGameState->m_postProcessCount = 0;
			XferLoad xl; xl.open(AsciiString(g_tmpPath.c_str()));
			try { xl.xferSnapshot(p); } catch (...) { try { xl.close(); } catch (...) {} throw; }
			xl.close();
			Rec("ret pload pp=%d", TheGameState->m_postProcessCount);
			p->loadPostProcess();
		}
		break; }
	default: Rec("ret nop"); break;
	}
}

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
	case 2: { unsigned id = r.u(); Team* t = TheTeamFactory->createTeam(teamName(id)); Rec("ret team %d", tid(t)); break; }
	case 3: { unsigned id = r.u(); Team* t = TheTeamFactory->createInactiveTeam(teamName(id)); Rec("ret team %d", tid(t)); break; }
	case 4: { unsigned ok = r.u(), ts = r.u(); Team* t = ts ? pickTeam(ts - 1) : NULL; if (ts && !t) { Rec("skip"); break; } pickObj(ok)->setTeam(t); break; }
	case 5: { unsigned ok = r.u(), sub = r.u(), a[8]; for (int k = 0; k < 8; ++k) a[k] = r.u(); objOp(pickObj(ok), sub, a); break; }
	case 6: { unsigned pk = r.u(), sub = r.u(), a[8]; for (int k = 0; k < 8; ++k) a[k] = r.u(); playerOp(pickPlayer(pk), sub, a); break; }
	default: Rec("ret nop"); break;
	}
}

/* The Player differential (tests/differential_player.rs): runs a program (the words) against the real Player.cpp functions (and the
** real Team.cpp) on the stand-ins, and writes the trace lines, each ended by '\n'. Returns the size of the text (written only
** if it fits `cap`). */
extern "C" int ref_player_run(const unsigned* words, int n, const char* tmpPath, char* out, int cap)
{
	g_tmpPath = tmpPath; g_saved.clear(); g_haveSave = false; g_upgrades.clear(); TheGameState->m_postProcessCount = 0;
	static NameKeyGenerator* gen = NULL;
	if (!gen) gen = new NameKeyGenerator;
	gen->reset();
	TheNameKeyGenerator = gen;
	resetKeys();
	for (int i = 0; i < 3; ++i) { g_groups[i].m_id = i + 1; g_groups[i].m_ids.clear(); }
	for (int i = 0; i < 2; ++i) g_triggers[i].m_id = i;
	std::vector<std::string> lines;
	g_lines = &lines;
	g_setTeamCalls = 0;
	g_aiCfg.clear();
	for (int i = 0; i < 8; ++i) { char b[16]; sprintf(b, "Sci%d", i); g_sciKey[i] = (int)TheNameKeyGenerator->nameToKey(b); }
	TheTeamFactory = new TeamFactory;
	TheTeamFactory->init();
	Reader r; r.w = words; r.n = n; r.p = 0;
	setupWorld(r);
	int nProbe = r.i();
	std::vector<std::string> probes;
	for (int i = 0; i < nProbe; ++i) probes.push_back(r.s());
	int nOps = r.i();
	bool threw = false;
	dumpState();
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
