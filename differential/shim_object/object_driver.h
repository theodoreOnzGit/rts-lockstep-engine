/* The driver of the Object differential (p1w6-object): the globals the stand-ins declare, the program reader,
** the operations, and the entry point ref_object_run. See tests/differential_object.rs for the program format. */
#ifndef SHIM_OBJECT_DRIVER_H
#define SHIM_OBJECT_DRIVER_H

/* ---- the globals ------------------------------------------------------------------------------------------ */
ObjCfg g_cfg;
std::vector<std::string>* g_lines = NULL;
const char* const* g_flagNames[4];
int g_flagNameCount[4];
KindOfMaskType KINDOFMASK_NONE_;
KindOfMaskType KINDOFMASK_FS_;
DisabledMaskType DISABLEDMASK_NONE_;
const AsciiString AsciiString::TheEmptyString;
NameKeyGenerator* TheNameKeyGenerator = NULL;
AudioManager* TheAudio = NULL;
PlayerList* ThePlayerList = NULL;
TeamFactory* TheTeamFactory = NULL;
Radar* TheRadar = NULL;
Eva* TheEva = NULL;
Anim2DCollection* TheAnim2DCollection = NULL;
InGameUI* TheInGameUI = NULL;
ControlBar* TheControlBar = NULL;
GameClient* TheGameClient = NULL;
BuildAssistant* TheBuildAssistant = NULL;
SpecialPowerStore* TheSpecialPowerStore = NULL;
UpgradeCenter* TheUpgradeCenter = NULL;
ScriptEngine* TheScriptEngine = NULL;
TerrainLogic* TheTerrainLogic = NULL;
AIManager* TheAI = NULL;
PartitionManager* ThePartitionManager = NULL;
GlobalData* TheGlobalData = NULL;
ModuleFactory* TheModuleFactory = NULL;
GameLogic* TheGameLogic = NULL;
ObjectID g_deletingId = INVALID_ID;

#include "kindof_masks.inc"

/* Xfer.cpp's xferUpgradeMask over the stand-in upgrade center (the list runs newest first) */
void Xfer::xferUpgradeMask(void* maskData)
{
	UpgradeMaskType* upgradeMaskData = (UpgradeMaskType*)maskData;
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xferVersion(&version, currentVersion);
	if (getXferMode() == XFER_SAVE)
	{
		AsciiString upgradeName;
		UnsignedShort count = 0;
		for (int i = (int)TheUpgradeCenter->list.size() - 1; i >= 0; --i)
			if (upgradeMaskData->testForAll(TheUpgradeCenter->list[i]->getUpgradeMask())) count++;
		xferUnsignedShort(&count);
		for (int i = (int)TheUpgradeCenter->list.size() - 1; i >= 0; --i)
			if (upgradeMaskData->testForAll(TheUpgradeCenter->list[i]->getUpgradeMask()))
			{
				upgradeName = TheUpgradeCenter->list[i]->getUpgradeName();
				xferAsciiString(&upgradeName);
			}
	}
	else if (getXferMode() == XFER_LOAD)
	{
		AsciiString upgradeName;
		UnsignedShort count;
		xferUnsignedShort(&count);
		upgradeMaskData->clear();
		for (UnsignedShort i = 0; i < count; ++i)
		{
			xferAsciiString(&upgradeName);
			const UpgradeTemplate* t = TheUpgradeCenter->findUpgrade(upgradeName);
			if (t == NULL) abort();
			upgradeMaskData->set(t->getUpgradeMask());
		}
	}
	else if (getXferMode() == XFER_CRC)
		xferImplementation(upgradeMaskData, sizeof(UpgradeMaskType));
}

/* ---- the program reader ----------------------------------------------------------------------------------- */
struct Reader
{
	const unsigned* p; const unsigned* e;
	unsigned u() { if (p >= e) abort(); return *p++; }
	int i() { return (int)u(); }
	bool b() { return u() != 0; }
	float f() { unsigned w = u(); float x; memcpy(&x, &w, 4); return x; }
	unsigned long long u64() { unsigned lo = u(); unsigned hi = u(); return ((unsigned long long)hi << 32) | lo; }
	bool more() const { return p < e; }
};

static const char* const g_moduleNames[13] = { "ProbeBody", "PhysicsBehavior", "InactiveBody", "RadarUpgrade", "PowerPlantUpgrade", "OverchargeBehavior",
	"AutoHealBehavior", "ToppleUpdate", "StickyBombUpdate", "RebuildHoleBehavior", "SquishCollide", "SpecialPowerCompletionDie", "AnyModule" };

/* the modules the next newObject's template names, in order (TheModuleFactory->newModule hands them out) */
static bool g_verbose = false;
static std::vector<BehaviorModule*> g_pendingModules;
static size_t g_pendingAt = 0;
BehaviorModule* ModuleFactory::newModule(Object* o, const AsciiString& name, const void* data, Int type)
{
	BehaviorModule* m = g_pendingModules[g_pendingAt++];
	m->m_object = o;
	return m;
}

/* ---- one world ---------------------------------------------------------------------------------------------- */
struct World
{
	std::vector<Object*> objs; std::vector<int> ids; std::vector<ThingTemplate*> templates; std::vector<UpgradeTemplate*> upgradeTemplates;
	std::vector<PolygonTrigger*> triggers; int kUpgrades;
	std::vector<Player*> players; std::vector<Team*> teams;
	World() : kUpgrades(0) {}
};
static World* g_w = NULL;

