/* Differential-test wrapper for ScriptActions.cpp (upstream commit 0a05454d), p1w13-sonnet, layer 11 (batches A1..):
**   GeneralsMD/Code/GameEngine/Include/GameLogic/ScriptActions.h              (SA_HEADER_DIR: build.rs's copy, see there)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptActions.cpp (SA_SOURCE: build.rs's copy, see there)
**   GeneralsMD/Code/GameEngine/Source/Common/PartitionSolver.cpp               (unchanged)
** on the stand-ins of differential/shim_actions (sa_world.h and its parts: the world answers from the configuration and its own state,
** every call with an effect writes a line), all inside namespace saw. Entry point (extern "C", tests/differential_script_actions.rs):
** ref_actions_run. */
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

namespace saw {
#define private public
#define protected public
#include SA_SOURCE
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/PartitionSolver.cpp"
#undef protected
#undef private

unsigned g_cfg[SA_CFG];
SaWorld g_world;
std::vector<std::string> g_log;
void saLog(const char *fmt, ...)
{
	char buf[512];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof(buf), fmt, ap);
	va_end(ap);
	g_log.push_back(buf);
}
AsciiString AsciiString::TheEmptyString;
NameKeyGenerator g_keys; NameKeyGenerator *TheNameKeyGenerator = &g_keys;
KindOfMaskType KINDOFMASK_NONE;
ThingTemplate g_templates[SA_TEMPLATES];
Waypoint g_waypoints[SA_WAYPOINTS];
PolygonTrigger g_triggers[8];
TeamPrototype g_protos[SA_PROTOS];
Player g_players[SA_PLAYERS];
PlayerList g_playerList; PlayerList *ThePlayerList = &g_playerList;
ScriptEngine g_engine; ScriptEngine *TheScriptEngine = &g_engine;
TerrainLogic g_terrain; TerrainLogic *TheTerrainLogic = &g_terrain;
View g_view; View *TheTacticalView = &g_view;
AI g_ai; AI *TheAI = &g_ai;
Pathfinder g_pf;
PartitionManager g_partition; PartitionManager *ThePartitionManager = &g_partition;
GameLogic g_logic; GameLogic *TheGameLogic = &g_logic;
ThingFactory g_factory; ThingFactory *TheThingFactory = &g_factory;
TeamFactory g_teamFactory; TeamFactory *TheTeamFactory = &g_teamFactory;
ControlBar g_controlBar; ControlBar *TheControlBar = &g_controlBar;
InGameUI g_ui; InGameUI *TheInGameUI = &g_ui;
GameAudio g_audio; GameAudio *TheAudio = &g_audio;
Radar g_radar; Radar *TheRadar = &g_radar;
Display g_display; Display *TheDisplay = &g_display;
CampaignManager g_campaign; CampaignManager *TheCampaignManager = &g_campaign;
GameWindowManager g_windows; GameWindowManager *TheWindowManager = &g_windows;
SpecialPowerStore g_powers; SpecialPowerStore *TheSpecialPowerStore = &g_powers;
GlobalData g_global; GlobalData *TheGlobalData = &g_global; GlobalData *TheWritableGlobalData = &g_global;
GameText g_text; GameText *TheGameText = &g_text;
ScienceStore g_sciences; ScienceStore *TheScienceStore = &g_sciences;
Mouse g_mouse; Mouse *TheMouse = &g_mouse;
GameEngine g_gameEngine; GameEngine *TheGameEngine = &g_gameEngine;
VictoryConditionsInterface g_victory; VictoryConditionsInterface *TheVictoryConditions = &g_victory;
UpgradeCenter g_upgrades; UpgradeCenter *TheUpgradeCenter = &g_upgrades;
SnowManager g_snow; SnowManager *TheSnowManager = &g_snow;
LookAtTranslator g_lookAt; LookAtTranslator *TheLookAtTranslator = &g_lookAt;
GlobalLanguage g_language; GlobalLanguage *TheGlobalLanguageData = &g_language;
GameClient g_client; GameClient *TheGameClient = &g_client;
FontLibrary g_fonts; FontLibrary *TheFontLibrary = &g_fonts;
Eva g_eva; Eva *TheEva = &g_eva;
const char *TheObjectFlagsNames[] = { "Enabled", "Powered", "Indestructible", "Unsellable", "Selectable", "AI Recruitable", "Player Targetable", NULL };
void oversizeTheTerrain(Int amount) { saLog("terrain oversize %d", amount); }
void doSkyBoxSet(Bool startDraw) { saLog("skybox %d", startDraw ? 1 : 0); }
void HideControlBar(Bool immediate) { saLog("controlbar hide %d", immediate ? 1 : 0); }
void ShowControlBar(Bool immediate) { saLog("controlbar show %d", immediate ? 1 : 0); }

