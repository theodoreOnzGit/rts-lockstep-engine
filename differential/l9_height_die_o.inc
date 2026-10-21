/* Differential-test driver for HeightDieUpdate (p1w13-sonnet, update batch O; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HeightDieUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: an Object with the REAL GeometryInfo (Geometry.cpp), a
** position the test sets, a container the test sets, a body (MBody, l9_body_m.inc: it logs the damage it is given and dies of a kill), a terrain logic whose three
** queries log and answer from tables the test sets (by a hash of the position, as the Rust side's cut points do), a partition manager whose range scan logs and
** answers with the list the test gives, a particle system manager whose destroyAttachedSystems logs, the game logic whose frame the test sets, and the REAL
** UpdateModule, INI and Xfer. Object::kill is Object.cpp lines 1954-1968 (the assertion dropped). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HeightDieUpdate.cpp"

namespace {

ModuleData *l9hdRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = HeightDieUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_hdSaved;
TerrainLogic g_hdTerrain;
PartitionManager g_hdPartition;
ParticleSystemManager g_hdPsm;
Real g_hdGround[8], g_hdLayerHeight[8];
int g_hdLayer[8];
std::vector<Object *> g_hdScan;

/* the Rust side's terrain_cell (object_cuts.rs) */
unsigned hdCell(Real x, Real y)
{
	unsigned a, b;
	memcpy(&a, &x, 4);
	memcpy(&b, &y, 4);
	b = (b << 13) | (b >> 19);
	return ((a ^ b) * 2654435761u >> 28) & 7;
}

}  // namespace

/* Object.cpp lines 1954-1968 (the DEBUG_ASSERTCRASH dropped) */
void Object::kill( DamageType damageType, DeathType deathType )
{
	DamageInfo damageInfo;

	// Do unmodifiable damage equal to their max health to kill.
	damageInfo.in.m_damageType = damageType;
	damageInfo.in.m_deathType = deathType;
	damageInfo.in.m_sourceID = INVALID_ID;
	damageInfo.in.m_amount = getBodyModule()->getMaxHealth();
	damageInfo.in.m_kill = TRUE; // Triggers object to die no matter what.
	attemptDamage( &damageInfo );
}
Real TerrainLogic::getGroundHeight( Real x, Real y, Coord3D* normal ) const
{
	ulog("ground %08x %08x", realBits(x), realBits(y));
	return g_hdGround[hdCell(x, y)];
}
Real TerrainLogic::getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D* normal, Bool clip) const
{
	ulog("layerheight %08x %08x %d", realBits(x), realBits(y), (int)layer);
	return g_hdLayerHeight[hdCell(x, y)];
}
PathfindLayerEnum TerrainLogic::getHighestLayerForDestination(const Coord3D *pos, Bool onlyHealthyBridges)
{
	ulog("highlayer %08x %08x %08x", realBits(pos->x), realBits(pos->y), realBits(pos->z));
	return (PathfindLayerEnum)g_hdLayer[hdCell(pos->x, pos->y)];
}
SimpleObjectIterator *PartitionManager::iterateObjectsInRange( const Object *obj, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters, IterOrderType order )
{
	ulog("scan %d %08x %d", (int)obj->m_oid, realBits(maxDist), (int)dc);
	SimpleObjectIterator *it = new SimpleObjectIterator;
	it->m_list = g_hdScan;
	return it;
}
void ParticleSystemManager::destroyAttachedSystems( Object *obj ) { ulog("psdestroy %d", (int)obj->m_oid); }

