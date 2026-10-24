/* Differential-test driver for BridgeTowerBehavior (p1w13-sonnet, batch R38; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeTowerBehavior.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: six objects (ids 1-6; the kinds BRIDGE, BRIDGE_TOWER and STRUCTURE,
** maybe the module build's MBody (a maximum of four, half health; attemptDamage and attemptHealing lines), up to three behaviour probes (the
** active-body driver's AbProbe: maybe a bridge interface, each with its four tower ids)), the tower (object 1), Object::attemptDamage (the
** flammable driver's: the body's), the REAL Object::attemptHealing (mod_gen/ObjectHealing.inc) and Object::kill (mod_gen/ObjectKill.inc),
** BehaviorModule, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeTowerBehavior.cpp"

namespace {

ModuleData *l9r38Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = BridgeTowerBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r38Saved;
Object *g_r38Objs[6];
Object *r38Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 6) ? g_r38Objs[(int)id - 1] : 0; }
Object *r38Find(unsigned id) { return r38Find((ObjectID)id); }
/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_r38Kinds[3] = { KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER, KINDOF_STRUCTURE };
/* the bodies' maximum health, by index (the same list in the Rust test) */
const Real g_r38Max[4] = { 100.0f, 250.0f, 1000.0f, 0.0f };
Real r38F(unsigned bits) { Real f; memcpy(&f, &bits, 4); return f; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a BridgeTowerBehavior and make it on object 1 of six, then setBridge(object cfg[1], NULL for none)
** and setTowerType(cfg[2]). `cfg` (24 words; 3-8 and 9-11 also settable by op 7): 3-8 objects 1-6: bits 0-2 their kinds (of g_r38Kinds), 3
** a body, 4-5 its maximum (of g_r38Max; read once, at the start), 6-7 the number of behaviour probes (0-3), 8-10 which probes have a bridge
** interface; 9-11 probe 0-2's tower ids (nibbles 0-3, 0 none, 7 no object). The data dump goes first (nothing). Then `n` ops of 4 words [op, a,
** b, c]: 0 onDamage (amount a as float bits, damage type b & 255, death type b >> 8, source c)  1 onHealing (amount a, source c; the damage type
** DAMAGE_HEALING)  2 onBodyDamageStateChange(info, a, b)  3 onDie  4 setBridge(object a)  5 setTowerType(a)  6 getBridgeID  7 cfg[a] = b
** 8 xfer save  9 xfer load of the last save  10 xfer CRC  11 getBridgeTowerBehaviorInterfaceFromObject(object a) (1 this module, 2 another,
** 0 NULL). Each op appends: the CATCH_ALL status, the result, the effect log (the bodies' damage and heal lines), the xfer status, bytes and
** CRC, the state (the bridge id, the tower type), and per object the body's health bits. */
int ref_l9r38_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r38Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r38Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[24];
	memcpy(cfg, cfg_in, sizeof(cfg));
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = 1;
	TheGameLogic = &gl;
	MBody bodies[6];
	ModuleData *probeData = new BehaviorModuleData;
	AbProbe *probes[6][3];
	BehaviorModule *behaviors[6][5];
	for (int k = 0; k < 6; ++k) {
		g_r38Objs[k] = newObject();
		g_r38Objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = g_r38Objs[k];
		bodies[k].m_max = g_r38Max[cfg[3 + k] >> 4 & 3];
		bodies[k].m_health = bodies[k].m_max * 0.5f;
		for (int i = 0; i < 3; ++i) probes[k][i] = new AbProbe(g_r38Objs[k], probeData, i + 1);
	}
	Object *me = g_r38Objs[0];
	BridgeTowerBehavior *m = new BridgeTowerBehavior(me, d);
	auto scene = [&]() {
		for (int k = 0; k < 6; ++k) {
			Object *o = g_r38Objs[k];
			unsigned f = cfg[3 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 3; ++b) if (f >> b & 1) o->m_kindOf.set(g_r38Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_body = (f >> 3 & 1) ? &bodies[k] : 0;
			int j = 0;
			if (k == 0) behaviors[k][j++] = m;
			unsigned np = f >> 6 & 3;
			for (unsigned i = 0; i < np && i < 3; ++i) {
				AbProbe *p = probes[k][i];
				p->m_bridge = (f >> (8 + i) & 1) != 0;
				for (int t = 0; t < 4; ++t) p->m_towers[t] = (ObjectID)(cfg[9 + i] >> (4 * t) & 15);
				behaviors[k][j++] = p;
			}
			behaviors[k][j] = 0;
			o->m_behaviors = behaviors[k];
		}
	};
	scene();
	g_modFindHook = r38Find;
	g_events.clear();
	g_ulog.clear();
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind != 100) continue;
			const std::string &l = g_ulog[(size_t)e.p];
			if (l.compare(0, 7, "damage ") == 0 || l.compare(0, 5, "heal ") == 0) log.push_back(l);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	m->setBridge(r38Find(cfg[1]));
	m->setTowerType((BridgeTowerType)cfg[2]);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 7 && ((o[1] >= 3 && o[1] <= 8) || (o[1] >= 9 && o[1] <= 11))) cfg[o[1]] = o[2];
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
			case 0: case 1: case 2: {
				DamageInfo di;
				di.in.m_amount = r38F(o[1]);
				di.in.m_damageType = o[0] == 1 ? DAMAGE_HEALING : (DamageType)(o[2] & 255);
				di.in.m_deathType = o[0] == 1 ? DEATH_NONE : (DeathType)(o[2] >> 8);
				di.in.m_sourceID = (ObjectID)o[3];
				if (o[0] == 0) m->onDamage(&di);
				else if (o[0] == 1) m->onHealing(&di);
				else m->onBodyDamageStateChange(&di, (BodyDamageType)o[1], (BodyDamageType)o[2]);
				break;
			}
			case 3: { DamageInfo di; m->onDie(&di); break; }
			case 4: m->setBridge(r38Find(o[1])); break;
			case 5: m->setTowerType((BridgeTowerType)o[1]); break;
			case 6: res = (unsigned)m->getBridgeID(); break;
			case 8: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r38Saved.assign(bytes, bytes + blen); break;
			case 9: {
				std::vector<unsigned char> b = g_r38Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 10: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 11: {
				BridgeTowerBehaviorInterface *bi = BridgeTowerBehavior::getBridgeTowerBehaviorInterfaceFromObject(r38Find(o[1]));
				res = bi == 0 ? 0 : (bi == static_cast<BridgeTowerBehaviorInterface *>(m) ? 1 : 2);
				break;
			}
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
		putU32((unsigned)m->getBridgeID());
		putU32((unsigned)m->m_type);
		for (int k = 0; k < 6; ++k) putU32(realBits(bodies[k].m_health));
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	delete m;
	delete d;
	for (int k = 0; k < 6; ++k) {
		Object *x = g_r38Objs[k];
		x->m_body = 0; x->m_behaviors = 0;
		for (int i = 0; i < 3; ++i) delete probes[k][i];
		delete x;
		g_r38Objs[k] = 0;
	}
	delete probeData;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