static int fnv(const std::vector<unsigned char>& v)
{
	unsigned h = 2166136261u;
	for (size_t i = 0; i < v.size(); ++i) h = (h ^ v[i]) * 16777619u;
	return (int)h;
}

static void readHeader(Reader& r)
{
	g_cfg = ObjCfg();
	World* w = g_w;
	if (r.u() != 0x4F424A31u) abort();
	int np = r.i();
	for (int p = 0; p < np; ++p)
	{
		ObjCfgPlayer c;
		c.index = p; c.defaultTeam = r.i(); c.color = r.u(); c.nightColor = r.u(); c.active = r.b(); c.playable = r.b(); c.skirmish = r.b();
		c.upgradesLo = r.u64(); c.upgradesHi = r.u64(); c.battlePlans = r.i(); c.productionLevel = r.i();
		g_cfg.players.push_back(c);
	}
	g_cfg.playerCount = np;
	int nt = r.i();
	for (int t = 0; t < nt; ++t)
	{
		ObjCfgTeam c;
		c.id = r.i(); c.player = r.i(); c.templateInfo = r.b(); c.attackInfo = r.b();
		char b[32]; snprintf(b, sizeof b, "team%d", c.id); c.name = b;
		g_cfg.teams.push_back(c);
	}
	int n = r.i(); for (int i = 0; i < n; ++i) { int a = r.i(), b = r.i(), rel = r.i(); g_cfg.teamRel[std::make_pair(a, b)] = rel; }
	n = r.i(); for (int i = 0; i < n; ++i) { int a = r.i(), b = r.i(), rel = r.i(); g_cfg.playerRel[std::make_pair(a, b)] = rel; }
	g_cfg.neutralDefaultTeam = r.i(); g_cfg.localPlayer = r.i();
	g_cfg.gravity = r.f(); g_cfg.followTimeOfDay = r.b(); g_cfg.night = r.b(); g_cfg.followWeather = r.b(); g_cfg.snowy = r.b();
	g_cfg.levelGainAnim = r.b(); g_cfg.enableRepulsors = r.b(); g_cfg.drawIconUI = r.b(); g_cfg.difficultyBonus = r.b();
	g_cfg.extentKnown = r.b(); g_cfg.extentHiX = r.f(); g_cfg.extentHiY = r.f();
	g_cfg.wallHeight = r.f();
	for (int i = 0; i < 4; ++i) g_cfg.veterancyUpgrade[i] = r.i();
	n = r.i(); for (int i = 0; i < n; ++i) g_cfg.specialPowerDenied.insert(r.i());
	n = r.i(); for (int i = 0; i < n; ++i) { int o = r.i(), s = r.i(); g_cfg.weapons.insert(std::make_pair(o, s)); }
	n = r.i(); for (int i = 0; i < n; ++i) g_cfg.weaponSetsWithWeapons.insert(r.i());
	n = r.i(); for (int i = 0; i < n; ++i) { int o = r.i(), s = r.i(), t = r.i(); g_cfg.aiTurrets[std::make_pair(o, s)] = t; }
	n = r.i(); for (int i = 0; i < n; ++i) { int o = r.i(), t = r.i(); g_cfg.turretsDisabled.insert(std::make_pair(o, t)); }
	n = r.i(); for (int i = 0; i < n; ++i) g_cfg.aiAirborne.insert(r.i());
	n = r.i(); for (int i = 0; i < n; ++i) g_cfg.dozerAIs.insert(r.i());
	n = r.i(); for (int i = 0; i < n; ++i) { int o = r.i(), t = r.i(); float f = r.f(); g_cfg.turretRotation[std::make_pair(o, t)] = f; }
	n = r.i(); for (int i = 0; i < n; ++i) { int o = r.i(), p = r.i(), s = r.i(); g_cfg.shroudedStatus[std::make_pair(o, p)] = s; }
	n = r.i(); for (int i = 0; i < n; ++i) g_cfg.potentialCollisions.push_back(r.i());
	n = r.i(); for (int i = 0; i < n; ++i) g_cfg.boobyTrapsInRange.push_back(r.i());
	w->kUpgrades = r.i();
	/* the polygon triggers */
	n = r.i();
	PolygonTrigger* prev = NULL;
	for (int i = 0; i < n; ++i)
	{
		PolygonTrigger* t = new PolygonTrigger();
		char b[32]; snprintf(b, sizeof b, "trigger%d", i); t->m_name = b;
		int np = r.i();
		for (int j = 0; j < np; ++j) { ICoord3D c; c.x = r.i(); c.y = r.i(); c.z = 0; t->m_points.push_back(c); }
		t->m_next = NULL;
		if (prev) prev->m_next = t; else PolygonTrigger::s_first = t;
		prev = t;
		w->triggers.push_back(t);
	}
}

