/* Differential-test driver for EMPUpdate and LeafletDropBehavior (p1w13-sonnet, batch R33; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/EMPUpdate.cpp   (#included, unchanged; its header's particle system fields name the
**   auto heal driver's parser, build.rs)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: five objects (ids 1-5, each with the module build's MBody, whose
** attemptDamage is a line; kinds, status bits, maybe a drawable, a team, a geometry and a position), the module's object (1), its producer (2,
** maybe with an AI whose victim the test names), the teams' relationships and players, the range scan (the objects the test lists; the call a
** line), the auto heal driver's particle system manager ("psys"; attachToObject, setSystemLifetime and setInitialDelay lines), the drawable's
** scale, tint and flash (lines; setTintStatus is the helper driver's record, dumped apart), setDisabledUntil (a line), the REAL Object::kill,
** GeometryInfo, UpdateModule, INI, RandomValue and Xfer; the game logic (its frame, its wake calls applied as GameLogic::friend_awakenUpdateModule
** applies them). */
#define saturateRGB EmpSaturateRGB
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/EMPUpdate.cpp"
#undef saturateRGB

namespace {

ModuleData *l9r33Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = kind ? LeafletDropBehavior::friend_newModuleData(&ini) : EMPUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r33Saved;
UpdateModule *g_r33Updating = 0;
void r33Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r33Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
void r33ScanHook(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	int n = 0;
	while (filters && filters[n]) ++n;
	ulog("scan %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), n);
}
Real r33F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_r33Kinds[8] = { KINDOF_VEHICLE, KINDOF_STRUCTURE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_AIRCRAFT, KINDOF_EMP_HARDENED, KINDOF_INFANTRY,
	KINDOF_FS_FACTORY, KINDOF_SELECTABLE };
Object *g_r33Objs[5];
Team g_r33Teams[3];
Player g_r33Players[3];
int g_r33Rel[3][3];
Object *r33Find(ObjectID id) { return (id >= 1 && id <= 5) ? g_r33Objs[id - 1] : 0; }
Relationship r33TeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return (Relationship)g_r33Rel[from->m_dummy - 1][to->m_dummy - 1];
}

}  // namespace

void Drawable::setInstanceScale(Real scale) { ulog("scale %d %08x", m_ownerObject ? (int)m_ownerObject->m_oid : 0, realBits(scale)); }
void Drawable::colorTint(const RGBColor *c) { ulog("colortint %08x %08x %08x", realBits(c->red), realBits(c->green), realBits(c->blue)); }
void Drawable::colorFlash(const RGBColor *c, UnsignedInt decayFrames, UnsignedInt attackFrames, UnsignedInt sustainAtPeak)
{
	ulog("colorflash %08x %08x %08x %u %u %u", realBits(c->red), realBits(c->green), realBits(c->blue), decayFrames, attackFrames, sustainAtPeak);
}
void ParticleSystem::setSystemLifetime( UnsignedInt frames ) { ulog("pslife %u %u", m_id, frames); }
void ParticleSystem::setInitialDelay( UnsignedInt delay ) { ulog("psdelay %u %u", m_id, delay); }

