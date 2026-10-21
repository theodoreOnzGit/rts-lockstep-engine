/* Differential-test driver for the layer-9 special power modules (p1w11-sonnet, special power batch H; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/{SpecialPowerModule, CleanupAreaPower, BaikonurLaunchPower, SpyVisionSpecialPower,
**   DefectorSpecialPower}.cpp   (#included, unchanged)
** with their headers unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_replace_object.inc, whose thing factory it
** uses for the view object and the detonation object, and l9_helpers.inc) on the stand-ins of differential/shim_module: three players (the shared
** special power timers, the academy stats, the relationship to the object's team, a default team), an Eva that logs, an in-game UI that logs the
** superweapons, a script engine that logs, an audio manager that logs (l9_dies_d.inc), the update modules the powers look for (CleanupHazardUpdate,
** SpyVisionUpdate, and a SpecialPowerUpdateInterface behaviour), a contain with a count, and the REAL SpecialPowerStore and INI (the template
** field is read through the hook of wrap_ini.cpp, as in batch C). Every call is a line of the effect log (ulog). */
#include "SpecialPowerModule_nokey.cpp"        /* (build.rs: the function-local static name key made a plain local) */
#include "CleanupAreaPower_nokey.cpp"        /* (build.rs: the function-local static name key made a plain local) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/BaikonurLaunchPower.cpp"
#include "SpyVisionSpecialPower_nokey.cpp"        /* (build.rs: the function-local static name key made a plain local) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/DefectorSpecialPower.cpp"

namespace {

/* the shared special power timers of the three players, by template id */
std::map<unsigned, UnsignedInt> g_spTimers[3];
Player *g_spLocal = 0;
Team *g_spObjectTeam = (Team *)(size_t)100;
GlobalData g_spGlobals;
int g_spRelations[3];            /* each player's relationship to the object's team (0 enemies, 1 neutral, 2 allies) */
bool g_spCleanupPresent = false;
bool g_spSpyPresent = false;
bool g_spLogRemove = false;

Eva g_spEva;
PlayerList g_spList;
ScriptEngine g_spScript;
InGameUI g_spUi;

class SpCleanup { public: int m_dummy; };
class SpSpy { public: int m_dummy; };
SpCleanup g_spCleanup;
SpSpy g_spSpy;

class SpContain : public UpgContain {
public:
	SpContain() : m_count(0) {}
	virtual UnsignedInt getContainCount() const { return m_count; }
	UnsignedInt m_count;
};
SpContain g_spContain;

/* a behaviour with a SpecialPowerUpdateInterface: whether it passes the science test and takes the intent */
class SpuBehavior : public TestBehavior, public SpecialPowerUpdateInterface {
public:
	SpuBehavior(Thing *t, const ModuleData *d) : TestBehavior(t, d), m_pass(false), m_take(false) {}
	virtual SpecialPowerUpdateInterface *getSpecialPowerUpdateInterface() { return this; }
	virtual Bool doesSpecialPowerUpdatePassScienceTest() const { return m_pass; }
	virtual ScienceType getExtraRequiredScience() const { return SCIENCE_INVALID; }
	virtual Bool initiateIntentToDoSpecialPower(const SpecialPowerTemplate *specialPowerTemplate, const Object *targetObj, const Coord3D *targetPos, const Waypoint *way, UnsignedInt commandOptions)
	{
		ulog("intent %u %d %d %d %u", ref_sp_raw_id(specialPowerTemplate), targetObj ? (int)targetObj->m_oid : 0, targetPos ? 1 : 0, way ? 1 : 0, commandOptions);
		return m_take;
	}
	virtual Bool isSpecialAbility() const { return false; }
	virtual Bool isSpecialPower() const { return true; }
	virtual Bool isActive() const { return false; }
	virtual CommandOption getCommandOption() const { return (CommandOption)0; }
	virtual Bool doesSpecialPowerHaveOverridableDestinationActive() const { return false; }
	virtual Bool doesSpecialPowerHaveOverridableDestination() const { return false; }
	virtual void setSpecialPowerOverridableDestination(const Coord3D *loc) {}
	virtual Bool isPowerCurrentlyInUse(const CommandButton *command = NULL) const { return false; }
	bool m_pass, m_take;
};

UpdateModule *spFindHook(const Object *o, NameKeyType key)
{
	if (o->m_oid != 1) return 0;
	if (g_spCleanupPresent && key == TheNameKeyGenerator->nameToKey("CleanupHazardUpdate")) return (UpdateModule *)&g_spCleanup;
	if (g_spSpyPresent && key == TheNameKeyGenerator->nameToKey("SpyVisionUpdate")) return (UpdateModule *)&g_spSpy;
	return 0;
}

ModuleData *l9spRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			switch (kind) {
			case 0: data = SpecialPowerModule::friend_newModuleData(&ini); break;
			case 1: data = CleanupAreaPower::friend_newModuleData(&ini); break;
			case 2: data = BaikonurLaunchPower::friend_newModuleData(&ini); break;
			case 3: data = SpyVisionSpecialPower::friend_newModuleData(&ini); break;
			default: data = DefectorSpecialPower::friend_newModuleData(&ini); break;
			}
		} catch (...) {
			char buff[1024];
			sprintf(buff, "Error parsing INI file '%s' (Line: '%s')\n", ini.m_filename.str(), currentLine.str());
			throw INIException(buff);
		}
	} catch (...) {
		ini.unPrepFile();
		throw;
	}
	ini.unPrepFile();
	return data;
}

