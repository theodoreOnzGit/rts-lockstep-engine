/* Differential-test driver for WaveGuideUpdate (p1w13-sonnet, batch R70; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/WaveGuideUpdate.cpp   (#included, unchanged but for the particle system parser
**   its field table names: this unit's l9ahParsePsTemplate, as for AutoHealBehavior; build.rs)
** with its header unchanged, in wrap_module.cpp's unit, at its end (batch R50's destroyObject rename), on the module build's stand-ins: the wave
** guide (id 1: maybe an AI whose two path orders are lines, disabled or not, a position; setDisabled a line, its transform Thing's, batch R28's
** way) and five others (kinds, wet, maybe a drawable whose two calls are lines, maybe a body whose attemptDamage is a line: batch R49's, a
** position), the terrain's waypoints (the test's path; TerrainLogic.h's getters), its bridges (the test's, by the cell of the position: their
** info's ends), the ground (batch O's cells), the scan (batch M's list; the call a line), the particle systems (batch M's, whose positions the
** driver keeps, and find again until the test takes one away; setLocalTransform a line), the water and the radar (lines), the audio (die batch
** D's line), the thing factory (WaterWaveBridge, when the test has one: a bare object), the REAL Object::topple (batch R28's; no object has a
** ToppleUpdate), UpdateModule, INI, Xfer and logic random values; the game logic (its frame, batch R35's wake calls). */
#define destroyObject r50DestroyObject   /* (as in l9_physics_r50.inc: the inline one loses to other units' out-of-line ones) */
#include "WaveGuideUpdate_edit.cpp"   /* (mod_gen: INI::parseParticleSystemTemplate is this unit's l9ahParsePsTemplate, build.rs) */
#undef destroyObject

namespace {

ModuleData *l9r70Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = WaveGuideUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r70Saved;
Object *g_r70Objs[6];
std::vector<Object *> g_r70New;
Waypoint g_r70Way[4];
Bridge g_r70Bridges[8];
bool g_r70HasBridgeTemplate = false;
int g_r70BridgeTmpl;   /* (its address is the template) */
std::map<unsigned, Coord3D> g_r70Ps;   /* the particle systems the driver keeps, by id */
std::map<unsigned, int> g_r70PsTmpl;   /* (and their templates, for the lines) */
ParticleSystem g_r70Found;
/* the positions, by index (the same list in the Rust test) */
const Real g_r70Pal[8] = { -200.0f, -60.0f, -15.0f, 0.0f, 15.0f, 60.0f, 200.0f, 500.0f };
/* the bridges' directions, by index 1-7 (from the origin) */
const Real g_r70Dir[8][2] = { { 0, 0 }, { 100, 0 }, { 0, 100 }, { -50, 50 }, { 70, -30 }, { 1, 1 }, { -100, -5 }, { 30, 90 } };
const KindOfType g_r70Kinds[3] = { KINDOF_WAVEGUIDE, KINDOF_BRIDGE_TOWER, KINDOF_BRIDGE };
int g_r70WayCount = 0;
bool g_r70WayNamed = true;
Real r70F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
Coord3D r70Pos(unsigned w)
{
	Coord3D p;
	p.x = g_r70Pal[w & 7];
	p.y = g_r70Pal[w >> 3 & 7];
	p.z = g_r70Pal[w >> 6 & 7];
	return p;
}
Object *r70Find(ObjectID id)
{
	if ((int)id >= 1 && (int)id <= 6) return g_r70Objs[(int)id - 1];
	for (size_t i = 0; i < g_r70New.size(); ++i) if (g_r70New[i]->m_oid == id) return g_r70New[i];
	return 0;
}
void r70PsPos(ParticleSystem *s, const Coord3D *pos)
{
	if (pos) { if (g_r70Ps.find(s->m_id) != g_r70Ps.end()) g_r70Ps[s->m_id] = *pos; }
	else { Coord3D z; z.zero(); g_r70Ps[s->m_id] = z; g_r70PsTmpl[s->m_id] = s->m_tmpl; }
}
ParticleSystem *r70PsFind(ParticleSystemID id)
{
	if (g_r70Ps.find(id) == g_r70Ps.end()) return 0;
	g_r70Found.m_id = id;
	g_r70Found.m_tmpl = g_r70PsTmpl[id];
	return &g_r70Found;
}
const ThingTemplate *r70FindTemplate(const AsciiString &name)
{
	if (g_r70HasBridgeTemplate && strcmp(name.str(), "WaterWaveBridge") == 0) return (const ThingTemplate *)&g_r70BridgeTmpl;
	return 0;
}
Object *r70NewObject(const ThingTemplate *, Team *team)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)(7 + g_r70New.size());
	o->m_team = team;
	o->m_matrix.Make_Identity();
	g_r70New.push_back(o);
	return o;
}

}  // namespace

