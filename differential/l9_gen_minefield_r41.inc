/* Differential-test driver for GenerateMinefieldBehavior (p1w13-sonnet, batch R41; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/GenerateMinefieldBehavior.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the generator (object 1: maybe a player of three, whose default
** team is the player's, a geometry, a position and an orientation (the transform posed as Thing's), its completed object upgrades) and
** objects 2-6 (the kinds STRUCTURE or not), the thing factory (MINE_A and MINE_B, whose geometry the test gives; newObject makes an object
** with one behaviour, a LandMineInterface whose setScootParms is a line, and logs; setOrientation a line), the terrain's eight cells (ground,
** layer, water, cliff), destroyObject (a line), the potential collisions (the objects the test lists; the call a line), the FX lists (FX_A, FX_B), the REAL
** UpgradeCenter (Upgrade_A and Upgrade_ChinaEMPMines, object upgrades, loaded by the test), the REAL UpgradeMux, GeometryInfo, RandomValue,
** UpdateModule, INI and Xfer; the game logic (its frame, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them). */
#define min(x,y) (((x)<(y)) ? (x) : (y))   /* Lib/BaseType.h, verbatim (the module build leaves the macro out) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/GenerateMinefieldBehavior.cpp"
#undef min

namespace {

ModuleData *l9r41Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = GenerateMinefieldBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r41Saved;
UpdateModule *g_r41Updating = 0;
void r41Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r41Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *g_r41Objs[6];
std::vector<Object *> g_r41Mines;
Object *r41Find(ObjectID id)
{
	int i = (int)id;
	if (i >= 1 && i <= 6) return g_r41Objs[i - 1];
	if (i >= 7 && i < 7 + (int)g_r41Mines.size()) return g_r41Mines[i - 7];
	return 0;
}
Real r41F(unsigned bits) { Real f; memcpy(&f, &bits, 4); return f; }
ThingTemplate g_r41Tmpl[2];
const ThingTemplate *r41FindTemplate(const AsciiString &name)
{
	if (strcmp(name.str(), "MINE_A") == 0) return &g_r41Tmpl[0];
	if (strcmp(name.str(), "MINE_B") == 0) return &g_r41Tmpl[1];
	return 0;
}
Team g_r41Teams[3];
Player g_r41Players[3];
bool g_r41Cliff[8];
/* the mine's one behaviour: a LandMineInterface whose setScootParms is a line (the Rust side's probe with trace_land_mine) */
class R41Mine : public BehaviorModule, public LandMineInterface
{
public:
	R41Mine(Thing *t, const ModuleData *d) : BehaviorModule(t, d) {}
	virtual NameKeyType getModuleNameKey() const { return NAMEKEY_INVALID; }
	virtual LandMineInterface *getLandMineInterface() { return this; }
	virtual void setScootParms(const Coord3D &s, const Coord3D &e)
	{
		ulog("scoot %d %08x %08x %08x %08x %08x %08x", (int)getObject()->m_oid, realBits(s.x), realBits(s.y), realBits(s.z), realBits(e.x), realBits(e.y), realBits(e.z));
	}
	virtual void disarm() {}
};
ModuleData *g_r41MineData = 0;
std::vector<R41Mine *> g_r41MineModules;
std::vector<BehaviorModule **> g_r41MineLists;
Object *r41NewObject(const ThingTemplate *tmplate, Team *team)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)(7 + g_r41Mines.size());
	o->m_team = team;
	o->m_tmplShared = tmplate;
	ulog("newobject %s %d", tmplate == &g_r41Tmpl[0] ? "MINE_A" : "MINE_B", team ? team->m_dummy : 0);
	R41Mine *b = new R41Mine(o, g_r41MineData);
	BehaviorModule **list = new BehaviorModule *[2];
	list[0] = b;
	list[1] = 0;
	o->m_behaviors = list;
	g_r41MineModules.push_back(b);
	g_r41MineLists.push_back(list);
	g_r41Mines.push_back(o);
	return o;
}
void r41Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
UpgradeMaskType r41Mask(unsigned bits)
{
	UpgradeMaskType m;
	const char *names[2] = { "Upgrade_A", "Upgrade_ChinaEMPMines" };
	for (int k = 0; k < 2; ++k) if (bits >> k & 1) {
		const UpgradeTemplate *u = TheUpgradeCenter->findUpgrade(AsciiString(names[k]));
		if (u) m.set(u->getUpgradeMask());
	}
	return m;
}

}  // namespace

