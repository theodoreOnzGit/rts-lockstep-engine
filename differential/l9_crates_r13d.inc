/* Differential-test driver for ConvertToCarBombCrateCollide (p1w13-sonnet, batch R13d; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ConvertToCarBombCrateCollide.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end (CrateCollide.cpp is compiled by l9_crates_f.inc), on the module build's stand-ins: the terrorist (id 1: player,
** level, name, vision and shroud clearing ranges, maybe an AI with a goal) and the vehicle (id 2: kinds, player, status, weapon set flags, its template's
** weapon sets, level, maybe an AI and a drawable); three players with default teams 11 to 13; a radar, a script engine and an FX list that log. The
** booby trap's answer is the contain driver's stand-in (l9_open_contain_r.inc), given a table of one object for the run. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ConvertToCarBombCrateCollide.cpp"

void Radar::removeObject(Object *obj) { ulog("radarremove %d", obj ? (int)obj->m_oid : 0); }
void Radar::addObject(Object *obj) { ulog("radaradd %d", obj ? (int)obj->m_oid : 0); }

namespace {

/* the kinds the test gives the vehicle, by bit (the same list in the Rust test) */
const KindOfType g_r13dKinds[8] = { KINDOF_VEHICLE, KINDOF_AIRCRAFT, KINDOF_BOAT, KINDOF_INFANTRY, KINDOF_STRUCTURE, KINDOF_IMMUNE_TO_CAPTURE,
	KINDOF_DRONE, KINDOF_TRANSPORT };
/* the weapon set flags a template's set may hold, by bit (the same list in the Rust test) */
const WeaponSetType g_r13dSetFlags[3] = { WEAPONSET_CARBOMB, WEAPONSET_VETERAN, WEAPONSET_CRATEUPGRADE_ONE };

void r13dSetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 8; ++i) if (bits >> i & 1) o->m_kindOf.set(g_r13dKinds[i]);
	o->m_tmpl.m_kindOf = o->m_kindOf;
}
/* the template's weapon sets: up to three, 4 bits each (bit 0 present, bits 1-3 its flags over g_r13dSetFlags) */
void r13dSetWeaponSets(Object *o, unsigned code)
{
	o->m_tmpl.m_weaponTemplateSets.clear();
	o->m_tmpl.m_weaponTemplateSetFinder.clear();
	for (int k = 0; k < 3; ++k) {
		const unsigned c = code >> (4 * k) & 15;
		if (!(c & 1)) continue;
		WeaponTemplateSet s;
		for (int f = 0; f < 3; ++f) if (c >> (1 + f) & 1) s.m_types.set(g_r13dSetFlags[f]);
		o->m_tmpl.m_weaponTemplateSets.push_back(s);
	}
}
void r13dDefect(Object *o, Team *t, UnsignedInt detectionTime) { ulog("defect %d %d %u", (int)o->m_oid, t ? (int)t->m_dummy : -1, detectionTime); }

