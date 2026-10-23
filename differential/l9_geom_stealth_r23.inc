/* Differential-test driver for DynamicGeometryInfoUpdate and GrantStealthBehavior (p1w13-sonnet, batch R23; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DynamicGeometryInfoUpdate.cpp
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/GrantStealthBehavior.cpp   (#included, unchanged; its header's particle system field
**   names the auto heal driver's parser, build.rs)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the module's object (id 1: a geometry, maybe a drawable, which logs "react" when
** the geometry changes and "flash" when it flashes, maybe effectively dead) and three more (ids 2 to 4: kinds, maybe a stealth update, whose
** receiveGrant logs, maybe a drawable); the auto heal driver's particle system manager ("psys") and range scan (the objects the test lists, the call a
** line); the game logic, whose destroyObject logs and whose wake calls the test counts; and the REAL UpdateModule, INI and Xfer. */
#define max(x,y) (((x)>(y)) ? (x) : (y))   /* Lib/BaseType.h:105, verbatim (the module build leaves the macro out) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DynamicGeometryInfoUpdate.cpp"
#undef max
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/GrantStealthBehavior.cpp"

namespace {

ModuleData *l9r23Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = kind ? GrantStealthBehavior::friend_newModuleData(&ini) : DynamicGeometryInfoUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r23Saved;
/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_r23Kinds[3] = { KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_STRUCTURE };
void r23SetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 3; ++i) if (bits >> i & 1) o->m_kindOf.set(g_r23Kinds[i]);
	o->m_tmpl.m_kindOf = o->m_kindOf;
}
void r23ScanHook(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	int n = 0;
	while (filters && filters[n]) ++n;
	ulog("scan %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), n);
}
void r23DestroyHook(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
Real r23F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a DynamicGeometryInfoUpdate (`kind` 0) or a GrantStealthBehavior (1) and make the module on object 1 of
** four at frame 0. `cfg` (6 words): [0 flags (bits 0-3 objects 1 to 4 have a drawable, 4-6 objects 2 to 4 have a stealth update, 7 object 1 has one, 8 object 1 is
** effectively dead, 12-20 the kinds of objects 2 to 4, 3 bits each, of g_r23Kinds, 21-23 object 1's kinds), 1 object 1's geometry type (0 sphere, 1 cylinder, 2 box) and is-small (bit 4),
** 2-4 its height and radii (float bits), 5 the range scan (nibbles of object numbers, 0 ends)]. The data dump goes first. Then the constructor's effect
** log and wake calls. Then `n` ops of 6 words [op, a, b, c, d, e]: 0 update()  1 flags = a  2 the scan = a  3 the frame = a  7 xfer save  8 xfer load
** of the last save  9 xfer CRC. Each op appends: the CATCH_ALL status, the result (update's sleep time), the effect log (count, then each), the wake
** calls (count, the last frame), the xfer status, bytes and CRC, the module's state words (13 for the geometry update, 2 for the stealth grant), and
** object 1's geometry (type, is-small, height, major and minor radius bits). */
int ref_l9r23_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r23Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	TheParticleSystemManager = &g_ahPsm;
	try { d = l9r23Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { TheParticleSystemManager = 0; return status; }
	if (kind) {
		const GrantStealthBehaviorModuleData *gd = (const GrantStealthBehaviorModuleData *)d;
		putU32(realBits(gd->m_startRadius));
		putU32(realBits(gd->m_finalRadius));
		putU32(realBits(gd->m_radiusGrowRate));
		for (int i = 0; i < KINDOF_COUNT; ++i) putU32(gd->m_kindOf.test(i) ? 1 : 0);
		const char *ps = gd->m_radiusParticleSystemTmpl ? g_ahPsNames[(size_t)gd->m_radiusParticleSystemTmpl->m_idx].c_str() : "";
		putStr(ps, (int)strlen(ps));
	} else {
		const DynamicGeometryInfoUpdateModuleData *dd = (const DynamicGeometryInfoUpdateModuleData *)d;
		putU32(dd->m_initialDelay);
		putU32(realBits(dd->m_initialHeight)); putU32(realBits(dd->m_initialMajorRadius)); putU32(realBits(dd->m_initialMinorRadius));
		putU32(realBits(dd->m_finalHeight)); putU32(realBits(dd->m_finalMajorRadius)); putU32(realBits(dd->m_finalMinorRadius));
		putU32(dd->m_transitionTime);
		putU32(dd->m_reverseAtTransitionTime ? 1 : 0);
	}
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	g_ahPsLastId = 0;
	g_modDestroyHook = r23DestroyHook;
	Object *objs[4];
	Drawable draws[4];
	StealthUpdate stealths[4];
	for (int k = 0; k < 4; ++k) {
		objs[k] = newObject();
		objs[k]->m_oid = (ObjectID)(k + 1);
		draws[k].m_ownerObject = objs[k];
		stealths[k].m_owner = objs[k];
	}
	Object *me = objs[0];
	me->m_geometry = GeometryInfo((GeometryType)(cfg[1] & 3), (cfg[1] >> 4 & 1) != 0, r23F(cfg[2]), r23F(cfg[3]), r23F(cfg[4]));
	unsigned flags = cfg[0];
	unsigned scan = cfg[5];
	/* the scene before the constructor (the stealth grant's particle system is placed at the object, at the origin) */
	for (int k = 0; k < 4; ++k) {
		objs[k]->m_drawable = (flags >> k & 1) ? &draws[k] : 0;
		objs[k]->m_stealth = (flags >> (k ? 3 + k : 7) & 1) ? &stealths[k] : 0;
		r23SetKinds(objs[k], k > 0 ? flags >> (12 + 3 * (k - 1)) & 7 : flags >> 21 & 7);
	}
	me->m_effDead = (flags >> 8 & 1) != 0;
	g_ahRangeHook = r23ScanHook;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	UpdateModule *m = 0;
	if (kind) m = new GrantStealthBehavior(me, d); else m = new DynamicGeometryInfoUpdate(me, d);
	{
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	}
	/* GameLogic::friend_awakenUpdateModule for an object not yet in the list (GameLogic.cpp:3035-3046), and the Rust world scheduling an update
	** module when it makes the object (frame 1 at frame 0) for a constructor that sets no wake frame (the batch R13e correction) */
	m->friend_setNextCallFrame(gl.m_awakenCalls ? gl.m_awakenFrame : 1);
	/* m_direction is never set upstream and xfer saves it: indeterminate bytes. The translation holds 0; so does this object from here on */
	if (!kind) ((DynamicGeometryInfoUpdate *)m)->m_direction = (DynamicGeometryInfoUpdate::DynamicGeometryDirection)0;
	me->m_pos.x = 10.0f;
	me->m_pos.y = -2.5f;
	me->m_pos.z = 1.0f;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 1) flags = o[1];
		if (o[0] == 2) scan = o[1];
		if (o[0] == 3) gl.m_frame = o[1];
		for (int k = 0; k < 4; ++k) {
			objs[k]->m_drawable = (flags >> k & 1) ? &draws[k] : 0;
			objs[k]->m_stealth = (flags >> (k ? 3 + k : 7) & 1) ? &stealths[k] : 0;
			r23SetKinds(objs[k], k > 0 ? flags >> (12 + 3 * (k - 1)) & 7 : flags >> 21 & 7);
		}
		me->m_effDead = (flags >> 8 & 1) != 0;
		g_ahRange.clear();
		for (int k = 0; k < 4; ++k) { unsigned id = scan >> (4 * k) & 15; if (id == 0) break; if (id <= 4) g_ahRange.push_back(objs[id - 1]); }
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
			case 0: res = (unsigned)m->update(); break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r23Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r23Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		if (kind) {
			GrantStealthBehavior *g = (GrantStealthBehavior *)m;
			putU32(g->m_radiusParticleSystemID);
			putU32(realBits(g->m_currentScanRadius));
		} else {
			DynamicGeometryInfoUpdate *g = (DynamicGeometryInfoUpdate *)m;
			putU32(g->m_startingDelayCountdown);
			putU32(g->m_timeActive);
			putU32(g->m_started ? 1 : 0);
			putU32(g->m_finished ? 1 : 0);
			putU32(g->m_reverseAtTransitionTime ? 1 : 0);
			putU32((unsigned)g->m_direction);
			putU32(g->m_switchedDirections ? 1 : 0);
			putU32(realBits(g->m_initialHeight)); putU32(realBits(g->m_initialMajorRadius)); putU32(realBits(g->m_initialMinorRadius));
			putU32(realBits(g->m_finalHeight)); putU32(realBits(g->m_finalMajorRadius)); putU32(realBits(g->m_finalMinorRadius));
		}
		const GeometryInfo &gi = me->getGeometryInfo();
		putU32((unsigned)gi.getGeomType());
		putU32(gi.getIsSmall() ? 1 : 0);
		putU32(realBits(gi.getMaxHeightAbovePosition()));
		putU32(realBits(gi.getMajorRadius()));
		putU32(realBits(gi.getMinorRadius()));
	}
	*len = finish(buf, cap);
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_modDestroyHook = 0;
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) { objs[k]->m_drawable = 0; objs[k]->m_stealth = 0; delete objs[k]; }
	TheGameLogic = 0;
	TheParticleSystemManager = 0;
	return 0;
}

}  /* extern "C" */
