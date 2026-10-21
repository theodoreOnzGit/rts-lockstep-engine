/* Differential-test driver for FloatUpdate (p1w13-sonnet, update batch O; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FloatUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: an Object with a position the test sets, an optional Drawable
** whose instance matrix calls log the game logic's frame at the call (the matrix maths is the client's), a terrain logic whose isUnderwater logs and answers from
** tables the test sets (by a hash of the position; it assigns waterZ only where it has water, as upstream does), the game logic whose frame the test sets, and the
** REAL UpdateModule, INI and Xfer. */
#define INT_TO_REAL(x) ((Real)(x))   /* Lib/BaseType.h:240, as the real header */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FloatUpdate.cpp"

namespace {

ModuleData *l9flrRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FloatUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_flrSaved;
bool g_flrHasWater[8];
Real g_flrWater[8];
Matrix3D g_flrMatrix;
GameLogic *g_flrLogic = 0;

}  // namespace

Bool TerrainLogic::isUnderwater( Real x, Real y, Real *waterZ, Real *terrainZ )
{
	ulog("underwater %08x %08x", realBits(x), realBits(y));
	unsigned c = hdCell(x, y);
	if (!g_flrHasWater[c]) return FALSE;
	if (waterZ) *waterZ = g_flrWater[c];
	return TRUE;
}
const Matrix3D *Drawable::getInstanceMatrix() const { return &g_flrMatrix; }
void Drawable::setInstanceMatrix(const Matrix3D *instance) { ulog("instance %u", g_flrLogic->m_frame); }

extern "C" {

/* Read `filename` (block `D`) as the data of a FloatUpdate and make the module on an Object (id 1) at the position cfg[1..3] (float bits) with the frame cfg[0], a
** drawable if cfg[4] is not 0. The data dump goes first (the enabled flag). Then `n` ops of 5 words [op, a, b, c, ...]: 0 update()  1 position = (a, b, c) float
** bits  2 the frame = a  3 the water of cell a = (b != 0, c as float bits)  4 setEnabled(a != 0)  5 the drawable present = (a != 0)  6 xfer save  7 xfer load of the
** last save  8 xfer CRC. The constructor's line and each op append: the CATCH_ALL status and the sleep time update() returned (0 for the others), the effect log (count,
** then each as a string: "underwater <x> <y>", "instance <frame>"), the xfer status, bytes and CRC, the module's state word (enabled) and the object's position. */
static int l9flrRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_flrSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9flrRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	putU32(((FloatUpdateModuleData *)d)->m_enabled ? 1 : 0);
	Object *obj = newObject();
	obj->m_oid = 1;
	memcpy(&obj->m_pos.x, &cfg[1], 4);
	memcpy(&obj->m_pos.y, &cfg[2], 4);
	memcpy(&obj->m_pos.z, &cfg[3], 4);
	obj->m_drawable = cfg[4] ? &g_cpDrawable : 0;
	for (int i = 0; i < 8; ++i) { g_flrHasWater[i] = false; g_flrWater[i] = 0.0f; }
	g_flrMatrix.Make_Identity();
	TerrainLogic terrain;
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	g_flrLogic = &gl;
	TheTerrainLogic = &terrain;
	g_events.clear();
	g_ulog.clear();
	FloatUpdate *m = 0;
	status = 0;
	try { m = new FloatUpdate(obj, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0; TheTerrainLogic = 0;
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
				case 1: memcpy(&obj->m_pos.x, &o[1], 4); memcpy(&obj->m_pos.y, &o[2], 4); memcpy(&obj->m_pos.z, &o[3], 4); break;
				case 2: gl.m_frame = o[1]; break;
				case 3: g_flrHasWater[o[1] & 7] = o[2] != 0; memcpy(&g_flrWater[o[1] & 7], &o[3], 4); break;
				case 4: m->setEnabled(o[1] != 0); break;
				case 5: obj->m_drawable = o[1] ? &g_cpDrawable : 0; break;
				case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_flrSaved.assign(bytes, bytes + blen); break;
				case 7: {
					std::vector<unsigned char> b = g_flrSaved;
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
		putU32(m->m_enabled ? 1 : 0);
		putU32(realBits(obj->m_pos.x));
		putU32(realBits(obj->m_pos.y));
		putU32(realBits(obj->m_pos.z));
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	TheTerrainLogic = 0;
	return 0;
}

int ref_l9flr_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9flrRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
