/* Differential-test driver for DockUpdate and RepairDockUpdate (p1w13-sonnet, batch R43; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/DockUpdate.cpp        (#included, edited by build.rs for MSVC 6's
**                                                                                                  for-loop scoping: mod_gen/DockUpdate_edit.cpp)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/RepairDockUpdate.cpp  (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the dock (object 1: maybe a drawable whose art has the test's
** bones, SUPPLY_SOURCE or IGNORE_DOCKING_BONES, a geometry, a position and an orientation; Object::convertBonePosToWorldPos, the contain
** driver's) and eleven dockers (objects 2-12: maybe a body, the module build's MBody; maybe a drawable, maybe with DOCKING_BEGINNING; DOZER and
** HARVESTER or not; an airborne locomotor or not; a position), findPositionAround (a line naming the options; the test's answer), the
** model condition changes (each a line naming every flag: g_mcHook), the REAL UpdateModule, INI and Xfer; the game logic (its frame, its wake
** calls applied as GameLogic::friend_awakenUpdateModule applies them). */
#include "DockUpdate_edit.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/RepairDockUpdate.cpp"

Bool (*g_findPosHook)(const Coord3D *, const FindPositionOptions *, Coord3D *) = 0;

namespace {

ModuleData *l9r43Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = RepairDockUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r43Saved;
UpdateModule *g_r43Updating = 0;
void r43Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r43Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *g_r43Objs[12];
Object *r43Find(ObjectID id) { return (id >= 1 && id <= 12) ? g_r43Objs[id - 1] : 0; }
Real r43F(unsigned bits) { Real f; memcpy(&f, &bits, 4); return f; }
const Real g_r43Radii[4] = { 5.0f, 20.0f, 1.0f, 0.0f };
/* the art's bones (the same in the Rust test): DockStart, DockAction and DockEnd if cfg[6] bits 0-2 say so (bit 3: DockStart at the origin),
** DockWaiting01 to DockWaitingNN for NN = cfg[6] bits 8-11 */
unsigned g_r43Bones = 0;
Coord3D g_r43Bone;
const Coord3D *r43Bone(const char *name)
{
	unsigned b = g_r43Bones;
	if (strcmp(name, "DockStart") == 0 && (b & 1)) { if (b & 8) g_r43Bone.zero(); else g_r43Bone.set(2.0f, 3.0f, 0.0f); return &g_r43Bone; }
	if (strcmp(name, "DockAction") == 0 && (b & 2)) { g_r43Bone.set(0.0f, 5.0f, 1.0f); return &g_r43Bone; }
	if (strcmp(name, "DockEnd") == 0 && (b & 4)) { g_r43Bone.set(-2.0f, 3.0f, 0.0f); return &g_r43Bone; }
	if (strncmp(name, "DockWaiting", 11) == 0 && strlen(name) == 13) {
		int k = atoi(name + 11);
		if (k >= 1 && k <= (int)(b >> 8 & 15)) { g_r43Bone.set(4.0f * k, -6.0f, 0.5f); return &g_r43Bone; }
	}
	return 0;
}
bool g_r43FindHas = false;
Coord3D g_r43Find;
Bool r43FindPos(const Coord3D *c, const FindPositionOptions *o, Coord3D *r)
{
	ulog("findpos %08x %08x %08x %08x %08x %d %d %d", realBits(c->x), realBits(c->y), realBits(c->z), realBits(o->minRadius), realBits(o->maxRadius), (int)o->flags,
		o->sourceToPathToDest ? (int)o->sourceToPathToDest->m_oid : 0, o->ignoreObject ? (int)o->ignoreObject->m_oid : 0);
	if (!g_r43FindHas) return FALSE;
	*r = g_r43Find;
	return TRUE;
}
std::string r43Bits(const ModelConditionFlags *f)
{
	std::string s = "[";
	bool first = true;
	for (int i = 0; i < MODELCONDITION_COUNT; ++i) if (f->test(i)) { char b[16]; sprintf(b, first ? "%d" : ", %d", i); s += b; first = false; }
	return s + "]";
}
void r43Mc(const Object *obj, const ModelConditionFlags *clr, const ModelConditionFlags *set)
{
	if (set) ulog("mc %d clear %s set %s", (int)obj->m_oid, r43Bits(clr).c_str(), r43Bits(set).c_str());
	else ulog("mc %d clear %s", (int)obj->m_oid, r43Bits(clr).c_str());
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a RepairDockUpdate and make it on object 1 of twelve at frame cfg[0]. `cfg` (48 words; 1-31
** also settable by op 2): 1 the dock: bit 0 a drawable, 1 SUPPLY_SOURCE, 2 IGNORE_DOCKING_BONES; 2 its major radius (of 5, 20, 1, 0); 3-5
** its x, y and orientation (float bits); 6 the art's bones (see r43Bone); 7 findPositionAround finds a position, at 8-9 (float bits; z 0);
** 10-20 dockers 2-12: bit 0 a body, 1 a drawable, 2 the drawable has DOCKING_BEGINNING, 3 DOZER, 4 HARVESTER, 5 an airborne locomotor;
** 21-31 their x (float bits; y 3, z 0); read once: 32-42 their health and maximum (bits 0-15 and 16-31, halves). Then `n` ops of 4 words
** [op, a, b, c] (a docker's body is cfg's first): 0 update()  1 the frame = a  2 cfg[a] = b  3 isClearToApproach(a)  4 reserveApproachPosition(a, b & 1 ? &pos : NULL, &c)
** 5 advanceApproachPosition(a, likewise)  6 isClearToEnter(a)  7 isClearToAdvance(a, c)  8-10 getEnterPosition, getDockPosition,
** getExitPosition(a, b & 1 ? &pos : NULL)  11-15 onApproachReached, onEnterReached, onDockReached, onExitReached, cancelDock(a)
** 16 setDockOpen(b & 1)  17 isDockOpen  18 setDockCrippled(b & 1)  19 isAllowPassthroughType  20 isRallyPointAfterDockType  21 action(a,
** c) (0 NULL)  22 xfer save  23 xfer load of the last save  24 xfer CRC  25 getDockUpdateInterface() == the module. `pos` starts at
** (77, 77, 77). Each op appends: the CATCH_ALL status, the result, pos and the index after the call, the effect log, the wake calls (count,
** last frame), the xfer status, bytes and CRC, the module's state (in the Rust module's state_words order) and every docker's health. */
int ref_l9r43_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r43Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r43Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[48];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const RepairDockUpdateModuleData *rd = (const RepairDockUpdateModuleData *)d;
		putI32(rd->m_numberApproachPositionsData);
		putU32(rd->m_isAllowPassthrough ? 1 : 0);
		putU32(realBits(rd->m_framesForFullHeal));
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	MBody bodies[12];
	Drawable draws[12];
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 12; ++k) {
		g_r43Objs[k] = newObject();
		g_r43Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r43Objs[k]->m_behaviors = none;
		bodies[k].m_owner = g_r43Objs[k];
		draws[k].m_ownerObject = g_r43Objs[k];
	}
	for (int k = 1; k < 12; ++k) {
		bodies[k].m_health = (Real)(cfg[31 + k] & 0xffff) * 0.5f;
		bodies[k].m_max = (Real)(cfg[31 + k] >> 16) * 0.5f;
	}
	Object *me = g_r43Objs[0];
	auto scene = [&]() {
		unsigned f = cfg[1];
		me->m_kindOf = KindOfMaskType();
		if (f & 2) me->m_kindOf.set(KINDOF_SUPPLY_SOURCE);
		if (f & 4) me->m_kindOf.set(KINDOF_IGNORE_DOCKING_BONES);
		me->m_tmpl.m_kindOf = me->m_kindOf;
		me->m_drawable = (f & 1) ? &draws[0] : 0;
		me->m_geometry = GeometryInfo(GEOMETRY_CYLINDER, false, 10.0f, g_r43Radii[cfg[2] & 3], g_r43Radii[cfg[2] & 3]);
		me->m_pos.x = r43F(cfg[3]);
		me->m_pos.y = r43F(cfg[4]);
		me->m_pos.z = 0.0f;
		me->m_orientation = r43F(cfg[5]);
		g_r43Bones = cfg[6];
		for (int k = 1; k < 12; ++k) {
			Object *o = g_r43Objs[k];
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
			o->m_pos.x = r43F(cfg[20 + k]);
			o->m_pos.y = 3.0f;
			o->m_pos.z = 0.0f;
		}
		g_r43FindHas = (cfg[7] & 1) != 0;
		g_r43Find.set(r43F(cfg[8]), r43F(cfg[9]), 0.0f);
	};
	scene();
	g_modFindHook = r43Find;
	g_r30BoneHook = r43Bone;
	g_findPosHook = r43FindPos;
	g_mcHook = r43Mc;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r43Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	RepairDockUpdate *m = new RepairDockUpdate(me, d);
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
		if (o[0] == 2 && o[1] >= 1 && o[1] <= 31) cfg[o[1]] = o[2];
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
		Object *docker = r43Find((ObjectID)o[1]);
		try {
			switch (o[0]) {
			case 0: g_r43Updating = m; res = (unsigned)m->update(); break;
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
			case 21: res = m->action(docker, r43Find((ObjectID)o[3])) ? 1 : 0; break;
			case 22: xst = runXfer(m, 0, xfer_path, bytes, 4096, &blen, &crc); g_r43Saved.assign(bytes, bytes + blen); break;
			case 23: {
				std::vector<unsigned char> b = g_r43Saved;
				if (b.size() > 4096) b.resize(4096);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 24: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 25: res = m->getDockUpdateInterface() == (DockUpdateInterface *)m ? 1 : 0; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r43Updating = 0;
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
		w.push_back((unsigned)m->m_lastRepair);
		w.push_back(realBits(m->m_healthToAddPerFrame));
		putU32((unsigned)w.size());
		for (size_t k = 0; k < w.size(); ++k) putU32(w[k]);
		for (int k = 1; k < 12; ++k) putU32(realBits(bodies[k].m_health));
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modFindHook = 0;
	g_r30BoneHook = 0;
	g_findPosHook = 0;
	g_mcHook = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 12; ++k) {
		Object *x = g_r43Objs[k];
		x->m_drawable = 0; x->m_body = 0; x->m_behaviors = 0;
		delete x;
		g_r43Objs[k] = 0;
	}
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
