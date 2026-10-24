/* Differential-test driver for StructureToppleUpdate (p1w13-sonnet, batch R36; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StructureToppleUpdate.cpp   (#included with one mechanical edit made by build.rs
**     into mod_gen/StructureToppleUpdate_edit.cpp: the crushing loop's `j`, which the function reads after the loop as MSVC's old `for` scope
**     allows, declared before it; and renames of its file-local names, which StructureCollapseUpdate.cpp and others define in this unit too)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the building (id 1: maybe a body, the module build's MBody, whose last
** damage is none or the test's and whose updateBodyParticleSystems is a line; maybe a drawable whose model condition calls are lines; maybe an AI
** whose markAsDead is a line; maybe the BoneFXUpdate the collapse driver's findUpdateModule finds; a veterancy, status bits, a name, a position, an
** orientation and the transform posed from them as Thing keeps it (the topple driver's), a box geometry), the attacker (id 2, at the test's position),
** the script engine's topple directions (the topple driver's list; the Coord2D overload is ScriptEngine.cpp's, verbatim), the terrain's eight cells,
** the FX list store (FX_A, FX_B; "fxpos", and doFXObj as the test log's "fx" entries), the object creation lists (OCL_A, OCL_B; "oclat"), the weapon
** store (WEAP_A, WEAP_B; createAndFireTempWeapon a line), the game logic (its frame, deselectObject, its wake calls applied as
** GameLogic::friend_awakenUpdateModule applies them), and the REAL UpdateModule, DieMuxData, INI, RandomValue (the logic's and the client's) and
** Xfer. */
#define MAX_IDX StMAX_IDX
#define parseOCL StParseOCL
#define inList StInList
#define buildNonDupRandomIndexList StBuildNonDupRandomIndexList
#include "StructureToppleUpdate_edit.cpp"
#undef MAX_IDX
#undef parseOCL
#undef inList
#undef buildNonDupRandomIndexList

namespace {

ModuleData *l9r36Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = StructureToppleUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r36Saved;
UpdateModule *g_r36Updating = 0;
void r36Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r36Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *g_r36Me = 0, *g_r36Attacker = 0;
Object *r36Find(ObjectID id)
{
	if (g_r36Me && id == g_r36Me->m_oid) return g_r36Me;
	if (g_r36Attacker && id == g_r36Attacker->m_oid) return g_r36Attacker;
	return 0;
}
/* the ground heights a cell can have, by nibble (the same table in the Rust test) */
const Real g_r36Grounds[8] = { 0.0f, 2.0f, 5.5f, -3.0f, 12.0f, 1.25f, 30.0f, -0.5f };

}  // namespace

