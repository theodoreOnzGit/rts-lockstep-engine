/* Differential-test driver for SquishCollide (p1w13-sonnet, update batch O; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/SquishCollide.cpp   (#included, with one mechanical edit: the `static` of its name key is removed,
**   build.rs, mod_gen/SquishCollide_nokey.cpp) with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: two Objects
** with the REAL GeometryInfo (Geometry.cpp), positions, orientations, crusher level, physics (the REAL PhysicsUpdate.h's PhysicsBehavior as a zeroed buffer with
** its velocity set), an AI with a goal object, a Hijacker and a SpecialAbilityUpdate the test gives, the REAL Object::getRelationship (Object.cpp, cut by
** build.rs) over a Team whose relationship is the test's, a body (MBody) that logs the damage the crusher deals, and the REAL PartitionManager::geomCollidesWithGeom
** (PartitionManager.cpp, compiled by wrap_partition.cpp) reached through ref_partition_geom_collides. */
#include "SquishCollide_nokey.cpp"

extern "C" int ref_partition_geom_collides(const void *p1, const void *g1, float a1, const void *p2, const void *g2, float a2);

namespace {

ModuleData *l9sqRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = SquishCollide::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_sqSaved;
Relationship g_sqRel = NEUTRAL;
bool g_sqHijacker = false;
unsigned char g_sqHijackerBuf[16] = { 0 };

UpdateModule *sqFindHook(const Object *, NameKeyType key)
{
	if (g_sqHijacker && key == TheNameKeyGenerator->nameToKey("HijackerUpdate")) return (UpdateModule *)(void *)g_sqHijackerBuf;
	return 0;
}

}  // namespace

#include "ObjectRelationship.inc"   /* Object::getRelationship, verbatim from Object.cpp (build.rs, mod_gen) */
Relationship Team::getRelationship(const Team *that) const { return that ? g_sqRel : NEUTRAL; }
Bool PartitionManager::geomCollidesWithGeom(const Coord3D* pos1, const GeometryInfo& geom1, Real angle1, const Coord3D* pos2, const GeometryInfo& geom2, Real angle2) const
{
	return ref_partition_geom_collides(pos1, &geom1, angle1, pos2, &geom2, angle2) != 0;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a SquishCollide and make the module on an Object (id 1) beside a second object (id 2, the crusher). `cfg`: [self has an
** AI, self has a body, the body's health (float bits), self has a team, the crusher has a team, the crusher's crusher level]. The data dump goes first (nothing: a CollideModuleData has no
** fields). Then `n` ops of 5 words [op, a, b, c, d]: 0 onCollide with the crusher (a = 0) or the ground (a = 1)  1 self position = (a, b, c) float bits  2 crusher
** position  3 orientations: self = a, crusher = b  4 self geometry: type a (0 sphere, 1 cylinder, 2 box), height b, major c, minor d  5 crusher geometry
** 6 the crusher's velocity = (a, b, c)  7 the AI's goal: none (a = 0), the crusher (1), self (2)  8 a Hijacker module present = a, a special ability update of power type
** b (0 none, 1 TNT attack, 2 another), active = c  9 undetected defector flags: self = a, crusher = b  10 the relationship (other's team to self's) = a (0 enemies,
** 1 neutral, 2 allies), a physics module present = b  11 xfer save  12 xfer load of the last save  13 xfer CRC. The constructor's line and each op append: the CATCH_ALL
** status, the effect log (count, then each as a string: "damage <object> <amount> <type> <death> <source>"), the xfer status, bytes and CRC, self's body health bits
** (0 without a body) and its undetected-defector flag. */
static int l9sqRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_sqSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9sqRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	Object *self = newObject();
	Object *crusher = newObject();
	self->m_oid = 1;
	crusher->m_oid = 2;
	Team teams[2];
	teams[0].m_dummy = 0;
	teams[1].m_dummy = 1;
	self->m_team = cfg[3] ? &teams[0] : 0;
	crusher->m_crusher = (UnsignedByte)cfg[5];
	crusher->m_team = cfg[4] ? &teams[1] : 0;
	AIUpdateInterface ai;
	self->m_ai = cfg[0] ? &ai : 0;
	MBody body;
	body.m_owner = self;
	memcpy(&body.m_health, &cfg[2], 4);
	body.m_max = body.m_health;
	self->m_body = cfg[1] ? &body : 0;
	alignas(16) static unsigned char physBuf[sizeof(PhysicsBehavior) + 16];
	memset(physBuf, 0, sizeof physBuf);
	PhysicsBehavior *physics = (PhysicsBehavior *)(void *)physBuf;
	crusher->m_physics = physics;
	SpecialAbilityUpdate sau;
	self->m_sau = &sau;
	self->m_sauType = -1;
	g_sqHijacker = false;
	g_sqRel = NEUTRAL;
	g_findUpdateHook = sqFindHook;
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	g_events.clear();
	g_ulog.clear();
	SquishCollide *m = 0;
	status = 0;
	try { m = new SquishCollide(self, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0; g_findUpdateHook = 0;
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
		if (phase) {
			g_events.clear();
			g_ulog.clear();
			try {
				switch (o[0]) {
				case 0: m->onCollide(o[1] ? 0 : crusher, 0, 0); break;
				case 1: memcpy(&self->m_pos.x, &o[1], 4); memcpy(&self->m_pos.y, &o[2], 4); memcpy(&self->m_pos.z, &o[3], 4); break;
				case 2: memcpy(&crusher->m_pos.x, &o[1], 4); memcpy(&crusher->m_pos.y, &o[2], 4); memcpy(&crusher->m_pos.z, &o[3], 4); break;
				case 3: memcpy(&self->m_orientation, &o[1], 4); memcpy(&crusher->m_orientation, &o[2], 4); break;
				case 4: { Real h, ma, mi; memcpy(&h, &o[2], 4); memcpy(&ma, &o[3], 4); memcpy(&mi, &o[4], 4); self->m_geometry.set((GeometryType)o[1], false, h, ma, mi); break; }
				case 5: { Real h, ma, mi; memcpy(&h, &o[2], 4); memcpy(&ma, &o[3], 4); memcpy(&mi, &o[4], 4); crusher->m_geometry.set((GeometryType)o[1], false, h, ma, mi); break; }
				case 6: memcpy(&physics->m_vel.x, &o[1], 4); memcpy(&physics->m_vel.y, &o[2], 4); memcpy(&physics->m_vel.z, &o[3], 4); break;
				case 7: ai.m_goal = o[1] == 0 ? 0 : (o[1] == 1 ? crusher : self); break;
				case 8:
					g_sqHijacker = o[1] != 0;
					self->m_sauType = o[2] == 0 ? -1 : (o[2] == 1 ? (int)SPECIAL_TANKHUNTER_TNT_ATTACK : (int)SPECIAL_DAISY_CUTTER);
					sau.m_active = o[3] != 0;
					break;
				case 9: self->m_undetected = o[1] != 0; crusher->m_undetected = o[2] != 0; break;
				case 10: g_sqRel = (Relationship)o[1]; crusher->m_physics = o[2] ? physics : 0; break;
				case 11: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_sqSaved.assign(bytes, bytes + blen); break;
				case 12: {
					std::vector<unsigned char> b = g_sqSaved;
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
		putU32(self->m_body ? realBits(body.m_health) : 0);
		putU32(self->m_undetected ? 1 : 0);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete self;
	delete crusher;
	TheGameLogic = 0;
	g_findUpdateHook = 0;
	return 0;
}

int ref_l9sq_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9sqRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