Bool TerrainLogic::isCliffCell( Real x, Real y ) const
{
	ulog("cliff %08x %08x", realBits(x), realBits(y));
	return g_r41Cliff[hdCell(x, y)];
}
SimpleObjectIterator *PartitionManager::iteratePotentialCollisions( const Coord3D *pos, const GeometryInfo &geom, Real angle, Bool use2D )
{
	ulog("potcol %08x %08x %08x %d %08x %08x %08x", realBits(pos->x), realBits(pos->y), realBits(pos->z), (int)geom.getGeomType(), realBits(geom.getMajorRadius()),
		realBits(geom.getMinorRadius()), realBits(angle));
	SimpleObjectIterator *it = new SimpleObjectIterator;
	it->m_list = g_ahRange;
	return it;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a GenerateMinefieldBehavior and make it on object 1 of six at frame cfg[0]. `cfg` (24 words;
** 1-8 also settable by op 2): 1 the generator's player + 1 (0 none), 2-6 objects 2-6: bit 0 STRUCTURE, 7 the potential collisions (nibbles
** of object numbers 1-6, 0 ends), 8 the generator's completed object upgrades (bit 0 Upgrade_A, 1 Upgrade_ChinaEMPMines); read once: 9 the
** logic random seed, 10 the generator's geometry (bits 0-1 the type: sphere, cylinder, box; 2-3 the major radius of 5, 20, 1, 0; 4-5 the minor likewise),
** 11 the mines' geometry (likewise), 12-13 the generator's x and y (float bits), 14 its orientation (float bits), 15-18 the eight cells'
** layers (nibbles: 1 ground, 2-3 bridges), ground heights (nibbles, as halves), water (bits 0-7) and cliffs (bits 8-15). Then `n` ops of 4
** words [op, a, b, c]: 0 update()  1 the frame = a  2 cfg[a] = b  3 onDie  4 attemptUpgrade(the mask of a's bits)  5 setMinefieldTarget(a ?
** (b, c, 0) as float bits : NULL)  6 xfer save  7 xfer load of the last save  8 xfer CRC. Each op appends: the CATCH_ALL status, the result,
** the effect log, the wake calls (count, last frame), the xfer status, bytes and CRC, the module's state, and the mines made so far (their
** count, then each one's id, position, producer and team). */
int ref_l9r41_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r41Saved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r41Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[24];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const GenerateMinefieldBehaviorModuleData *gd = (const GenerateMinefieldBehaviorModuleData *)d;
		putStr(gd->m_mineName.str(), (int)gd->m_mineName.getLength());
		putStr(gd->m_mineNameUpgraded.str(), (int)gd->m_mineNameUpgraded.getLength());
		putStr(gd->m_mineUpgradeTrigger.str(), (int)gd->m_mineUpgradeTrigger.getLength());
		putI32(fxId(gd->m_genFX));
		putU32(realBits(gd->m_distanceAroundObject));
		putU32(realBits(gd->m_minesPerSquareFoot));
		putU32(realBits(gd->m_randomJitter));
		putU32(realBits(gd->m_skipIfThisMuchUnderStructure));
		putU32((gd->m_onDeath ? 1 : 0) | (gd->m_borderOnly ? 2 : 0) | (gd->m_alwaysCircular ? 4 : 0) | (gd->m_upgradable ? 8 : 0) | (gd->m_smartBorder ? 16 : 0) |
			(gd->m_smartBorderSkipInterior ? 32 : 0));
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[9]);
	for (int t = 0; t < 3; ++t) {
		g_r41Teams[t].m_dummy = t + 1;
		g_r41Teams[t].m_controller = &g_r41Players[t];
		g_r41Players[t].m_idx = t;
		g_r41Players[t].m_defaultTeam = &g_r41Teams[t];
	}
	const Real radii[4] = { 5.0f, 20.0f, 1.0f, 0.0f };
	auto geomOf = [&](unsigned g) {
		unsigned gt = g & 3;
		GeometryType type = gt == 0 ? GEOMETRY_SPHERE : (gt == 1 ? GEOMETRY_CYLINDER : GEOMETRY_BOX);
		return GeometryInfo(type, false, 5.0f, radii[g >> 2 & 3], radii[g >> 4 & 3]);
	};
	g_r29Geom = geomOf(cfg[11]);
	for (int k = 0; k < 6; ++k) {
		g_r41Objs[k] = newObject();
		g_r41Objs[k]->m_oid = (ObjectID)(k + 1);
	}
	for (int c = 0; c < 8; ++c) {
		g_hdLayer[c] = (int)(cfg[15] >> (4 * c) & 15);
		g_hdGround[c] = (Real)(cfg[16] >> (4 * c) & 15) * 0.5f;
		g_flrHasWater[c] = (cfg[17] >> c & 1) != 0;
		g_r41Cliff[c] = (cfg[17] >> (8 + c) & 1) != 0;
	}
	Object *me = g_r41Objs[0];
	me->m_geometry = geomOf(cfg[10]);
	me->m_pos.x = r41F(cfg[12]);
	me->m_pos.y = r41F(cfg[13]);
	me->m_pos.z = 0.0f;
	g_objXformFromPose = true;
	g_r28ThingPose = true;
	me->m_orientation = r41F(cfg[14]);
	me->poseMatrix();
	auto scene = [&]() {
		unsigned tp = cfg[1];
		me->m_team = (tp >= 1 && tp <= 3) ? &g_r41Teams[tp - 1] : 0;
		me->m_player = (tp >= 1 && tp <= 3) ? &g_r41Players[tp - 1] : 0;
		for (int k = 1; k < 6; ++k) {
			Object *o = g_r41Objs[k];
			o->m_kindOf = KindOfMaskType();
			if (cfg[1 + k] & 1) o->m_kindOf.set(KINDOF_STRUCTURE);
			o->m_tmpl.m_kindOf = o->m_kindOf;
		}
		me->m_objectUpgradesCompleted = r41Mask(cfg[8] & 3);
		g_ahRange.clear();
		for (int k = 0; k < 6; ++k) {
			unsigned id = cfg[7] >> (4 * k) & 15;
			if (id == 0) break;
			if (id <= 6) g_ahRange.push_back(g_r41Objs[id - 1]);
		}
	};
	scene();
	g_r41MineData = new BehaviorModuleData;
	g_modFindHook = r41Find;
	g_findTemplateHook = r41FindTemplate;
	g_newObjectHook = r41NewObject;
	g_modDestroyHook = r41Destroy;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r41Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1 && e.p) { char b[32]; sprintf(b, "fx %d", fxId((const FXList *)e.p)); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	GenerateMinefieldBehavior *m = new GenerateMinefieldBehavior(me, d);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	putLog();
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 2 && o[1] >= 1 && o[1] <= 8) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		gl.m_awakenCalls = 0;
		gl.m_awakenFrame = 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		static unsigned char bytes[65536];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: g_r41Updating = m; res = (unsigned)m->update(); break;
			case 3: { DamageInfo di; m->onDie(&di); break; }
			case 4: res = m->attemptUpgrade(r41Mask(o[1] & 3)) ? 1 : 0; break;
			case 5: {
				Coord3D p; p.x = r41F(o[2]); p.y = r41F(o[3]); p.z = 0.0f;
				m->setMinefieldTarget(o[1] ? &p : NULL);
				break;
			}
			case 6: xst = runXfer(m, 0, xfer_path, bytes, 65536, &blen, &crc); g_r41Saved.assign(bytes, bytes + blen); break;
			case 7: {
				std::vector<unsigned char> b = g_r41Saved;
				if (b.size() > 65536) b.resize(65536);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 8: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r41Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		/* the state, in the Rust module's state_words order */
		std::vector<unsigned> w;
		w.push_back(m->isAlreadyUpgraded() ? 1 : 0);
		w.push_back(m->m_generated ? 1 : 0);
		w.push_back(m->m_hasTarget ? 1 : 0);
		w.push_back(m->m_upgraded ? 1 : 0);
		w.push_back(realBits(m->m_target.x)); w.push_back(realBits(m->m_target.y)); w.push_back(realBits(m->m_target.z));
		w.push_back((unsigned)m->m_mineList.size());
		for (std::list<ObjectID>::iterator it = m->m_mineList.begin(); it != m->m_mineList.end(); ++it) w.push_back((unsigned)*it);
		putU32((unsigned)w.size());
		for (size_t k = 0; k < w.size(); ++k) putU32(w[k]);
		putU32((unsigned)g_r41Mines.size());
		for (size_t k = 0; k < g_r41Mines.size(); ++k) {
			Object *mo = g_r41Mines[k];
			putU32((unsigned)mo->m_oid);
			putU32(realBits(mo->m_pos.x)); putU32(realBits(mo->m_pos.y)); putU32(realBits(mo->m_pos.z));
			putU32((unsigned)mo->m_producerID);
			putU32(mo->m_team ? (unsigned)mo->m_team->m_dummy : 0);
		}
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modFindHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_modDestroyHook = 0;
	g_objXformFromPose = false;
	g_r28ThingPose = false;
	g_ahRange.clear();
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (size_t k = 0; k < g_r41Mines.size(); ++k) {
		Object *x = g_r41Mines[k];
		x->m_behaviors = 0; x->m_team = 0; x->m_tmplShared = 0;
		delete x;
		delete g_r41MineModules[k];
		delete[] g_r41MineLists[k];
	}
	g_r41Mines.clear();
	g_r41MineModules.clear();
	g_r41MineLists.clear();
	delete g_r41MineData;
	g_r41MineData = 0;
	for (int k = 0; k < 6; ++k) {
		Object *x = g_r41Objs[k];
		x->m_team = 0; x->m_player = 0;
		delete x;
		g_r41Objs[k] = 0;
	}
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