static void buildWorld()
{
	World* w = g_w;
	/* kept across runs: the `static NameKeyType` locals of Object.cpp hold keys of the first run */
	if (!TheNameKeyGenerator) TheNameKeyGenerator = new NameKeyGenerator();
	TheAudio = new AudioManager(); TheRadar = new Radar(); TheEva = new Eva(); TheInGameUI = new InGameUI(); TheControlBar = new ControlBar();
	TheGameClient = new GameClient(); TheBuildAssistant = new BuildAssistant(); TheSpecialPowerStore = new SpecialPowerStore();
	TheUpgradeCenter = new UpgradeCenter(); TheScriptEngine = new ScriptEngine(); TheTerrainLogic = new TerrainLogic(); TheAI = new AIManager();
	ThePartitionManager = new PartitionManager(); TheGlobalData = new GlobalData(); TheModuleFactory = new ModuleFactory(); TheGameLogic = new GameLogic();
	ThePlayerList = new PlayerList(); TheTeamFactory = new TeamFactory();
	TheAnim2DCollection = g_cfg.levelGainAnim ? new Anim2DCollection() : NULL;
	TheAI->data.m_enableRepulsors = g_cfg.enableRepulsors;
	TheGlobalData->m_forceModelsToFollowTimeOfDay = g_cfg.followTimeOfDay; TheGlobalData->m_timeOfDay = g_cfg.night ? TIME_OF_DAY_NIGHT : TIME_OF_DAY_MORNING;
	TheGlobalData->m_forceModelsToFollowWeather = g_cfg.followWeather; TheGlobalData->m_weather = g_cfg.snowy ? WEATHER_SNOWY : WEATHER_NORMAL;
	TheGlobalData->m_levelGainAnimationName = g_cfg.levelGainAnim ? "LevelGain" : "";
	TheGlobalData->m_levelGainAnimationDisplayTimeInSeconds = 3.0f; TheGlobalData->m_levelGainAnimationZRisePerSecond = 12.0f; TheGlobalData->m_gravity = g_cfg.gravity;
	for (size_t i = 0; i < g_cfg.players.size(); ++i)
	{
		Player* p = new Player(); p->m_index = (Int)i; ThePlayerList->players.push_back(p); w->players.push_back(p);
	}
	for (size_t i = 0; i < g_cfg.teams.size(); ++i)
	{
		Team* t = new Team();
		t->m_id = (TeamID)g_cfg.teams[i].id; t->m_player = g_cfg.teams[i].player; t->m_name = g_cfg.teams[i].name.c_str();
		t->m_proto.m_hasInfo = g_cfg.teams[i].templateInfo; t->m_proto.m_info.m_initialTeamAttitude = g_cfg.teams[i].id;
		if (g_cfg.teams[i].attackInfo) { char b[32]; snprintf(b, sizeof b, "%d", g_cfg.teams[i].id); t->m_proto.m_attackName = b; }
		TheTeamFactory->teams[g_cfg.teams[i].id] = t; w->teams.push_back(t);
	}
	for (int k = 0; k < w->kUpgrades; ++k)
	{
		UpgradeTemplate* u = new UpgradeTemplate();
		char b[32]; snprintf(b, sizeof b, "Upgrade_%d", k); u->m_name = b; u->m_mask.set(k, 1);
		TheUpgradeCenter->list.push_back(u); w->upgradeTemplates.push_back(u);
	}
	for (int i = 0; i < WEAPONSET_COUNT; ++i) (void)TheWeaponSetTypeToModelConditionTypeMap[i];
	initKindOfMasks();
}

static const char* const g_helperTagNames[8] = { "ModuleTag_SMCHelper", "ModuleTag_StatusDamageHelper", "ModuleTag_SubdualDamageHelper", "ModuleTag_RepulsorHelper",
	"ModuleTag_DefectionHelper", "ModuleTag_WeaponStatusHelper", "ModuleTag_FiringTrackerHelper", "ModuleTag_TempWeaponBonusHelper" };

static void createObject(Reader& r)
{
	World* w = g_w;
	ThingTemplate* tt = new ThingTemplate();
	char nb[32]; snprintf(nb, sizeof nb, "Template%d", (int)w->templates.size());
	tt->name = nb; tt->templateID = (int)w->templates.size() + 1;
	int nk = r.i(); for (int i = 0; i < nk; ++i) tt->kindof.set(r.i(), 1);
	tt->vision = r.f(); tt->shroudClearing = r.f(); tt->shroudRevealToAll = r.f();
	{ int gt = r.i(); bool small = r.b(); float h = r.f(), maj = r.f(), mnr = r.f(); tt->geom.set((GeometryType)gt, small, h, maj, mnr); }
	tt->energyProduction = r.i(); tt->energyBonus = r.i(); tt->buildCost = r.i(); tt->threatValue = r.i(); tt->crusherLevel = r.i(); tt->crushableLevel = r.i();
	tt->transportSlots = r.i();
	for (int i = 0; i < 4; ++i) tt->expValues[i] = r.i();
	for (int i = 0; i < 4; ++i) tt->expRequired[i] = r.i();
	tt->trainable = r.b(); tt->enterGuard = r.b(); tt->occlusionDelay = r.u(); tt->radarPriority = (RadarPriorityType)r.i();
	int nws = r.i();
	for (int i = 0; i < nws; ++i)
	{
		WeaponTemplateSet s; int nf = r.i(); for (int j = 0; j < nf; ++j) s.m_types.set(r.i(), 1); s.m_hasWeapon = r.b();
		tt->weaponSets.push_back(s);
	}
	int teamId = r.i(); unsigned long long st = r.u64();
	ObjectStatusMaskType status;
	for (int i = 0; i < 64; ++i) if ((st >> i) & 1) status.set(i, 1);
	int nm = r.i();
	g_pendingModules.clear(); g_pendingAt = 0;
	for (int m = 0; m < nm; ++m)
	{
		int nameIdx = r.i();
		BehaviorModule* b = new BehaviorModule();
		char tag[32]; snprintf(tag, sizeof tag, "ModuleTag_%d", m);
		b->m_className = g_moduleNames[nameIdx]; b->m_tag = tag;
		b->m_classKey = NAMEKEY(g_moduleNames[nameIdx]); b->m_tagKey = NAMEKEY(tag);
		b->m_interfaces = r.u(); b->m_stealth = r.b(); b->m_ai = r.b();
		b->maxHealth = r.f(); b->health = b->maxHealth;
		int trig = r.i(); if (trig > 0) b->upgradeTrigger.set(trig - 1, 1);
		ProbeAnswers& a = b->a;
		a.yes = r.b(); a.has = r.u(); a.salvage = r.b(); a.subObjects = r.b(); a.enclosing = r.b(); a.garrisonable = r.b(); a.kickOut = r.b();
		a.passengerMayFire = r.b(); a.zeroSlot = r.b(); a.heroInside = r.b(); a.specialPowerType = r.i(); a.shortcut = r.b(); a.powerTemplate = r.u();
		a.rider = r.i(); int nc = r.i(); for (int i = 0; i < nc; ++i) a.contained.push_back(r.i());
		a.slaver = r.i(); a.deckHeight = r.f(); a.apparentPlayer = r.i(); a.countermeasure = r.i(); a.collideSetsNoCollisions = r.b();
		tt->modules.v.push_back(std::make_pair(AsciiString(g_moduleNames[nameIdx]), AsciiString(tag)));
		g_pendingModules.push_back(b);
	}
	w->templates.push_back(tt);
	Team* team = teamId ? TheTeamFactory->findTeamByID((TeamID)teamId) : NULL;
	/* ThingFactory::newObject after the template side */
	Object* obj = new Object(tt, status, team);
	for (BehaviorModule** b = obj->m_behaviors; *b; ++b)
	{
		CreateModuleInterface* c = (*b)->getCreate();
		if (c) c->onCreate();
	}
	ThePartitionManager->registerObject(obj);
	obj->initObject();
	if (g_verbose)
		for (BehaviorModule** b = obj->m_behaviors; *b; ++b)
			Rec("debug", "Module").e("class", TheNameKeyGenerator->keyToName((*b)->getModuleNameKey()).str()).e("tag", TheNameKeyGenerator->keyToName((*b)->getModuleTagNameKey()).str());
	w->objs.push_back(obj);
	w->ids.push_back((int)obj->getID());
	Rec("ret", "Created").i("idx", (long long)w->objs.size() - 1).i("id", obj->getID());
}

