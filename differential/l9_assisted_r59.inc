/* Differential-test driver for AssistedTargetingUpdate (p1w13-sonnet, batch R59; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AssistedTargetingUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: three objects (ids 1-3: the assisting unit,
** the requesting object and the victim; positions the test's), the unit (maybe an AI, whose aiAttackObject is a line; NO_ATTACK or not
** (isAbleToAttack's stand-in); maybe a current weapon, its status the test's; a team of three, each team its player's default team), the
** thing factory (the templates LaserA, with a LaserUpdate client module, and LaserB, without; newObject makes a laser object, ids 4, 5, ...,
** with a drawable), the laser's LaserUpdate (initLaser a line), setWeaponLock (batch R55's line), destroyObject (a line), the REAL
** UpdateModule, INI and Xfer; the game logic (its frame). */
/* (batch R64: the stand-in Weapon's getStatus is now Weapon.cpp's own, whose answer depends on the frames; this test gives the status
** directly, so the file's one call, `getCurrentWeapon()->getStatus()`, is renamed by macro to the stand-in's r59GetStatus for the include) */
#define getStatus r59GetStatus
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AssistedTargetingUpdate.cpp"
#undef getStatus

void LaserUpdate::initLaser(const Object *parent, const Object *target, const Coord3D *startPos, const Coord3D *endPos, AsciiString parentBoneName, Int sizeDeltaFrames)
{
	/* batch R75: a client drawable's laser (ParticleUplinkCannonUpdate's beams), its growth frames */
	if (m_r75Drawable) {
		ulog("laserdraw %d %u,%u,%u %u,%u,%u %d", (int)m_r75Drawable, realBits(startPos->x), realBits(startPos->y), realBits(startPos->z), realBits(endPos->x),
			realBits(endPos->y), realBits(endPos->z), (int)sizeDeltaFrames);
		return;
	}
	ulog("laser %d %d %d %u,%u,%u %u,%u,%u %s", m_r59Owner ? (int)m_r59Owner->m_oid : 0, parent ? (int)parent->m_oid : 0, target ? (int)target->m_oid : 0,
		realBits(startPos->x), realBits(startPos->y), realBits(startPos->z), realBits(endPos->x), realBits(endPos->y), realBits(endPos->z), parentBoneName.str());
}

