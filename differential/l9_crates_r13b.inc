/* Differential-test driver for UnitCrateCollide and VeterancyCrateCollide (p1w13-sonnet, batch R13b; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/{Unit,Veterancy}CrateCollide.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end (CrateCollide.cpp is compiled by l9_crates_f.inc), on the module build's stand-ins: the crate (id 1: a crate, or the
** pilot, with its player, level, name and maybe an AI with a goal), the collector (id 2: kinds, player, position, orientation, level, whether it trains,
** whether its locomotor is airborne) and two more objects (ids 3 and 4) for the range scan; three players with default teams 11 to 13; a thing factory
** that knows the templates U1 and U2 and makes a bare object for each (ids 5 up); findPositionAround, setOrientation, the experience tracker's gain, the
** range scan and the script engine's name transfer are lines of the effect log (ulog). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/UnitCrateCollide.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/VeterancyCrateCollide.cpp"

/* ExperienceTracker.cpp:122-145's logic for the two questions the crate asks, verbatim but for the level read through the stand-in; the gain is a line */
Bool ExperienceTracker::isTrainable() const { return m_trainable; }
Bool ExperienceTracker::canGainExpForLevel(Int levelsToGain) const
{
	Int newLevel = (Int)getVeterancyLevel() + levelsToGain;
	// return true if we can gain levels, even if we can't gain ALL the levels requested
	if (newLevel > LEVEL_LAST)
		newLevel = LEVEL_LAST;
	return (newLevel > getVeterancyLevel());
}
Bool ExperienceTracker::gainExpForLevel(Int levelsToGain, Bool canScaleForBonus)
{
	ulog("gainexp %d %d %d", (int)m_owner->m_oid, (int)levelsToGain, canScaleForBonus ? 1 : 0);
	return canGainExpForLevel(levelsToGain);
}
void ScriptEngine::transferObjectName(const AsciiString &unitName, Object *pNewObject) { ulog("transfername %s %d", unitName.str(), pNewObject ? (int)pNewObject->m_oid : 0); }