static Object* objAt(int k)
{
	World* w = g_w;
	if (k < 0 || k >= (int)w->objs.size()) return NULL;
	return TheGameLogic->findObjectByID((ObjectID)w->ids[k]);
}

static void snapshotAll()
{
	World* w = g_w;
	for (size_t k = 0; k < w->objs.size(); ++k)
	{
		Object* o = objAt((int)k);
		if (!o) continue;
		XferSave xs;
		xs.xferSnapshot(o);
		if (g_verbose)
		{
			std::string hex; char b[4];
			for (size_t i = 0; i < xs.bytes.size(); ++i) { snprintf(b, sizeof b, "%02x", xs.bytes[i]); hex += b; }
			Rec("bytes", "Object").i("idx", (long long)k); g_lines->back() += " " + hex;
		}
		Rec("snap", "Object").i("idx", (long long)k).i("id", o->getID()).i("len", (long long)xs.bytes.size()).i("hash", fnv(xs.bytes));
		Rec("extra", "Object").i("idx", (long long)k).b("radar", o->friend_getRadarData() != NULL).b("partition", o->friend_getPartitionData() != NULL)
			.i("contained_by", o->getContainedBy() ? o->getContainedBy()->getID() : 0).i("drawable", o->getDrawable() ? o->getDrawable()->getID() : 0)
			.i("team", o->getTeam() ? o->getTeam()->getID() : 0).b("destroyed", o->isDestroyed()).i("frame_vision", 0);
	}
}

static std::vector<Object*> g_objectsToDestroy;

void GameLogic::destroyObject(Object* obj)
{
	if (obj->isDestroyed()) return;
	for (BehaviorModule** b = obj->m_behaviors; *b; ++b)
	{
		DestroyModuleInterface* d = (*b)->getDestroy();
		if (d) d->onDestroy();
	}
	obj->setStatus(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_DESTROYED));
	if (obj->getAIUpdateInterface()) CUT("AiStopForDestroy").i("object", obj->getID());
	g_objectsToDestroy.push_back(obj);
	obj->onDestroy();
	if (obj->isKindOf(KINDOF_WALK_ON_TOP_OF_WALL)) TheAI->pathfinder()->removeWallPiece(obj);
	if (obj->hasAnySpecialPower() && TheGameLogic && ThePlayerList->getLocalPlayer() == obj->getControllingPlayer()) TheControlBar->markUIDirty();
}
static void processDestroyList()
{
	std::set<Object*> deleted;
	for (size_t i = 0; i < g_objectsToDestroy.size(); ++i)
	{
		Object* o = g_objectsToDestroy[i];
		/* an object queued twice (a module destroying its own object from onDestroy, before the destroyed bit is set) is
		** deleted twice upstream, undefined behaviour; here, as in the Rust world, the second entry is skipped */
		if (!deleted.insert(o).second) continue;
		o->removeFromList(&TheGameLogic->m_objList);
		TheGameLogic->removeObjectFromLookupTable(o);
		g_deletingId = o->getID();
		o->friend_deleteInstance();
	}
	g_objectsToDestroy.clear();
}