extern "C" {

/* Read `filename` (block `D`) as the data of an EMPUpdate (`kind` 0) or a LeafletDropBehavior (1) and make the module on object 1 of five at
** frame cfg[0], with the logic random seed cfg[1]. `cfg` (20 words; 2-17 also settable by op 3): 2-6 objects 1-5: bits 0-7 their kinds (of
** g_r33Kinds), 8 airborne, 9 a drawable, 10-11 their team (0 none, else 1-3); 7 object 1's producer id; 8 object 2 has an AI (bit 0) and its
** victim (bits 4-7: an object id, 0 none); 9-13 the objects' x (float bits; y and z 0); 14 the teams' relationships (2 bits each, row-major: team
** a to team b, 0 enemies, 1 neutral, 2 allies); 15 the scan (nibbles of object numbers 1-5, 0 ends); 16 the victims' geometry height (float
** bits; a box of radii 5 and 3; object 1 a sphere of 10); 17 unused. The data dump goes first, then the constructor's effects and wake calls.
** Then `n` ops of 4 words [op, a, b, c]: 0 update()  1 onDie (LeafletDropBehavior)  2 the frame = a  3 cfg[a] = b  4 xfer save  5 xfer load of
** the last save  6 xfer CRC. Each op appends: the CATCH_ALL status, the result, the effect log, the tint records, the wake calls (count, last
** frame), the xfer status, bytes and CRC, the module's state words (the constructor's orientation is its "setorient" line). */
int ref_l9r33_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r33Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	TheParticleSystemManager = &g_ahPsm;
	try { d = l9r33Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { TheParticleSystemManager = 0; return status; }
	unsigned cfg[20];
	memcpy(cfg, cfg_in, sizeof(cfg));
	if (kind) {
		const LeafletDropBehaviorModuleData *ld = (const LeafletDropBehaviorModuleData *)d;
		putU32(ld->m_delayFrames);
		putU32(ld->m_disabledDuration);
		putU32(realBits(ld->m_radius));
		const char *nm = ahPsName(ld->m_leafletFXParticleSystem);
		putStr(nm, (int)strlen(nm));
	} else {
		const EMPUpdateModuleData *ed = (const EMPUpdateModuleData *)d;
		putU32(ed->m_lifeFrames);
		putU32(ed->m_startFadeFrame);
		putU32(ed->m_disabledDuration);
		putU32(realBits(ed->m_startScale)); putU32(realBits(ed->m_targetScaleMin)); putU32(realBits(ed->m_targetScaleMax));
		putU32(realBits(ed->m_startColor.red)); putU32(realBits(ed->m_startColor.green)); putU32(realBits(ed->m_startColor.blue));
		putU32(realBits(ed->m_endColor.red)); putU32(realBits(ed->m_endColor.green)); putU32(realBits(ed->m_endColor.blue));
		const char *nm = ahPsName(ed->m_disableFXParticleSystem);
		putStr(nm, (int)strlen(nm));
		putU32(realBits(ed->m_sparksPerCubicFoot));
		putU32(realBits(ed->m_effectRadius));
		putU32((unsigned)ed->m_rejectMask);
		for (int i = 0; i < KINDOF_COUNT; ++i) putU32((ed->m_victimKindOf.test(i) ? 1 : 0) | (ed->m_victimKindOfNot.test(i) ? 2 : 0));
		putU32(ed->m_doesNotAffectMyOwnBuildings ? 1 : 0);
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[1]);
	g_ahPsLastId = 0;
	g_ahPsNext = 0;
	MBody bodies[5];
	Drawable draws[5];
	AIUpdateInterface ai;
	for (int k = 0; k < 5; ++k) {
		g_r33Objs[k] = newObject();
		g_r33Objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = g_r33Objs[k];
		g_r33Objs[k]->m_body = &bodies[k];
		draws[k].m_ownerObject = g_r33Objs[k];
	}
	for (int t = 0; t < 3; ++t) {
		g_r33Players[t].m_idx = t;
		g_r33Teams[t].m_dummy = t + 1;
		g_r33Teams[t].m_controller = &g_r33Players[t];
	}
	ai.m_owner = g_r33Objs[1];
	Object *me = g_r33Objs[0];
	auto scene = [&]() {
		for (int k = 0; k < 5; ++k) {
			Object *o = g_r33Objs[k];
			unsigned f = cfg[2 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 8; ++b) if (f >> b & 1) o->m_kindOf.set(g_r33Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_status.clear();
			if (f >> 8 & 1) o->m_status.set(OBJECT_STATUS_AIRBORNE_TARGET);
			o->m_drawable = (f >> 9 & 1) ? &draws[k] : 0;
			unsigned t = f >> 10 & 3;
			o->m_team = t ? &g_r33Teams[t - 1] : 0;
			o->m_player = t ? &g_r33Players[t - 1] : 0;
			memcpy(&o->m_pos.x, &cfg[9 + k], 4);
			o->m_pos.y = 0.0f;
			o->m_pos.z = 0.0f;
			o->m_geometry = k ? GeometryInfo(GEOMETRY_BOX, false, r33F(cfg[16]), 5.0f, 3.0f) : GeometryInfo(GEOMETRY_SPHERE, false, 10.0f, 10.0f, 10.0f);
		}
		me->m_producerID = (ObjectID)cfg[7];
		g_r33Objs[1]->m_ai = (cfg[8] & 1) ? &ai : 0;
		unsigned v = cfg[8] >> 4 & 15;
		ai.m_victim = (v >= 1 && v <= 5) ? g_r33Objs[v - 1] : 0;
		for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r33Rel[a][b] = (int)(cfg[14] >> (2 * (3 * a + b)) & 3);
		g_ahRange.clear();
		for (int k = 0; k < 5; ++k) { unsigned id = cfg[15] >> (4 * k) & 15; if (id == 0) break; if (id <= 5) g_ahRange.push_back(g_r33Objs[id - 1]); }
	};
	scene();
	g_ahRangeHook = r33ScanHook;
	g_teamRelHook = r33TeamRel;
	g_modFindHook = r33Find;
	g_events.clear();
	g_ulog.clear();
	g_hlog.clear();
	gl.m_awakenHook = r33Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	UpdateModule *m = kind ? (UpdateModule *)new LeafletDropBehavior(me, d) : (UpdateModule *)new EMPUpdate(me, d);
	{
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	}
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 3 && o[1] >= 2 && o[1] <= 17) cfg[o[1]] = o[2];
		if (o[0] == 2) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		g_hlog.clear();
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
			case 0: g_r33Updating = m; res = (unsigned)m->update(); break;
			case 1: if (kind) { DamageInfo di; ((LeafletDropBehavior *)m)->onDie(&di); } break;
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r33Saved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_r33Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 6: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r33Updating = 0;
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)g_hlog.size());
		for (size_t j = 0; j < g_hlog.size(); ++j) { putI32(g_hlog[j].first); putU32(g_hlog[j].second); }
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		if (kind) {
			LeafletDropBehavior *l = (LeafletDropBehavior *)m;
			putU32(l->m_startFrame);
			putU32(l->m_fxFired ? 1 : 0);
		} else {
			EMPUpdate *e = (EMPUpdate *)m;
			putU32(e->m_dieFrame);
			putU32(e->m_tintEnvFadeFrames);
			putU32(e->m_tintEnvPlayFrame);
			putU32(realBits(e->m_targetScale));
			putU32(realBits(e->m_currentScale));
		}
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_teamRelHook = 0;
	g_modFindHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 5; ++k) {
		Object *x = g_r33Objs[k];
		x->m_drawable = 0; x->m_body = 0; x->m_player = 0; x->m_team = 0; x->m_ai = 0;
		delete x;
		g_r33Objs[k] = 0;
	}
	TheGameLogic = 0;
	TheParticleSystemManager = 0;
	return 0;
}

}  /* extern "C" */
