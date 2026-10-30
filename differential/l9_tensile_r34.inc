/* Differential-test driver for TensileFormationUpdate (p1w13-sonnet, batch R34; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/TensileFormationUpdate.cpp   (#included with one mechanical edit made by build.rs
**     into mod_gen/TensileFormationUpdate_edit.cpp: getTFU's name key is not static, as the test resets the name keys every case)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: five objects (ids 1-5; each maybe a member of the formation, which
** findUpdateModule answers; maybe a drawable, maybe the module build's MBody, whose damage state the test sets and whose setDamageState is a line;
** maybe a shrub; a position; object 1, the module's, a sphere of the test's radius), the scan (the height die driver's, the objects the test lists,
** filtered as the partition manager does; the call a line), the closest object (the test's; the call a line), the terrain (the height die
** driver's eight cells, with the test's normals; each call a line), the pathfinder's wall (lines), the audio (isCurrentlyPlaying the test's, a
** line; addAudioEvent the die driver's line), the drawable's flags (lines), the REAL Object::topple (l9_topple_r28.inc; no object has a
** ToppleUpdate), Thing::setOrientation as the topple driver poses it, the REAL UpdateModule, INI, RandomValue and Xfer; the game logic (its frame,
** its wake calls applied as GameLogic::friend_awakenUpdateModule applies them). */
/* QuickTrig.h (which the file includes and does not use) declares `register` locals, which C++17 rejects */
#define register
#include "TensileFormationUpdate_edit.cpp"
#undef register

namespace {

ModuleData *l9r34Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = TensileFormationUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r34Saved;
UpdateModule *g_r34Updating = 0;
void r34Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r34Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Real r34F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
/* the ground heights and normals a cell can have, by nibble (the same tables in the Rust test) */
const Real g_r34Grounds[8] = { 0.0f, 2.0f, 5.5f, -3.0f, 12.0f, 1.25f, 30.0f, -0.5f };
const Coord3D g_r34Normals[8] = { { 0.0f, 0.0f, 1.0f }, { 0.3f, 0.0f, 0.954f }, { -0.2f, 0.4f, 0.894f }, { 0.0f, -0.6f, 0.8f },
	{ 0.5f, 0.5f, 0.707f }, { -0.7f, -0.1f, 0.707f }, { 0.05f, 0.02f, 0.998f }, { 0.0f, 0.0f, 0.0f } };
Object *g_r34Objs[5];
bool g_r34Member[5];
UpdateModule *g_r34Module = 0;
Object *g_r34Closest = 0;
bool g_r34Playing = false;
Object *r34Find(ObjectID id) { return (id >= 1 && id <= 5) ? g_r34Objs[id - 1] : 0; }
UpdateModule *r34FindUpdate(const Object *obj, NameKeyType key)
{
	if (key != TheNameKeyGenerator->nameToKey("TensileFormationUpdate")) return 0;
	for (int k = 0; k < 5; ++k) if (g_r34Objs[k] == obj && g_r34Member[k]) return g_r34Module;
	return 0;
}
class R34Audio : public AudioManager { public: virtual void stopAudio(AudioAffect) {} };

}  // namespace