/* --- out-of-line members of the stand-ins --- */
static int oid(const Object *o) { return o ? o->getID() : 0; }
void AIGroup::groupAttackTeam(const Team *t, Int maxShots, CommandSourceType cmd) { saLog("group %d attackteam %d %d %d", m_id, t->getID(), maxShots, (int)cmd); }
void AIGroup::groupAttackObject(Object *o, Int maxShots, CommandSourceType cmd) { saLog("group %d attackobj %d %d %d", m_id, oid(o), maxShots, (int)cmd); }
void AIGroup::groupEnter(Object *o, CommandSourceType cmd) { saLog("group %d enter %d %d", m_id, oid(o), (int)cmd); }
void AIGroup::groupGuardObject(Object *o, GuardMode m, CommandSourceType cmd) { saLog("group %d guardobj %d %d %d", m_id, oid(o), (int)m, (int)cmd); }
Bool AIGroup::getCenter(Coord3D *c)
{
	if (m_members.empty()) return FALSE;
	c->x = c->y = c->z = 0;
	for (size_t i = 0; i < m_members.size(); ++i) { c->x += m_members[i]->m_pos.x; c->y += m_members[i]->m_pos.y; c->z += m_members[i]->m_pos.z; }
	c->x /= m_members.size(); c->y /= m_members.size(); c->z /= m_members.size();
	return TRUE;
}
void AIGroup::groupDoCommandButton(const CommandButton *b, CommandSourceType cmd) { saLog("group %d button %d %d", m_id, b->m_index, (int)cmd); }
void AIGroup::groupDoCommandButtonAtObject(const CommandButton *b, Object *o, CommandSourceType cmd) { saLog("group %d buttonobj %d %d %d", m_id, b->m_index, oid(o), (int)cmd); }
void AIGroup::groupDoCommandButtonAtPosition(const CommandButton *b, const Coord3D *p, CommandSourceType cmd) { saLog("group %d buttonpos %d %08x %08x %08x %d", m_id, b->m_index, fb(p->x), fb(p->y), fb(p->z), (int)cmd); }
Object *AIGroup::getSpecialPowerSourceObject(UnsignedInt type) { for (size_t i = 0; i < m_members.size(); ++i) if (saHash(SA_SEED, (unsigned)m_members[i]->getID(), 600u + type) & 1) return m_members[i]; return NULL; }
Object *AIGroup::getCommandButtonSourceObject(GUICommandType type) { for (size_t i = 0; i < m_members.size(); ++i) if (saHash(SA_SEED, (unsigned)m_members[i]->getID(), 700u + (unsigned)type) & 1) return m_members[i]; return NULL; }
void Pathfinder::addObjectToPathfindMap(Object *o) { saLog("pathfind add %d", oid(o)); }
void AIUpdateInterface::aiForceAttackObject(Object *o, Int maxShots, CommandSourceType cmd) { saLog("ai %d forceattack %d %d %d", m_obj, oid(o), maxShots, (int)cmd); }
void AIUpdateInterface::aiAttackTeam(const Team *t, Int maxShots, CommandSourceType cmd) { saLog("ai %d attackteam %d %d %d", m_obj, t->getID(), maxShots, (int)cmd); }
void AIUpdateInterface::aiEnter(Object *o, CommandSourceType cmd) { saLog("ai %d enter %d %d", m_obj, oid(o), (int)cmd); }
void AIUpdateInterface::aiExit(Object *o, CommandSourceType cmd) { saLog("ai %d exit %d %d", m_obj, oid(o), (int)cmd); }
void AIUpdateInterface::aiFaceObject(Object *o, CommandSourceType cmd) { saLog("ai %d faceobj %d %d", m_obj, oid(o), (int)cmd); }
void AIUpdateInterface::aiMoveToObject(Object *o, CommandSourceType cmd) { saLog("ai %d moveobj %d %d", m_obj, oid(o), (int)cmd); }
void AIUpdateInterface::setAttackInfo(const AttackPriorityInfo *info) { saLog("ai %d attackinfo %s", m_obj, info ? info->getName().str() : "-"); }

