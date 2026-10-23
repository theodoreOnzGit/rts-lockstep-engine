/* Differential-test driver for AutoFindHealingUpdate and PilotFindVehicleUpdate (p1w13-sonnet, batch R19; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AutoFindHealingUpdate.cpp
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PilotFindVehicleUpdate.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the module's object (id 1: a controlling player, human or computer, whose
** getAiBaseCenter answers from the test; maybe an AI, idle or not; maybe a body; up to two collide modules, whose wouldLikeToCollideWith logs and
** answers from the test) and three more (ids 2 to 4: kinds, maybe a body); the partition manager's range scan (the auto heal driver's: the objects
** the test lists, the call a line) and getDistanceSquared (the ActiveBody driver's table, the call a line); aiGetHealed, aiEnter and
** aiMoveToPosition (lines); the game logic, and the REAL UpdateModule, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AutoFindHealingUpdate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PilotFindVehicleUpdate.cpp"

namespace {
bool g_r19BaseHas = false;
Coord3D g_r19Base;
}  // namespace

void AIUpdateInterface::aiGetHealed(Object *healDepot, CommandSourceType cmdSource) { ulog("aiheal %d %d %d", (int)m_owner->m_oid, (int)healDepot->m_oid, (int)cmdSource); }
void AIUpdateInterface::aiEnter(Object *obj, CommandSourceType cmdSource) { ulog("aienter %d %d %d", (int)m_owner->m_oid, (int)obj->m_oid, (int)cmdSource); }
Bool Player::getAiBaseCenter(Coord3D *pos)
{
	ulog("basecenter %d", (int)m_idx);
	if (!g_r19BaseHas) return FALSE;
	*pos = g_r19Base;
	return TRUE;
}

namespace {

/* a collide module of the pilot: wouldLikeToCollideWith logs and answers whether the other is in its mask (bit k: object k + 1) */
class R19Collide : public CollideModule {
public:
	R19Collide(Thing *t, const ModuleData *d) : CollideModule(t, d) {}
	COMMON_TEST_MODULE_BITS
	virtual void onCollide(Object *, const Coord3D *, const Coord3D *) {}
	virtual Bool wouldLikeToCollideWith(const Object *other) const
	{
		ulog("wouldcollide %d %d", (int)getObject()->m_oid, (int)other->m_oid);
		return (m_mask >> (other->m_oid - 1) & 1) != 0;
	}
	unsigned m_mask = 0;
};