/* ScriptEngine.cpp lines 7543-7555, verbatim */
void ScriptEngine::adjustToppleDirection( Object *object, Coord2D *direction)
{
	if (!(object && direction)) {
		return;
	}

	Coord3D dir;
	dir.x = (*direction).x;
	dir.y = (*direction).y;
	adjustToppleDirection(object, &dir);
	(*direction).x = dir.x;
	(*direction).y = dir.y;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a StructureToppleUpdate and make the module on object 1 at frame cfg[0]. `cfg` (20 words; 3-5 and
** 12-17 also settable by op 3): 0 the frame, 1 the logic random seed, 2 the client random seed, 3 flags (0 drawable, 1 AI, 2 body, 3 a BoneFXUpdate,
** 4 a last damage, 5 the name "Tower1"), 4 the veterancy, 5 the status bits (8), 6-7 the position (float bits; z 0), 8 the orientation (float bits),
** 9-11 the box geometry's height, major and minor radius (float bits), 12-13 the attacker's position (float bits), 14 the last damage's type, 15
** the script's topple direction for "Tower1" (0 none, else an eighth of a turn, of length 3), 16 the eight cells' ground heights (nibbles of
** g_r36Grounds), 17-19 unused. The data dump goes first, then the constructor's effects and wake calls. Then `n` ops of 4 words [op, a, b, c]: 0
** update()  1 onDie(a death of type a, from object b)  2 the frame = a  3 cfg[a] = b  4 xfer save  5 xfer load of the last save  6 xfer CRC. Each op
** appends: the CATCH_ALL status, the result, the effect log (the orientation lines left out), the wake calls (count, last frame), the xfer status,
** bytes and CRC, the state words, the transform (3 x 4), the position and the orientation. */
int ref_l9r36_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r36Saved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r36Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[20];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const StructureToppleUpdateModuleData *sd = (const StructureToppleUpdateModuleData *)d;
		putI32(sd->m_minToppleDelay);
		putI32(sd->m_maxToppleDelay);
		putI32(sd->m_minToppleBurstDelay);
		putI32(sd->m_maxToppleBurstDelay);
		putU32(realBits(sd->m_structuralIntegrity));
		putU32(realBits(sd->m_structuralDecay));
		unsigned w[2] = { 0, 0 };
		for (int t = 0; t < DAMAGE_NUM_TYPES && t < 64; ++t) if (getDamageTypeFlag(sd->m_damageFXTypes, (DamageType)t)) w[t / 32] |= 1u << (t % 32);
		putU32(w[0]);
		putU32(w[1]);
		const FXList *fxs[5] = { sd->m_toppleStartFXList, sd->m_toppleDelayFXList, sd->m_toppleFXList, sd->m_toppleDoneFXList, sd->m_crushingFXList };
		for (int k = 0; k < 5; ++k) putI32(fxId(fxs[k]));
		putStr(sd->m_crushingWeaponName.str(), (int)sd->m_crushingWeaponName.getLength());
		for (int ph = 0; ph < ST_PHASE_COUNT; ++ph) {
			putU32((unsigned)sd->m_ocls[ph].size());
			for (size_t i = 0; i < sd->m_ocls[ph].size(); ++i) putI32(oclCode(sd->m_ocls[ph][i]));
			putU32(sd->m_oclCount[ph]);
		}
		putU32((unsigned)sd->angleFX.size());
		for (size_t i = 0; i < sd->angleFX.size(); ++i) { putU32(realBits(sd->angleFX[i].angle)); putI32(fxId(sd->angleFX[i].fxList)); }
		putU32((unsigned)sd->fxbones.size());
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitRandom(cfg[2]);
	InitGameLogicRandom(cfg[1]);
	ScriptEngine script;
	TheScriptEngine = &script;
	TheWeaponStore = &g_kStore;
	g_findUpdateHook = r29FindUpdate;
	g_xformHook = r28Xform;
	g_r28ThingPose = true;
	g_modFindHook = r36Find;
	Object *me = newObject(), *attacker = newObject();
	me->m_oid = 1;
	attacker->m_oid = 2;
	g_r36Me = me;
	g_r36Attacker = attacker;
	g_r29Me = me;
	MBody body;
	body.m_owner = me;
	static DamageInfo lastDamage;
	Drawable draw;
	draw.m_ownerObject = me;
	AIUpdateInterface ai;
	ai.m_owner = me;
	auto scene = [&]() {
		unsigned f = cfg[3];
		me->m_drawable = (f & 1) ? &draw : 0;
		me->m_ai = (f >> 1 & 1) ? &ai : 0;
		me->m_body = (f >> 2 & 1) ? &body : 0;
		g_r29BoneFx = (f >> 3 & 1) != 0;
		lastDamage = DamageInfo();
		lastDamage.in.m_damageType = (DamageType)cfg[14];
		body.m_lastDamage = (f >> 4 & 1) ? &lastDamage : 0;
		me->m_name = (f >> 5 & 1) ? AsciiString("Tower1") : AsciiString::TheEmptyString;
		me->m_level = (VeterancyLevel)cfg[4];
		me->m_status.clear();
		for (int b = 0; b < 8; ++b) if (cfg[5] >> b & 1) me->m_status.set((ObjectStatusTypes)b);
		memcpy(&attacker->m_pos.x, &cfg[12], 4);
		memcpy(&attacker->m_pos.y, &cfg[13], 4);
		attacker->m_pos.z = 0.0f;
		g_r28ToppleDirs.clear();
		if (cfg[15]) {
			Coord3D dir;
			Real a = (Real)(cfg[15] % 8) * (2.0f * PI / 8.0f);
			dir.x = 3.0f * cosf(a);
			dir.y = 3.0f * sinf(a);
			dir.z = 0.0f;
			g_r28ToppleDirs.push_back(std::make_pair(AsciiString("Tower1"), dir));
		}
		for (int c = 0; c < 8; ++c) g_hdGround[c] = g_r36Grounds[cfg[16] >> (4 * c) & 7];
	};
	/* the pose: the position, the orientation, the transform from them, the geometry (before the constructor, which reads its height) */
	memcpy(&me->m_pos.x, &cfg[6], 4);
	memcpy(&me->m_pos.y, &cfg[7], 4);
	me->m_pos.z = 0.0f;
	memcpy(&me->m_orientation, &cfg[8], 4);
	me->poseMatrix();
	me->m_geometry = GeometryInfo(GEOMETRY_BOX, false, r34F(cfg[9]), r34F(cfg[10]), r34F(cfg[11]));
	scene();
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r36Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) {
				const std::string &l = g_ulog[(size_t)e.p];
				if (l.compare(0, 10, "setorient ") == 0 || l.compare(0, 13, "settransform ") == 0) continue;   /* the transform and the orientation are the dump's */
				log.push_back(l);
			} else if (e.kind == 1 && e.p) {
				char b[32]; sprintf(b, "fx %d", fxId((const FXList *)e.p)); log.push_back(b);
			}
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	StructureToppleUpdate *m = new StructureToppleUpdate(me, d);
	putLog();
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 3 && ((o[1] >= 3 && o[1] <= 5) || (o[1] >= 12 && o[1] <= 17))) cfg[o[1]] = o[2];
		if (o[0] == 2) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		gl.m_awakenCalls = 0;
		gl.m_awakenFrame = 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: g_r36Updating = m; res = (unsigned)m->update(); break;
			case 1: {
				DamageInfo di;
				di.in.m_deathType = (DeathType)o[1];
				di.in.m_sourceID = (ObjectID)o[2];
				m->onDie(&di);
				break;
			}
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r36Saved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_r36Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 6: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r36Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_toppleFrame);
		putU32(realBits(m->m_toppleDirection.x));
		putU32(realBits(m->m_toppleDirection.y));
		putU32((unsigned)m->m_toppleState);
		putU32(realBits(m->m_toppleVelocity));
		putU32(realBits(m->m_accumulatedAngle));
		putU32(realBits(m->m_structuralIntegrity));
		putU32(realBits(m->m_lastCrushedLocation));
		putU32((unsigned)m->m_nextBurstFrame);
		putU32(realBits(m->m_delayBurstLocation.x));
		putU32(realBits(m->m_delayBurstLocation.y));
		putU32(realBits(m->m_delayBurstLocation.z));
		putU32(realBits(m->m_buildingHeight));
		const Matrix3D &mx = me->m_matrix;
		for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) putU32(realBits(mx[r][c]));
		putU32(realBits(me->m_pos.x));
		putU32(realBits(me->m_pos.y));
		putU32(realBits(me->m_pos.z));
		putU32(realBits(me->m_orientation));
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_findUpdateHook = 0;
	g_xformHook = 0;
	g_r28ThingPose = false;
	g_modFindHook = 0;
	g_r28ToppleDirs.clear();
	g_r29BoneFx = false;
	g_r29Me = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	me->m_body = 0; me->m_drawable = 0; me->m_ai = 0;
	delete me;
	delete attacker;
	g_r36Me = g_r36Attacker = 0;
	TheGameLogic = 0;
	TheScriptEngine = 0;
	TheWeaponStore = 0;
	return 0;
}

}  /* extern "C" */
