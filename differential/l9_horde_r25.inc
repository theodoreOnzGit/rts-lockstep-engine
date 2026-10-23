/* Differential-test driver for HordeUpdate (p1w13-sonnet, batch R25; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HordeUpdate.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the module's object (id 1: kinds, maybe an AI whose evaluateMoraleBonus logs,
** maybe a drawable whose terrain decal calls log and which keeps its decal type, the weapon bonus conditions, off the map or not, effectively dead or not,
** a geometry) and three more (ids 2 to 4: kinds, maybe a horde interface whose isTrueHordeMember answers from the test, maybe sharing the object's
** template, off the map or not, their team's relationship to the object's); the height-die driver's object range scan (the objects the test lists, the
** call a line, filtered by the module's own PartitionFilterHordeMember here) and the ActiveBody driver's distances; the REAL Object::getRelationship
** with the team relationships the test gives; the game logic (its frame, its draw-icon flag, the logic random values), whose wake calls the test
** counts; and the REAL UpdateModule, INI and Xfer. */
#define DEFINE_HORDEACTION_NAMES
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HordeUpdate.cpp"

void AIUpdateInterface::evaluateMoraleBonus( void ) { ulog("morale %d", (int)m_owner->m_oid); }

namespace {

ModuleData *l9r25Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = HordeUpdate::friend_newModuleData(&ini);
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

/* a behaviour with a horde interface; isTrueHordeMember answers from the test */
class R25Horde : public TestBehavior, public HordeUpdateInterface {
public:
	R25Horde(Thing *t, const ModuleData *d) : TestBehavior(t, d), m_true(false) {}
	virtual HordeUpdateInterface *getHordeUpdateInterface() { return this; }
	virtual Bool isInHorde() const { return m_true; }
	virtual Bool hasFlag() const { return false; }
	virtual Bool isTrueHordeMember() const { return m_true; }
	virtual Bool isAllowedNationalism() const { return true; }
	bool m_true;
};

std::vector<unsigned char> g_r25Saved;
/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_r25Kinds[4] = { KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_PORTABLE_STRUCTURE, KINDOF_AIRCRAFT };
void r25SetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 4; ++i) if (bits >> i & 1) o->m_kindOf.set(g_r25Kinds[i]);
	o->m_tmpl.m_kindOf = o->m_kindOf;
}
Team g_r25Teams[4];
int g_r25Rel[4];   /* the relationship of the object's team (1) to team k + 1 (0 enemies, 1 neutral, 2 allies) */
Relationship r25TeamRel(const Team *a, const Team *b)
{
	if (a->m_dummy == 1 && b->m_dummy >= 1 && b->m_dummy <= 4) return (Relationship)g_r25Rel[b->m_dummy - 1];
	return NEUTRAL;
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a HordeUpdate and make the module on object 1 of four at frame cfg[0], with the logic random seed cfg[1].
** `cfg` (12 words): [0 the frame, 1 the seed, 2 object 1's flags (bits 0-3 kinds of g_r25Kinds, 4 an AI, 5 a drawable, 6 off the map, 7 effectively
** dead, 8 nationalism, 9 fanaticism), 3-5 objects 2 to 4's flags (bits 0-3 kinds, 4 a horde interface, 5 a true member, 6 off the map, 7 object 1's
** template, 8-9 their team's relationship), 6 object 1's major radius (float bits), 7 the draw-icon flag, 8 the scan (nibbles of object numbers, 0
** ends), 9-11 the distances squared from object 1 to objects 2 to 4 (float bits)]. The data dump goes first, then the constructor's wake calls. Then
** `n` ops of 6 words [op, a, b, c, d, e]: 0 update()  1 object a's flags = b  2 the scan = a  3 the frame = a  4 the distance to object a = b
** 5 onDrawableBoundToObject  6 the draw-icon flag = a  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op appends: the CATCH_ALL status,
** the result (update's sleep time), the effect log (count, then each), the wake calls (count, the last frame), the xfer status, bytes and CRC, the
** state (the refresh frame, the in-horde, true-member and flag bits) and the drawable's decal type. */
int ref_l9r25_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r25Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r25Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const HordeUpdateModuleData *hd = (const HordeUpdateModuleData *)d;
		putU32(hd->m_updateRate);
		for (int i = 0; i < KINDOF_COUNT; ++i) putU32(hd->m_kindof.test(i) ? 1 : 0);
		putI32(hd->m_minCount);
		putU32(realBits(hd->m_minDist));
		putU32(realBits(hd->m_rubOffRadius));
		putU32(hd->m_alliesOnly ? 1 : 0);
		putU32(hd->m_exactMatch ? 1 : 0);
		putI32((int)hd->m_action);
		putU32((unsigned)hd->m_flagSubObjNames.size());
		for (size_t j = 0; j < hd->m_flagSubObjNames.size(); ++j) putStr(hd->m_flagSubObjNames[j].str(), (int)hd->m_flagSubObjNames[j].getLength());
		putU32(hd->m_allowedNationalism ? 1 : 0);
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	gl.m_drawIconUI = cfg[7] != 0;
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[1]);
	g_teamRelHook = r25TeamRel;
	g_hdApplyFilters = true;
	Object *objs[4];
	Drawable draw;
	AIUpdateInterface ai;
	ModuleData plain;
	R25Horde hordes[4] = { R25Horde(0, &plain), R25Horde(0, &plain), R25Horde(0, &plain), R25Horde(0, &plain) };
	BehaviorModule *lists[4][3];
	for (int k = 0; k < 4; ++k) {
		objs[k] = newObject();
		objs[k]->m_oid = (ObjectID)(k + 1);
		g_r25Teams[k].m_dummy = k + 1;
		objs[k]->m_team = &g_r25Teams[k];
		lists[k][0] = lists[k][1] = lists[k][2] = 0;
		objs[k]->m_behaviors = lists[k];
	}
	Object *me = objs[0];
	draw.m_ownerObject = me;
	ai.m_owner = me;
	memcpy(&me->m_geometry.m_majorRadius, &cfg[6], 4);
	unsigned flags[4] = { cfg[2], cfg[3], cfg[4], cfg[5] };
	unsigned scan = cfg[8];
	for (int k = 0; k < 3; ++k) memcpy(&g_abDist[0][k + 1], &cfg[9 + k], 4);
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	/* the scene before the constructor: object 1's kinds and its own module first in its list */
	r25SetKinds(me, flags[0] & 15);
	HordeUpdate *m = new HordeUpdate(me, d);
	lists[0][0] = m;
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	/* GameLogic::friend_awakenUpdateModule for an object not yet in the list (GameLogic.cpp:3035-3046; a wake at the frame the module already
	** holds, 0, returns early, GameLogic.cpp:2995), then registerObject's "a frame of 0 is now, and now is at least 1" (GameLogic.cpp:3896-3923):
	** UpdateRate 0 draws GameLogicRandomValue(1, 0) = 0, a wake at frame 0 */
	{
		UnsignedInt when = gl.m_awakenCalls ? gl.m_awakenFrame : 0;
		if (when == 0) when = gl.m_frame ? gl.m_frame : 1;
		m->friend_setNextCallFrame(when);
	}
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 1) flags[(o[1] - 1) % 4] = o[2];
		if (o[0] == 2) scan = o[1];
		if (o[0] == 3) gl.m_frame = o[1];
		if (o[0] == 4) memcpy(&g_abDist[0][(o[1] - 1) % 4], &o[2], 4);
		if (o[0] == 6) gl.m_drawIconUI = o[1] != 0;
		/* the scene */
		r25SetKinds(me, flags[0] & 15);
		me->m_ai = (flags[0] >> 4 & 1) ? &ai : 0;
		me->m_drawable = (flags[0] >> 5 & 1) ? &draw : 0;
		me->m_offMap = (flags[0] >> 6 & 1) != 0;
		me->m_effDead = (flags[0] >> 7 & 1) != 0;
		me->m_weaponBonusCondition = ((flags[0] >> 8 & 1) << WEAPONBONUSCONDITION_NATIONALISM) | ((flags[0] >> 9 & 1) << WEAPONBONUSCONDITION_FANATICISM);
		for (int k = 1; k < 4; ++k) {
			unsigned f = flags[k];
			Object *ob = objs[k];
			r25SetKinds(ob, (f >> 7 & 1) ? (flags[0] & 15) : (f & 15));   /* (a shared template: the object's kinds) */
			ob->m_tmplShared = (f >> 7 & 1) ? me->getTemplate() : 0;
			hordes[k].m_true = (f >> 5 & 1) != 0;
			lists[k][0] = (f >> 4 & 1) ? &hordes[k] : 0;
			ob->m_offMap = (f >> 6 & 1) != 0;
			g_r25Rel[k] = (int)(f >> 8 & 3) % 3;
		}
		g_hdScan.clear();
		for (int k = 0; k < 4; ++k) { unsigned id = scan >> (4 * k) & 15; if (id == 0) break; if (id <= 4) g_hdScan.push_back(objs[id - 1]); }
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
			case 0: res = (unsigned)m->update(); break;
			case 5: m->onDrawableBoundToObject(); break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r25Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r25Saved;
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
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_lastHordeRefreshFrame);
		putU32(m->m_inHorde ? 1 : 0);
		putU32(m->m_trueHordeMember ? 1 : 0);
		putU32(m->m_hasFlag ? 1 : 0);
		putI32((int)draw.m_terrainDecalType);
	}
	*len = finish(buf, cap);
	g_hdApplyFilters = false;
	g_hdScan.clear();
	g_teamRelHook = 0;
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) { objs[k]->m_ai = 0; objs[k]->m_drawable = 0; objs[k]->m_behaviors = 0; objs[k]->m_team = 0; objs[k]->m_tmplShared = 0; delete objs[k]; }
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
