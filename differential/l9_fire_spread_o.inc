/* Differential-test driver for FireSpreadUpdate (p1w13-sonnet, update batch O; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireSpreadUpdate.cpp   (#included with three mechanical edits, build.rs, mod_gen/FireSpreadUpdate_edit.cpp:
**   the two function-local `static` name keys lose their `static`, and `fu->wouldIgnite()` / `fu->tryToIgnite()` call l9fsWouldIgnite / l9fsTryToIgnite below)
** with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: Objects with status bits, the findUpdateModule hook (the
** candidates' FlammableUpdate is a pointer the test gives), a partition manager whose getClosestObject logs and runs the filters over the list the test gives (nearest
** first) and returns the first object that passes them all, the object creation lists OCL_A and OCL_B of l9_upgrades_b.inc (they log), the game logic whose frame
** the test sets and whose wake recorder counts the calls setWakeFrame makes, and the REAL UpdateModule, INI, Xfer and logic random values. */
Bool l9fsWouldIgnite( FlammableUpdate *fu );
void l9fsTryToIgnite( FlammableUpdate *fu );
#include "FireSpreadUpdate_edit.cpp"   /* FireSpreadUpdate.cpp with three mechanical edits (build.rs, mod_gen) */

namespace {

ModuleData *l9fsRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FireSpreadUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_fsSaved;
Object *g_fsObjs[4];
bool g_fsHasFlammable[4], g_fsWould[4];
unsigned char g_fsBufs[4][16];
std::vector<Object *> g_fsList;

UpdateModule *fsFindHook(const Object *obj, NameKeyType key)
{
	for (int k = 0; k < 4; ++k)
		if (g_fsObjs[k] == obj && g_fsHasFlammable[k] && key == TheNameKeyGenerator->nameToKey("FlammableUpdate")) return (UpdateModule *)(void *)g_fsBufs[k];
	return 0;
}

int fsIndexOf(const void *p)
{
	for (int k = 0; k < 4; ++k) if ((const void *)g_fsBufs[k] == p) return k;
	return -1;
}

}  // namespace

Bool l9fsWouldIgnite( FlammableUpdate *fu ) { int k = fsIndexOf(fu); ulog("wouldignite %d", k + 1); return g_fsWould[k]; }
void l9fsTryToIgnite( FlammableUpdate *fu ) { ulog("tryignite %d", fsIndexOf(fu) + 1); }
Object *PartitionManager::getClosestObject( const Object *obj, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters )
{
	ulog("closest %d %08x %d", (int)obj->m_oid, realBits(maxDist), (int)dc);
	for (size_t i = 0; i < g_fsList.size(); ++i) {
		bool ok = true;
		for (PartitionFilter **f = filters; f && *f; ++f) if (!(*f)->allow(g_fsList[i])) { ok = false; break; }
		if (ok) return g_fsList[i];
	}
	return 0;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a FireSpreadUpdate and make the module on an Object (id 1) with the logic random seed cfg[1] and the frame cfg[0]; objects
** 2 to 4 are the candidates. The data dump goes first (the list (0 none, 1 OCL_A, 2 OCL_B), the two delays, the range). Then the constructor's wake calls. Then `n`
** ops of 5 words [op, a, b, ...]: 0 update()  1 startFireSpreading  2 the module's object is aflame = (a != 0)  3 the candidate list: the objects of the mask a over
** [2, 3, 4], reversed if b  4 candidate a (2 to 4) has a FlammableUpdate = (b & 1), which would ignite = (b & 2)  5 the frame = a  6 xfer save  7 xfer load of the last
** save  8 xfer CRC. Each op appends: the CATCH_ALL status, the sleep time update() returned (0 for the others), the effect log (count, then each as a string: "ocl <i>",
** "closest <object> <range> <dc>", "wouldignite <object>", "tryignite <object>"), the number of wake calls and the frame of the last, the xfer status, bytes and CRC. */
static int l9fsRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fsSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9fsRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const FireSpreadUpdateModuleData *fd = (const FireSpreadUpdateModuleData *)d;
		putU32(fd->m_oclEmbers == 0 ? 0 : (fd->m_oclEmbers == &g_upOcls[0] ? 1 : 2));
		putU32(fd->m_minSpreadTryDelayData);
		putU32(fd->m_maxSpreadTryDelayData);
		putU32(realBits(fd->m_spreadTryRange));
	}
	for (int k = 0; k < 4; ++k) {
		g_fsObjs[k] = newObject();
		g_fsObjs[k]->m_oid = k + 1;
		g_fsHasFlammable[k] = false;
		g_fsWould[k] = false;
		memset(g_fsBufs[k], 0, 16);
	}
	g_fsList.clear();
	g_findUpdateHook = fsFindHook;
	ThePartitionManager = new PartitionManager;
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[1]);
	g_events.clear();
	g_ulog.clear();
	FireSpreadUpdate *m = 0;
	status = 0;
	try { m = new FireSpreadUpdate(g_fsObjs[0], d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0; g_findUpdateHook = 0; delete ThePartitionManager; ThePartitionManager = 0;
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
			gl.m_awakenCalls = 0;
			gl.m_awakenFrame = 0;
			try {
				switch (o[0]) {
				case 0: sleep = (int)m->update(); break;
				case 1: m->startFireSpreading(); break;
				case 2: { ObjectStatusMaskType a; a.set(OBJECT_STATUS_AFLAME); if (o[1]) g_fsObjs[0]->m_status.set(a); else g_fsObjs[0]->m_status.clear(a); break; }
				case 3:
					g_fsList.clear();
					for (int k = 0; k < 3; ++k) if (o[1] >> k & 1) g_fsList.push_back(g_fsObjs[1 + k]);
					if (o[2]) std::reverse(g_fsList.begin(), g_fsList.end());
					break;
				case 4: g_fsHasFlammable[1 + o[1] % 3] = (o[2] & 1) != 0; g_fsWould[1 + o[1] % 3] = (o[2] & 2) != 0; break;
				case 5: gl.m_frame = o[1]; break;
				case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_fsSaved.assign(bytes, bytes + blen); break;
				case 7: {
					std::vector<unsigned char> b = g_fsSaved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				default: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
		}
		if (phase) { putI32(st); putI32(sleep); }
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		if (phase) {
			putI32(xst);
			putU32((unsigned)blen);
			for (int j = 0; j < blen; ++j) putU32(bytes[j]);
			putU32(crc);
		}
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) delete g_fsObjs[k];
	delete ThePartitionManager;
	ThePartitionManager = 0;
	TheGameLogic = 0;
	g_findUpdateHook = 0;
	return 0;
}

int ref_l9fs_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9fsRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
