/* Differential-test driver for ProjectileStreamUpdate (p1w13-sonnet, batch R42; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProjectileStreamUpdate.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: eight objects (ids 1-8; the stream is object 1; each maybe a
** VEHICLE, with one geometry for all and a position; an object the test removes is no object to findObjectByID), destroyObject (a line),
** Thing::setPosition (the shim's: the position kept), the REAL GeometryInfo, UpdateModule, INI and Xfer; the game logic (its frame, its wake
** calls applied as GameLogic::friend_awakenUpdateModule applies them). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProjectileStreamUpdate.cpp"

namespace {

ModuleData *l9r42Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = ProjectileStreamUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r42Saved;
UpdateModule *g_r42Updating = 0;
void r42Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r42Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *g_r42Objs[8];
unsigned g_r42Gone = 0;
Object *r42Find(ObjectID id)
{
	int i = (int)id;
	if (i >= 1 && i <= 8 && !(g_r42Gone >> (i - 1) & 1)) return g_r42Objs[i - 1];
	return 0;
}
Real r42F(unsigned bits) { Real f; memcpy(&f, &bits, 4); return f; }
void r42Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
/* the positions a shot or setPosition names, by index (the same table in the Rust test) */
const Real g_r42Spots[8][3] = { { 0, 0, 0 }, { 10, 0, 0 }, { 10, 0, 0.5f }, { -5, 7.5f, 2 }, { 30, -2, 0 }, { 0, 0, -0.0f }, { 2.5f, 2.5f, 12 }, { -40, 1, 3 } };

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a ProjectileStreamUpdate and make it on object 1 of eight at frame cfg[0]. `cfg` (32 words; 2-27
** also settable by op 2): 2 the VEHICLE objects (bit k for object k + 1), 3 every object's geometry (bits 0-1 the type: sphere, cylinder, box;
** 2-3 the major radius of 5, 20, 1, 0; 4-5 the minor likewise; 6-7 the height likewise), 4-27 objects 1-8's x, y and z (float bits). Then `n`
** ops of 4 words [op, a, b, c]: 0 update()  1 the frame = a  2 cfg[a] = b  3 addProjectile(a, b, c & 15, c >> 4 ? the spot c >> 4 - 1 : NULL)
** 4 getAllPoints  5 setPosition(the spot a)  6 remove object a (2-8)  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op appends:
** the CATCH_ALL status, the result, the effect log, the wake calls (count, last frame), the xfer status, bytes and CRC, the module's state (in
** the Rust module's state_words order), the points getAllPoints wrote (count, then x, y, z; none for the other ops) and object 1's position. */
int ref_l9r42_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r42Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r42Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	g_r42Gone = 0;
	const Real radii[4] = { 5.0f, 20.0f, 1.0f, 0.0f };
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 8; ++k) {
		g_r42Objs[k] = newObject();
		g_r42Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r42Objs[k]->m_behaviors = none;
	}
	Object *me = g_r42Objs[0];
	auto scene = [&]() {
		unsigned g = cfg[3];
		unsigned gt = g & 3;
		GeometryType type = gt == 0 ? GEOMETRY_SPHERE : (gt == 1 ? GEOMETRY_CYLINDER : GEOMETRY_BOX);
		for (int k = 0; k < 8; ++k) {
			Object *o = g_r42Objs[k];
			o->m_kindOf = KindOfMaskType();
			if (cfg[2] >> k & 1) o->m_kindOf.set(KINDOF_VEHICLE);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_geometry = GeometryInfo(type, false, radii[g >> 6 & 3], radii[g >> 2 & 3], radii[g >> 4 & 3]);
			o->m_pos.x = r42F(cfg[4 + 3 * k]);
			o->m_pos.y = r42F(cfg[5 + 3 * k]);
			o->m_pos.z = r42F(cfg[6 + 3 * k]);
		}
	};
	scene();
	g_modFindHook = r42Find;
	g_modDestroyHook = r42Destroy;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r42Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	ProjectileStreamUpdate *m = new ProjectileStreamUpdate(me, d);
	/* the constructor fills a local array that shadows the member, which it leaves uninitialised: the port's value (R23's precedent) */
	for (int k = 0; k < MAX_PROJECTILE_STREAM; ++k) m->m_projectileIDs[k] = INVALID_ID;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	putLog();
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 27) {
			cfg[o[1]] = o[2];
			scene();
		}
		if (o[0] == 1) gl.m_frame = o[1];
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
		Vector3 points[MAX_PROJECTILE_STREAM];
		Int count = 0;
		try {
			switch (o[0]) {
			case 0: g_r42Updating = m; res = (unsigned)m->update(); break;
			case 3: {
				Coord3D p;
				unsigned s = o[3] >> 4;
				if (s) { p.x = g_r42Spots[(s - 1) & 7][0]; p.y = g_r42Spots[(s - 1) & 7][1]; p.z = g_r42Spots[(s - 1) & 7][2]; }
				m->addProjectile((ObjectID)o[1], (ObjectID)o[2], (ObjectID)(o[3] & 15), s ? &p : NULL);
				break;
			}
			case 4: m->getAllPoints(points, &count); break;
			case 5: {
				Coord3D p; p.x = g_r42Spots[o[1] & 7][0]; p.y = g_r42Spots[o[1] & 7][1]; p.z = g_r42Spots[o[1] & 7][2];
				m->setPosition(&p);
				/* the scene keeps the stream where it now is */
				memcpy(&cfg[4], &me->m_pos.x, 4); memcpy(&cfg[5], &me->m_pos.y, 4); memcpy(&cfg[6], &me->m_pos.z, 4);
				break;
			}
			case 6: if (o[1] >= 2 && o[1] <= 8) g_r42Gone |= 1u << (o[1] - 1); break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r42Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r42Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r42Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		for (int k = 0; k < MAX_PROJECTILE_STREAM; ++k) putU32((unsigned)m->m_projectileIDs[k]);
		putU32((unsigned)m->m_nextFreeIndex);
		putU32((unsigned)m->m_firstValidIndex);
		putU32((unsigned)m->m_owningObject);
		putU32((unsigned)m->m_targetObject);
		putU32(realBits(m->m_targetPosition.x)); putU32(realBits(m->m_targetPosition.y)); putU32(realBits(m->m_targetPosition.z));
		putU32((unsigned)count);
		for (int k = 0; k < count; ++k) { putU32(realBits(points[k].X)); putU32(realBits(points[k].Y)); putU32(realBits(points[k].Z)); }
		putU32(realBits(me->m_pos.x)); putU32(realBits(me->m_pos.y)); putU32(realBits(me->m_pos.z));
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 8; ++k) {
		Object *x = g_r42Objs[k];
		x->m_behaviors = 0;
		delete x;
		g_r42Objs[k] = 0;
	}
	g_r42Gone = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