Waypoint *TerrainLogic::getWaypointByName( AsciiString name )
{
	if (!g_r70WayNamed || g_r70WayCount == 0 || strcmp(name.str(), "WaveGuide1") != 0) return 0;
	return &g_r70Way[0];
}
Bridge *TerrainLogic::findBridgeAt(const Coord3D *pLoc) const
{
	ulog("bridgeat %08x %08x %08x", realBits(pLoc->x), realBits(pLoc->y), realBits(pLoc->z));
	Bridge *b = &g_r70Bridges[hdCell(pLoc->x, pLoc->y)];
	return b->m_cell >= 0 ? b : 0;
}
void TerrainLogic::deleteBridge( Bridge *bridge )
{
	if (!bridge) { ulog("deletebridge null"); return; }
	ulog("deletebridge %08x %08x %08x %08x", realBits(bridge->m_bridgeInfo.from.x), realBits(bridge->m_bridgeInfo.from.y), realBits(bridge->m_bridgeInfo.to.x),
		realBits(bridge->m_bridgeInfo.to.y));
}
TerrainVisual *TheTerrainVisual = 0;
void TerrainVisual::addWaterVelocity( Real worldX, Real worldY, Real velocity, Real preferredHeight )
{
	ulog("water %08x %08x %08x %08x", realBits(worldX), realBits(worldY), realBits(velocity), realBits(preferredHeight));
}
void Radar::refreshTerrain( TerrainLogic *terrain ) { ulog("radarrefresh"); }
void r70Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
void ParticleSystem::getPosition( Coord3D *pos ) { *pos = g_r70Ps[m_id]; }
void ParticleSystem::setLocalTransform( const Matrix3D *m )
{
	ulog("pstransform %u %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x", m_id, realBits((*m)[0][0]), realBits((*m)[0][1]), realBits((*m)[0][2]),
		realBits((*m)[0][3]), realBits((*m)[1][0]), realBits((*m)[1][1]), realBits((*m)[1][2]), realBits((*m)[1][3]), realBits((*m)[2][0]), realBits((*m)[2][1]),
		realBits((*m)[2][2]), realBits((*m)[2][3]));
}
void AIUpdateInterface::aiFollowWaypointPath( const Waypoint *way, CommandSourceType cmdSource ) { ulog("aifollowpath %d %d %d", (int)m_owner->m_oid, way->getID(), (int)cmdSource); }
void AIUpdateInterface::setPathExtraDistance( Real dist ) { ulog("aipathextra %d %08x", (int)m_owner->m_oid, realBits(dist)); }
/* Thing.cpp 367-386, verbatim but for the transform (getTransformMatrix) */
void Object::transformPoint( const Coord3D *in, Coord3D *out )
{

	// santiy
	if( in == NULL || out == NULL )
		return;

	// for conversion
	Vector3 vectorIn;
	Vector3 vectorOut;

	///@ todo this is dumb and we should not have to convert types
	// convert to Vector3 datatypes
	vectorIn.X = in->x;
	vectorIn.Y = in->y;
	vectorIn.Z = in->z;

	// do the transform
	getTransformMatrix()->Transform_Vector( *getTransformMatrix(), vectorIn, &vectorOut );

	// convert back to Coord3D datatype
	out->x = vectorOut.X;
	out->y = vectorOut.Y;
	out->z = vectorOut.Z;

}

