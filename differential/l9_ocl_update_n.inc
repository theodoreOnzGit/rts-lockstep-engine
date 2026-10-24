/* Differential-test driver for OCLUpdate (p1w12-sonnet, update batch N; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/OCLUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: an Object with a position, an orientation, status bits
** and a disabled flag, three players (a colour, a playable flag and a template with a side), a terrain logic whose findClosestEdgePoint adds an offset the
** test sets, the object creation lists OCL_A and OCL_B of batch B (the five-argument create is new here: it logs), the game logic whose frame the test
** sets, and the REAL UpdateModule, INI, Xfer and logic random values. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/OCLUpdate.cpp"

namespace {

ModuleData *l9ocRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = OCLUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_ocSaved;
TerrainLogic g_ocTerrain;
Coord3D g_ocEdgeOffset;

int oclCode(const ObjectCreationList *o) { return o == 0 ? 0 : (o == &g_upOcls[0] ? 1 : 2); }

}  // namespace

TerrainLogic *TheTerrainLogic = 0;
Coord3D TerrainLogic::findClosestEdgePoint( const Coord3D *closestTo ) const
{
	Coord3D r;
	r.x = closestTo->x + g_ocEdgeOffset.x;
	r.y = closestTo->y + g_ocEdgeOffset.y;
	r.z = closestTo->z + g_ocEdgeOffset.z;
	return r;
}
Object *ObjectCreationList::createInternal(const Object *primaryObj, const Coord3D *primary, const Coord3D *secondary, Real angle, UnsignedInt lifetimeFrames) const
{
	if (!secondary) {   /* batch R29: StructureCollapseUpdate gives no secondary */
		ulog("oclat %d %08x,%08x,%08x null %08x", this == &g_upOcls[0] ? 0 : 1, realBits(primary->x), realBits(primary->y), realBits(primary->z), realBits(angle));
		return 0;
	}
	ulog("oclat %d %08x,%08x,%08x %08x,%08x,%08x %08x", this == &g_upOcls[0] ? 0 : 1, realBits(primary->x), realBits(primary->y), realBits(primary->z),
		realBits(secondary->x), realBits(secondary->y), realBits(secondary->z), realBits(angle));
	return 0;
}

extern "C" {

/* Read `filename` (block `D`) as the data of an OCLUpdate and make the module on an Object at the frame cfg[0], with the logic random seed cfg[1]. The
** object is controlled by player cfg[2] of three (0 none), at (11, 22, 5), with orientation 0.5. The players have the colours 100, 200, 300, are playable,
** and have templates with the sides "China", "USA", "GLA". The data dump goes first (the list, then the faction lists (count, then name and list), the
** delays, the flags). Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 disabled = (a != 0)  3 under construction = (a != 0)  4 the
** controlling player = a (0 none)  5 player b's colour = c  6 player b's playable flag = (c != 0)  7 player b's template: none if c = 0, else the side of
** player c - 1's  8 resetTimer  9 getCountdownPercent  10 getRemainingFrames  11 xfer save  12 xfer load of the last save  13 xfer CRC  14 the position = (a, b, c)
** float bits  15 the orientation = a  16 the edge offset = (a, b, c). Each op appends: the CATCH_ALL status, the result (the sleep time update() returned, the
** float bits or the count), the effect log (count, then each as a string: "oclat <list> <pos> <pos2> <angle>"), the xfer status, bytes and CRC, and the
** module's four state words. The constructor's line has the state words only. */
static int l9ocRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_ocSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9ocRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const OCLUpdateModuleData *od = (const OCLUpdateModuleData *)d;
		putU32((unsigned)oclCode(od->m_ocl));
		putU32((unsigned)od->m_factionOCL.size());
		for (OCLUpdateModuleData::FactionOCLList::const_iterator it = od->m_factionOCL.begin(); it != od->m_factionOCL.end(); ++it) {
			putStr(it->m_factionName.c_str(), (int)it->m_factionName.size());
			putU32((unsigned)oclCode(it->m_ocl));
		}
		putU32(od->m_minDelay);
		putU32(od->m_maxDelay);
		putU32(od->m_isCreateAtEdge ? 1 : 0);
		putU32(od->m_isFactionTriggered ? 1 : 0);
	}
	static const char *sides[3] = { "China", "USA", "GLA" };
	Player players[3];
	PlayerTemplate templates[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_color = 100 * (i + 1);
		players[i].m_playable = true;
		templates[i].m_side = sides[i];
		players[i].m_template = &templates[i];
	}
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_player = cfg[2] ? &players[cfg[2] - 1] : 0;
	obj->m_pos.x = 11.0f; obj->m_pos.y = 22.0f; obj->m_pos.z = 5.0f;
	obj->m_orientation = 0.5f;
	g_ocEdgeOffset.x = g_ocEdgeOffset.y = g_ocEdgeOffset.z = 0.0f;
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	TheTerrainLogic = &g_ocTerrain;
	InitGameLogicRandom(cfg[1]);
	g_events.clear();
	g_ulog.clear();
	OCLUpdate *m = 0;
	status = 0;
	try { m = new OCLUpdate(obj, d); }
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
		unsigned res = 0;
		if (phase) {
			g_events.clear();
			g_ulog.clear();
			try {
				switch (o[0]) {
				case 0: res = (unsigned)m->update(); break;
				case 1: gl.m_frame = o[1]; break;
				case 2: obj->m_disabled = o[1] != 0; break;
				case 3: { ObjectStatusMaskType uc; uc.set(OBJECT_STATUS_UNDER_CONSTRUCTION); if (o[1]) obj->m_status.set(uc); else obj->m_status.clear(uc); break; }
				case 4: obj->m_player = o[1] ? &players[(o[1] - 1) % 3] : 0; break;
				case 5: players[o[2] % 3].m_color = (Color)o[3]; break;
				case 6: players[o[2] % 3].m_playable = o[3] != 0; break;
				case 7: players[o[2] % 3].m_template = o[3] ? &templates[(o[3] - 1) % 3] : 0; break;
				case 8: m->resetTimer(); break;
				case 9: { Real r = m->getCountdownPercent(); res = realBits(r); break; }
				case 10: res = m->getRemainingFrames(); break;
				case 11: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_ocSaved.assign(bytes, bytes + blen); break;
				case 12: {
					std::vector<unsigned char> b = g_ocSaved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				case 13: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
				case 14: memcpy(&obj->m_pos.x, &o[1], 4); memcpy(&obj->m_pos.y, &o[2], 4); memcpy(&obj->m_pos.z, &o[3], 4); break;
				case 15: memcpy(&obj->m_orientation, &o[1], 4); break;
				default: memcpy(&g_ocEdgeOffset.x, &o[1], 4); memcpy(&g_ocEdgeOffset.y, &o[2], 4); memcpy(&g_ocEdgeOffset.z, &o[3], 4); break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
			putI32(st);
			putU32(res);
		}
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		if (phase) {
			putU32((unsigned)log.size());
			for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
			putI32(xst);
			putU32((unsigned)blen);
			for (int j = 0; j < blen; ++j) putU32(bytes[j]);
			putU32(crc);
		}
		putU32(m->m_nextCreationFrame);
		putU32(m->m_timerStartedFrame);
		putU32(m->m_isFactionNeutral ? 1 : 0);
		putU32((unsigned)m->m_currentPlayerColor);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	TheTerrainLogic = 0;
	return 0;
}

int ref_l9oc_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9ocRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