std::vector<unsigned char> g_spSaved;

}  // namespace

GlobalData *TheGlobalData = &g_spGlobals;
Eva *TheEva = 0;
PlayerList *ThePlayerList = 0;
void Eva::setShouldPlay(EvaMessage m) { ulog("eva %d", (int)m); }
Player *PlayerList::getLocalPlayer() { return g_spLocal; }
void ScriptEngine::notifyOfTriggeredSpecialPower(Int playerIndex, const AsciiString &completedPower, ObjectID sourceObj) { ulog("script %d %s %d", (int)playerIndex, completedPower.str(), (int)sourceObj); }
void InGameUI::addSuperweapon(Int playerIndex, const AsciiString &powerName, ObjectID id, const SpecialPowerTemplate *powerTemplate)
{
	ulog("addsuper %d %s %d %u", (int)playerIndex, powerName.str(), (int)id, ref_sp_raw_id(powerTemplate));
}
Bool InGameUI::removeSuperweapon(Int playerIndex, const AsciiString &powerName, ObjectID id, const SpecialPowerTemplate *powerTemplate)
{
	if (g_spLogRemove) ulog("removesuper %d %s %d %u", (int)playerIndex, powerName.str(), (int)id, ref_sp_raw_id(powerTemplate));
	return TRUE;
}
void Player::expressSpecialPowerReadyFrame(const SpecialPowerTemplate *temp, UnsignedInt frame) { g_spTimers[m_idx][ref_sp_raw_id(temp)] = frame; }
UnsignedInt Player::getOrStartSpecialPowerReadyFrame(const SpecialPowerTemplate *temp)
{
	std::map<unsigned, UnsignedInt>::iterator it = g_spTimers[m_idx].find(ref_sp_raw_id(temp));
	if (it != g_spTimers[m_idx].end()) return it->second;
	UnsignedInt now = TheGameLogic->getFrame();
	g_spTimers[m_idx][ref_sp_raw_id(temp)] = now;
	return now;
}
void Player::resetOrStartSpecialPowerReadyFrame(const SpecialPowerTemplate *temp)
{
	std::map<unsigned, UnsignedInt>::iterator it = g_spTimers[m_idx].find(ref_sp_raw_id(temp));
	UnsignedInt now = TheGameLogic->getFrame();
	if (it != g_spTimers[m_idx].end()) it->second = now + temp->getReloadTime();
	else g_spTimers[m_idx][ref_sp_raw_id(temp)] = now;
}
Relationship Player::getRelationship(const Team *that) const { return (Relationship)g_spRelations[m_idx]; }
void PowerAcademyStats::recordSpecialPowerUsed(const SpecialPowerTemplate *t) { ulog("academy %d %u", (int)((Player *)this)->m_idx, ref_sp_raw_id(t)); }
void Object::defect(Team *newTeam, UnsignedInt detectionTime) { ulog("defect %d %d %u", (int)m_oid, (int)(size_t)newTeam, detectionTime); }
void Object::setShroudClearingRange(Real r) { m_shroudRange = r; }
void CleanupHazardUpdate::setCleanupAreaParameters(const Coord3D *pos, Real range) { ulog("cleanup %u,%u,%u %u", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(range)); }
void SpyVisionUpdate::activateSpyVision(UnsignedInt duration) { ulog("spyvision %u", duration); }