ModuleData *l9r13dRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = ConvertToCarBombCrateCollide::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r13dSaved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a ConvertToCarBombCrateCollide and make the module on the terrorist (id 1). `cfg` (12 words): [0 the
** terrorist's player (0xFFFFFFFF none), 1 the vehicle's player, 2 the vehicle's kinds (bits of g_r13dKinds), 3 flags (bit 0 the terrorist has an AI, 1
** its goal is the vehicle, 2 the vehicle has an AI, 3 the vehicle is effectively dead, 4 the terrorist is, 5 the vehicle has a drawable, 6 it is a car
** bomb already (status), 7 its car bomb weapon set flag, 8 a booby trap goes off, 9-10 the terrorist's level, 11-12 the vehicle's, 13-15 players 0 to 2
** are computers), 4 the vehicle template's weapon sets (r13dSetWeaponSets), 5 the terrorist's vision range, 6 its shroud clearing range (float bits)].
** The data dump goes first (the crate base and the FX list). Then `n` ops of 6 words [op, a, b, c]: 0 onCollide(a ? vehicle : NULL)
** 1 wouldLikeToCollideWith(a ? vehicle : NULL)  2 the vehicle's player = a - 1 (0 none)  3 flags bits 0-12 = a  4 the vehicle's kinds = a  5 its
** template's weapon sets = a  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op appends: the CATCH_ALL status, the result, the effect log
** (count, then each), the xfer status, bytes and CRC, then the vehicle's car bomb status bit, car bomb weapon set flag, vision range, shroud clearing
** range and level. */
int ref_l9r13d_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r13dSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r13dRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned scienceKeys[5];
	for (int i = 0; i < 5; ++i) scienceKeys[i] = (unsigned)NAMEKEY(SCIENCE_NAMES_CF[i]);
	cfPutBase((CrateCollideModuleData *)d, scienceKeys);
	putI32(fxId(((const ConvertToCarBombCrateCollideModuleData *)d)->m_fxList));
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
		players[i].m_type = (flags >> (13 + i) & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN;   /* the stand-in has no constructor */
	}
	g_spPlayerPtrs[0] = &players[0]; g_spPlayerPtrs[1] = &players[1]; g_spPlayerPtrs[2] = &players[2];
	g_spPlayerCount = 3;
	g_spLocal = 0;
	ThePlayerList = &g_spList;
	TheRadar = &g_sabRadar;
	TheScriptEngine = &g_ddScript;
	g_modDestroyHook = cfDestroyHook;
	g_defectHook = r13dDefect;
	/* the booby trap's stand-in answers through the contain driver's table: one object, the terrorist (it asks about its victim) */
	const int savedCtN = g_ctN;
	g_ctN = 1;
	Team teams[2];
	Object *obj[2];
	ExperienceTracker xps[2];
	for (int k = 0; k < 2; ++k) {
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
	r13dSetKinds(other, cfg[2]);
	r13dSetWeaponSets(other, cfg[4]);
	crate->m_vision = ctF(cfg[5]);
	crate->m_shroudRange = ctF(cfg[6]);
	AIUpdateInterface crateAi;
	crateAi.m_owner = crate;
	int otherAi = 0;
	Drawable draw;
	draw.m_ownerObject = other;
	CrateCollide *m = new ConvertToCarBombCrateCollide(crate, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (i == 0 || o[0] == 3) {
			if (o[0] == 3) flags = (flags & ~0x1FFFu) | (o[1] & 0x1FFFu);
			crate->m_ai = (flags & 1) ? &crateAi : 0;
			crateAi.m_goal = (flags & 2) ? other : 0;
			other->m_ai = (flags & 4) ? (AIUpdateInterface *)&otherAi : 0;
			other->m_effDead = (flags & 8) != 0;
			crate->m_effDead = (flags & 16) != 0;
			other->m_drawable = (flags & 32) ? &draw : 0;
			if (flags & 64) other->m_status.set(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_IS_CARBOMB)); else other->m_status.clear(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_IS_CARBOMB));
			if (flags & 128) other->m_weaponSetFlags[0] |= 1ULL << WEAPONSET_CARBOMB; else other->m_weaponSetFlags[0] &= ~(1ULL << WEAPONSET_CARBOMB);
			g_ctObjs[0].boobyTrapped = (flags & 256) ? 1 : 0;
			crate->m_level = (VeterancyLevel)((flags >> 9) & 3);
			other->m_level = (VeterancyLevel)((flags >> 11) & 3);
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
			case 4: r13dSetKinds(other, o[1]); break;
			case 5: r13dSetWeaponSets(other, o[1]); break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r13dSaved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r13dSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
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
		putU32(other->m_status.test(OBJECT_STATUS_IS_CARBOMB) ? 1 : 0);
		putU32((other->m_weaponSetFlags[0] >> WEAPONSET_CARBOMB & 1) ? 1 : 0);
		putU32(realBits(other->m_vision));
		putU32(realBits(other->m_shroudRange));
		putU32((unsigned)other->m_level);
	}
	*len = finish(buf, cap);
	g_modDestroyHook = 0;
	g_defectHook = 0;
	g_ctObjs[0].boobyTrapped = 0;
	g_ctN = savedCtN;
	delete m;
	delete d;
	for (int k = 0; k < 2; ++k) { obj[k]->m_xp = 0; obj[k]->m_drawable = 0; obj[k]->m_ai = 0; delete obj[k]; }
	TheGameLogic = 0;
	TheRadar = 0;
	TheScriptEngine = 0;
	ThePlayerList = 0;
	return 0;
}

}  /* extern "C" */