void Pathfinder::createAWallFromMyFootprint( Object *obj ) { ulog("wallcreate %d", (int)obj->m_oid); }
void Pathfinder::removeWallFromMyFootprint( Object *obj ) { ulog("wallremove %d", (int)obj->m_oid); }
Object *(*g_closestAtHook)(PartitionFilter **filters) = 0;   /* batch R79: the candidates the test lists, through the caller's filters */
Object *PartitionManager::getClosestObject( const Coord3D *pos, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters )
{
	ulog("closestat %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), (int)dc);
	if (g_closestAtHook) return g_closestAtHook(filters);
	return g_r34Closest;
}
Bool AudioEventRTS::isCurrentlyPlaying() const
{
	ulog("isplaying %u", m_playingHandle);
	return g_r34Playing;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a TensileFormationUpdate and make it on object 1 of five, at frame cfg[0], with the logic random seed
** cfg[1]. `cfg` (24 words; those op 2 may set marked *): 2-6 objects 1-5: bit 0 a member of the formation (object 1 always is), 1 a drawable*,
** 2 a shrub*, 3 a body, 4-5 its damage state*; 7-11 their x (float bits; 8-11*), 12-16 their y (13-16*; z 0); 17* the scan (nibbles of object
** numbers 1-5, 0 ends); 18* the closest object (0 none); 19* the audio is playing; 20* the eight cells' ground heights (nibbles of
** g_r34Grounds); 21* their normals (nibbles of g_r34Normals); 22* object 1's radius (float bits); 23 unused. The data dump goes first, then the
** constructor's effects and wake calls. Then `n` ops of 4 words [op, a, b, c]: 0 update()  1 the frame = a  2 cfg[a] = b  3 xfer save  4 xfer load
** of the last save  5 xfer CRC. Each op appends: the CATCH_ALL status, the result, the effect log (the orientation lines left out), the wake
** calls (count, last frame), the xfer status, bytes and CRC, the module's state words, object 1's position and orientation. */
int ref_l9r34_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r34Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r34Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[24];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const TensileFormationUpdateModuleData *td = (const TensileFormationUpdateModuleData *)d;
		putU32(td->m_enabled ? 1 : 0);
		const AsciiString &nm = td->m_crackSound.getEventName();
		putStr(nm.str(), (int)nm.getLength());
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[1]);
	R34Audio audio;
	TheAudio = &audio;
	AI ai;
	TheAI = &ai;
	ThePartitionManager = &g_ahPm;
	g_audioHandleLast = 0;
	MBody bodies[5];
	Drawable draws[5];
	for (int k = 0; k < 5; ++k) {
		g_r34Objs[k] = newObject();
		g_r34Objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = g_r34Objs[k];
		bodies[k].m_traceState = true;
		draws[k].m_ownerObject = g_r34Objs[k];
		g_r34Member[k] = k == 0 || (cfg[2 + k] & 1);
		g_r34Objs[k]->m_body = (cfg[2 + k] >> 3 & 1) ? &bodies[k] : 0;
		bodies[k].m_state = (BodyDamageType)(cfg[2 + k] >> 4 & 3);
		memcpy(&g_r34Objs[k]->m_pos.x, &cfg[7 + k], 4);
		memcpy(&g_r34Objs[k]->m_pos.y, &cfg[12 + k], 4);
		g_r34Objs[k]->m_pos.z = 0.0f;
	}
	Object *me = g_r34Objs[0];
	/* the words op 2 may set */
	auto apply = [&](unsigned a) {
		if (a >= 2 && a <= 6) {
			int k = (int)a - 2;
			unsigned f = cfg[a];
			Object *o = g_r34Objs[k];
			o->m_drawable = (f >> 1 & 1) ? &draws[k] : 0;
			o->m_kindOf = KindOfMaskType();
			if (f >> 2 & 1) o->m_kindOf.set(KINDOF_SHRUBBERY);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			bodies[k].m_state = (BodyDamageType)(f >> 4 & 3);
		} else if (a >= 8 && a <= 11) {
			memcpy(&g_r34Objs[a - 7]->m_pos.x, &cfg[a], 4);
		} else if (a >= 13 && a <= 16) {
			memcpy(&g_r34Objs[a - 12]->m_pos.y, &cfg[a], 4);
		} else if (a == 17) {
			g_hdScan.clear();
			for (int k = 0; k < 5; ++k) { unsigned id = cfg[17] >> (4 * k) & 15; if (id == 0) break; if (id <= 5) g_hdScan.push_back(g_r34Objs[id - 1]); }
		} else if (a == 18) {
			g_r34Closest = (cfg[18] >= 1 && cfg[18] <= 5) ? g_r34Objs[cfg[18] - 1] : 0;
		} else if (a == 19) {
			g_r34Playing = cfg[19] != 0;
		} else if (a == 20 || a == 21) {
			for (int c = 0; c < 8; ++c) {
				g_hdGround[c] = g_r34Grounds[cfg[20] >> (4 * c) & 7];
				g_hdNormal[c] = g_r34Normals[cfg[21] >> (4 * c) & 7];
			}
		} else if (a == 22) {
			me->m_geometry = GeometryInfo(GEOMETRY_SPHERE, false, r34F(cfg[22]), r34F(cfg[22]), r34F(cfg[22]));
		}
	};
	for (unsigned a = 2; a <= 22; ++a) apply(a);
	g_hdApplyFilters = true;
	g_hdNormalOn = true;
	g_r34SetFlagsLog = true;
	g_r28ThingPose = true;
	g_modFindHook = r34Find;
	g_findUpdateHook = r34FindUpdate;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r34Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			if (g_events[j].kind != 100) continue;
			const std::string &l = g_ulog[(size_t)g_events[j].p];
			if (l.compare(0, 10, "setorient ") == 0) continue;   /* the orientation is the dump's */
			log.push_back(l);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	TensileFormationUpdate *m = new TensileFormationUpdate(me, d);
	g_r34Module = m;
	/* the module's name key (MAKE_STANDARD_MODULE_MACRO) and Object::topple's (l9_topple_r28.inc) are function-local statics, as in the module
	** build everywhere; they are made here, in the same case, so they are told apart in every later one (Object::topple must never take this
	** module for a ToppleUpdate). Object 5's behaviours are none: the topple does nothing. */
	(void)m->getModuleNameKey();
	{ Coord3D none = { 0.0f, 0.0f, 0.0f }; g_r34Objs[4]->topple(&none, 0.0f, 0); }
	putLog();
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 22 && o[1] != 7 && o[1] != 12) { cfg[o[1]] = o[2]; apply(o[1]); }
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
		try {
			switch (o[0]) {
			case 0: g_r34Updating = m; res = (unsigned)m->update(); break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r34Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r34Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r34Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_enabled ? 1 : 0);
		putU32(m->m_linksInited ? 1 : 0);
		putU32(m->m_motionlessCounter);
		putU32(m->m_life);
		putU32(realBits(m->m_lowestSlideElevation));
		putU32(realBits(m->m_inertia.x)); putU32(realBits(m->m_inertia.y)); putU32(realBits(m->m_inertia.z));
		for (int t = 0; t < 4; ++t) {
			putU32((unsigned)m->m_links[t].id);
			putU32(realBits(m->m_links[t].tensor.x)); putU32(realBits(m->m_links[t].tensor.y)); putU32(realBits(m->m_links[t].tensor.z));
		}
		putU32(m->m_crackSound.getPlayingHandle());
		putU32(realBits(me->m_pos.x)); putU32(realBits(me->m_pos.y)); putU32(realBits(me->m_pos.z));
		putU32(realBits(me->m_orientation));
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_hdApplyFilters = false;
	g_hdNormalOn = false;
	g_r34SetFlagsLog = false;
	g_r28ThingPose = false;
	g_hdScan.clear();
	g_modFindHook = 0;
	g_findUpdateHook = 0;
	g_r34Module = 0;
	g_r34Closest = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 5; ++k) {
		Object *x = g_r34Objs[k];
		x->m_drawable = 0; x->m_body = 0;
		delete x;
		g_r34Objs[k] = 0;
	}
	TheGameLogic = 0;
	TheAudio = 0;
	TheAI = 0;
	ThePartitionManager = 0;
	return 0;
}

}  /* extern "C" */