void ContainModuleInterface::addToContain(Object *o)
{
	Object *me = g_world.objects[m_obj - 1];
	saLog("contain %d add %d", m_obj, oid(o));
	o->m_containedBy = me;
	me->m_contained.push_back(o);
}
UnsignedInt ContainModuleInterface::getContainCount() const { return (UnsignedInt)g_world.objects[m_obj - 1]->m_contained.size(); }
UnsignedInt ContainModuleInterface::getContainMax() const { const Object *me = g_world.objects[m_obj - 1]; return me->m_tmpl ? me->m_tmpl->getContainMax() : (me->w(1) >> 18 & 7); }
Bool ContainModuleInterface::isValidContainerFor(const Object *o, Bool checkCapacity) const
{
	if (o->isKindOf(KINDOF_STRUCTURE) || !(saHash(SA_SEED, (unsigned)m_obj, 90u + (unsigned)o->getID()) & 3)) return FALSE;
	return !checkCapacity || getContainCount() < getContainMax();
}
void ContainModuleInterface::iterateContained(ContainIterateFunc f, void *userData, Bool reverse)
{
	std::vector<Object *> c = g_world.objects[m_obj - 1]->m_contained;
	if (reverse) std::reverse(c.begin(), c.end());
	for (size_t i = 0; i < c.size(); ++i) f(c[i], userData);
}
void ContainModuleInterface::removeAllContained(Bool expose)
{
	Object *me = g_world.objects[m_obj - 1];
	saLog("contain %d removeall %d", m_obj, expose ? 1 : 0);
	for (size_t i = 0; i < me->m_contained.size(); ++i) me->m_contained[i]->m_containedBy = NULL;
	me->m_contained.clear();
}
Bool ContainModuleInterface::isGarrisonable() const { return (saHash(SA_SEED, (unsigned)m_obj, 91u) & 1) != 0; }
PlayerMaskType ContainModuleInterface::getPlayerWhoEntered() const { int p = (int)(saHash(SA_SEED, (unsigned)m_obj, 92u) % 9u); return p == 8 ? 0u : 1u << p; }
void SpecialPowerModuleInterface::doSpecialPowerAtObject(Object *o, UnsignedInt opts) { saLog("spm %d %d atobj %d %u", m_obj, m_power, oid(o), opts); }
const ThingTemplate *DeliverPayloadAIUpdate::getPutInContainerTemplateViaModuleData() const
{
	const Object *me = g_world.objects[m_obj - 1];
	int t = me->m_tmpl ? (int)(me->m_tmpl->w() >> 24 & 7) : 7;
	return t < saNumTemplates() ? &g_templates[t] : NULL;
}
void StickyBombUpdate::initStickyBomb(Object *target, const Object *bomber, const Coord3D *pos) { saLog("sticky %d init %d %d %08x %08x %08x", m_obj, oid(target), oid(bomber), fb(pos->x), fb(pos->y), fb(pos->z)); }
Object *Weapon::forceFireWeapon(Object *src, const Coord3D *p)
{
	saLog("weapon %d forcefire %08x %08x %08x", m_obj, fb(p->x), fb(p->y), fb(p->z));
	return (saHash(SA_SEED, (unsigned)m_obj, 61u) & 1) ? g_world.objects[m_obj - 1] : NULL;
}
void Object::giveUpgrade(const UpgradeTemplate *u) { saLog("obj %d giveupgrade %d", getID(), u ? u->m_index : -1); }
UpdateModule *Object::findUpdateModule(NameKeyType key) const
{
	const Object *me = this;
	if (key == NAMEKEY("DeliverPayloadAIUpdate")) { if (!(m_tmpl && m_tmpl->hasDeliverPayload())) return NULL; m_dp.m_obj = getID(); return &m_dp; }
	if (key == NAMEKEY("CommandButtonHuntUpdate")) { if (!(saHash(SA_SEED, (unsigned)getID(), 51u) & 1)) return NULL; m_cbh.m_obj = getID(); return &m_cbh; }
	if (key == NAMEKEY("StickyBombUpdate")) { if (!(saHash(SA_SEED, (unsigned)getID(), 52u) & 1)) return NULL; m_sticky.m_obj = getID(); return &m_sticky; }
	if (key == NAMEKEY("SupplyWarehouseDockUpdate")) { if (!(saHash(SA_SEED, (unsigned)getID(), 53u) & 1)) return NULL; m_ware.m_obj = getID(); return &m_ware; }
	if (key == NAMEKEY("RailroadBehavior")) { if (!(saHash(SA_SEED, (unsigned)getID(), 54u) & 1)) return NULL; m_rail.m_obj = getID(); return &m_rail; }
	(void)me;
	return NULL;
}
void Object::doCommandButton(const CommandButton *b, CommandSourceType cmd) { saLog("obj %d button %d %d", getID(), b->m_index, (int)cmd); }
void Object::doCommandButtonAtObject(const CommandButton *b, Object *o, CommandSourceType cmd) { saLog("obj %d buttonobj %d %d %d", getID(), b->m_index, oid(o), (int)cmd); }
void Object::doCommandButtonAtPosition(const CommandButton *b, const Coord3D *p, CommandSourceType cmd) { saLog("obj %d buttonpos %d %08x %08x %08x %d", getID(), b->m_index, fb(p->x), fb(p->y), fb(p->z), (int)cmd); }
void Object::doCommandButtonUsingWaypoints(const CommandButton *b, const Waypoint *w, CommandSourceType cmd) { saLog("obj %d buttonpath %d %d %d", getID(), b->m_index, w->m_index, (int)cmd); }
const TeamTemplateInfo *TeamPrototype::getTemplateInfo() const
{
	unsigned v = g_cfg[104 + m_index];
	TeamTemplateInfo &i = m_info;
	i.m_startReinforceWaypoint = saName("wp", (int)(v & 7));
	i.m_transportUnitType = (v >> 3 & 1) ? saName("tmpl", (int)(v >> 4 & 7)) : AsciiString();
	i.m_numUnitsInfo = (Int)(v >> 7 & 3);
	for (int k = 0; k < 3; ++k) { i.m_unitsInfo[k].unitThingName = saName("tmpl", (int)(v >> (9 + 5 * k) & 7)); i.m_unitsInfo[k].maxUnits = (Int)(v >> (12 + 5 * k) & 3); i.m_unitsInfo[k].minUnits = 0; }
	i.m_teamStartsFull = (v >> 24 & 1) != 0;
	i.m_transportsExit = (v >> 25 & 1) != 0;
	i.m_productionPriority = (Int)(v >> 26 & 15);
	return &i;
}
Team *TeamFactory::make(const AsciiString &name, const char *how)
{
	TeamPrototype *p = findTeamPrototype(name);
	if (!p) { saLog("teamfactory %s %s none", how, name.str()); return NULL; }
	Team *t = new Team;
	t->m_index = (Int)g_world.teams.size();
	t->m_w = 4;
	t->m_proto = p;
	t->m_controller = NULL;
	g_world.teams.push_back(t);
	p->m_instances.push_back(t);
	saLog("teamfactory %s %s %d", how, name.str(), t->getID());
	return t;
}
Bool Player::calcClosestConstructionZoneLocation(const ThingTemplate *t, Coord3D *p)
{
	if (!(saHash(SA_SEED, (unsigned)m_index, 100u + (unsigned)t->m_index) & 1)) return FALSE;
	p->x = (Real)m_index * 7.0f; p->y = (Real)t->m_index * 3.0f; p->z = 0.0f;
	return TRUE;
}
Bool Player::computeSuperweaponTarget(const SpecialPowerTemplate *sp, Coord3D *pos, Int playerIndex, Real weight)
{
	saLog("player %d superweapontarget %d %d %08x", m_index, sp->m_index, playerIndex, fb(weight));
	if (!(saHash(SA_SEED, (unsigned)m_index, 110u + (unsigned)sp->m_index) & 1)) return FALSE;
	pos->x = 50.0f + m_index; pos->y = 60.0f + sp->m_index; pos->z = 0.0f;
	return TRUE;
}
Player *ScriptEngine::getPlayerFromAsciiString(const AsciiString &name)
{
	if (name == "<This Player>") return getCurrentPlayer();
	if (name == "<This Player's Enemy>") return getSkirmishEnemyPlayer();
	int k = saNum(name.str(), "player");
	return saPlayer(k);
}
Team *ScriptEngine::getTeamNamed(const AsciiString &name)
{
	if (name == "<This Team>") { unsigned t = g_cfg[5] & 255; return t < g_world.teams.size() ? g_world.teams[t] : NULL; }
	TeamPrototype *p = g_teamFactory.findTeamPrototype(name);
	if (!p) { int k = saNum(name.str(), "lone"); return k >= 0 && k < (int)g_world.teams.size() && !g_world.teams[k]->m_proto ? g_world.teams[k] : NULL; }
	return p->m_instances.empty() ? NULL : p->m_instances[0];
}
Object *ScriptEngine::getUnitNamed(const AsciiString &name)
{
	if (name.isEmpty()) return NULL;
	for (size_t i = 0; i < g_world.objects.size(); ++i) if (g_world.objects[i]->m_name == name) return g_world.objects[i];
	return NULL;
}
ObjectTypes *ScriptEngine::getObjectTypes(const AsciiString &name)
{
	int k = saNum(name.str(), "list");
	if (k < 0 || k >= 4) return NULL;
	m_types.m_types.clear();
	for (int j = 0; j < 9; ++j) if (saHash(SA_SEED, (unsigned)k, 120u + (unsigned)j) & 1) m_types.m_types.push_back(saName("tmpl", j));
	return &m_types;
}
Waypoint *TerrainLogic::getClosestWaypointOnPath(const Coord3D *p, const AsciiString &label)
{
	Waypoint *best = NULL; Real bestD = 0;
	for (int k = 0; k < SA_WAYPOINTS; ++k) {
		if (!saWaypointExists(k) || !isPurposeOfPath(&g_waypoints[k], label)) continue;
		const Coord3D *l = g_waypoints[k].getLocation();
		Real d = (l->x - p->x) * (l->x - p->x) + (l->y - p->y) * (l->y - p->y);
		if (!best || d < bestD) { best = &g_waypoints[k]; bestD = d; }
	}
	return best;
}
/* the partition's searches: the objects that exist, in id order, that a hash of the query keeps (the filters are the partition's) */
static SimpleObjectIterator *saSearch(unsigned salt)
{
	SimpleObjectIterator *it = new SimpleObjectIterator;
	for (size_t i = 0; i < g_world.objects.size(); ++i) if (saHash(SA_SEED ^ g_cfg[16], (unsigned)i, salt) & 1) it->m_list.push_back(g_world.objects[i]);
	return it;
}
static unsigned saFilterSalt(PartitionFilter *const filters[])
{
	unsigned s = 0;
	for (int i = 0; filters[i]; ++i) s = s * 7 + (unsigned)filters[i]->kind();
	return s;
}
SimpleObjectIterator *PartitionManager::iterateObjectsInRange(const Coord3D *pos, Real range, DistanceCalculationType d, PartitionFilter *const filters[], IterOrderType order)
{
	saLog("partition iterate %08x %08x %08x %d %d", fb(pos->x), fb(pos->y), fb(range), (int)d, (int)order);
	return saSearch(1000u + saFilterSalt(filters));
}
SimpleObjectIterator *PartitionManager::iterateObjectsInRange(const Object *obj, Real range, DistanceCalculationType d, PartitionFilter *const filters[], IterOrderType order)
{
	saLog("partition iterateobj %d %08x %d %d", oid(obj), fb(range), (int)d, (int)order);
	return saSearch(2000u + saFilterSalt(filters));
}
Object *PartitionManager::getClosestObject(const Coord3D *pos, Real range, DistanceCalculationType d, PartitionFilter *const filters[], Real *closestDist, Coord3D *closestDistVec)
{
	SimpleObjectIterator *it = saSearch(3000u + saFilterSalt(filters));
	Object *o = it->first();
	delete it;
	if (o && closestDist) *closestDist = (Real)(saHash(SA_SEED, (unsigned)o->getID(), fb(pos->x)) % 1000u);
	return o;
}
Object *PartitionManager::getClosestObject(const Object *obj, Real range, DistanceCalculationType d, PartitionFilter *const filters[], Real *closestDist, Coord3D *closestDistVec)
{
	return getClosestObject(obj->getPosition(), range, d, filters, closestDist, closestDistVec);
}
Bool PartitionManager::getNearestGroupWithValue(Int playerIndex, UnsignedInt allowed, ValueOrThreat vot, const Coord3D *src, Int value, Bool greater, Coord3D *out)
{
	saLog("partition nearestgroup %d %u %d %08x %08x %d %d", playerIndex, allowed, (int)vot, fb(src->x), fb(src->y), value, greater ? 1 : 0);
	if (!(saHash(SA_SEED, (unsigned)playerIndex, 130u + (unsigned)value) & 1)) return FALSE;
	out->x = src->x + 10.0f; out->y = src->y - 10.0f; out->z = 0.0f;
	return TRUE;
}
void GameLogic::setControlBarOverride(const AsciiString &cs, Int slot, const CommandButton *b) { saLog("logic controlbaroverride %s %d %d", cs.str(), slot, b ? b->m_index : -1); }
Object *ThingFactory::newObject(const ThingTemplate *t, Team *team)
{
	Object *o = new Object;
	o->m_index = (Int)g_world.objects.size();
	memset(o->m_w, 0, sizeof(o->m_w));
	o->m_tmpl = t;
	o->m_team = team;
	o->m_pos.zero();
	o->m_angle = 0;
	o->m_killed = FALSE;
	o->m_containedBy = NULL;
	g_world.objects.push_back(o);
	if (team) team->m_members.push_back(o);
	saLog("factory newobject %d %d %d", o->getID(), t ? t->m_index : -1, team ? team->getID() : 0);
	return o;
}
const SpecialPowerTemplate *CommandButton::getSpecialPowerTemplate() const
{
	int n = (int)(g_cfg[11] >> 4 & 15);
	return (saHash(SA_SEED, (unsigned)m_index, 82u) & 1) && n > 0 ? &g_powers.m_powers[m_index % n] : NULL;
}
Bool CommandButton::isValidToUseOn(const Object *src, const Object *target, const Coord3D *pos, CommandSourceType cmd) const
{
	return (saHash(SA_SEED, (unsigned)(m_index * 64 + oid(src)), 83u + (unsigned)oid(target)) & 1) != 0;
}
const CommandButton *CommandSet::getCommandButton(Int i) const
{
	int n = (int)(g_cfg[11] >> 8 & 15);
	unsigned h = saHash(SA_SEED, (unsigned)m_index, 84u + (unsigned)i);
	return n > 0 && (h & 1) ? &g_controlBar.m_buttons[(h >> 4) % n] : NULL;
}