namespace {

ThingTemplate g_r13bTemplates[2];
std::vector<Object *> g_r13bMade;
std::vector<ExperienceTracker *> g_r13bTrackers;
int g_r13bNextId = 5;
const ThingTemplate *r13bFindTemplate(const AsciiString &name)
{
	for (int k = 0; k < 2; ++k) if (g_r13bTemplates[k].m_name.compare(name) == 0) return &g_r13bTemplates[k];
	return 0;
}
Object *r13bNewObject(const ThingTemplate *tmplate, Team *team)
{
	ulog("newobject %s %d", tmplate->getName().str(), team ? (int)team->m_dummy : -1);
	Object *o = newObject();
	o->m_oid = (ObjectID)g_r13bNextId++;
	o->m_team = team;
	ExperienceTracker *xp = new ExperienceTracker;
	xp->m_owner = o;
	o->m_xp = xp;
	g_r13bTrackers.push_back(xp);
	g_r13bMade.push_back(o);
	return o;
}

ModuleData *l9r13bRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			if (kind == 0) data = UnitCrateCollide::friend_newModuleData(&ini);
			else data = VeterancyCrateCollide::friend_newModuleData(&ini);
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

void r13bScan(Object **obj, unsigned a)
{
	g_hdScan.clear();
	for (int k = 0; k < 4; ++k) if (a >> k & 1) g_hdScan.push_back(obj[k]);
	if (a & 16) std::reverse(g_hdScan.begin(), g_hdScan.end());
}

std::vector<unsigned char> g_r13bSaved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of crate `kind` (0 UnitCrateCollide, 1 VeterancyCrateCollide) and make the module on the crate (id 1). `cfg`
** (12 words): [0 the crate's player (0xFFFFFFFF none), 1 the collector's player, 2 the collector's kinds (bits of g_sabKinds), 3 flags (bit 0 the crate
** has an AI, 1 its goal is the collector, 4 the collector has an AI, 5 it is effectively dead, 6 it is trainable, 7 its locomotor is airborne, 8-9 the
** crate's level, 10-11 the collector's, 12-14 players 0 to 2 are computers), 4-6 the collector's position (float bits), 7 its orientation (float bits), 8 findPositionAround finds a position,
** 9-10 its x and y (float bits; z 0), 11 the range scan (bits 0-3 over objects 1 to 4, bit 4 reversed)]. The data dump goes first (the crate base and the
** class's fields). Then `n` ops of 6 words [op, a, b, c]: 0 onCollide(a ? collector : NULL)  1 wouldLikeToCollideWith(a ? collector : NULL)  2 the
** collector's player = a - 1 (0 none)  3 flags bits 0-11 = a  4 the collector's kinds = a  5 the collector's z = a (float bits)  6 findPositionAround: has
** = a & 1, x = b, y = c  7 xfer save  8 xfer load of the last save  9 xfer CRC  10 the range scan = a. Each op appends: the CATCH_ALL status, the result,
** the effect log (count, then each), the xfer status, bytes and CRC, then the objects made so far (count, then each one's id, team and position). */
int ref_l9r13b_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r13bSaved.clear();
	g_r13bMade.clear();
	g_r13bNextId = 5;
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r13bRead(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned scienceKeys[5];
	for (int i = 0; i < 5; ++i) scienceKeys[i] = (unsigned)NAMEKEY(SCIENCE_NAMES_CF[i]);
	cfPutBase((CrateCollideModuleData *)d, scienceKeys);
	if (kind == 0) {
		const UnitCrateCollideModuleData *u = (const UnitCrateCollideModuleData *)d;
		putU32(u->m_unitCount);
		putStr(u->m_unitType.str(), u->m_unitType.getLength());
	} else {
		const VeterancyCrateCollideModuleData *v = (const VeterancyCrateCollideModuleData *)d;
		putU32(v->m_rangeOfEffect);
		putU32(v->m_addsOwnerVeterancy ? 1 : 0);
		putU32(v->m_isPilot ? 1 : 0);
	}
	GameLogic gl;
	gl.m_frame = 0;
	gl.m_drawIconUI = false;
	TheGameLogic = &gl;
	Team defaults[3];
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		defaults[i].m_dummy = 11 + i;
		players[i].m_defaultTeam = &defaults[i];
		players[i].m_type = (cfg[3] >> (12 + i) & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN;   /* the stand-in has no constructor */
	}
	g_spPlayerPtrs[0] = &players[0]; g_spPlayerPtrs[1] = &players[1]; g_spPlayerPtrs[2] = &players[2];
	g_spPlayerCount = 3;
	g_spLocal = 0;
	ThePlayerList = &g_spList;
	TheAudio = &g_ddAudio;
	TheScriptEngine = &g_ddScript;
	g_ddGravity = -1.0f;
	g_modDestroyHook = cfDestroyHook;
	g_cfMisc.m_crateFreeUnit.setEventName(AsciiString("crateFreeUnit"));
	g_r13bTemplates[0].m_name = "U1";
	g_r13bTemplates[1].m_name = "U2";
	g_findTemplateHook = r13bFindTemplate;
	g_newObjectHook = r13bNewObject;
	unsigned flags = cfg[3];
	Team teams[4];
	Object *obj[4];
	ExperienceTracker xps[4];
	for (int k = 0; k < 4; ++k) {
		obj[k] = newObject();
		obj[k]->m_oid = (ObjectID)(k + 1);
		teams[k].m_dummy = k + 1;
		obj[k]->m_team = &teams[k];
		obj[k]->m_next = k ? obj[k - 1] : 0;
		xps[k].m_owner = obj[k];
		obj[k]->m_xp = &xps[k];
	}
	Object *crate = obj[0], *other = obj[1];
	crate->m_name = "Pilot";
	crate->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	other->m_player = cfg[1] == 0xFFFFFFFFu ? 0 : &players[cfg[1]];
	sabSetKinds(other, cfg[2]);
	other->m_pos.x = ctF(cfg[4]);
	other->m_pos.y = ctF(cfg[5]);
	other->m_pos.z = ctF(cfg[6]);
	other->m_orientation = ctF(cfg[7]);
	g_ctFindPosHas = cfg[8] != 0;
	g_ctFindPos.x = ctF(cfg[9]);
	g_ctFindPos.y = ctF(cfg[10]);
	g_ctFindPos.z = 0;
	r13bScan(obj, cfg[11]);
	AIUpdateInterface crateAi;
	crateAi.m_owner = crate;
	int otherAi = 0;
	CrateCollide *m = kind == 0 ? (CrateCollide *)new UnitCrateCollide(crate, d) : (CrateCollide *)new VeterancyCrateCollide(crate, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (i == 0 || o[0] == 3) {
			if (o[0] == 3) flags = (flags & ~0xFF3u) | (o[1] & 0xFF3u);
			crate->m_ai = (flags & 1) ? &crateAi : 0;
			crateAi.m_goal = (flags & 2) ? other : 0;
			other->m_ai = (flags & 16) ? (AIUpdateInterface *)&otherAi : 0;
			other->m_effDead = (flags & 32) != 0;
			xps[1].m_trainable = (flags & 64) != 0;
			other->m_airborneLoco = (flags & 128) && (flags & 16);   /* upstream asks the AI's locomotor: none without an AI */
			crate->m_level = (VeterancyLevel)((flags >> 8) & 3);
			other->m_level = (VeterancyLevel)((flags >> 10) & 3);
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
		try {
			switch (o[0]) {
			case 0: m->onCollide(o[1] ? other : 0, 0, 0); break;
			case 1: res = m->wouldLikeToCollideWith(o[1] ? other : 0) ? 1 : 0; break;
			case 2: other->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			case 3: break;
			case 4: sabSetKinds(other, o[1]); break;
			case 5: other->m_pos.z = ctF(o[1]); break;
			case 6: g_ctFindPosHas = (o[1] & 1) != 0; g_ctFindPos.x = ctF(o[2]); g_ctFindPos.y = ctF(o[3]); break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r13bSaved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r13bSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 10: r13bScan(obj, o[1]); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putI32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32((unsigned)g_r13bMade.size());
		for (size_t j = 0; j < g_r13bMade.size(); ++j) {
			const Object *x = g_r13bMade[j];
			putU32((unsigned)x->m_oid);
			putI32(x->m_team ? (int)x->m_team->m_dummy : -1);
			putU32(realBits(x->m_pos.x));
			putU32(realBits(x->m_pos.y));
			putU32(realBits(x->m_pos.z));
		}
	}
	*len = finish(buf, cap);
	g_modDestroyHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_ctFindPosHas = false;
	g_hdScan.clear();
	delete m;
	delete d;
	for (size_t j = 0; j < g_r13bMade.size(); ++j) { g_r13bMade[j]->m_xp = 0; delete g_r13bMade[j]; }
	g_r13bMade.clear();
	for (size_t j = 0; j < g_r13bTrackers.size(); ++j) delete g_r13bTrackers[j];
	g_r13bTrackers.clear();
	for (int k = 0; k < 4; ++k) { obj[k]->m_xp = 0; obj[k]->m_ai = 0; delete obj[k]; }
	TheGameLogic = 0;
	TheAudio = 0;
	TheScriptEngine = 0;
	ThePlayerList = 0;
	g_ddGravity = 0;
	return 0;
}

}  /* extern "C" */