static SpecialPowerTemplate g_spTemplates[3];
static std::map<int, AIGroup*> g_groups;
static std::map<int, std::vector<unsigned char> > g_saved;
static int moduleIndex(Object* o, BehaviorModule* m)
{
	if (!m) return -1;
	int i = 0;
	for (BehaviorModule** b = o->m_behaviors; *b; ++b, ++i)
		if (*b == m) return i;
	return -1;
}

static void runOps(Reader& r)
{
	World* w = g_w;
	while (r.more())
	{
		int op = r.i();
		if (op == 1) { createObject(r); snapshotAll(); continue; }
		int k = r.i();
		Object* o = objAt(k);
		/* the arguments are always read, so that both sides stay in step when the object is gone */
		switch (op)
		{
		case 2: { unsigned long long m = r.u64(); bool set = r.b(); if (!o) break; ObjectStatusMaskType mask; for (int i = 0; i < 64; ++i) if ((m >> i) & 1) mask.set(i, 1); o->setStatus(mask, set); break; }
		case 3: { int bit = r.i(); bool set = r.b(); if (!o) break; o->setScriptStatus((ObjectScriptStatusBit)(1 << bit), set); break; }
		case 4: { int t = r.i(); unsigned f = r.u(); if (!o) break; o->setDisabledUntil((DisabledType)t, f); break; }
		case 5: { int t = r.i(); if (!o) break; o->setDisabled((DisabledType)t); break; }
		case 6: { int t = r.i(); if (!o) break; bool res = o->clearDisabled((DisabledType)t); Rec("ret", "ClearDisabled").b("result", res); break; }
		case 7: { if (!o) break; o->checkDisabledStatus(); break; }
		case 8: { unsigned f = r.u(); TheGameLogic->m_frame = f; break; }
		case 9: { int wst = r.i(); bool set = r.b(); if (!o) break; if (set) o->setWeaponSetFlag((WeaponSetType)wst); else o->clearWeaponSetFlag((WeaponSetType)wst); break; }
		case 10: { int wbc = r.i(); bool set = r.b(); if (!o) break; if (set) o->setWeaponBonusCondition((WeaponBonusConditionType)wbc); else o->clearWeaponBonusCondition((WeaponBonusConditionType)wbc); break; }
		case 11: { int ast = r.i(); int mode = r.i(); if (!o) break; if (!o->getBodyModule()) break; if (mode == 0) o->setArmorSetFlag((ArmorSetType)ast); else if (mode == 1) o->clearArmorSetFlag((ArmorSetType)ast); else { bool res = o->testArmorSetFlag((ArmorSetType)ast); Rec("ret", "TestArmorSetFlag").b("result", res); } break; }
		case 12: { int team = r.i(); int mode = r.i(); if (!o) break; Team* t = team ? TheTeamFactory->findTeamByID((TeamID)team) : NULL; if (mode == 0) o->setTeam(t); else if (mode == 1) o->setTemporaryTeam(t); else o->restoreOriginalTeam(); break; }
		case 13: { int team = r.i(); unsigned time = r.u(); if (!o) break; Team* t = TheTeamFactory->findTeamByID((TeamID)team); if (t) o->defect(t, time); break; }
		case 14: { unsigned time = r.u(); if (!o) break; o->goInvulnerable(time); break; }
		case 15:
		{
			float amount = r.f(); int dtype = r.i(); bool kill = r.b(); int src = r.i(); float swAmount = r.f(), swRadius = r.f(), taper = r.f(), vx = r.f(), vy = r.f(), vz = r.f(); unsigned smask = r.u();
			if (!o || !o->getBodyModule()) break;
			DamageInfo info; info.in.m_amount = amount; info.in.m_damageType = (DamageType)dtype; info.in.m_kill = kill; info.in.m_sourceID = (ObjectID)src;
			info.in.m_shockWaveAmount = swAmount; info.in.m_shockWaveRadius = swRadius; info.in.m_shockWaveTaperOff = taper;
			info.in.m_shockWaveVector.x = vx; info.in.m_shockWaveVector.y = vy; info.in.m_shockWaveVector.z = vz; info.in.m_sourcePlayerMask = (PlayerMaskType)smask;
			o->attemptDamage(&info);
			Rec("ret", "Damage").f("dealt", info.out.m_actualDamageDealt).f("clipped", info.out.m_actualDamageClipped).b("no_effect", info.out.m_noEffect);
			break;
		}
		case 16: { float amount = r.f(); int src = r.i(); if (!o) break; Object* s = objAt(src - 1); o->attemptHealing(amount, s); break; }
		case 17: { int dt = r.i(); int death = r.i(); if (!o || !o->getBodyModule()) break; o->kill((DamageType)dt, (DeathType)death); break; }
		case 18: { if (!o) break; o->healCompletely(); break; }
		case 19: { float amount = r.f(); int dt = r.i(); if (!o) break; DamageInfoInput in; in.m_amount = amount; in.m_damageType = (DamageType)dt; float res = o->estimateDamage(in); Rec("ret", "EstimateDamage").f("result", res); break; }
		case 20: { Coord3D p; p.x = r.f(); p.y = r.f(); p.z = r.f(); if (!o) break; o->setPosition(&p); break; }
		case 21: { float a = r.f(); if (!o) break; o->setOrientation(a); break; }
		case 22: { float z = r.f(); if (!o) break; o->setPositionZ(z); break; }
		case 23:
		{
			int kind = r.i(); int v = r.i(); if (!o) break;
			ExperienceTracker* x = o->getExperienceTracker();
			if (kind == 0) x->addExperiencePoints(v); else if (kind == 1) x->setVeterancyLevel((VeterancyLevel)v); else if (kind == 2) x->setExperienceAndLevel(v);
			else if (kind == 3) x->setMinVeterancyLevel((VeterancyLevel)v); else if (kind == 4) x->setExperienceSink((ObjectID)v);
			break;
		}
		case 24: { int victim = r.i(); if (!o) break; Object* v = objAt(victim); if (v) o->scoreTheKill(v); break; }
		case 25: { int bit = r.i(); if (!o) break; o->giveUpgrade(g_w->upgradeTemplates[bit]); break; }
		case 26: { int bit = r.i(); if (!o) break; o->removeUpgrade(g_w->upgradeTemplates[bit]); break; }
		case 27: { if (!o) break; o->updateUpgradeModules(); break; }
		case 28: { int bit = r.i(); if (!o) break; bool h = o->hasUpgrade(g_w->upgradeTemplates[bit]); bool a = o->affectedByUpgrade(g_w->upgradeTemplates[bit]); Rec("ret", "Upgrade").b("has", h).b("affected", a); break; }
		case 29: { int by = r.i(); if (!o) break; Object* c = objAt(by); if (c && c->getContain() && !c->isDestroyed()) o->onContainedBy(c); /* not into a container that is going away */ break; }
		case 30: { if (!o) break; o->onRemovedFrom(o->getContainedBy()); break; }
		case 31: { int kind = r.i(); float v = r.f(); if (!o) break; if (kind == 0) o->setVisionRange(v); else if (kind == 1) o->setShroudClearingRange(v); else o->setShroudRange(v); break; }
		case 32: { bool setting = r.b(); int player = r.i(); if (!o) break; o->setVisionSpied(setting, player); break; }
		case 33: { if (!o) break; o->handlePartitionCellMaintenance(); break; }
		case 34: { bool s = r.b(); if (!o) break; o->setSelectable(s); break; }
		case 35: { int mc = r.i(); unsigned frames = r.u(); if (!o) break; o->setSpecialModelConditionState((ModelConditionFlagType)mc, frames); break; }
		case 36: { if (!o) break; o->clearSpecialModelConditionStates(); break; }
		case 37:
		{
			if (!o) break;
			/* what a contain module does in onDelete: let go of the objects it holds (a pointer to a deleted container
			** dangles upstream) */
			for (size_t j = 0; j < w->ids.size(); ++j)
			{
				Object* other = objAt((int)j);
				if (other && other->getContainedBy() == o) other->onRemovedFrom(o);
			}
			TheGameLogic->destroyObject(o);
			break;
		}
		case 38: { processDestroyList(); break; }
		case 39: { int other = r.i(); if (!o) break; Object* t = objAt(other); Coord3D loc; loc.x = 1; loc.y = 2; loc.z = 3; if (t) o->onCollide(t, &loc, &loc); break; }
		case 41: { bool m = r.b(); if (!o) break; o->maskObject(m); break; }
		case 42: { bool d = r.b(); if (!o) break; o->setEffectivelyDead(d); break; }
		case 43: { bool c = r.b(); if (!o) break; o->setCaptured(c); break; }
		case 44: { if (!o) break; XferCRC x; x.xferSnapshot(o); Rec("ret", "Crc").i("crc", (long long)x.getCRC()); break; }
		case 45: { if (!o) break; bool res = o->isAbleToAttack(); Rec("ret", "AbleToAttack").b("result", res); break; }
		case 46: { if (!o) break; bool res = o->isHero(); Rec("ret", "IsHero").b("result", res); break; }
		case 47: { if (!o) break; int res = o->getTransportSlotCount(); Rec("ret", "TransportSlots").i("result", res); break; }
		case 48:
		{
			int kind = r.i(); int tmpl = r.i(); bool forced = r.b(); unsigned opts = r.u(); int target = r.i();
			Coord3D loc; loc.x = r.f(); loc.y = r.f(); loc.z = r.f(); float angle = r.f();
			if (!o) break;
			const SpecialPowerTemplate* t = tmpl ? &g_spTemplates[tmpl] : NULL;
			if (kind == 0) o->doSpecialPower(t, opts, forced);
			else if (kind == 1) o->doSpecialPowerAtObject(t, objAt(target), opts, forced);
			else if (kind == 2) o->doSpecialPowerAtLocation(t, &loc, angle, opts, forced);
			else { static Waypoint way; o->doSpecialPowerUsingWaypoints(t, &way, opts, forced); }
			break;
		}
		case 49:
		{
			int kind = r.i(); int type = r.i(); int tmpl = r.i();
			if (!o) break;
			BehaviorModule* m = NULL;
			switch (kind)
			{
			case 0: m = o->findSpecialPowerModuleInterface((SpecialPowerType)type); break;
			case 1: m = o->findAnyShortcutSpecialPowerModuleInterface(); break;
			case 2: m = o->findSpecialAbilityUpdate((SpecialPowerType)type); break;
			case 3: m = o->findSpecialPowerCompletionDie(); break;
			case 4: m = o->findSpecialPowerWithOverridableDestinationActive((SpecialPowerType)type); break;
			case 5: m = o->findSpecialPowerWithOverridableDestination((SpecialPowerType)type); break;
			case 6: m = o->getProductionUpdateInterface(); break;
			case 7: m = o->getDockUpdateInterface(); break;
			case 8: m = o->getSpawnBehaviorInterface(); break;
			case 9: m = o->getProjectileUpdateInterface(); break;
			case 10: m = o->getCountermeasuresBehaviorInterface(); break;
			case 11: m = o->getObjectExitInterface(); break;
			default: m = o->getSpecialPowerModule(tmpl ? &g_spTemplates[tmpl] : NULL); break;
			}
			Rec("ret", "Find").i("kind", kind).i("index", kind == 11 ? (m ? 0 : -1) : moduleIndex(o, m)); /* the Rust side answers whether the exit interface exists */
			break;
		}
		case 50: { Coord3D d; d.x = r.f(); d.y = r.f(); d.z = r.f(); float speed = r.f(); unsigned opts = r.u(); if (!o) break; o->topple(&d, speed, opts); break; }
		case 51: { int other = r.i(); int tt = r.i(); if (!o) break; Object* t = objAt(other); bool res = o->canCrushOrSquish(t, (CrushSquishTestType)tt); Rec("ret", "Crush").b("result", res); break; }
		case 52:
		{
			if (!o) break;
			int pr = o->getRadarPriority();
			Coord3D pos; o->getHealthBoxPosition(pos);
			Real h = 0, wd = 0; bool dims = o->getHealthBoxDimensions(h, wd);
			Rec("ret", "HealthBox").i("priority", pr).f("x", pos.x).f("y", pos.y).f("z", pos.z).b("dims", dims).f("height", h).f("width", wd);
			break;
		}
		case 54:
		{
			int kind = r.i(); int gt = r.i(); bool small = r.b(); float h = r.f(), maj = r.f(), mnr = r.f(); float z = r.f();
			if (!o) break;
			if (kind == 0) { GeometryInfo g((GeometryType)gt, small, h, maj, mnr); o->setGeometryInfo(g); }
			else o->setGeometryInfoZ(z);
			break;
		}
		case 55:
		{
			int kind = r.i(); unsigned color = r.u();
			if (!o) break;
			if (kind == 0) o->setCustomIndicatorColor(color);
			else if (kind == 1) o->removeCustomIndicatorColor();
			else Rec("ret", "Indicator").i("color", (long long)o->getIndicatorColor()).i("night", (long long)o->getNightIndicatorColor()).b("custom", o->hasCustomIndicatorColor());
			break;
		}
		case 56:
		{
			int kind = r.i(); int g = r.i();
			if (!o) break;
			if (kind == 0) { AIGroup*& grp = g_groups[g]; if (!grp) { grp = new AIGroup(); grp->m_id = (UnsignedInt)g; } o->enterGroup(grp); }
			else o->leaveGroup();
			break;
		}
		case 57: { int which = r.i(); int obj = r.i(); if (!o) break; Object* t = obj ? objAt(obj - 1) : NULL; if (which == 0) o->setProducer(t); else o->setBuilder(t); break; }
		case 58: { int kind = r.i(); int layer = r.i(); if (!o) break; if (kind == 0) o->setLayer((PathfindLayerEnum)layer); else o->setDestinationLayer((PathfindLayerEnum)layer); break; }
		case 59: { int kind = r.i(); if (!o) break; if (kind == 0) o->friend_prepareForMapBoundaryAdjust(); else o->friend_notifyOfNewMapBoundary(); break; }
		case 60: { bool incoming = r.b(); if (!o) break; o->friend_adjustPowerForPlayer(incoming); break; }
		case 61: { bool receive = r.b(); if (!o) break; o->setReceivingDifficultyBonus(receive); break; }
		case 62: { int oldP = r.i(), newP = r.i(); if (!o) break; o->onCapture(ThePlayerList->getNthPlayer(oldP), ThePlayerList->getNthPlayer(newP)); break; }
		case 63:
		{
			float amount = r.f(); int src = r.i(); unsigned duration = r.u();
			if (!o) break;
			Object* s = src ? objAt(src - 1) : NULL;
			bool res = o->attemptHealingFromSoleBenefactor(amount, s, duration);
			Rec("ret", "SoleHeal").b("result", res).i("benefactor", o->getSoleHealingBenefactor());
			break;
		}
		case 64:
		{
			int kind = r.i(); int other = r.i();
			if (!o) break;
			Object* t = objAt(other);
			if (kind == 0) { bool res = o->hasCountermeasures(); Rec("ret", "HasCountermeasures").b("result", res); }
			else if (kind == 1) { if (t) o->reportMissileForCountermeasures(t); }
			else if (t) { ObjectID id = o->calculateCountermeasureToDivertTo(*t); Rec("ret", "Divert").i("id", id); }
			break;
		}
		case 65: { Matrix3D m; for (int i = 0; i < 3; ++i) for (int j = 0; j < 4; ++j) m.Row[i][j] = r.f(); if (!o) break; o->setTransformMatrix(&m); break; }
		case 66:
		{
			int kind = r.i(); int trig = r.i();
			if (!o) break;
			const PolygonTrigger* t = trig < (int)g_w->triggers.size() ? g_w->triggers[trig] : NULL;
			bool res = kind == 0 ? o->didEnter(t) : kind == 1 ? o->didExit(t) : o->isInside(t);
			Rec("ret", "Trigger").i("kind", kind).b("result", res);
			break;
		}
		case 67:
		{
			if (!o) break;
			Coord2D pt; o->calcNaturalRallyPoint(&pt);
			Real deck = o->getCarrierDeckHeight();
			Rec("ret", "Rally").f("x", pt.x).f("y", pt.y).f("deck", deck);
			break;
		}
		case 68: { int victim = r.i(); if (!o) break; Object* v = victim ? objAt(victim - 1) : NULL; bool res = o->checkAndDetonateBoobyTrap(v); Rec("ret", "Booby").b("result", res); break; }
		case 69:
		{
			int kind = r.i(); float amount = r.f(); int status = r.i(); unsigned dur = r.u();
			if (!o) break;
			if (kind == 0) o->notifySubdualDamage(amount);
			else if (kind == 1) o->doStatusDamage((ObjectStatusTypes)status, amount);
			else o->doTempWeaponBonus((WeaponBonusConditionType)status, dur);
			break;
		}
		case 70: { int turret = r.i(); float oldRot = r.f(); if (!o) break; o->reactToTurretChange((WhichTurretType)turret, oldRot, 0.0f); break; }
		case 71:
		{
			int other = r.i(); int player = r.i(); int dtype = r.i();
			if (!o) break;
			Object* t = objAt(other);
			Rec q("ret", "Queries");
			q.b("mobile", o->isMobile()).b("selectable", o->isSelectable()).b("mass", o->isMassSelectable()).b("local", o->isLocallyControlled()).b("neutral", o->isNeutralControlled())
				.i("relationship", t ? (long long)o->getRelationship(t) : -1).b("faction", o->isFactionStructure()).b("nonfaction", o->isNonFactionStructure())
				.i("shrouded", (long long)o->getShroudedStatus(player)).b("airborne", o->isUsingAirborneLocomotor()).i("level", (long long)o->getVeterancyLevel())
				.f("vision", o->getVisionRange()).f("clearing", o->getShroudClearingRange()).f("shroud", o->getShroudRange())
				.i("disabled_until", (long long)o->getDisabledUntil((DisabledType)dtype)).i("disabled_any", (long long)o->getDisabledUntil(DISABLED_ANY)).b("salvage", o->isSalvageCrate())
				.b("contained", o->isContained()).b("construction_complete", o->getConstructionPercent() == CONSTRUCTION_COMPLETE).b("effectively_dead", o->isEffectivelyDead())
				.b("captured", o->isCaptured()).b("off_map", o->isOffMap()).b("undetected_defector", o->getIsUndetectedDefector()).b("has_special", o->hasAnySpecialPower())
				.f("height", o->getHeightAboveTerrain()).b("hero", false).i("crusher", o->getCrusherLevel()).i("crushable", o->getCrushableLevel())
				.b("airborne_target", o->isAirborneTarget()).b("destroyed", o->isDestroyed()).b("modules_ready", o->areModulesReady()).i("script_status", o->testScriptStatusBit((ObjectScriptStatusBit)1) ? 1 : 0)
				.i("weapon_set", (long long)o->getWeaponSetFlags().m_bits.to_ullong()).i("weapon_bonus", (long long)o->getWeaponBonusCondition()).b("single_use", o->hasSingleUseCommandBeenUsed());
			break;
		}
		case 72: { if (!o) break; XferSave xs; xs.xferSnapshot(o); g_saved[k] = xs.bytes; break; }
		case 73:
		{
			if (!o) break;
			std::map<int, std::vector<unsigned char> >::iterator it = g_saved.find(k);
			if (it == g_saved.end()) break;
			XferLoad xl; xl.bytes = it->second;
			xl.xferSnapshot(o);
			o->loadPostProcess();
			break;
		}
		default: abort();
		}
		Rec("end", "Op").i("op", op).i("object", k);
		snapshotAll();
	}
}

