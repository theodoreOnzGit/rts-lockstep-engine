/* Differential-test driver for CheckpointUpdate (p1w12-sonnet, update batch N; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CheckpointUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: an Object with the REAL GeometryInfo (Geometry.cpp),
** a vision range and an optional Drawable that logs the calls it is given, an AI whose two queries log and answer from flags the test sets, the game logic
** whose frame the test sets and whose wake recorder counts the calls setWakeFrame makes, and the REAL UpdateModule, INI, Xfer and logic random values. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CheckpointUpdate.cpp"

namespace {

ModuleData *l9cpRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = CheckpointUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_cpSaved;
Drawable g_cpDrawable;
Object g_cpEnemy, g_cpAlly;
bool g_cpEnemyNear = false, g_cpAllyNear = false;
AI g_cpAi;

}  // namespace

/* Object.cpp lines 770-784, without the partition manager (the stand-in object is never registered with it) */
void Object::setGeometryInfo(const GeometryInfo &geom)
{
	m_geometry = geom;
	if (m_drawable)
		m_drawable->reactToGeometryChange();
}
void Drawable::reactToGeometryChange() { ulog("react"); }
Object *AI::findClosestEnemy( const Object *me, Real range, UnsignedInt qualifiers, const AttackPriorityInfo *info, PartitionFilter *optionalFilter )
{
	ulog("ai enemy %08x %u %08x", realBits(range), qualifiers, realBits(me->getGeometryInfo().getMinorRadius()));
	return g_cpEnemyNear ? &g_cpEnemy : 0;
}
Object *AI::findClosestAlly( const Object *me, Real range, UnsignedInt qualifiers )
{
	ulog("ai ally %08x %u %08x", realBits(range), qualifiers, realBits(me->getGeometryInfo().getMinorRadius()));
	return g_cpAllyNear ? &g_cpAlly : 0;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a CheckpointUpdate and make the module on an Object whose geometry has the minor radius cfg[0] (float bits;
** the rest of the geometry is a sphere of radius 1), with a drawable if cfg[1] is not 0, the logic random seed cfg[2], the vision range cfg[3] (float
** bits) and the frame cfg[4]. The data dump goes first (the scan delay time). Then `n` ops of 5 words [op, a, ...]: 0 update()  1 an enemy is near = (a != 0)
** 2 an ally is near = (a != 0)  3 the vision range = a (float bits)  4 the drawable present = (a != 0)  5 the geometry's minor radius = a (float bits)  6 xfer
** save  7 xfer load of the last save  8 xfer CRC. The constructor's line and each op append: the CATCH_ALL status and the sleep time update() returned (0 for
** the constructor and the other ops), the effect log (count, then each as a string: "ai enemy <range> <qualifiers> <minor radius>", "ai ally ...", "react", "clearset <clear>
** <set>"), the xfer status, bytes and CRC, the module's four state words (enemy near, ally near, maximum minor radius bits, scan delay) and the object's minor
** radius bits. */
static int l9cpRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_cpSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9cpRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	putU32(((CheckpointUpdateModuleData *)d)->m_enemyScanDelayTime);
	Object *obj = newObject();
	obj->m_oid = 1;
	{ Real minor, vision; memcpy(&minor, &cfg[0], 4); memcpy(&vision, &cfg[3], 4); obj->m_geometry.setMinorRadius(minor); obj->m_vision = vision; }
	obj->m_drawable = cfg[1] ? &g_cpDrawable : 0;
	GameLogic gl;
	gl.m_frame = cfg[4];
	TheGameLogic = &gl;
	TheAI = &g_cpAi;
	g_cpEnemyNear = g_cpAllyNear = false;
	InitGameLogicRandom(cfg[2]);
	g_events.clear();
	g_ulog.clear();
	CheckpointUpdate *m = 0;
	status = 0;
	try { m = new CheckpointUpdate(obj, d); }
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
				case 2: g_cpAllyNear = o[1] != 0; break;
				case 3: memcpy(&obj->m_vision, &o[1], 4); break;
				case 4: obj->m_drawable = o[1] ? &g_cpDrawable : 0; break;
				case 5: { Real r; memcpy(&r, &o[1], 4); obj->m_geometry.setMinorRadius(r); break; }
				case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_cpSaved.assign(bytes, bytes + blen); break;
				case 7: {
					std::vector<unsigned char> b = g_cpSaved;
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
		putU32(m->m_enemyNear ? 1 : 0);
		putU32(m->m_allyNear ? 1 : 0);
		putU32(realBits(m->m_maxMinorRadius));
		putU32(m->m_enemyScanDelay);
		putU32(realBits(obj->m_geometry.getMinorRadius()));
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	TheAI = 0;
	return 0;
}

int ref_l9cp_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9cpRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