ModuleData *l9r19Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = kind ? PilotFindVehicleUpdate::friend_newModuleData(&ini) : AutoFindHealingUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r19Saved;
/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_r19Kinds[2] = { KINDOF_HEAL_PAD, KINDOF_VEHICLE };
void r19SetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 2; ++i) if (bits >> i & 1) o->m_kindOf.set(g_r19Kinds[i]);
	o->m_tmpl.m_kindOf = o->m_kindOf;
}
void r19ScanHook(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	int n = 0;
	while (filters && filters[n]) ++n;
	ulog("scan %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), n);
}
Real r19F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of an AutoFindHealingUpdate (`kind` 0) or a PilotFindVehicleUpdate (1) and make the module on object 1
** of four. `cfg` (15 words): [0 flags: bit 0 the player is a computer, 1 an AI, 2 the AI is idle, 3 a body, 4 the player AI has a base centre,
** 8-10 objects 2 to 4 have a body, 12-17 the kinds of objects 2 to 4 (2 bits each, of g_r19Kinds); 1 and 2 object 1's health and maximum;
** 3-5 the health of objects 2 to 4 (maximum 100); 6-9 the distance squared from object 1 to objects 1 to 4; 10 the range scan (4 nibbles of
** ids, 0 ends); 11 the collide modules (bits 0-1 how many, then 4 bits each: the objects it would collide with); 12-14 the base centre]. The data
** dump goes first. Then `n` ops of 6 words [op, a, b, c, d, e]: 0 update()  1 flags = a  2 the scan = a  3 the health of object a = b
** 4 the distance to object a = b  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op appends: the CATCH_ALL status, the result
** (update's sleep time), the effect log (count, then each), the xfer status, bytes and CRC, the module's field. */
int ref_l9r19_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r19Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r19Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	if (kind) {
		const PilotFindVehicleUpdateModuleData *pd = (const PilotFindVehicleUpdateModuleData *)d;
		putU32(pd->m_scanFrames);
		putU32(realBits(pd->m_scanRange));
		putU32(realBits(pd->m_minHealth));
	} else {
		const AutoFindHealingUpdateModuleData *ad = (const AutoFindHealingUpdateModuleData *)d;
		putU32(ad->m_scanFrames);
		putU32(realBits(ad->m_scanRange));
		putU32(realBits(ad->m_neverHeal));
		putU32(realBits(ad->m_alwaysHeal));
	}
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	Player player;
	player.m_idx = 0;
	g_r19BaseHas = false;
	g_r19Base.x = r19F(cfg[12]);
	g_r19Base.y = r19F(cfg[13]);
	g_r19Base.z = r19F(cfg[14]);
	Object *objs[4];
	MBody bodies[4];
	AIUpdateInterface ai;
	for (int k = 0; k < 4; ++k) {
		objs[k] = newObject();
		objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = objs[k];
		bodies[k].m_max = 100.0f;
	}
	Object *me = objs[0];
	me->m_player = &player;
	me->m_pos.x = 10.0f;
	me->m_pos.y = -2.5f;
	me->m_pos.z = 1.0f;
	ai.m_owner = me;
	bodies[0].m_health = r19F(cfg[1]);
	bodies[0].m_max = r19F(cfg[2]);
	for (int k = 1; k < 4; ++k) bodies[k].m_health = r19F(cfg[2 + k]);
	for (int k = 0; k < 4; ++k) g_abDist[0][k] = r19F(cfg[6 + k]);
	ModuleData plain;
	R19Collide collides[2] = { R19Collide(me, &plain), R19Collide(me, &plain) };
	int nCollide = (int)(cfg[11] & 3) % 3;
	BehaviorModule *behaviors[3] = { 0, 0, 0 };
	for (int k = 0; k < nCollide; ++k) {
		collides[k].m_mask = cfg[11] >> (4 + 4 * k) & 15;
		behaviors[k] = &collides[k];
	}
	me->m_behaviors = behaviors;
	unsigned flags = cfg[0];
	unsigned scan = cfg[10];
	g_ahRangeHook = r19ScanHook;
	gl.m_awakenCalls = 0;
	UpdateModule *m = 0;
	if (kind) m = new PilotFindVehicleUpdate(me, d); else m = new AutoFindHealingUpdate(me, d);
	/* GameLogic::friend_awakenUpdateModule for an object not yet in the list (GameLogic.cpp:3035-3046), and the Rust world scheduling an update
	** module when it makes the object (frame 1 at frame 0) for a constructor that sets no wake frame (the batch R13e correction) */
	m->friend_setNextCallFrame(gl.m_awakenCalls ? gl.m_awakenFrame : 1);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 1) flags = o[1];
		if (o[0] == 2) scan = o[1];
		if (o[0] == 3) bodies[(o[1] - 1) % 4].m_health = r19F(o[2]);
		if (o[0] == 4) g_abDist[0][(o[1] - 1) % 4] = r19F(o[2]);
		player.m_type = (flags & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN;   /* the stand-in has no constructor */
		me->m_ai = (flags & 2) ? &ai : 0;
		ai.m_idle = (flags & 4) != 0;
		me->m_body = (flags & 8) ? &bodies[0] : 0;
		g_r19BaseHas = (flags & 16) != 0;
		for (int k = 1; k < 4; ++k) {
			objs[k]->m_body = (flags >> (7 + k) & 1) ? &bodies[k] : 0;
			r19SetKinds(objs[k], flags >> (12 + 2 * (k - 1)) & 3);
		}
		g_ahRange.clear();
		for (int k = 0; k < 4; ++k) { unsigned id = scan >> (4 * k) & 15; if (id == 0) break; if (id <= 4) g_ahRange.push_back(objs[id - 1]); }
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
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r19Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r19Saved;
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
		putU32(kind ? (unsigned)((PilotFindVehicleUpdate *)m)->m_didMoveToBase : (unsigned)((AutoFindHealingUpdate *)m)->m_nextScanFrames);
	}
	*len = finish(buf, cap);
	g_ahRangeHook = 0;
	g_ahRange.clear();
	delete m;
	delete d;
	me->m_behaviors = 0;
	for (int k = 0; k < 4; ++k) { objs[k]->m_ai = 0; objs[k]->m_body = 0; delete objs[k]; }
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