/* the world from the configuration */
void scene()
{
	for (size_t i = 0; i < g_world.objects.size(); ++i) delete g_world.objects[i];
	for (size_t i = 0; i < g_world.teams.size(); ++i) delete g_world.teams[i];
	for (size_t i = 0; i < g_ai.m_groups.size(); ++i) delete g_ai.m_groups[i];
	g_world.objects.clear(); g_world.teams.clear(); g_ai.m_groups.clear();
	g_world.nextGroup = 0;
	g_ai.m_pf = &g_pf;
	g_windows.m_next = 0;
	for (int k = 0; k < SA_TEMPLATES; ++k) { g_templates[k].m_index = k; g_templates[k].m_name = saName("tmpl", k); g_waypoints[k].m_index = k; g_triggers[k].m_index = k; }
	for (int k = 0; k < 16; ++k) { g_powers.m_powers[k].m_index = k; g_controlBar.m_buttons[k].m_index = k; }
	for (int t = 0; t < SA_TEAMS0; ++t) {
		Team *tm = new Team;
		tm->m_index = t; tm->m_w = 0; tm->m_proto = NULL; tm->m_controller = NULL;
		g_world.teams.push_back(tm);
	}
	for (int k = 0; k < SA_PROTOS; ++k) {
		g_protos[k].m_index = k;
		g_protos[k].m_instances.clear();
		for (int t = 0; t < SA_TEAMS0; ++t) {
			if ((g_cfg[96 + k] >> (4 + t) & 1) && !g_world.teams[t]->m_proto) { g_world.teams[t]->m_proto = &g_protos[k]; g_protos[k].m_instances.push_back(g_world.teams[t]); }
		}
	}
	for (int p = 0; p < SA_PLAYERS; ++p) {
		g_players[p].m_index = p;
		g_players[p].m_money.m_player = p;
		g_players[p].m_money.m_amount = g_cfg[128 + 4 * p + 2];
		g_players[p].m_teams.clear();
		for (int k = 0; k < SA_PROTOS; ++k) if ((int)(g_cfg[96 + k] & 15) == p) g_players[p].m_teams.push_back(&g_protos[k]);
	}
	for (int j = 0; j < SA_OBJS0; ++j) {
		Object *o = new Object;
		o->m_index = j;
		memset(o->m_w, 0, sizeof(o->m_w));
		unsigned w0 = g_cfg[40 + 4 * j], w3 = g_cfg[40 + 4 * j + 3];
		o->m_name = (w0 & 1) ? saName("unit", j) : AsciiString();
		int t = (int)(w0 >> 12 & 7);
		o->m_tmpl = t < saNumTemplates() ? &g_templates[t] : NULL;
		int tm = (int)(w0 >> 20 & 15);
		o->m_team = g_world.teams[tm];
		g_world.teams[tm]->m_members.push_back(o);
		o->m_pos.x = (Real)(w3 & 255) * 1.5f; o->m_pos.y = (Real)(w3 >> 8 & 255) * 1.5f; o->m_pos.z = 0.0f;
		o->m_angle = 0.0f;
		o->m_killed = FALSE;
		o->m_containedBy = NULL;
		g_world.objects.push_back(o);
	}
	TheCampaignManager = (g_cfg[0] >> 10 & 1) ? NULL : &g_campaign;
	g_global.m_timeOfDay = (TimeOfDay)(g_cfg[15] >> 4 & 7);
	g_global.m_clearAlpha = (UnsignedByte)(g_cfg[15] >> 8 & 255);
	g_global.m_shroudAlpha = (UnsignedByte)(g_cfg[15] >> 16 & 255);
	g_global.m_framesPerSecondLimit = 30;
	g_global.m_useFpsLimit = TRUE;
	g_global.m_scriptOverrideInfantryLightScale = -1.0f;
	g_global.m_disableMilitaryCaption = FALSE;
	g_global.m_disableScriptedInputDisabling = FALSE;
}