extern "C" {

/* Read `filename` (block `D`) as the data of power module `kind` (0 SpecialPowerModule, 1 CleanupAreaPower, 2 BaikonurLaunchPower, 3 SpyVisionSpecialPower,
** 4 DefectorSpecialPower) and make the module on an Object (id 1, team 100) beside a second object (id 2). `cfg`: [the controlling player of both (0xFFFFFFFF
** none), the local player, flags (bit 0 disabled after the constructor, 1 under construction, 2 a structure, 3 has a contain, 4 has a SpyVisionUpdate, 5 has a CleanupHazardUpdate,
** 6 has a special power update behaviour that passes the science test, 7 and takes the intent, 8 the view object RO_A is defined, 9 the global view object
** name is RO_A, 10 the object has a drawable after the constructor), the contain count, the players' relationships to the object's team (2 bits each), the first new object's id, the game logic's frame]. The
** three real special powers SP_0..SP_3 are the real store's (the test loaded them). Then `n` ops of 6 words [op, a, b, c, d]: 0 doSpecialPower(options a)
** 1 doSpecialPowerAtObject(object 2, options a)  2 doSpecialPowerAtLocation((a, b, c) float bits, options d)  3 doSpecialPowerUsingWaypoints(a)  4 pauseCountdown(a & 1)
** 5 onSpecialPowerCreation  6 setReadyFrame(a)  7 markSpecialPowerTriggered(a != 0: location (b, c, d))  8 startPowerRecharge  9 queries only  10 the frame = a
** 11 the objects' disabled = a & 1 and controlling player = b - 1 (0 none)  12 xfer save  13 xfer load of the last save  14 xfer CRC  15 loadPostProcess
** 16 the local player = a - 1 (0 none)  17 the relationships = a (2 bits each). Each op appends: the CATCH_ALL status, isReady, getPercentReady (bits),
** getReadyFrame, isScriptOnly, the effect log (count, then each as a string), the xfer status, bytes and CRC, the module's four state words, and the
** objects the module made (count, then id, team, position and shroud range bits of each). The constructor's effect log (count, then each as a string) comes
** before the first op. */
int ref_l9sp_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_spSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	ref_ini_sp_bind(ref_sp_find_raw);
	try { d = l9spRead(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_ini_sp_bind(0);
	if (status) return status;
	{
		const SpecialPowerModuleData *sd = (const SpecialPowerModuleData *)d;
		putU32(ref_sp_raw_id(sd->m_specialPowerTemplate));
		putU32(sd->m_updateModuleStartsAttack ? 1 : 0);
		putU32(sd->m_startsPaused ? 1 : 0);
		putU32(sd->m_scriptedSpecialPowerOnly ? 1 : 0);
		putStr(sd->m_initiateSound.getEventName().str(), (int)sd->m_initiateSound.getEventName().getLength());
		switch (kind) {
		case 1: putU32(realBits(((CleanupAreaPowerModuleData *)d)->m_cleanupMoveRange)); break;
		case 2: { const AsciiString &s = ((BaikonurLaunchPowerModuleData *)d)->m_detonationObject; putStr(s.str(), (int)s.getLength()); break; }
		case 3: {
			const SpyVisionSpecialPowerModuleData *sv = (const SpyVisionSpecialPowerModuleData *)d;
			putU32(sv->m_baseDurationInFrames); putU32(sv->m_bonusDurationPerCapturedInFrames); putU32(sv->m_maxDurationInFrames);
			break;
		}
		case 4: putU32(realBits(((DefectorSpecialPowerModuleData *)d)->m_fatCursorRadius)); break;
		default: break;
		}
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_defaultTeam = (Team *)(size_t)(101 + i);
		g_spTimers[i].clear();
		g_roPlayers[i] = 0;
	}
	for (int t = 0; t < 256; ++t) g_roTeamPlayer[t] = -1;
	g_spLocal = (int)cfg[1] >= 0 && cfg[1] < 3 ? &players[cfg[1]] : 0;
	for (int i = 0; i < 3; ++i) g_spRelations[i] = (cfg[4] >> (2 * i)) & 3;
	g_spCleanupPresent = (cfg[2] >> 5 & 1) != 0;
	g_spSpyPresent = (cfg[2] >> 4 & 1) != 0;
	g_spContain.m_count = cfg[3];
	g_findUpdateHook = spFindHook;
	g_roTmpl[0].defined = (cfg[2] >> 8) & 1;
	g_roTmpl[0].n = 0;
	g_roTmpl[1].defined = 0;
	g_roTmpl[2].defined = 0;
	g_roObjects.clear();
	g_roModules.clear();
	g_roTemplateOf.clear();
	g_roNextId = (int)cfg[5];
	TheGameLogic = 0;
	GameLogic gl;
	gl.m_frame = cfg[6];
	TheGameLogic = &gl;
	TheAudio = &g_ddAudio;
	TheEva = &g_spEva;
	ThePlayerList = &g_spList;
	TheScriptEngine = &g_spScript;
	TheInGameUI = &g_spUi;
	ThingFactory factory;
	TheThingFactory = &factory;
	g_modDestroyHook = 0;
	g_spGlobals.m_specialPowerViewObjectName = (cfg[2] >> 9 & 1) ? AsciiString("RO_A") : AsciiString();
	Object *obj = newObject();
	obj->m_oid = 1;
	Object *other = newObject();
	other->m_oid = 2;
	Object *objs[2] = { obj, other };
	for (int k = 0; k < 2; ++k) {
		Object *o = objs[k];
		o->m_team = g_spObjectTeam;
		o->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
		o->m_disabled = false;     /* (a new object is never disabled: bit 0 of the flags is applied after the constructor) */
		if (cfg[2] & 2) { ObjectStatusMaskType sm; sm.set(OBJECT_STATUS_UNDER_CONSTRUCTION); o->m_status = sm; }
		if (cfg[2] & 4) o->m_kindOf.set(KINDOF_STRUCTURE);
	}
	obj->m_pos.x = 11.0f; obj->m_pos.y = 22.0f; obj->m_pos.z = 5.0f;
	other->m_pos.x = -3.0f; other->m_pos.y = 8.0f; other->m_pos.z = 1.5f;
	obj->m_contain = (cfg[2] >> 3 & 1) ? &g_spContain : 0;
	UpdateModuleData behaviorData;
	SpuBehavior *spu = 0;
	BehaviorModule *list[2] = { 0, 0 };
	if (cfg[2] >> 6 & 1) {
		spu = new SpuBehavior(obj, &behaviorData);
		spu->m_pass = true;
		spu->m_take = (cfg[2] >> 7 & 1) != 0;
		list[0] = spu;
	}
	obj->m_behaviors = list;
	g_events.clear();
	g_ulog.clear();
	SpecialPowerModule *m = 0;
	switch (kind) {
	case 0: m = new SpecialPowerModule(obj, d); break;
	case 1: m = new CleanupAreaPower(obj, d); break;
	case 2: m = new BaikonurLaunchPower(obj, d); break;
	case 3: m = new SpyVisionSpecialPower(obj, d); break;
	default: m = new DefectorSpecialPower(obj, d); break;
	}
	{
		/* the constructor's effects, then the objects it made */
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100 && g_ulog[(size_t)e.p].compare(0, 9, "newobject") != 0) log.push_back(g_ulog[(size_t)e.p]);   /* (the thing factory's own line is not the module's) */
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	}
	g_spLogRemove = false;
	for (int k = 0; k < 2; ++k) objs[k]->m_disabled = (cfg[2] & 1) != 0;
	Drawable drawable;
	obj->m_drawable = (cfg[2] >> 10 & 1) ? &drawable : 0;     /* (bound after the constructor, whose effects are logged before) */
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		try {
			switch (o[0]) {
			case 0: m->doSpecialPower(o[1]); break;
			case 1: m->doSpecialPowerAtObject(other, o[1]); break;
			case 2: { Coord3D loc; memcpy(&loc.x, &o[1], 4); memcpy(&loc.y, &o[2], 4); memcpy(&loc.z, &o[3], 4); m->doSpecialPowerAtLocation(&loc, 0.0f, o[4]); break; }
			case 3: m->doSpecialPowerUsingWaypoints(o[1] ? (const Waypoint *)(size_t)o[1] : 0, o[2]); break;
			case 4: m->pauseCountdown((o[1] & 1) != 0); break;
			case 5: m->onSpecialPowerCreation(); break;
			case 6: m->setReadyFrame(o[1]); break;
			case 7: {
				Coord3D loc;
				memcpy(&loc.x, &o[2], 4); memcpy(&loc.y, &o[3], 4); memcpy(&loc.z, &o[4], 4);
				m->markSpecialPowerTriggered(o[1] ? &loc : 0);
				break;
			}
			case 8: m->startPowerRecharge(); break;
			case 9: break;
			case 10: gl.m_frame = o[1]; break;
			case 11:
				for (int k = 0; k < 2; ++k) { objs[k]->m_disabled = (o[1] & 1) != 0; objs[k]->m_player = o[2] ? &players[o[2] - 1] : 0; }
				break;
			case 12: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_spSaved.assign(bytes, bytes + blen); break;
			case 13: {
				std::vector<unsigned char> b = g_spSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 14: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 15: m->loadPostProcess(); break;
			case 16: g_spLocal = o[1] ? &players[o[1] - 1] : 0; break;
			case 17: for (int p = 0; p < 3; ++p) g_spRelations[p] = (o[1] >> (2 * p)) & 3; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100 && g_ulog[(size_t)e.p].compare(0, 9, "newobject") != 0) log.push_back(g_ulog[(size_t)e.p]);   /* (the thing factory's own line is not the module's) */
		}
		putI32(st);
		/* the queries (they log nothing) */
		g_events.clear();
		g_ulog.clear();
		putU32(m->isReady() ? 1 : 0);
		putU32(realBits(m->getPercentReady()));
		/* (the interface's const getReadyFrame, not the header's non-const one that returns m_availableOnFrame) */
		putU32(((const SpecialPowerModule *)m)->getReadyFrame());
		putU32(m->isScriptOnly() ? 1 : 0);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_availableOnFrame);
		putU32((unsigned)m->m_pausedCount);
		putU32(m->m_pausedOnFrame);
		putU32(realBits(m->m_pausedPercent));
		putU32((unsigned)(g_roObjects.size()));
		for (size_t j = 0; j < g_roObjects.size(); ++j) {
			const Object *ob = g_roObjects[j];
			putI32(ob->m_oid);
			putI32((int)(size_t)ob->m_team);
			putU32(realBits(ob->m_pos.x)); putU32(realBits(ob->m_pos.y)); putU32(realBits(ob->m_pos.z));
			putU32(realBits(ob->m_shroudRange));
		}
	}
	*len = finish(buf, cap);
	g_spLogRemove = false;
	delete m;
	delete spu;
	delete d;
	obj->m_drawable = 0;
	delete obj;
	delete other;
	for (size_t j = 0; j < g_roObjects.size(); ++j) { delete[] g_roObjects[j]->m_behaviors; g_roObjects[j]->m_behaviors = 0; delete g_roObjects[j]; }
	g_roObjects.clear();
	g_roTemplateOf.clear();
	g_findUpdateHook = 0;
	TheGameLogic = 0;
	TheAudio = 0;
	TheEva = 0;
	ThePlayerList = 0;
	TheScriptEngine = 0;
	TheInGameUI = 0;
	TheThingFactory = 0;
	return 0;
}

}  /* extern "C" */
