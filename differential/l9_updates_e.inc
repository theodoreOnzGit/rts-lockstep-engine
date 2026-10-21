/* Differential-test driver for two layer-9 update modules (p1w11-sonnet, batch E; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PowerPlantUpdate.cpp, RadarUpdate.cpp   (#included, unchanged)
** with their headers unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_upgrades.inc, whose effect log it uses) on the
** stand-ins of differential/shim_module: an Object with or without a Drawable, a Drawable that logs the model-condition calls it is given (a set, a
** clear of a set of flags, a clear-and-set: the bits in ascending order), the game logic whose frame the test sets and whose wake recorder
** (m_awakenCalls, m_awakenFrame) counts the calls `setWakeFrame` makes, and the REAL UpdateModule, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PowerPlantUpdate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/RadarUpdate.cpp"

namespace {

std::string flagBitsE(const ModelConditionFlags &f)
{
	std::string s;
	for (int i = 0; i < MODELCONDITION_COUNT; ++i) {
		if (f.test(i)) {
			if (!s.empty()) s += ",";
			s += std::to_string(i);
		}
	}
	return s;
}

ModuleData *l9ueRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = kind == 0 ? (ModuleData *)PowerPlantUpdate::friend_newModuleData(&ini) : (ModuleData *)RadarUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_ueSaved;
Drawable g_ueDrawable;

}  // namespace

void Drawable::clearModelConditionFlags(const ModelConditionFlags &clr) { ulog("clearflags %s", flagBitsE(clr).c_str()); }
void Drawable::clearAndSetModelConditionState(ModelConditionFlagType clr, ModelConditionFlagType set) { ulog("clearset %d %d", (int)clr, (int)set); }

extern "C" {

/* Read `filename` (block `D`) as the data of update module `kind` (0 PowerPlantUpdate, 1 RadarUpdate), make the module on an Object (with a drawable if
** cfg[0] is not 0) and run `n` ops of 6 words [op, a, ...]: 0 update()  1 extendRods(a != 0) / extendRadar()  2 the game logic's frame = a
** 3 the drawable present = (a != 0)  4 xfer save  5 xfer load of the last save  6 xfer CRC. The data dump goes first (the time: u32 for a power plant, float
** bits for a radar), then the constructor's wake calls (count, frame of the last). Each op appends: the CATCH_ALL status, the sleep time update() returned (0 for the other ops), the effect log (count, then each as a
** string), the number of wake calls and the frame of the last, the xfer status, bytes and CRC, and the module's state words (count, then each). */
int ref_l9ue_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_ueSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9ueRead(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	if (kind == 0) putU32(((PowerPlantUpdateModuleData *)d)->m_rodsExtendTime);
	else putU32(realBits(((RadarUpdateModuleData *)d)->m_radarExtendTime));
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_drawable = cfg[0] ? &g_ueDrawable : 0;
	GameLogic gl;
	TheGameLogic = &gl;
	UpdateModule *m = 0;
	if (kind == 0) m = new PowerPlantUpdate(obj, d);
	else m = new RadarUpdate(obj, d);
	putU32((unsigned)gl.m_awakenCalls);        /* the constructor's wake calls, and the frame of the last */
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
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
		int sleep = 0;
		try {
			switch (o[0]) {
			case 0: sleep = (int)m->update(); break;
			case 1: if (kind == 0) ((PowerPlantUpdate *)m)->extendRods(o[1] != 0); else ((RadarUpdate *)m)->extendRadar(); break;
			case 2: gl.m_frame = o[1]; break;
			case 3: obj->m_drawable = o[1] ? &g_ueDrawable : 0; break;
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_ueSaved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_ueSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			default: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putI32(sleep);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		if (kind == 0) {
			putU32(1);
			putU32(((PowerPlantUpdate *)m)->m_extended ? 1 : 0);
		} else {
			RadarUpdate *r = (RadarUpdate *)m;
			putU32(3);
			putU32(r->m_extendDoneFrame);
			putU32(r->m_extendComplete ? 1 : 0);
			putU32(r->m_radarActive ? 1 : 0);
		}
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
