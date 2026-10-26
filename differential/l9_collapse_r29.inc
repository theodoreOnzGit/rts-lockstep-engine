/* Differential-test driver for StructureCollapseUpdate (p1w13-sonnet, batch R29; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StructureCollapseUpdate.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the building (id 1: a body, the module build's MBody, whose
** updateBodyParticleSystems is a line; a drawable whose model condition calls are lines and whose instance matrix is a line of its translation; an AI
** whose markAsDead is a line (l9_slow_death_q.inc); a BoneFXUpdate that findUpdateModule finds, whose stopAllBoneFX is a line; a veterancy, status
** bits, a position, an orientation; a template whose geometry the test gives), the FX list store (FX_A, FX_B; l9_slow_death_q.inc; FXList::doFXPos is
** "fxpos", l9_dies_j.inc), the object creation list store (OCL_A, OCL_B; ObjectCreationList::createInternal is "oclat", l9_ocl_update_n.inc), the
** global data's gravity, the game logic (its frame, deselectObject, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them), and
** the REAL UpdateModule, DieMuxData, INI, RandomValue (the logic's and the client's) and Xfer. */
/* FireWeaponWhenDeadBehavior.cpp (l9_behaviors_k.inc) and SlowDeathBehavior.cpp (l9_slow_death_q.inc) define the same three names in this unit */
#define MAX_IDX ScMAX_IDX
#define parseFX ScParseFX
#define parseOCL ScParseOCL
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StructureCollapseUpdate.cpp"
#undef MAX_IDX
#undef parseFX
#undef parseOCL

namespace {

ModuleData *l9r29Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = StructureCollapseUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r29Saved;
UpdateModule *g_r29Updating = 0;
void r29Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r29Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
/* the template's geometry (a box of the test's height) */
GeometryInfo g_r29Geom(GEOMETRY_BOX, false, 10.0f, 10.0f, 10.0f);
/* the BoneFXUpdate findUpdateModule finds (stopAllBoneFX touches nothing of it) */
bool g_r29BoneFx = false;
Object *g_r29Me = 0;
UpdateModule *r29FindUpdate(const Object *, NameKeyType key)
{
	static char bfx;
	return (g_r29BoneFx && key == TheNameKeyGenerator->nameToKey("BoneFXUpdate")) ? (UpdateModule *)(void *)&bfx : 0;
}
void r29Instance(const Drawable *, const Matrix3D *m)
{
	ulog("instance %08x %08x %08x", realBits(m->Get_X_Translation()), realBits(m->Get_Y_Translation()), realBits(m->Get_Z_Translation()));
}

}  // namespace

const GeometryInfo *(*g_r77GeomHook)(const ThingTemplate *) = 0;   /* p1w13-sonnet, batch R77: a driver's own templates (l9_bridge_r77.inc) */
const GeometryInfo &ThingTemplate::getTemplateGeometryInfo() const
{
	if (g_r77GeomHook) { const GeometryInfo *g = g_r77GeomHook(this); if (g) return *g; }
	return g_r29Geom;
}
void BoneFXUpdate::stopAllBoneFX() { ulog("stopbonefx %d", g_r29Me ? (int)g_r29Me->m_oid : 0); }

