/* Differential-test driver for ConvertToHijackedVehicleCrateCollide and HijackerUpdate (p1w13-sonnet, batch R13e; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ConvertToHijackedVehicleCrateCollide.cpp
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HijackerUpdate.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end (CrateCollide.cpp is compiled by l9_crates_f.inc), on the module build's stand-ins: the hijacker (id 1: player,
** level, name, vision and shroud clearing ranges, maybe an AI with a goal, a drawable, the crate module and maybe a REAL HijackerUpdate) and the
** vehicle (id 2: kinds, player, team, status, level, position, maybe an AI that may be a dozer, a contain with passengers, an EjectPilotDie); three
** players with default teams 11 to 13 and the local player; the teams' relationship the test gives; a radar, an Eva, a script engine, an audio manager,
** the partition manager and a thing factory that log (the parachute template U1 makes a bare object: no contain). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ConvertToHijackedVehicleCrateCollide.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HijackerUpdate.cpp"

namespace {

/* the vehicle's dozer AI: a cancelled task is a line */
class R13eDozer : public DozerAIInterface {
public:
	virtual void cancelTask(DozerTask task) { ulog("canceltask %d %d", (int)m_oid, (int)task); }
	int m_oid = 2;
};
R13eDozer g_r13eDozer;
const AIUpdateInterface *g_r13eDozerAi = 0;
/* the vehicle's contain: the passengers the test gives */
class R13eContain : public UpgContain {
public:
	virtual UnsignedInt getContainCount() const { return m_count; }
	UnsignedInt m_count = 0;
};
/* the vehicle's EjectPilotDie: only its interface is asked for */
class R13eEject : public TestBehavior {
public:
	R13eEject(Thing *t, const ModuleData *d) : TestBehavior(t, d) {}
	virtual DieModuleInterface *getEjectPilotDieInterface() { return (DieModuleInterface *)(void *)this; }   /* only tested for NULL */
};

/* a parachute the factory makes (cfg[10]): its contain's answers are lines */
class R13eChute : public UpgContain {
public:
	virtual Bool isValidContainerFor(const Object *obj, Bool checkCapacity) const
	{
		ulog("validfor %d %d %d", m_oid, obj ? (int)obj->m_oid : 0, checkCapacity ? 1 : 0);
		return m_valid;
	}
	virtual void addToContain(Object *obj) { ulog("addtocontain %d %d", m_oid, obj ? (int)obj->m_oid : 0); }
	int m_oid = 0;
	Bool m_valid = false;
};
int g_r13eChuteKind = 0;   /* 0 no contain, 1 a contain that refuses, 2 one that accepts */
std::vector<R13eChute *> g_r13eChutes;
Object *r13eNewObject(const ThingTemplate *tmplate, Team *team)
{
	Object *o = r13bNewObject(tmplate, team);
	if (g_r13eChuteKind) {
		R13eChute *c = new R13eChute;
		c->m_oid = (int)o->m_oid;
		c->m_valid = g_r13eChuteKind == 2;
		o->m_contain = c;
		g_r13eChutes.push_back(c);
	}
	return o;
}
int g_r13eRel = 0;
UpdateModule *g_r13eHijacker = 0;
Relationship r13eTeamRel(const Team *a, const Team *b) { return (a->m_dummy == 1 && b->m_dummy == 2) ? (Relationship)g_r13eRel : ALLIES; }
UpdateModule *r13eFindUpdate(const Object *, NameKeyType key) { return (g_r13eHijacker && key == TheNameKeyGenerator->nameToKey("HijackerUpdate")) ? g_r13eHijacker : 0; }
Object *g_r13eObjs[2];
Object *r13eFind(ObjectID id) { return (id == 1 || id == 2) ? g_r13eObjs[id - 1] : 0; }
void r13eSetTeam(Object *o, Team *t) { ulog("setteam %d %d", (int)o->m_oid, t ? (int)t->m_dummy : -1); }

/* the kinds the test gives the vehicle, by bit (the same list in the Rust test) */
const KindOfType g_r13eKinds[8] = { KINDOF_VEHICLE, KINDOF_AIRCRAFT, KINDOF_BOAT, KINDOF_INFANTRY, KINDOF_STRUCTURE, KINDOF_IMMUNE_TO_CAPTURE,
	KINDOF_DRONE, KINDOF_TRANSPORT };