extern "C" {

/* Read `filename` (block `D`) as the data of a WaveGuideUpdate and make the module on object 1 of six at frame cfg[0]. `cfg` (32 words; all but
** 17-18 and 20 also settable by op 2): 1 the logic random seed; 2 the wave guide: bit 0 an AI, bit 1 disabled, bits 8-16 its position (3 bits
** each of x, y, z: of g_r70Pal); 3-7 objects 2-6: bits 0-2 their kinds (of g_r70Kinds), bit 3 wet, bit 4 a drawable, bit 5 a body, bits 8-16 their
** position; 8 the scan (nibbles of object numbers 1-6, 0 ends); 9-16 the ground of the 8 cells (float bits); read once, 17 the waypoints'
** positions (6 bits each: x, y of g_r70Pal), 18: bits 0-2 their number (0-4), bit 3 the first named WaveGuide1, bits 4-5 the links (0 a chain, 1
** a chain with a second link from node bits 6-7, 2 a chain whose first node has none); 19 the bridges of the 8 cells (3 bits each: 0 none,
** else a direction of g_r70Dir from the origin); 20 bit 0: the thing factory has WaterWaveBridge. The data dump goes first (the ten reals, the
** frequency, the angle fudge, the two sounds' names, the bridge particle's (empty: NULL)). Then `n` ops
** of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3 xfer save  4 xfer load of the last save  5 xfer CRC  6 the client
** ends particle system a  7 the wave guide moves to position a (as cfg's). Each op appends: the CATCH_ALL status, the result, the effect log
** (without the setorient lines: the orientation is in the state), the xfer status, bytes and CRC, the module's state (as the Rust state_words,
** the arrays to the point count), the wave guide's position and orientation, the wet bits of objects 1-6, and the new objects (id, position,
** orientation). */
static int l9r70RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r70Saved.clear();
	g_r70Ps.clear();
	g_r70PsTmpl.clear();
	g_r70New.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r70Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const WaveGuideUpdateModuleData *wd = (const WaveGuideUpdateModuleData *)d;
		const Real reals[10] = { wd->m_waveDelay, wd->m_ySize, wd->m_linearWaveSpacing, wd->m_waveBendMagnitude, wd->m_waterVelocity, wd->m_preferredHeight,
			wd->m_shorelineEffectDistance, wd->m_damageRadius, wd->m_damageAmount, wd->m_toppleForce };
		for (int i = 0; i < 10; ++i) putU32(realBits(reals[i]));
		putU32((unsigned)wd->m_randomSplashSoundFrequency);
		putU32(realBits(wd->m_bridgeParticleAngleFudge));
		putStr(wd->m_randomSplashSound.getEventName().str(), wd->m_randomSplashSound.getEventName().getLength());
		putStr(wd->m_loopingSound.getEventName().str(), wd->m_loopingSound.getEventName().getLength());
		std::string bp = wd->m_bridgeParticle ? g_ahPsNames[(size_t)wd->m_bridgeParticle->m_idx] : std::string();
		putStr(bp.c_str(), (int)bp.size());
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	gl.m_awakenHook = r35Awaken;
	InitGameLogicRandom(cfg[1]);
	ThePartitionManager = &g_ahPm;
	TerrainLogic terrain;
	TheTerrainLogic = &terrain;
	TerrainVisual visual;
	TheTerrainVisual = &visual;
	Radar radar;
	TheRadar = &radar;
	AbAudio audio;   /* (batch P's: addAudioEvent is die batch D's line) */
	TheAudio = &audio;
	TheParticleSystemManager = &g_ahPsm;
	g_ahPsLastId = 0;
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 6; ++k) {
		g_r70Objs[k] = newObject();
		g_r70Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r70Objs[k]->m_behaviors = none;
		g_r70Objs[k]->m_matrix.Make_Identity();
	}
	Object *me = g_r70Objs[0];
	AIUpdateInterface ai;
	ai.m_owner = me;
	Drawable draws[6];
	SdBody *bodies[6] = { 0, 0, 0, 0, 0, 0 };
	for (int k = 1; k < 6; ++k) bodies[k] = new NmBody(g_r70Objs[k], d);
	/* the path (read once) */
	g_r70WayCount = (int)(cfg[18] & 7);
	if (g_r70WayCount > 4) g_r70WayCount = 4;
	g_r70WayNamed = (cfg[18] >> 3 & 1) != 0;
	for (int i = 0; i < 4; ++i) {
		Waypoint &w = g_r70Way[i];
		w.m_id = 100 + i;
		w.m_location.x = g_r70Pal[cfg[17] >> (6 * i) & 7];
		w.m_location.y = g_r70Pal[cfg[17] >> (6 * i + 3) & 7];
		w.m_location.z = 0.0f;
		w.m_numLinks = 0;
		for (int j = 0; j <= Waypoint::MAX_LINKS; ++j) w.m_links[j] = 0;
	}
	unsigned topo = cfg[18] >> 4 & 3;
	for (int i = 0; i + 1 < g_r70WayCount; ++i) {
		if (topo == 2 && i == 0) continue;
		g_r70Way[i].m_links[g_r70Way[i].m_numLinks++] = &g_r70Way[i + 1];
	}
	if (topo == 1 && g_r70WayCount > 0) {
		/* a second link (the walk stops at it before it can loop): node k to the first node, or the first node to the last (a map's links are
		** never to the node itself nor twice the same: TerrainLogic::addWaypointLink) */
		int k = (int)(cfg[18] >> 6 & 3) % g_r70WayCount;
		int to = k != 0 ? 0 : g_r70WayCount - 1;
		if (g_r70Way[k].m_numLinks == 1 && to != k && to != k + 1) g_r70Way[k].m_links[g_r70Way[k].m_numLinks++] = &g_r70Way[to];
	}
	for (int c = 0; c < 8; ++c) {
		unsigned b = cfg[19] >> (3 * c) & 7;
		g_r70Bridges[c].m_cell = b ? c : -1;
		g_r70Bridges[c].m_bridgeInfo.from.zero();
		g_r70Bridges[c].m_bridgeInfo.to.x = g_r70Dir[b][0];
		g_r70Bridges[c].m_bridgeInfo.to.y = g_r70Dir[b][1];
		g_r70Bridges[c].m_bridgeInfo.to.z = 0.0f;
	}
	g_r70HasBridgeTemplate = (cfg[20] & 1) != 0;
	auto scene = [&]() {
		me->m_ai = (cfg[2] & 1) ? &ai : 0;
		me->m_disabled = (cfg[2] >> 1 & 1) != 0;
		for (int k = 1; k < 6; ++k) {
			Object *o = g_r70Objs[k];
			unsigned f = cfg[2 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 3; ++b) if (f >> b & 1) o->m_kindOf.set(g_r70Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_status.set(OBJECT_STATUS_WET, (f >> 3 & 1) != 0);
			o->m_drawable = (f >> 4 & 1) ? &draws[k] : 0;
			o->m_body = (f >> 5 & 1) ? bodies[k] : 0;
			o->m_pos = r70Pos(f >> 8);
		}
		for (int c = 0; c < 8; ++c) g_hdGround[c] = r70F(cfg[9 + c]);
		g_ahRange.clear();
		for (int k = 0; k < 6; ++k) { unsigned id = cfg[8] >> (4 * k) & 15; if (id == 0) break; if (id <= 6) g_ahRange.push_back(g_r70Objs[id - 1]); }
	};
	me->m_pos = r70Pos(cfg[2] >> 8);
	scene();
	g_r28ThingPose = true;
	g_objXformSyncPos = true;
	g_ahRangeHook = r35ScanHook;
	g_modFindHook = r70Find;
	g_psPosHook = r70PsPos;
	g_psFindHook = r70PsFind;
	g_findTemplateHook = r70FindTemplate;
	g_newObjectHook = r70NewObject;
	g_modDestroyHook = r70Destroy;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) {
			const std::string &l = g_ulog[(size_t)g_events[j].p];
			if (l.compare(0, 10, "setorient ") == 0) continue;
			log.push_back(l);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	WaveGuideUpdate *m = 0;
	try { m = newInstance(WaveGuideUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	/* (no constructor call frame: the module sets no wake, so the frame is the game logic's registration, which this driver does not do) */
	auto putPoint = [&](const Coord3D &p) { putU32(realBits(p.x)); putU32(realBits(p.y)); putU32(realBits(p.z)); };
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 16) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		if (o[0] == 7) me->m_pos = r70Pos(o[1]);
		if (o[0] == 6) g_r70Ps.erase(o[1] ? o[1] : g_ahPsLastId);
		scene();
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[4096];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 4096, &blen, &crc); g_r70Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r70Saved;
				if (b.size() > 4096) b.resize(4096);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		int count = m->m_shapePointCount;
		int shown = count < 0 ? 0 : (count > WaveGuideUpdate::MAX_WAVEGUIDE_SHAPE_POINTS ? (int)WaveGuideUpdate::MAX_WAVEGUIDE_SHAPE_POINTS : count);
		putU32(m->m_activeFrame);
		putU32(m->m_needDisable ? 1 : 0);
		putU32(m->m_initialized ? 1 : 0);
		putU32((unsigned)count);
		putU32(m->m_splashSoundFrame);
		putPoint(m->m_finalDestination);
		for (int j = 0; j < shown; ++j) putPoint(m->m_shapePoints[j]);
		for (int j = 0; j < shown; ++j) putPoint(m->m_transformedShapePoints[j]);
		for (int j = 0; j < shown; ++j) for (int k = 0; k < WaveGuideUpdate::MAX_SHAPE_EFFECTS; ++k) putU32(m->m_shapeEffects[j][k]);
		putPoint(me->m_pos);
		putU32(realBits(me->m_orientation));
		unsigned wet = 0;
		for (int k = 0; k < 6; ++k) if (g_r70Objs[k]->m_status.test(OBJECT_STATUS_WET)) wet |= 1u << k;
		putU32(wet);
		putU32((unsigned)g_r70New.size());
		for (size_t j = 0; j < g_r70New.size(); ++j) {
			putU32((unsigned)g_r70New[j]->m_oid);
			putPoint(g_r70New[j]->m_pos);
			putU32(realBits(g_r70New[j]->m_orientation));
		}
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_modFindHook = 0;
	g_psPosHook = 0;
	g_psFindHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_modDestroyHook = 0;
	g_r28ThingPose = false;
	g_objXformSyncPos = false;
	g_r70Ps.clear();
	me->m_behaviors = 0;
	me->m_ai = 0;
	delete m;
	for (int k = 1; k < 6; ++k) { g_r70Objs[k]->m_body = 0; delete bodies[k]; }
	delete d;
	for (int k = 0; k < 6; ++k) {
		Object *x = g_r70Objs[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_drawable = 0;
		delete x;
		g_r70Objs[k] = 0;
	}
	for (size_t j = 0; j < g_r70New.size(); ++j) { g_r70New[j]->m_behaviors = 0; delete g_r70New[j]; }
	g_r70New.clear();
	TheAudio = 0;
	TheParticleSystemManager = 0;
	TheRadar = 0;
	TheTerrainVisual = 0;
	TheTerrainLogic = 0;
	ThePartitionManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r70_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r70RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