extern "C" {

/* Read `filename` (block `D`) as the data of a HeightDieUpdate and make the module on an Object (id 1) at the position cfg[1..3] (float bits) with the frame
** cfg[0], a body (health cfg[5], maximum cfg[6], float bits; none if cfg[4] is 0), contained in object 2 if cfg[7] is not 0, and a box geometry of major radius
** cfg[8], minor radius cfg[9] and height cfg[10]; objects 2 to 4 are boxes of height cfg[11] to cfg[13], the other candidates of the range scan. The data
** dump goes first (the six fields). Then `n` ops of 5 words [op, a, b, c, ...]: 0 update()  1 position = (a, b, c) float bits  2 terrain table a (0 ground height,
** 1 layer, 2 layer height) cell b = c (float bits, or the layer)  3 the range scan list: the objects of the mask a over [1, 2, 3, 4], reversed if b  4 contained
** = (a != 0)  5 the frame = a  6 xfer save  7 xfer load of the last save  8 xfer CRC. The constructor's line and each op append: the CATCH_ALL status and the
** sleep time update() returned (0 for the constructor and the other ops), the effect log (count, then each as a string: "ground <x> <y>", "highlayer <x> <y> <z>",
** "layerheight <x> <y> <layer>", "scan <object> <range> <dc>", "psdestroy <object>", "damage <object> <amount> <type> <death> <source>"), the xfer status, bytes
** and CRC, the module's six state words (died, particles destroyed, last position x y z, earliest death frame), the object's position (3 words) and its body's
** health bits. */
static int l9hdRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_hdSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9hdRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const HeightDieUpdateModuleData *hd = (const HeightDieUpdateModuleData *)d;
		putU32(realBits(hd->m_targetHeightAboveTerrain));
		putU32(hd->m_targetHeightIncludesStructures ? 1 : 0);
		putU32(hd->m_onlyWhenMovingDown ? 1 : 0);
		putU32(realBits(hd->m_destroyAttachedParticlesAtHeight));
		putU32(hd->m_snapToGroundOnDeath ? 1 : 0);
		putU32(hd->m_initialDelay);
	}
	Object *objs[4];
	for (int k = 0; k < 4; ++k) {
		objs[k] = newObject();
		objs[k]->m_oid = k + 1;
		objs[k]->m_pos.x = objs[k]->m_pos.y = objs[k]->m_pos.z = 0.0f;
	}
	{
		Real major, minor, height;
		memcpy(&major, &cfg[8], 4);
		memcpy(&minor, &cfg[9], 4);
		memcpy(&height, &cfg[10], 4);
		objs[0]->m_geometry.set(GEOMETRY_BOX, false, height, major, minor);
		for (int k = 1; k < 4; ++k) {
			Real h;
			memcpy(&h, &cfg[10 + k], 4);
			objs[k]->m_geometry.set(GEOMETRY_BOX, false, h, 1.0f, 1.0f);
		}
		memcpy(&objs[0]->m_pos.x, &cfg[1], 4);
		memcpy(&objs[0]->m_pos.y, &cfg[2], 4);
		memcpy(&objs[0]->m_pos.z, &cfg[3], 4);
	}
	MBody body;
	body.m_owner = objs[0];
	memcpy(&body.m_health, &cfg[5], 4);
	memcpy(&body.m_max, &cfg[6], 4);
	objs[0]->m_body = cfg[4] ? &body : 0;
	objs[0]->m_containedBy = cfg[7] ? objs[1] : 0;
	for (int i = 0; i < 8; ++i) { g_hdGround[i] = 0.0f; g_hdLayerHeight[i] = 0.0f; g_hdLayer[i] = LAYER_GROUND; }
	g_hdScan.clear();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	TheTerrainLogic = &g_hdTerrain;
	ThePartitionManager = &g_hdPartition;
	TheParticleSystemManager = &g_hdPsm;
	g_events.clear();
	g_ulog.clear();
	HeightDieUpdate *m = 0;
	status = 0;
	try { m = new HeightDieUpdate(objs[0], d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0; TheTerrainLogic = 0; ThePartitionManager = 0; TheParticleSystemManager = 0;
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
				case 1: memcpy(&objs[0]->m_pos.x, &o[1], 4); memcpy(&objs[0]->m_pos.y, &o[2], 4); memcpy(&objs[0]->m_pos.z, &o[3], 4); break;
				case 2:
					if (o[1] == 0) memcpy(&g_hdGround[o[2] & 7], &o[3], 4);
					else if (o[1] == 1) g_hdLayer[o[2] & 7] = (int)o[3];
					else memcpy(&g_hdLayerHeight[o[2] & 7], &o[3], 4);
					break;
				case 3:
					g_hdScan.clear();
					for (int k = 0; k < 4; ++k) if (o[1] >> k & 1) g_hdScan.push_back(objs[k]);
					if (o[2]) std::reverse(g_hdScan.begin(), g_hdScan.end());
					break;
				case 4: objs[0]->m_containedBy = o[1] ? objs[1] : 0; break;
				case 5: gl.m_frame = o[1]; break;
				case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_hdSaved.assign(bytes, bytes + blen); break;
				case 7: {
					std::vector<unsigned char> b = g_hdSaved;
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
		putU32(m->m_hasDied ? 1 : 0);
		putU32(m->m_particlesDestroyed ? 1 : 0);
		putU32(realBits(m->m_lastPosition.x));
		putU32(realBits(m->m_lastPosition.y));
		putU32(realBits(m->m_lastPosition.z));
		putU32(m->m_earliestDeathFrame);
		putU32(realBits(objs[0]->m_pos.x));
		putU32(realBits(objs[0]->m_pos.y));
		putU32(realBits(objs[0]->m_pos.z));
		putU32(objs[0]->m_body ? realBits(body.m_health) : 0);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) delete objs[k];
	TheGameLogic = 0;
	TheTerrainLogic = 0;
	ThePartitionManager = 0;
	TheParticleSystemManager = 0;
	return 0;
}

int ref_l9hd_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9hdRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