void r13eSetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 8; ++i) if (bits >> i & 1) o->m_kindOf.set(g_r13eKinds[i]);
	o->m_tmpl.m_kindOf = o->m_kindOf;
}

/* the two blocks: D (the crate) and H (the HijackerUpdate) */
void l9r13eRead(const char *filename, ModuleData **crateData, ModuleData **hijackData)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	try {
		for (int b = 0; b < 2; ++b) {
			ini.readLine();
			AsciiString currentLine = ini.m_buffer;
			try {
				if (b == 0) *crateData = ConvertToHijackedVehicleCrateCollide::friend_newModuleData(&ini);
				else *hijackData = HijackerUpdate::friend_newModuleData(&ini);
			} catch (...) {
				char buff[1024];
				sprintf(buff, "Error parsing INI file '%s' (Line: '%s')\n", ini.m_filename.str(), currentLine.str());
				throw INIException(buff);
			}
		}
	} catch (...) {
		ini.unPrepFile();
		throw;
	}
	ini.unPrepFile();
}

std::vector<unsigned char> g_r13eSaved;

}  // namespace

DozerAIInterface *AIUpdateInterface::getDozerAIInterface() { return (g_r13eDozerAi && this == g_r13eDozerAi) ? &g_r13eDozer : 0; }

extern "C" {

/* Read `filename` (blocks `D` and `H`) as the data of a ConvertToHijackedVehicleCrateCollide and of a HijackerUpdate and make the modules on the
** hijacker (id 1). `cfg` (12 words): [0 the hijacker's player (0xFFFFFFFF none), 1 the vehicle's player, 2 the vehicle's kinds (bits of g_r13eKinds),
** 3 flags (bit 0 the hijacker has an AI, 1 its goal is the vehicle, 2 the vehicle has an AI, 3 it is a dozer, 4 the vehicle is effectively dead, 5 it
** is hijacked (status), 6 it has a contain, 7 an EjectPilotDie, 8 the hijacker has a drawable, 9 a HijackerUpdate, 10-11 the hijacker's level, 12-13 the
** vehicle's, 14-15 the hijacker team's relationship to the vehicle's (0 enemies, 1 neutral, 2 allies), 16-17 the local player (3 none), 18-20 players
** 0 to 2 are computers), 4 the passengers, 5-7 the vehicle's position (float bits), 8 the hijacker's vision range, 9 its shroud clearing range (float
** bits), 10 the parachute the factory makes: 0 no contain, 1 a contain that refuses the hijacker, 2 one that accepts him]. The data dump goes first (the crate base, then the HijackerUpdate's bone and parachute names). Then `n` ops of 6 words [op, a, b, c]:
** 0 onCollide(a ? vehicle : NULL)  1 wouldLikeToCollideWith(a ? vehicle : NULL)  2 the vehicle's player = a - 1 (0 none)  3 flags bits 0-17 but 7 and 9 = a
** 4 the vehicle's kinds = a  5 the vehicle's z = a (float bits)  6 HijackerUpdate::update()  7 xfer save of the crate (b 0) or the HijackerUpdate (b 1)
** 8 xfer load of the last save of that module  9 xfer CRC of that module  10 the vehicle is gone (no longer found; the later ops leave it alone)  11 HijackerUpdate::loadPostProcess().
** Each op appends: the CATCH_ALL status, the result, the effect log (count, then each), the xfer status, bytes and CRC, then the hijacker's position, its
** status bits NO_COLLISIONS, MASKED and UNSELECTABLE, the vehicle's HIJACKED bit, both levels, the vehicle's vision and shroud clearing ranges (0 for a
** vehicle that is gone), and the
** HijackerUpdate's target, eject position and three flags (0 for none). */
int ref_l9r13e_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r13eSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0, *hd = 0;
	try { l9r13eRead(filename, &d, &hd); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { delete d; delete hd; return status; }
	unsigned scienceKeys[5];
	for (int i = 0; i < 5; ++i) scienceKeys[i] = (unsigned)NAMEKEY(SCIENCE_NAMES_CF[i]);
	cfPutBase((CrateCollideModuleData *)d, scienceKeys);
	const HijackerUpdateModuleData *hud = (const HijackerUpdateModuleData *)hd;
	putStr(hud->m_attachToBone.str(), hud->m_attachToBone.getLength());
	putStr(hud->m_parachuteName.str(), hud->m_parachuteName.getLength());
	GameLogic gl;
	gl.m_frame = 0;
	gl.m_drawIconUI = false;
	TheGameLogic = &gl;
	unsigned flags = cfg[3];
	Team defaults[3];
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		defaults[i].m_dummy = 11 + i;
		players[i].m_defaultTeam = &defaults[i];
		players[i].m_type = (flags >> (18 + i) & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN;   /* the stand-in has no constructor */
	}
	g_spPlayerPtrs[0] = &players[0]; g_spPlayerPtrs[1] = &players[1]; g_spPlayerPtrs[2] = &players[2];
	g_spPlayerCount = 3;
	ThePlayerList = &g_spList;
	TheEva = &g_spEva;
	TheRadar = &g_sabRadar;
	TheAudio = &g_ddAudio;
	TheScriptEngine = &g_ddScript;
	g_ddGravity = -1.0f;
	g_modDestroyHook = cfDestroyHook;
	g_teamRelHook = r13eTeamRel;
	g_findUpdateHook = r13eFindUpdate;
	g_setTeamHook = r13eSetTeam;
	g_modFindHook = r13eFind;
	g_r13bTemplates[0].m_name = "U1";
	g_r13bTemplates[1].m_name = "U2";
	g_findTemplateHook = r13bFindTemplate;
	g_newObjectHook = r13eNewObject;
	g_r13eChuteKind = (int)cfg[10];
	g_r13bMade.clear();
	g_r13bNextId = 3;
	Team teams[2];
	ExperienceTracker xps[2];
	for (int k = 0; k < 2; ++k) {
		g_r13eObjs[k] = newObject();
		g_r13eObjs[k]->m_oid = (ObjectID)(k + 1);
		teams[k].m_dummy = k + 1;
		g_r13eObjs[k]->m_team = &teams[k];
		g_r13eObjs[k]->m_next = k ? g_r13eObjs[k - 1] : 0;
		xps[k].m_owner = g_r13eObjs[k];
		g_r13eObjs[k]->m_xp = &xps[k];
	}
	Object *crate = g_r13eObjs[0], *other = g_r13eObjs[1];
	crate->m_name = "Pilot";
	crate->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	other->m_player = cfg[1] == 0xFFFFFFFFu ? 0 : &players[cfg[1]];
	r13eSetKinds(other, cfg[2]);
	other->m_pos.x = ctF(cfg[5]);
	other->m_pos.y = ctF(cfg[6]);
	other->m_pos.z = ctF(cfg[7]);
	crate->m_vision = ctF(cfg[8]);
	crate->m_shroudRange = ctF(cfg[9]);
	AIUpdateInterface crateAi, otherAi;
	crateAi.m_owner = crate;
	otherAi.m_owner = other;
	Drawable draw;
	draw.m_ownerObject = crate;
	R13eContain contain;
	contain.m_count = cfg[4];
	ModuleData plain;
	R13eEject eject(other, &plain);
	BehaviorModule *otherMods[2] = { 0, 0 };
	CrateCollide *m = new ConvertToHijackedVehicleCrateCollide(crate, d);
	HijackerUpdate *hu = new HijackerUpdate(crate, hd);
	/* the Rust world schedules the module when it makes the object (frame 1 at frame 0); as the contain driver does, the word is set to match */
	hu->friend_setNextCallFrame(1);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (i == 0 || o[0] == 3) {
			if (o[0] == 3) flags = (flags & ~0x3FD7Fu) | (o[1] & 0x3FD7Fu);   /* not bits 7 and 9: the Rust object's modules are fixed */
			crate->m_ai = (flags & 1) ? &crateAi : 0;
			crateAi.m_goal = (flags & 2) ? other : 0;
			other->m_ai = (flags & 4) ? &otherAi : 0;
			g_r13eDozerAi = (flags & 8) ? &otherAi : 0;
			other->m_effDead = (flags & 16) != 0;
			if (flags & 32) other->m_status.set(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_HIJACKED)); else other->m_status.clear(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_HIJACKED));
			other->m_contain = (flags & 64) ? &contain : 0;
			otherMods[0] = (flags & 128) ? &eject : 0;
			other->m_behaviors = otherMods;
			crate->m_drawable = (flags & 256) ? &draw : 0;
			g_r13eHijacker = (flags & 512) ? hu : 0;
			crate->m_level = (VeterancyLevel)((flags >> 10) & 3);
			other->m_level = (VeterancyLevel)((flags >> 12) & 3);
			g_r13eRel = (int)((flags >> 14) & 3);
			g_spLocal = ((flags >> 16) & 3) == 3 ? 0 : &players[(flags >> 16) & 3];
		}
		g_events.clear();
		g_ulog.clear();
		int st = 0, res = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		Snapshot *xs = o[2] ? (Snapshot *)hu : (Snapshot *)m;
		try {
			switch (o[0]) {
			case 0: m->onCollide(o[1] ? other : 0, 0, 0); break;
			case 1: res = m->wouldLikeToCollideWith(o[1] ? other : 0) ? 1 : 0; break;
			case 2: other->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			case 3: break;
			case 4: r13eSetKinds(other, o[1]); break;
			case 5: other->m_pos.z = ctF(o[1]); break;
			case 6: if (g_r13eHijacker) res = (int)hu->update(); break;
			case 7: xst = runXfer(xs, 0, xfer_path, bytes, 512, &blen, &crc); g_r13eSaved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r13eSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(xs, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(xs, 2, "crc", bytes, 0, &blen, &crc); break;
			case 10: g_r13eObjs[1] = 0; break;   /* for the rest of the case: the test's later ops leave the vehicle alone */
			case 11: if (g_r13eHijacker) hu->loadPostProcess(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putI32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			/* FXList::doFXObj of the real header does nothing for a NULL list (FXList.h:151-156); the stand-in records it: dropped */
			else if (e.kind == 1 && e.p) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(realBits(crate->m_pos.x));
		putU32(realBits(crate->m_pos.y));
		putU32(realBits(crate->m_pos.z));
		putU32((crate->m_status.test(OBJECT_STATUS_NO_COLLISIONS) ? 1 : 0) | (crate->m_status.test(OBJECT_STATUS_MASKED) ? 2 : 0) | (crate->m_status.test(OBJECT_STATUS_UNSELECTABLE) ? 4 : 0));
		const bool gone = g_r13eObjs[1] == 0;   /* the vehicle's fields are 0 once it is gone */
		putU32(!gone && other->m_status.test(OBJECT_STATUS_HIJACKED) ? 1 : 0);
		putU32((unsigned)crate->m_level);
		putU32(gone ? 0 : (unsigned)other->m_level);
		putU32(gone ? 0 : realBits(other->m_vision));
		putU32(gone ? 0 : realBits(other->m_shroudRange));
		if (g_r13eHijacker) {
			putU32((unsigned)hu->m_targetID);
			putU32(realBits(hu->m_ejectPos.x));
			putU32(realBits(hu->m_ejectPos.y));
			putU32(realBits(hu->m_ejectPos.z));
			putU32(hu->m_update ? 1 : 0);
			putU32(hu->m_isInVehicle ? 1 : 0);
			putU32(hu->m_wasTargetAirborne ? 1 : 0);
		} else {
			for (int j = 0; j < 7; ++j) putU32(0);
		}
	}
	*len = finish(buf, cap);
	g_modDestroyHook = 0;
	g_teamRelHook = 0;
	g_findUpdateHook = 0;
	g_setTeamHook = 0;
	g_modFindHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_r13eHijacker = 0;
	g_r13eDozerAi = 0;
	g_ddGravity = 0;
	delete m;
	delete hu;
	delete d;
	delete hd;
	for (size_t j = 0; j < g_r13bMade.size(); ++j) { g_r13bMade[j]->m_xp = 0; g_r13bMade[j]->m_contain = 0; delete g_r13bMade[j]; }
	for (size_t j = 0; j < g_r13eChutes.size(); ++j) delete g_r13eChutes[j];
	g_r13eChutes.clear();
	g_r13bMade.clear();
	for (size_t j = 0; j < g_r13bTrackers.size(); ++j) delete g_r13bTrackers[j];
	g_r13bTrackers.clear();
	crate->m_xp = 0; crate->m_ai = 0; crate->m_drawable = 0;
	other->m_xp = 0; other->m_ai = 0; other->m_contain = 0; other->m_behaviors = 0;
	delete crate;
	delete other;
	g_r13eObjs[0] = g_r13eObjs[1] = 0;
	TheGameLogic = 0;
	TheEva = 0;
	TheRadar = 0;
	TheAudio = 0;
	TheScriptEngine = 0;
	ThePlayerList = 0;
	g_spLocal = 0;
	return 0;
}

}  /* extern "C" */
