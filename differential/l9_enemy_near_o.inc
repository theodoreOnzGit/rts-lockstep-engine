/* Differential-test driver for EnemyNearUpdate (p1w13-sonnet, update batch O; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/EnemyNearUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: an Object with a vision range and an optional Drawable that
** logs the model-condition calls, an AI whose findClosestEnemy logs and answers from the flag the test sets (CheckpointUpdate's, l9_checkpoint_n.inc), and the REAL
** UpdateModule, INI, Xfer and logic random values. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/EnemyNearUpdate.cpp"

namespace {

ModuleData *l9enRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = EnemyNearUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_enSaved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of an EnemyNearUpdate and make the module on an Object (id 1) with a drawable if cfg[0] is not 0, the logic random seed
** cfg[1], the vision range cfg[2] (float bits). The data dump goes first (the scan delay time). Then `n` ops of 5 words [op, a, ...]: 0 update()  1 an enemy is near
** = (a != 0)  2 the vision range = a (float bits)  3 the drawable present = (a != 0)  4 xfer save  5 xfer load of the last save  6 xfer CRC. The constructor's line and
** each op append: the CATCH_ALL status and the sleep time update() returned (0 for the others), the effect log (count, then each as a string: "ai enemy <range>
** <qualifiers> <minor radius>", "mc <n>", "mcclear <n>"), the xfer status, bytes and CRC, the module's two state words (scan delay, enemy flag). */
static int l9enRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_enSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9enRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	putU32(((EnemyNearUpdateModuleData *)d)->m_enemyScanDelayTime);
	Object *obj = newObject();
	obj->m_oid = 1;
	memcpy(&obj->m_vision, &cfg[2], 4);
	obj->m_drawable = cfg[0] ? &g_cpDrawable : 0;
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	TheAI = &g_cpAi;
	g_cpEnemyNear = g_cpAllyNear = false;
	InitGameLogicRandom(cfg[1]);
	g_events.clear();
	g_ulog.clear();
	EnemyNearUpdate *m = 0;
	status = 0;
	try { m = new EnemyNearUpdate(obj, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0; TheAI = 0;
		return 5;
	}
	for (int phase = 0; phase < 1 + n; ++phase) {
		const unsigned *o = phase ? ops + 5 * (phase - 1) : 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		int sleep = 0;
		if (phase) {
			g_events.clear();
			g_ulog.clear();
			try {
				switch (o[0]) {
				case 0: sleep = (int)m->update(); break;
				case 1: g_cpEnemyNear = o[1] != 0; break;
				case 2: memcpy(&obj->m_vision, &o[1], 4); break;
				case 3: obj->m_drawable = o[1] ? &g_cpDrawable : 0; break;
				case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_enSaved.assign(bytes, bytes + blen); break;
				case 5: {
					std::vector<unsigned char> b = g_enSaved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				default: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
		}
		putI32(st);
		putI32(sleep);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_enemyScanDelay);
		putU32(m->m_enemyNear ? 1 : 0);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	TheAI = 0;
	return 0;
}

int ref_l9en_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9enRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
