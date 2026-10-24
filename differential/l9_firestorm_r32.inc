/* Differential-test driver for FirestormDynamicGeometryInfoUpdate (p1w13-sonnet, batch R32; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FirestormDynamicGeometryInfoUpdate.cpp   (#included with one mechanical edit made by
**     build.rs into mod_gen/FirestormDynamicGeometryInfoUpdate_edit.cpp: its particle system parser is this unit's l9ahParsePsTemplate, asserted
**     sixteen times; its base DynamicGeometryInfoUpdate.cpp is l9_geom_stealth_r23.inc's)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: four objects (ids 1-4, each with the module build's MBody, whose
** attemptDamage is a line, and a height), the firestorm (id 1: a geometry, maybe a drawable, which logs "react" when the geometry changes; maybe a
** player, whose academy's firestorm count is a line), the terrain's ground height (the height die driver's table), the auto heal driver's
** particle system manager ("psys"; its systems, which the active body driver's findParticleSystem finds unless their id is dead, take a radius
** as a line "psradius"), the range scan (the objects the test lists; the call a line), the FX lists ("fx"), the game client's scorch (a line),
** the game logic (its frame, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them), and the REAL UpdateModule, INI and
** Xfer. */
#include "FirestormDynamicGeometryInfoUpdate_edit.cpp"