/* Program: [header][ops]. ref_object_run writes the lines (separated by '\n') to out and returns their length,
** or -(length) when the buffer is too small. `names`: the four flag name tables, each "count\nname\n..." */
extern "C" int ref_object_run(const unsigned* prog, int nprog, const char* names, int nnames, char* out, int cap, int verbose)
{
	g_verbose = verbose != 0;
	std::vector<std::string> lines;
	g_lines = &lines;
	/* the name tables */
	static std::vector<std::string> store[4];
	static std::vector<const char*> ptrs[4];
	{
		const char* p = names; const char* end = names + nnames;
		for (int t = 0; t < 4; ++t)
		{
			store[t].clear(); ptrs[t].clear();
			int count = atoi(p); while (p < end && *p != '\n') ++p; ++p;
			for (int i = 0; i < count; ++i) { const char* q = p; while (p < end && *p != '\n') ++p; store[t].push_back(std::string(q, p - q)); ++p; }
			for (size_t i = 0; i < store[t].size(); ++i) ptrs[t].push_back(store[t][i].c_str());
			g_flagNames[t] = &ptrs[t][0]; g_flagNameCount[t] = (int)ptrs[t].size();
		}
	}
	World world;
	g_w = &world;
	for (int i = 0; i < 3; ++i) g_spTemplates[i].m_id = i;
	g_groups.clear();
	g_saved.clear();
	PolygonTrigger::s_first = NULL;
	g_objectsToDestroy.clear();
	Reader r; r.p = prog; r.e = prog + nprog;
	try
	{
		readHeader(r);
		buildWorld();
		runOps(r);
	}
	catch (SaveCode c)
	{
		char b[64]; snprintf(b, sizeof b, "EXCEPTION SaveCode %d", (int)c); lines.push_back(b);
	}
	catch (std::exception& e)
	{
		lines.push_back(std::string("EXCEPTION ") + e.what());
	}
	std::string all;
	for (size_t i = 0; i < lines.size(); ++i) { all += lines[i]; all += '\n'; }
	g_lines = NULL;
	g_w = NULL;
	if ((int)all.size() > cap) return -(int)all.size();
	memcpy(out, all.data(), all.size());
	return (int)all.size();
}

#endif
