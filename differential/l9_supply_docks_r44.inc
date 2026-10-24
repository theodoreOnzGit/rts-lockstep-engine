/* Differential-test driver for SupplyWarehouseDockUpdate and SupplyCenterDockUpdate (p1w13-sonnet, batch R44; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/SupplyWarehouseDockUpdate.cpp  (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/SupplyCenterDockUpdate.cpp     (#included, unchanged)
** on DockUpdate.cpp (batch R44's, already in the unit), in wrap_module.cpp's unit, at its end, on the module build's stand-ins and batch R44's
** scene (differential/l9_repair_dock_r44.inc), plus: the dock's controlling player (one of three, whose money, score and colour are the
** update batch L driver's), its STEALTHED and DETECTED bits, the local player; each docker's AI (maybe, a supply truck's or not: its boxes,
** whether it takes one, its boost; each call a line), stealth module (whether its grant is temporary) and CAN_STEALTH bit; the docker-to-dock
** distances (the ActiveBody driver's getDistanceSquared, through g_distHook), the ground's heights (the HeightDie driver's), TheGlobalData's
** box value, the REAL Object::kill, the REAL RandomValue; updateDrawableSupplyStatus a line. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/SupplyWarehouseDockUpdate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/SupplyCenterDockUpdate.cpp"

UnsignedInt Player::getSupplyBoxValue()
{
	/// @todo This would be the hookup for difficulty level modifiers and special economy buildings
	return TheGlobalData->m_baseValuePerSupplyBox;
}
void Drawable::updateDrawableSupplyStatus( Int maxSupply, Int currentSupply ) { ulog("supply %d %d %d", m_ownerObject ? (int)m_ownerObject->m_oid : 0, (int)maxSupply, (int)currentSupply); }

namespace {

ModuleData *l9r44Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = kind ? SupplyCenterDockUpdate::friend_newModuleData(&ini) : SupplyWarehouseDockUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r44Saved;
UpdateModule *g_r44Updating = 0;
void r44Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r44Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *g_r44Objs[12];
Object *r44Find(ObjectID id) { return (id >= 1 && id <= 12) ? g_r44Objs[id - 1] : 0; }
Real r44F(unsigned bits) { Real f; memcpy(&f, &bits, 4); return f; }
const Real g_r44Radii[4] = { 5.0f, 20.0f, 1.0f, 0.0f };
/* the art's bones (the same in the Rust test): DockStart, DockAction and DockEnd if cfg[6] bits 0-2 say so (bit 3: DockStart at the origin),
** DockWaiting01 to DockWaitingNN for NN = cfg[6] bits 8-11 */
unsigned g_r44Bones = 0;
Coord3D g_r44Bone;
const Coord3D *r44Bone(const char *name)
{
	unsigned b = g_r44Bones;
	if (strcmp(name, "DockStart") == 0 && (b & 1)) { if (b & 8) g_r44Bone.zero(); else g_r44Bone.set(2.0f, 3.0f, 0.0f); return &g_r44Bone; }
	if (strcmp(name, "DockAction") == 0 && (b & 2)) { g_r44Bone.set(0.0f, 5.0f, 1.0f); return &g_r44Bone; }
	if (strcmp(name, "DockEnd") == 0 && (b & 4)) { g_r44Bone.set(-2.0f, 3.0f, 0.0f); return &g_r44Bone; }
	if (strncmp(name, "DockWaiting", 11) == 0 && strlen(name) == 13) {
		int k = atoi(name + 11);
		if (k >= 1 && k <= (int)(b >> 8 & 15)) { g_r44Bone.set(4.0f * k, -6.0f, 0.5f); return &g_r44Bone; }
	}
	return 0;
}
bool g_r44FindHas = false;
/* the docker's supply truck AI: its boxes, whether it takes one, its boost; each call a line (the Rust side's probe with probe_effect_trace) */
class R44Truck : public SupplyTruckAIInterface
{
public:
	Object *m_owner = 0;
	Int m_boxes = 0, m_boost = 0;
	Bool m_gain = false;
	virtual Bool loseOneBox() { Bool lost = m_boxes > 0; if (lost) --m_boxes; ulog("losebox %d %d", (int)m_owner->m_oid, lost ? 1 : 0); return lost; }
	virtual Bool gainOneBox( Int remainingStock ) { if (m_gain) ++m_boxes; ulog("gainbox %d %d %d", (int)m_owner->m_oid, (int)remainingStock, m_gain ? 1 : 0); return m_gain; }
	virtual void setForceWantingState(Bool v) { ulog("forcewant %d %d", (int)m_owner->m_oid, v ? 1 : 0); }
	virtual Int getUpgradedSupplyBoost() const { return m_boost; }
};
Real g_r44Dist[12];
Real r44Dist(const Object *obj, const Object *other)
{
	ulog("distsq %d %d", (int)obj->m_oid, (int)other->m_oid);
	return g_r44Dist[(obj->m_oid - 1) % 12];
}
GlobalData g_r44Gd;
void r44Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
Coord3D g_r44Find;
Bool r44FindPos(const Coord3D *c, const FindPositionOptions *o, Coord3D *r)
{
	ulog("findpos %08x %08x %08x %08x %08x %d %d %d", realBits(c->x), realBits(c->y), realBits(c->z), realBits(o->minRadius), realBits(o->maxRadius), (int)o->flags,
		o->sourceToPathToDest ? (int)o->sourceToPathToDest->m_oid : 0, o->ignoreObject ? (int)o->ignoreObject->m_oid : 0);
	if (!g_r44FindHas) return FALSE;
	*r = g_r44Find;
	return TRUE;
}
std::string r44Bits(const ModelConditionFlags *f)
{
	std::string s = "[";
	bool first = true;
	for (int i = 0; i < MODELCONDITION_COUNT; ++i) if (f->test(i)) { char b[16]; sprintf(b, first ? "%d" : ", %d", i); s += b; first = false; }
	return s + "]";
}
void r44Mc(const Object *obj, const ModelConditionFlags *clr, const ModelConditionFlags *set)
{
	if (set) ulog("mc %d clear %s set %s", (int)obj->m_oid, r44Bits(clr).c_str(), r44Bits(set).c_str());
	else ulog("mc %d clear %s", (int)obj->m_oid, r44Bits(clr).c_str());
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a SupplyWarehouseDockUpdate (`kind` 0) or a SupplyCenterDockUpdate (1) and make it on object 1
** of twelve at frame cfg[0]. `cfg` (64 words; 1-31 and 43-56 also settable by op 2): 1-42 batch R43's (l9_repair_dock_r43.inc); 43 the
** dock: bits 0-1 its controlling player (0 none, else players 0-2 + 1), 2 STEALTHED, 3 DETECTED, 4-5 the local player likewise; 44 the box
** value; 45-55 dockers 2-12: bit 0 an AI, 1 a supply truck's, 2 it takes a box, 3 a stealth module, 4 its grant is temporary, 5 CAN_STEALTH,
** bits 8-15 its distance squared to the dock (as a quarter); 56 the eight cells' ground heights (nibbles, halves); read once: 57-67 the
** trucks' boxes (bits 0-7) and boosts (bits 8-15, signed). The ops are batch R43's (21 the action, 23 the load then loadPostProcess), plus
** 26 setCashValue(a, as Int)  27 getBoxesStored  28 onObjectCreated (26-28 kind 0). Each op appends batch R43's (with the state, then for
** kind 0 m_boxesStored), then every docker's position (x, y) and every truck's boxes. */
int ref_l9r44_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r44Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r44Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[72];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const DockUpdateModuleData *dd = (const DockUpdateModuleData *)d;
		putI32(dd->m_numberApproachPositionsData);
		putU32(dd->m_isAllowPassthrough ? 1 : 0);
		if (kind) putU32(((const SupplyCenterDockUpdateModuleData *)d)->m_grantTemporaryStealthFrames);
		else {
			const SupplyWarehouseDockUpdateModuleData *wd = (const SupplyWarehouseDockUpdateModuleData *)d;
			putI32(wd->m_startingBoxesData);
			putU32(wd->m_deleteWhenEmpty ? 1 : 0);
		}
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	MBody bodies[12];
	Drawable draws[12];
	AIUpdateInterface ais[12];
	R44Truck trucks[12];
	StealthUpdate stealths[12];
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_money.m_idx = i;
		players[i].m_score.m_idx = i;
		players[i].m_color = 0x00112233 * (i + 1);
		g_spPlayerPtrs[i] = &players[i];
	}
	g_spPlayerCount = 3;
	ThePlayerList = &g_spList;
	TheGameText = &g_ulText;
	TheInGameUI = &g_cfUi;
	TheGlobalData = &g_r44Gd;
	InitGameLogicRandom(cfg[0] * 7919u + 1u);
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 12; ++k) {
		g_r44Objs[k] = newObject();
		g_r44Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r44Objs[k]->m_behaviors = none;
		bodies[k].m_owner = g_r44Objs[k];
		draws[k].m_ownerObject = g_r44Objs[k];
		ais[k].m_owner = g_r44Objs[k];
		trucks[k].m_owner = g_r44Objs[k];
		stealths[k].m_owner = g_r44Objs[k];
		g_r44Objs[k]->m_geometry = GeometryInfo(GEOMETRY_SPHERE, false, 3.0f, 3.0f, 3.0f);
	}
	for (int k = 1; k < 12; ++k) {
		trucks[k].m_boxes = (Int)(cfg[56 + k] & 0xff);
		trucks[k].m_boost = (Int)(signed char)(cfg[56 + k] >> 8 & 0xff);
	}
	for (int k = 1; k < 12; ++k) {
		bodies[k].m_health = (Real)(cfg[31 + k] & 0xffff) * 0.5f;
		bodies[k].m_max = (Real)(cfg[31 + k] >> 16) * 0.5f;
	}
	Object *me = g_r44Objs[0];
	auto scene = [&]() {
		unsigned f = cfg[1];
		me->m_kindOf = KindOfMaskType();
		if (f & 2) me->m_kindOf.set(KINDOF_SUPPLY_SOURCE);
		if (f & 4) me->m_kindOf.set(KINDOF_IGNORE_DOCKING_BONES);
		me->m_tmpl.m_kindOf = me->m_kindOf;
		me->m_drawable = (f & 1) ? &draws[0] : 0;
		me->m_geometry = GeometryInfo(GEOMETRY_CYLINDER, false, 10.0f, g_r44Radii[cfg[2] & 3], g_r44Radii[cfg[2] & 3]);
		me->m_pos.x = r44F(cfg[3]);
		me->m_pos.y = r44F(cfg[4]);
		me->m_pos.z = 0.0f;
		me->m_orientation = r44F(cfg[5]);
		g_r44Bones = cfg[6];
		for (int k = 1; k < 12; ++k) {
			Object *o = g_r44Objs[k];
			unsigned g = cfg[9 + k];
			o->m_body = (cfg_in[9 + k] & 1) ? &bodies[k] : 0;   /* the body, once (the Rust side's is a module) */
			o->m_drawable = (g & 2) ? &draws[k] : 0;
			draws[k].m_condFlags = ModelConditionFlags();
			if (g & 4) draws[k].m_condFlags.set(MODELCONDITION_DOCKING_BEGINNING);
			o->m_kindOf = KindOfMaskType();
			if (g & 8) o->m_kindOf.set(KINDOF_DOZER);
			if (g & 16) o->m_kindOf.set(KINDOF_HARVESTER);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_airborneLoco = (g & 32) != 0;
			o->m_pos.x = r44F(cfg[20 + k]);
			o->m_pos.y = 3.0f;
			o->m_pos.z = 0.0f;
		}
		unsigned dk = cfg[43];
		me->m_player = (dk & 3) ? &players[(dk & 3) - 1] : 0;
		me->m_status.clear();
		if (dk & 4) me->m_status.set(OBJECT_STATUS_STEALTHED);
		if (dk & 8) me->m_status.set(OBJECT_STATUS_DETECTED);
		g_spLocal = (dk >> 4 & 3) ? &players[(dk >> 4 & 3) - 1] : 0;
		g_r44Gd.m_baseValuePerSupplyBox = (Int)cfg[44];
		for (int k = 1; k < 12; ++k) {
			Object *o = g_r44Objs[k];
			unsigned a = cfg[44 + k];
			o->m_ai = (a & 1) ? &ais[k] : 0;
			o->m_airborneLoco = o->m_airborneLoco && o->m_ai;   /* isUsingAirborneLocomotor asks the AI's locomotor: none without an AI */
			ais[k].m_supplyTruck = (a & 2) ? &trucks[k] : 0;
			trucks[k].m_gain = (a & 4) != 0;
			o->m_stealth = (a & 8) ? &stealths[k] : 0;
			stealths[k].m_temporaryGrant = (a & 16) != 0;
			o->m_status.clear();
			if (a & 32) o->m_status.set(OBJECT_STATUS_CAN_STEALTH);
			g_r44Dist[k] = (Real)(a >> 8 & 0xff) * 0.25f;
		}
		for (int c = 0; c < 8; ++c) g_hdGround[c] = (Real)(cfg[56] >> (4 * c) & 15) * 0.5f;
		g_r44FindHas = (cfg[7] & 1) != 0;
		g_r44Find.set(r44F(cfg[8]), r44F(cfg[9]), 0.0f);
	};
	scene();
	g_modFindHook = r44Find;
	g_r30BoneHook = r44Bone;
	g_findPosHook = r44FindPos;
	g_mcHook = r44Mc;
	g_distHook = r44Dist;
	g_modDestroyHook = r44Destroy;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r44Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	DockUpdate *m = kind ? (DockUpdate *)new SupplyCenterDockUpdate(me, d) : (DockUpdate *)new SupplyWarehouseDockUpdate(me, d);
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	putLog();
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 2 && ((o[1] >= 1 && o[1] <= 31) || (o[1] >= 43 && o[1] <= 56))) cfg[o[1]] = o[2];
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
		static unsigned char bytes[4096];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		Coord3D pos;
		pos.set(77.0f, 77.0f, 77.0f);
		Int index = (Int)o[3];
		Coord3D *pp = (o[2] & 1) ? &pos : NULL;
		Object *docker = r44Find((ObjectID)o[1]);
		try {
			switch (o[0]) {
			case 0: g_r44Updating = m; res = (unsigned)m->update(); break;
			case 3: res = m->isClearToApproach(docker) ? 1 : 0; break;
			case 4: res = m->reserveApproachPosition(docker, pp, &index) ? 1 : 0; break;
			case 5: res = m->advanceApproachPosition(docker, pp, &index) ? 1 : 0; break;
			case 6: res = m->isClearToEnter(docker) ? 1 : 0; break;
			case 7: res = m->isClearToAdvance(docker, (Int)o[3]) ? 1 : 0; break;
			case 8: m->getEnterPosition(docker, pp); break;
			case 9: m->getDockPosition(docker, pp); break;
			case 10: m->getExitPosition(docker, pp); break;
			case 11: m->onApproachReached(docker); break;
			case 12: m->onEnterReached(docker); break;
			case 13: m->onDockReached(docker); break;
			case 14: m->onExitReached(docker); break;
			case 15: m->cancelDock(docker); break;
			case 16: m->setDockOpen((o[2] & 1) != 0); break;
			case 17: res = m->isDockOpen() ? 1 : 0; break;
			case 18: m->setDockCrippled((o[2] & 1) != 0); break;
			case 19: res = m->isAllowPassthroughType() ? 1 : 0; break;
			case 20: res = m->isRallyPointAfterDockType() ? 1 : 0; break;
			case 21: res = m->action(docker, r44Find((ObjectID)o[3])) ? 1 : 0; break;
			case 22: xst = runXfer(m, 0, xfer_path, bytes, 4096, &blen, &crc); g_r44Saved.assign(bytes, bytes + blen); break;
			case 23: {
				std::vector<unsigned char> b = g_r44Saved;
				if (b.size() > 4096) b.resize(4096);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 24: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 25: res = m->getDockUpdateInterface() == (DockUpdateInterface *)m ? 1 : 0; break;
			case 26: if (!kind) ((SupplyWarehouseDockUpdate *)m)->setCashValue((Int)o[1]); break;
			case 27: if (!kind) res = (unsigned)((SupplyWarehouseDockUpdate *)m)->getBoxesStored(); break;
			case 28: if (!kind) m->onObjectCreated(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r44Updating = 0;
		putI32(st);
		putU32(res);
		putU32(realBits(pos.x)); putU32(realBits(pos.y)); putU32(realBits(pos.z));
		putI32(index);
		putLog();
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		/* the state, in the Rust module's state_words order */
		std::vector<unsigned> w;
		const Coord3D *cs[3] = { &m->m_enterPosition, &m->m_dockPosition, &m->m_exitPosition };
		for (int k = 0; k < 3; ++k) { w.push_back(realBits(cs[k]->x)); w.push_back(realBits(cs[k]->y)); w.push_back(realBits(cs[k]->z)); }
		w.push_back((unsigned)m->m_numberApproachPositions);
		w.push_back((unsigned)m->m_numberApproachPositionBones);
		w.push_back(m->m_positionsLoaded ? 1 : 0);
		w.push_back((unsigned)m->m_activeDocker);
		w.push_back(m->m_dockerInside ? 1 : 0);
		w.push_back(m->m_dockCrippled ? 1 : 0);
		w.push_back(m->m_dockOpen ? 1 : 0);
		w.push_back((unsigned)m->m_approachPositions.size());
		for (size_t k = 0; k < m->m_approachPositions.size(); ++k) {
			const Coord3D &p = m->m_approachPositions[k];
			w.push_back(realBits(p.x)); w.push_back(realBits(p.y)); w.push_back(realBits(p.z));
			w.push_back(k < m->m_approachPositionOwners.size() ? (unsigned)m->m_approachPositionOwners[k] : 0xffffffffu);
			w.push_back(k < m->m_approachPositionReached.size() ? (m->m_approachPositionReached[k] ? 1u : 0u) : 0xffffffffu);
		}
		if (!kind) w.push_back((unsigned)((SupplyWarehouseDockUpdate *)m)->m_boxesStored);
		putU32((unsigned)w.size());
		for (size_t k = 0; k < w.size(); ++k) putU32(w[k]);
		for (int k = 1; k < 12; ++k) putU32(realBits(bodies[k].m_health));
		for (int k = 1; k < 12; ++k) { putU32(realBits(g_r44Objs[k]->m_pos.x)); putU32(realBits(g_r44Objs[k]->m_pos.y)); }
		for (int k = 1; k < 12; ++k) putI32(trucks[k].m_boxes);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modFindHook = 0;
	g_r30BoneHook = 0;
	g_findPosHook = 0;
	g_mcHook = 0;
	g_distHook = 0;
	g_modDestroyHook = 0;
	ThePlayerList = 0;
	TheGameText = 0;
	TheInGameUI = 0;
	TheGlobalData = 0;
	g_spLocal = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 12; ++k) {
		Object *x = g_r44Objs[k];
		x->m_drawable = 0; x->m_body = 0; x->m_behaviors = 0; x->m_ai = 0; x->m_stealth = 0; x->m_player = 0;
		delete x;
		g_r44Objs[k] = 0;
	}
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