namespace {

ModuleData *l9r32Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FirestormDynamicGeometryInfoUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r32Saved;
UpdateModule *g_r32Updating = 0;
void r32Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r32Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
void r32ScanHook(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	int n = 0;
	while (filters && filters[n]) ++n;
	ulog("scan %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), n);
}
Real r32F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
ParticleSystemInfo::EmissionVolumeType g_r32Volume = ParticleSystemInfo::SPHERE;

}  // namespace

void PowerAcademyStats::recordFirestormCreated() { ulog("firestorm %d", (int)((Player *)(void *)this)->m_idx); }
void GameClient::addScorch(const Coord3D *pos, Real radius, Scorches type)
{
	ulog("scorch %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(radius), (int)type);
}
ParticleSystemInfo::EmissionVolumeType ParticleSystem::getEmisionVolumeType() const { return g_r32Volume; }
void ParticleSystem::setEmissionVolumeSphereRadius( Real radius ) { ulog("psradius %u %08x", m_id, realBits(radius)); }
void ParticleSystem::setEmissionVolumeCylinderRadius( Real radius ) { ulog("psradius %u %08x", m_id, realBits(radius)); }

extern "C" {

/* Read `filename` (block `D`) as the data of a FirestormDynamicGeometryInfoUpdate and make the module on object 1 of four at frame cfg[0]. `cfg`
** (16 words; 1-12 also settable by op 1): 0 the frame, 1 flags (0 drawable, 1 a player, 2-3 the systems' volume: 0 sphere, 1 cylinder, 2 box),
** 2 object 1's geometry type (0 sphere, 1 cylinder, 2 box) and is-small (bit 4), 3-5 its height and radii (float bits), 6 the scan (nibbles of
** object numbers 1-4, 0 ends), 7-10 the objects' z (float bits), 11 the ground height (float bits), 12 the dead particle systems (modulus in
** the low byte, remainder in the next; modulus 0 none), 13-15 unused. The data dump goes first. Then the constructor's wake calls. Then `n` ops of
** 4 words [op, a, b, c]: 0 update()  1 cfg[a] = b  2 the frame = a  3 xfer save  4 xfer load of the last save  5 xfer CRC. Each op appends: the
** CATCH_ALL status, the result, the effect log, the wake calls (count, last frame), the xfer status, bytes and CRC, the base's 13 state words,
** this class's (16 ids, the two flags, the last damage frame), object 1's geometry (type, is-small, height, major and minor radius bits) and the
** player's firestorm count. */
int ref_l9r32_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r32Saved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	TheParticleSystemManager = &g_ahPsm;
	try { d = l9r32Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { TheParticleSystemManager = 0; return status; }
	unsigned cfg[16];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const FirestormDynamicGeometryInfoUpdateModuleData *fd = (const FirestormDynamicGeometryInfoUpdateModuleData *)d;
		putU32(fd->m_initialDelay);
		putU32(realBits(fd->m_initialHeight)); putU32(realBits(fd->m_initialMajorRadius)); putU32(realBits(fd->m_initialMinorRadius));
		putU32(realBits(fd->m_finalHeight)); putU32(realBits(fd->m_finalMajorRadius)); putU32(realBits(fd->m_finalMinorRadius));
		putU32(fd->m_transitionTime);
		putU32(fd->m_reverseAtTransitionTime ? 1 : 0);
		for (int i = 0; i < MAX_FIRESTORM_SYSTEMS; ++i) { const char *nm = ahPsName(fd->m_particleSystem[i]); putStr(nm, (int)strlen(nm)); }
		putI32(fxId(fd->m_fxList));
		putU32(realBits(fd->m_particleOffsetZ));
		putU32(realBits(fd->m_scorchSize));
		putU32(realBits(fd->m_delayBetweenDamageFrames));
		putU32(realBits(fd->m_damageAmount));
		putU32(realBits(fd->m_maxHeightForDamage));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	g_ahPsLastId = 0;
	g_ahPsNext = 0;
	TerrainLogic terrain;
	TheTerrainLogic = &terrain;
	Player player;
	player.m_idx = 0;
	Object *objs[4];
	MBody bodies[4];
	Drawable draw;
	for (int k = 0; k < 4; ++k) {
		objs[k] = newObject();
		objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = objs[k];
		objs[k]->m_body = &bodies[k];
		objs[k]->m_pos.x = 10.0f * (Real)k;
		objs[k]->m_pos.y = -2.5f;
	}
	Object *me = objs[0];
	draw.m_ownerObject = me;
	me->m_geometry = GeometryInfo((GeometryType)(cfg[2] & 3), (cfg[2] >> 4 & 1) != 0, r32F(cfg[3]), r32F(cfg[4]), r32F(cfg[5]));
	auto scene = [&]() {
		unsigned f = cfg[1];
		me->m_drawable = (f & 1) ? &draw : 0;
		me->m_player = (f >> 1 & 1) ? &player : 0;
		unsigned vol = f >> 2 & 3;
		g_r32Volume = vol == 0 ? ParticleSystemInfo::SPHERE : vol == 1 ? ParticleSystemInfo::CYLINDER : ParticleSystemInfo::BOX;
		g_ahRange.clear();
		for (int k = 0; k < 4; ++k) { unsigned id = cfg[6] >> (4 * k) & 15; if (id == 0) break; if (id <= 4) g_ahRange.push_back(objs[id - 1]); }
		for (int k = 0; k < 4; ++k) objs[k]->m_pos.z = r32F(cfg[7 + k]);
		for (int c = 0; c < 8; ++c) g_hdGround[c] = r32F(cfg[11]);
		g_abDeadMod = cfg[12] & 255;
		g_abDeadRem = cfg[12] >> 8 & 255;
	};
	scene();
	g_ahRangeHook = r32ScanHook;
	gl.m_awakenHook = r32Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	FirestormDynamicGeometryInfoUpdate *m = new FirestormDynamicGeometryInfoUpdate(me, d);
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	/* the Rust world schedules an update module when it makes the object (frame 1 at frame 0) for a constructor that sets no wake frame */
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	/* m_direction is never set upstream and xfer saves it: indeterminate bytes. The translation holds 0; so does this object from here on */
	m->m_direction = (DynamicGeometryInfoUpdate::DynamicGeometryDirection)0;
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	unsigned firestorms = 0;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 1 && o[1] >= 1 && o[1] <= 12 && (o[1] < 2 || o[1] > 5)) cfg[o[1]] = o[2];
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
		unsigned char bytes[1024];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: g_r32Updating = m; res = (unsigned)m->update(); break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 1024, &blen, &crc); g_r32Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r32Saved;
				if (b.size() > 1024) b.resize(1024);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r32Updating = 0;
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) {
				const std::string &l = g_ulog[(size_t)e.p];
				if (l.compare(0, 10, "firestorm ") == 0) { ++firestorms; continue; }
				log.push_back(l);
			} else if (e.kind == 1 && e.p) {
				char b[32]; sprintf(b, "fx %d", fxId((const FXList *)e.p)); log.push_back(b);
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
		putU32(m->m_startingDelayCountdown);
		putU32(m->m_timeActive);
		putU32(m->m_started ? 1 : 0);
		putU32(m->m_finished ? 1 : 0);
		putU32(m->m_reverseAtTransitionTime ? 1 : 0);
		putU32((unsigned)m->m_direction);
		putU32(m->m_switchedDirections ? 1 : 0);
		putU32(realBits(m->m_initialHeight)); putU32(realBits(m->m_initialMajorRadius)); putU32(realBits(m->m_initialMinorRadius));
		putU32(realBits(m->m_finalHeight)); putU32(realBits(m->m_finalMajorRadius)); putU32(realBits(m->m_finalMinorRadius));
		for (int j = 0; j < MAX_FIRESTORM_SYSTEMS; ++j) putU32(m->m_myParticleSystemID[j]);
		putU32(m->m_effectsFired ? 1 : 0);
		putU32(m->m_scorchPlaced ? 1 : 0);
		putU32(m->m_lastDamageFrame);
		const GeometryInfo &gi = me->getGeometryInfo();
		putU32((unsigned)gi.getGeomType());
		putU32(gi.getIsSmall() ? 1 : 0);
		putU32(realBits(gi.getMaxHeightAbovePosition()));
		putU32(realBits(gi.getMajorRadius()));
		putU32(realBits(gi.getMinorRadius()));
		putU32(firestorms);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_abDeadMod = g_abDeadRem = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) { objs[k]->m_drawable = 0; objs[k]->m_body = 0; objs[k]->m_player = 0; delete objs[k]; }
	TheGameLogic = 0;
	TheTerrainLogic = 0;
	TheParticleSystemManager = 0;
	return 0;
}

}  /* extern "C" */