namespace {

ModuleData *l9r59Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = AssistedTargetingUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r59Saved;
Object *g_r59Objs[3];
int g_r59Tmpl[2];   /* the two templates' identities (their addresses) */
const char *const g_r59TmplNames[2] = { "LaserA", "LaserB" };
std::vector<Object *> g_r59Lasers;
std::vector<Drawable *> g_r59LaserDraws;
std::vector<LaserUpdate *> g_r59LaserUpdates;
unsigned g_r59NextId = 4;
Object *r59Find(ObjectID id)
{
	if ((int)id >= 1 && (int)id <= 3) return g_r59Objs[(int)id - 1];
	for (size_t i = 0; i < g_r59Lasers.size(); ++i) if (g_r59Lasers[i]->m_oid == id) return g_r59Lasers[i];
	return 0;
}
int r59Code(const ThingTemplate *t) { return t == 0 ? 0 : (t == (const ThingTemplate *)&g_r59Tmpl[0] ? 1 : 2); }
const ThingTemplate *r59FindTemplate(const AsciiString &name)
{
	for (int i = 0; i < 2; ++i) if (strcmp(name.str(), g_r59TmplNames[i]) == 0) return (const ThingTemplate *)&g_r59Tmpl[i];
	return 0;
}
Object *r59NewObject(const ThingTemplate *tmplate, Team *team)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)g_r59NextId++;
	o->m_team = team;
	Drawable *draw = new Drawable;
	draw->m_ownerObject = o;
	o->m_drawable = draw;
	if (r59Code(tmplate) == 1) {
		LaserUpdate *u = new LaserUpdate;
		u->m_r59Owner = o;
		draw->m_r59Laser = (ClientUpdateModule *)u;
		g_r59LaserUpdates.push_back(u);
	}
	g_r59Lasers.push_back(o);
	g_r59LaserDraws.push_back(draw);
	return o;
}
void r59Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
Real r59F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of an AssistedTargetingUpdate and make the module on object 1 of three at frame cfg[0]. `cfg` (8 words;
** 1-7 also settable by op 2): 1 the unit: bit 0 an AI, 1 NO_ATTACK, 2-4 its current weapon's status (0-4; else none), 5-6 its team (0 none,
** else 1-3); 2-4 objects 1-3's x, 5-7 their y (float bits; z 0). The data dump goes first (the clip size, the slot, the two names), then the
** state. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3 isFreeToAssist()  4 assistAttack(object a,
** object b; else NULL)  5 xfer save  6 xfer load of the last save, then loadPostProcess  7 xfer CRC. Each op appends: the CATCH_ALL status,
** the result, the effect log, the xfer status, bytes and CRC, the state (the two templates: 0 NULL, 1 LaserA, 2 LaserB), the lasers made. */
static int l9r59RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r59Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r59Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[8];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const AssistedTargetingUpdateModuleData *ad = (const AssistedTargetingUpdateModuleData *)d;
		putI32(ad->m_clipSize);
		putU32((unsigned)ad->m_weaponSlot);
		putStr(ad->m_laserFromAssistedName.str(), (int)ad->m_laserFromAssistedName.getLength());
		putStr(ad->m_laserToTargetName.str(), (int)ad->m_laserToTargetName.getLength());
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	g_r59NextId = 4;
	Player players[3];
	Team teams[3];
	for (int p = 0; p < 3; ++p) {
		players[p].m_idx = p;
		teams[p].m_dummy = p + 1;
		teams[p].m_controller = &players[p];
		players[p].m_defaultTeam = &teams[p];
	}
	AIUpdateInterface ai;
	Weapon weapon;
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 3; ++k) {
		g_r59Objs[k] = newObject();
		g_r59Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r59Objs[k]->m_behaviors = none;
	}
	Object *me = g_r59Objs[0];
	ai.m_owner = me;
	auto scene = [&]() {
		unsigned f = cfg[1];
		me->m_ai = (f & 1) ? &ai : 0;
		me->m_status.clear();
		if (f >> 1 & 1) me->m_status.set(OBJECT_STATUS_NO_ATTACK);
		unsigned ws = f >> 2 & 7;
		weapon.m_r59Status = (Int)ws;
		me->m_curWeapon = ws <= 4 ? &weapon : 0;
		unsigned t = f >> 5 & 3;
		me->m_team = t ? &teams[t - 1] : 0;
		me->m_player = t ? &players[t - 1] : 0;
		for (int k = 0; k < 3; ++k) {
			g_r59Objs[k]->m_pos.x = r59F(cfg[2 + k]);
			g_r59Objs[k]->m_pos.y = r59F(cfg[5 + k]);
			g_r59Objs[k]->m_pos.z = 0.0f;
		}
	};
	scene();
	g_modFindHook = r59Find;
	g_findTemplateHook = r59FindTemplate;
	g_newObjectHook = r59NewObject;
	g_modDestroyHook = r59Destroy;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	AssistedTargetingUpdate *m = 0;
	try { m = newInstance(AssistedTargetingUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	putU32((unsigned)r59Code(m->m_laserFromAssisted));
	putU32((unsigned)r59Code(m->m_laserToTarget));
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 1 && o[1] <= 7) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
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
			case 3: res = m->isFreeToAssist() ? 1 : 0; break;
			case 4: m->assistAttack(r59Find((ObjectID)o[1]), r59Find((ObjectID)o[2])); break;
			case 5: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r59Saved.assign(bytes, bytes + blen); break;
			case 6: {
				std::vector<unsigned char> b = g_r59Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 7: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
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
		putU32((unsigned)r59Code(m->m_laserFromAssisted));
		putU32((unsigned)r59Code(m->m_laserToTarget));
		putU32((unsigned)g_r59Lasers.size());
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_modDestroyHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 3; ++k) {
		Object *x = g_r59Objs[k];
		x->m_ai = 0; x->m_team = 0; x->m_player = 0; x->m_curWeapon = 0; x->m_behaviors = 0;
		delete x;
		g_r59Objs[k] = 0;
	}
	for (size_t i = 0; i < g_r59Lasers.size(); ++i) { g_r59Lasers[i]->m_drawable = 0; g_r59Lasers[i]->m_team = 0; delete g_r59Lasers[i]; }
	for (size_t i = 0; i < g_r59LaserDraws.size(); ++i) delete g_r59LaserDraws[i];
	for (size_t i = 0; i < g_r59LaserUpdates.size(); ++i) delete g_r59LaserUpdates[i];
	g_r59Lasers.clear();
	g_r59LaserDraws.clear();
	g_r59LaserUpdates.clear();
	TheGameLogic = 0;
	return 0;
}

int ref_l9r59_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r59RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