extern "C" {

/* Read `filename` (block `D`) as the data of a StructureCollapseUpdate and make the module on object 1 at frame cfg[0]. `cfg` (12 words; each of
** 3-9 and 11 also settable by op 3): 0 the frame, 1 the logic random seed, 2 the client random seed, 3 flags (0 drawable, 1 AI, 2 body, 3 a
** BoneFXUpdate), 4 the veterancy, 5 the status bits (8), 6-8 the position (float bits), 9 the orientation (float bits), 10 the template's height
** (float bits), 11 the gravity (float bits). The data dump goes first (the delays but the maximum burst delay, which the constructor leaves alone;
** the frequency, the damping, the shudder; per phase the FX list and OCL codes), then the constructor's wake calls. Then `n` ops of 4 words
** [op, a, b, c]: 0 update()  1 onDie(a death of type a)  2 the frame = a  3 cfg[a] = b  4 xfer save  5 xfer load of the last save  6 xfer CRC.
** Each op appends: the CATCH_ALL status, the result, the effect log, the wake calls (count, last frame), the xfer status, bytes and CRC, the
** state (the two frames, the state, the velocity, the height). */
int ref_l9r29_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r29Saved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r29Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[12];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const StructureCollapseUpdateModuleData *sd = (const StructureCollapseUpdateModuleData *)d;
		putI32(sd->m_minCollapseDelay);
		putI32(sd->m_maxCollapseDelay);
		putI32(sd->m_minBurstDelay);
		putI32(sd->m_bigBurstFrequency);
		putU32(realBits(sd->m_collapseDamping));
		putU32(realBits(sd->m_maxShudder));
		for (int ph = 0; ph < SC_PHASE_COUNT; ++ph) {
			putU32((unsigned)sd->m_fxs[ph].size());
			for (size_t i = 0; i < sd->m_fxs[ph].size(); ++i) putI32(fxId(sd->m_fxs[ph][i]));
			putU32((unsigned)sd->m_ocls[ph].size());
			for (size_t i = 0; i < sd->m_ocls[ph].size(); ++i) putI32(sd->m_ocls[ph][i] == 0 ? 0 : (sd->m_ocls[ph][i] == &g_upOcls[0] ? 1 : 2));
			putU32(sd->m_fxCount[ph]);
			putU32(sd->m_oclCount[ph]);
		}
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitRandom(cfg[2]);
	InitGameLogicRandom(cfg[1]);
	g_findUpdateHook = r29FindUpdate;
	g_instanceHook = r29Instance;
	Real gravity0 = TheGlobalData->m_gravity;
	Object *me = newObject();
	me->m_oid = 1;
	g_r29Me = me;
	{
		Real h;
		memcpy(&h, &cfg[10], 4);
		g_r29Geom = GeometryInfo(GEOMETRY_BOX, false, h, 10.0f, 10.0f);
	}
	MBody body;
	body.m_owner = me;
	Drawable draw;
	draw.m_ownerObject = me;
	AIUpdateInterface ai;
	ai.m_owner = me;
	auto scene = [&]() {
		unsigned f = cfg[3];
		me->m_drawable = (f & 1) ? &draw : 0;
		me->m_ai = (f >> 1 & 1) ? &ai : 0;
		me->m_body = (f >> 2 & 1) ? &body : 0;
		g_r29BoneFx = (f >> 3 & 1) != 0;
		me->m_level = (VeterancyLevel)cfg[4];
		me->m_status.clear();
		for (int b = 0; b < 8; ++b) if (cfg[5] >> b & 1) me->m_status.set((ObjectStatusTypes)b);
		memcpy(&me->m_pos.x, &cfg[6], 4);
		memcpy(&me->m_pos.y, &cfg[7], 4);
		memcpy(&me->m_pos.z, &cfg[8], 4);
		memcpy(&me->m_orientation, &cfg[9], 4);
		memcpy(&TheGlobalData->m_gravity, &cfg[11], 4);
	};
	scene();
	gl.m_awakenHook = r29Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	StructureCollapseUpdate *m = new StructureCollapseUpdate(me, d);
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 3 && o[1] >= 3 && o[1] < 12 && o[1] != 10) cfg[o[1]] = o[2];
		if (o[0] == 2) gl.m_frame = o[1];
		scene();
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
			case 0: g_r29Updating = m; res = (unsigned)m->update(); break;
			case 1: {
				DamageInfo di;
				di.in.m_deathType = (DeathType)o[1];
				m->onDie(&di);
				break;
			}
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r29Saved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_r29Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 6: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r29Updating = 0;
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) {
				const std::string &l = g_ulog[(size_t)e.p];
				if (l.compare(0, 10, "setorient ") == 0) continue;   /* (no pose: setOrientation(getOrientation()) changes nothing the dump holds) */
				log.push_back(l);
			}
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_collapseFrame);
		putU32(m->m_burstFrame);
		putU32((unsigned)m->m_collapseState);
		putU32(realBits(m->m_collapseVelocity));
		putU32(realBits(m->m_currentHeight));
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_findUpdateHook = 0;
	g_instanceHook = 0;
	TheGlobalData->m_gravity = gravity0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	me->m_body = 0; me->m_drawable = 0; me->m_ai = 0;
	delete me;
	g_r29Me = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