std::vector<unsigned> g_out;
void put(unsigned u) { g_out.push_back(u); }
void putStr(const std::string &s)
{
	put((unsigned)s.size());
	for (size_t i = 0; i < s.size(); i += 4) {
		unsigned w = 0;
		for (size_t k = 0; k < 4 && i + k < s.size(); ++k) w |= (unsigned)(unsigned char)s[i + k] << (8 * k);
		put(w);
	}
}

}  // namespace saw

extern "C" {

/* `cfg` (SA_CFG words, see the test); `acts`: the count, then per action its type, its parameter count and per parameter its type, the Int,
** the Real's bits, the coordinate's three words, the string's length and its bytes packed 4 to a word. `ops` [op, a, b]: 0 execute
** action a, 1 cfg[a] = b (the world is not rebuilt), 2 init, 3 rebuild the world from the configuration. Each op appends its lines,
** then the actions' state (m_suppressNewWindows, the message window's number) and the global data the actions write. */
int ref_actions_run(const unsigned *cfg_in, const unsigned *acts, const unsigned *ops, int n, unsigned *out, int cap, int *len)
{
	using namespace saw;
	memcpy(g_cfg, cfg_in, sizeof(g_cfg));
	g_out.clear();
	g_log.clear();
	std::vector<ScriptAction> as;
	std::vector<Parameter> ps(12 * 16);
	unsigned na = *acts++;
	as.resize(na);
	for (unsigned a = 0; a < na; ++a) {
		ScriptAction &sa = as[a];
		sa.m_actionType = (ScriptAction::ScriptActionType)*acts++;
		sa.m_numParms = (Int)*acts++;
		for (int i = 0; i < MAX_PARMS; ++i) sa.m_parms[i] = NULL;
		for (int i = 0; i < sa.m_numParms; ++i) {
			Parameter &p = ps[a * 12 + i];
			p.m_type = (Int)*acts++;
			p.m_int = (Int)*acts++;
			unsigned rb = *acts++;
			memcpy(&p.m_real, &rb, 4);
			unsigned cx = *acts++, cy = *acts++, cz = *acts++;
			memcpy(&p.m_coord.x, &cx, 4); memcpy(&p.m_coord.y, &cy, 4); memcpy(&p.m_coord.z, &cz, 4);
			unsigned sl = *acts++;
			std::string s;
			for (unsigned k = 0; k < sl; k += 4) {
				unsigned w = *acts++;
				for (unsigned b = 0; b < 4 && k + b < sl; ++b) s.push_back((char)(w >> (8 * b) & 255));
			}
			p.m_string.m_s = s;
			sa.m_parms[i] = &p;
		}
	}
	scene();
	ScriptActions *sa = new ScriptActions;
	ScriptActions::m_messageWindow = NULL;
	sa->init();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 3 * i;
		g_log.clear();
		if (o[0] == 1 && o[1] < SA_CFG) g_cfg[o[1]] = o[2];
		if (o[0] == 3) scene();
		if (o[0] == 0 && o[1] < na) sa->executeAction(&as[o[1]]);
		if (o[0] == 2) sa->init();
		put((unsigned)g_log.size());
		for (size_t j = 0; j < g_log.size(); ++j) putStr(g_log[j]);
		put(sa->m_suppressNewWindows ? 1 : 0);
		put(ScriptActions::m_messageWindow ? (unsigned)ScriptActions::m_messageWindow->m_id : 0);
		put(fb(g_global.m_scriptOverrideInfantryLightScale));
		put((unsigned)g_global.m_framesPerSecondLimit);
		put(g_global.m_useFpsLimit ? 1 : 0);
	}
	delete sa;
	*len = (int)g_out.size();
	if ((int)g_out.size() > cap) return 1;
	memcpy(out, &g_out[0], g_out.size() * 4);
	return 0;
}

}  /* extern "C" */
