/* Differential-test driver for MobMemberSlavedUpdate (p1w13-sonnet, batch R58; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/MobMemberSlavedUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: four objects (ids 1-4; positions the test's;
** each a drawable or not, the module build's MBody, whose attemptDamage is a line), the member (object 1: maybe an AI with a locomotor, moving
** or not, a current victim, a last command source, a next mood target, a goal position and a distance to it; its drawable's flags), the nexus
** and others (objects 2-4: maybe an AI, moving, idle, attacking, a victim, a goal position and a distance to it; maybe a spawn behaviour that
** may let members pick their targets), the distances (batch R57's hooks to PartitionManager.cpp's procedures), the AI's calls (lines), the
** drawable's flag clear (batch R43's line), setStatus and clearDisabled (lines), the REAL Object::kill, logic random values (the test's seed),
** UpdateModule, INI and Xfer; the game logic (its frame). */
/* (the file's two file-level constants are SlavedUpdate.cpp's names too, in this one unit: renamed for the include, the text unchanged) */
#define CLOSE_ENOUGH R58_CLOSE_ENOUGH
#define CLOSE_ENOUGH_SQR R58_CLOSE_ENOUGH_SQR
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/MobMemberSlavedUpdate.cpp"
#undef CLOSE_ENOUGH
#undef CLOSE_ENOUGH_SQR

namespace {

ModuleData *l9r58Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = MobMemberSlavedUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r58Saved;
Object *g_r58Objs[4];
Object *r58Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 4) ? g_r58Objs[(int)id - 1] : 0; }
Real r58F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a MobMemberSlavedUpdate and make the module on object 1 of four at frame cfg[0] (the constructor
** draws from the logic random values, seeded with cfg[1]), then onObjectCreated. `cfg` (24 words; 2-23 also settable by op 2): 2 the member:
** bit 0 an AI, 1 its locomotor, 2 moving, 3 a drawable, 4 its flags have WEAPONSET_PLAYER_UPGRADE, 5-7 its victim (objects 1-4, else NULL),
** 8-11 its last command source, 12-14 its next mood target (likewise); 3-5 objects 2-4: bit 0 an AI, 1 moving, 2 idle, 3 attacking, 4 a
** drawable, 5 a spawn behaviour, 6 it may let members self-task, 7-9 the AI's victim; 6 the member's distance to its goal, 7 the others'
** (float bits); 8-11 objects 1-4's x, 12-15 their y (float bits; z 0); 16-17 the member's goal (x, y), 18-19 the others' goal (float bits;
** z 0). The data dump goes first, then the constructor's state and onObjectCreated's effects. Then `n` ops of 5 words [op, a, b, c, d]:
** 0 update()  1 the frame = a  2 cfg[a] = b  3 onEnslave(object a, else NULL)  4 onSlaverDie(NULL)  5 onSlaverDamage(damage type a, amount b
** bits)  6 xfer save  7 xfer load of the last save  8 xfer CRC  9 onObjectCreated  10 getSlaverID. Each op appends: the CATCH_ALL status, the
** result, the effect log, the xfer status, bytes and CRC, the state (the slaver, the countdown, the mob state, the colour, the primary
** victim, the squirrelliness, self-tasking, the crisis timer) and the member's UNSELECTABLE bit. */
static int l9r58RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r58Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r58Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[24];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const MobMemberSlavedUpdateModuleData *md = (const MobMemberSlavedUpdateModuleData *)d;
		putI32(md->m_mustCatchUpRadius);
		putI32(md->m_noNeedToCatchUpRadius);
		putU32(realBits(md->m_squirrellinessRatio));
		putU32(md->m_catchUpCrisisBailTime);
	}
	InitGameLogicRandom(cfg[1]);
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	ThePartitionManager = &g_ahPm;
	MBody bodies[4];
	Drawable draws[4];
	AIUpdateInterface ais[4];
	SpawnBehaviorInterface spawns[4];
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 4; ++k) {
		g_r58Objs[k] = newObject();
		g_r58Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r58Objs[k]->m_behaviors = none;
		bodies[k].m_owner = g_r58Objs[k];
		bodies[k].m_max = 200.0f;
		bodies[k].m_health = 100.0f;
		draws[k].m_ownerObject = g_r58Objs[k];
		ais[k].m_owner = g_r58Objs[k];
	}
	Object *me = g_r58Objs[0];
	auto obj = [&](unsigned v) -> Object * { return (v >= 1 && v <= 4) ? g_r58Objs[v - 1] : 0; };
	auto scene = [&]() {
		unsigned f = cfg[2];
		me->m_ai = (f & 1) ? &ais[0] : 0;
		ais[0].m_hasLoco = (f >> 1 & 1) != 0;
		ais[0].m_moving = (f >> 2 & 1) != 0;
		me->m_drawable = (f >> 3 & 1) ? &draws[0] : 0;
		draws[0].m_condFlags.clear();
		if (f >> 4 & 1) draws[0].m_condFlags.set(MODELCONDITION_WEAPONSET_PLAYER_UPGRADE);
		ais[0].m_victim = obj(f >> 5 & 7);
		ais[0].m_lastCmdSource = (Int)(f >> 8 & 15);
		ais[0].m_r58Mood = obj(f >> 12 & 7);
		ais[0].m_r58Dist = r58F(cfg[6]);
		ais[0].m_r58Goal.x = r58F(cfg[16]);
		ais[0].m_r58Goal.y = r58F(cfg[17]);
		ais[0].m_r58Goal.z = 0.0f;
		for (int k = 1; k < 4; ++k) {
			Object *o = g_r58Objs[k];
			unsigned g = cfg[2 + k];
			o->m_ai = (g & 1) ? &ais[k] : 0;
			ais[k].m_moving = (g >> 1 & 1) != 0;
			ais[k].m_idle = (g >> 2 & 1) != 0;
			ais[k].m_r58Attacking = (g >> 3 & 1) != 0;
			o->m_drawable = (g >> 4 & 1) ? &draws[k] : 0;
			o->m_spawn = (g >> 5 & 1) ? &spawns[k] : 0;
			spawns[k].m_r58MaySelfTask = (g >> 6 & 1) != 0;
			ais[k].m_victim = obj(g >> 7 & 7);
			ais[k].m_r58Dist = r58F(cfg[7]);
			ais[k].m_r58Goal.x = r58F(cfg[18]);
			ais[k].m_r58Goal.y = r58F(cfg[19]);
			ais[k].m_r58Goal.z = 0.0f;
		}
		for (int k = 0; k < 4; ++k) {
			Object *o = g_r58Objs[k];
			o->m_body = &bodies[k];   /* always: Object::kill reads the body unchecked */
			o->m_pos.x = r58F(cfg[8 + k]);
			o->m_pos.y = r58F(cfg[12 + k]);
			o->m_pos.z = 0.0f;
		}
	};
	scene();
	g_modFindHook = r58Find;
	g_distDcHook = r57Dist;
	g_distPosDcHook = r57DistPos;
	g_setStatusHook = r57SetStatus;
	g_mcHook = r43Mc;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	MobMemberSlavedUpdate *m = 0;
	try { m = newInstance(MobMemberSlavedUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	auto putState = [&]() {
		putU32((unsigned)m->m_slaver);
		putU32((unsigned)m->m_framesToWait);
		putU32((unsigned)m->m_mobState);
		putU32(realBits(m->m_personalColor.red));
		putU32(realBits(m->m_personalColor.green));
		putU32(realBits(m->m_personalColor.blue));
		putU32((unsigned)m->m_primaryVictimID);
		putU32(realBits(m->m_squirrellinessRatio));
		putU32(m->m_isSelfTasking ? 1 : 0);
		putU32(m->m_catchUpCrisisTimer);
		putU32(me->m_status.test(OBJECT_STATUS_UNSELECTABLE) ? 1 : 0);
	};
	putState();
	g_events.clear();
	g_ulog.clear();
	m->onObjectCreated();
	putLog();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 23) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
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
			case 0: res = (unsigned)m->update(); break;
			case 3: m->onEnslave(r58Find((ObjectID)o[1])); break;
			case 4: m->onSlaverDie(0); break;
			case 5: { DamageInfo di; di.in.m_damageType = (DamageType)o[1]; di.in.m_amount = r58F(o[2]); m->onSlaverDamage(&di); break; }
			case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r58Saved.assign(bytes, bytes + blen); break;
			case 7: {
				std::vector<unsigned char> b = g_r58Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 8: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 9: m->onObjectCreated(); break;
			case 10: res = (unsigned)m->getSlaverID(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putState();
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_distDcHook = 0;
	g_distPosDcHook = 0;
	g_setStatusHook = 0;
	g_mcHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) {
		Object *x = g_r58Objs[k];
		x->m_ai = 0; x->m_body = 0; x->m_drawable = 0; x->m_spawn = 0; x->m_behaviors = 0;
		delete x;
		g_r58Objs[k] = 0;
	}
	ThePartitionManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r58_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r58RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
