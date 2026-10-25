/* Differential-test driver for RailedTransportDockUpdate (p1w13-sonnet, batch R61; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/RailedTransportDockUpdate.cpp   (#included, unchanged; its base
**                                                                                                        DockUpdate.cpp is batch R43's)
** in wrap_module.cpp's unit, at its end, on batch R43's stand-ins (the dock, object 1, and eleven dockers, objects 2-12; findPositionAround;
** the model condition changes) and: the dock's contain module (maybe: an OpenContain whose isValidContainerFor, addToContain,
** removeFromContain and iterateContained are the test's, over its list of the dockers the test puts inside; the first three lines), the
** dockers' AIs (aiIdle and aiMoveToPosition lines), deselectObject, setStatus, setDisabled, clearDisabled and setOrientation (lines), the
** single-flag model condition changes of Object (batch N's and the upgrades' lines), the terrain's ground height (batch O's line and
** table), the partition manager's distances (batch R57's real procedures), the art's bones (batch R43's, looked up case-blind as the art's
** are). */
#include "RailedTransportDockUpdate_r61.h"   /* the header but its interface class, which the contain driver copied (build.rs) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/RailedTransportDockUpdate.cpp"

namespace {

ModuleData *l9r61Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = RailedTransportDockUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r61Saved;
UpdateModule *g_r61Updating = 0;
void r61Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r61Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *g_r61Objs[12];
Object *r61Find(ObjectID id) { return (id >= 1 && id <= 12) ? g_r61Objs[id - 1] : 0; }
Real r61F(unsigned bits) { Real f; memcpy(&f, &bits, 4); return f; }
const Real g_r61Radii[4] = { 5.0f, 20.0f, 1.0f, 0.0f };
/* the art's bones (the same in the Rust test): DockStart, DockAction and DockEnd if cfg[6] bits 0-2 say so (bit 3: DockStart at the origin),
** DockWaiting01 to DockWaitingNN for NN = cfg[6] bits 8-11 */
unsigned g_r61Bones = 0;
Coord3D g_r61Bone;
const Coord3D *r61Bone(const char *name)
{
	unsigned b = g_r61Bones;
	if (strcasecmp(name, "DockStart") == 0 && (b & 1)) { if (b & 8) g_r61Bone.zero(); else g_r61Bone.set(2.0f, 3.0f, 0.0f); return &g_r61Bone; }
	if (strcasecmp(name, "DockAction") == 0 && (b & 2)) { g_r61Bone.set(0.0f, 5.0f, 1.0f); return &g_r61Bone; }
	if (strcasecmp(name, "DockEnd") == 0 && (b & 4)) { g_r61Bone.set(-2.0f, 3.0f, 0.0f); return &g_r61Bone; }
	if (strncasecmp(name, "DockWaiting", 11) == 0 && strlen(name) == 13) {
		int k = atoi(name + 11);
		if (k >= 1 && k <= (int)(b >> 8 & 15)) { g_r61Bone.set(4.0f * k, -6.0f, 0.5f); return &g_r61Bone; }
	}
	return 0;
}
bool g_r61FindHas = false;
Coord3D g_r61Find;
Bool r61FindPos(const Coord3D *c, const FindPositionOptions *o, Coord3D *r)
{
	ulog("findpos %08x %08x %08x %08x %08x %d %d %d", realBits(c->x), realBits(c->y), realBits(c->z), realBits(o->minRadius), realBits(o->maxRadius), (int)o->flags,
		o->sourceToPathToDest ? (int)o->sourceToPathToDest->m_oid : 0, o->ignoreObject ? (int)o->ignoreObject->m_oid : 0);
	if (!g_r61FindHas) return FALSE;
	*r = g_r61Find;
	return TRUE;
}
std::string r61Bits(const ModelConditionFlags *f)
{
	std::string s = "[";
	bool first = true;
	for (int i = 0; i < MODELCONDITION_COUNT; ++i) if (f->test(i)) { char b[16]; sprintf(b, first ? "%d" : ", %d", i); s += b; first = false; }
	return s + "]";
}
void r61Mc(const Object *obj, const ModelConditionFlags *clr, const ModelConditionFlags *set)
{
	if (set) ulog("mc %d clear %s set %s", (int)obj->m_oid, r61Bits(clr).c_str(), r61Bits(set).c_str());
	else ulog("mc %d clear %s", (int)obj->m_oid, r61Bits(clr).c_str());
}

/* the dock's contain module: an OpenContain (upstream casts the interface to one) over the test's list */
class R61Contain : public OpenContain {
public:
	R61Contain(Thing *t, const ModuleData *d) : OpenContain(t, d) {}
	virtual ~R61Contain() {}
	virtual Bool isValidContainerFor(const Object *obj, Bool checkCapacity) const
	{
		ulog("validfor %d %d %d", (int)getObject()->m_oid, obj ? (int)obj->m_oid : 0, (int)checkCapacity);
		return m_valid;
	}
	virtual void addToContain(Object *obj) { ulog("addtocontain %d %d", (int)getObject()->m_oid, (int)obj->m_oid); }
	virtual void removeFromContain(Object *obj, Bool)
	{
		ulog("removefromcontain %d %d", (int)getObject()->m_oid, (int)obj->m_oid);
		for (size_t i = 0; i < m_list.size(); ++i) if (m_list[i] == obj) { m_list.erase(m_list.begin() + i); break; }
	}
	virtual void iterateContained(ContainIterateFunc func, void *userData, Bool reverse)
	{
		std::vector<Object *> l = m_list;
		if (reverse) std::reverse(l.begin(), l.end());
		for (size_t i = 0; i < l.size(); ++i) func(l[i], userData);
	}
	Bool m_valid = true;
	std::vector<Object *> m_list;
};
AIUpdateInterface g_r61Ais[12];

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a RailedTransportDockUpdate and make it on object 1 of twelve at frame cfg[0]. `cfg` (48 words;
** 1-31 also settable by op 2): batch R43's, but: 1 bit 3 the dock has a contain module (read once), bit 4 it takes a docker; 10-20 bit 6
** the docker has an AI; read once: 43 the dockers inside the contain module (bit k: object k + 2), 44 the terrain's ground heights (cell k:
** bits 4k-4k+3, halves). Then `n` ops of 4 words [op, a, b, c]: batch R43's 0-25 (21 action(a, c)), and 26 isLoadingOrUnloading()
** 27 unloadAll()  28 unloadSingleObject(a). Each op appends: the CATCH_ALL status, the result, pos and the index after the call, the effect
** log, the wake calls (count, last frame), the xfer status, bytes and CRC, the module's state (in the Rust module's state_words order), the
** dockers' positions and the contain module's list. */
int ref_l9r61_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r61Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r61Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[48];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const RailedTransportDockUpdateModuleData *rd = (const RailedTransportDockUpdateModuleData *)d;
		putI32(rd->m_numberApproachPositionsData);
		putU32(rd->m_isAllowPassthrough ? 1 : 0);
		putU32(rd->m_pullInsideDurationInFrames);
		putU32(rd->m_pushOutsideDurationInFrames);
		putU32(realBits(rd->m_toleranceDistance));
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	TheTerrainLogic = &g_ocTerrain;
	for (int k = 0; k < 8; ++k) g_hdGround[k] = (Real)(cfg_in[44] >> (4 * k) & 15) * 0.5f;
	MBody bodies[12];
	Drawable draws[12];
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 12; ++k) {
		g_r61Objs[k] = newObject();
		g_r61Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r61Objs[k]->m_behaviors = none;
		bodies[k].m_owner = g_r61Objs[k];
		draws[k].m_ownerObject = g_r61Objs[k];
		g_r61Ais[k].m_owner = g_r61Objs[k];
	}
	for (int k = 1; k < 12; ++k) {
		bodies[k].m_health = (Real)(cfg[31 + k] & 0xffff) * 0.5f;
		bodies[k].m_max = (Real)(cfg[31 + k] >> 16) * 0.5f;
	}
	Object *me = g_r61Objs[0];
	OpenContainModuleData *cd = 0;
	R61Contain *contain = 0;
	if (cfg_in[1] & 8) {
		cd = new OpenContainModuleData;
		contain = new R61Contain(me, cd);
		for (int k = 0; k < 11; ++k) if (cfg_in[43] >> k & 1) contain->m_list.push_back(g_r61Objs[k + 1]);
	}
	me->m_contain = contain;
	auto scene = [&]() {
		unsigned f = cfg[1];
		me->m_kindOf = KindOfMaskType();
		if (f & 2) me->m_kindOf.set(KINDOF_SUPPLY_SOURCE);
		if (f & 4) me->m_kindOf.set(KINDOF_IGNORE_DOCKING_BONES);
		me->m_tmpl.m_kindOf = me->m_kindOf;
		me->m_drawable = (f & 1) ? &draws[0] : 0;
		me->m_geometry = GeometryInfo(GEOMETRY_CYLINDER, false, 10.0f, g_r61Radii[cfg[2] & 3], g_r61Radii[cfg[2] & 3]);
		me->m_pos.x = r61F(cfg[3]);
		me->m_pos.y = r61F(cfg[4]);
		me->m_pos.z = 0.0f;
		me->m_orientation = r61F(cfg[5]);
		if (contain) contain->m_valid = (f & 16) != 0;
		g_r61Bones = cfg[6];
		for (int k = 1; k < 12; ++k) {
			Object *o = g_r61Objs[k];
			unsigned g = cfg[9 + k];
			o->m_body = (cfg_in[9 + k] & 1) ? &bodies[k] : 0;   /* the body, once (the Rust side's is a module) */
			o->m_drawable = (g & 2) ? &draws[k] : 0;
			draws[k].m_condFlags = ModelConditionFlags();
			if (g & 4) draws[k].m_condFlags.set(MODELCONDITION_DOCKING_BEGINNING);
			o->m_kindOf = KindOfMaskType();
			if (g & 8) o->m_kindOf.set(KINDOF_DOZER);
			if (g & 16) o->m_kindOf.set(KINDOF_HARVESTER);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_airborneLoco = (g & 32) != 0 && (g & 64) != 0;   /* (isUsingAirborneLocomotor asks the AI: none, no locomotor) */
			o->m_ai = (g & 64) ? &g_r61Ais[k] : 0;
		}
		g_r61FindHas = (cfg[7] & 1) != 0;
		g_r61Find.set(r61F(cfg[8]), r61F(cfg[9]), 0.0f);
	};
	/* the dockers' positions are the scene's once; the dock moves them */
	for (int k = 1; k < 12; ++k) { g_r61Objs[k]->m_pos.x = r61F(cfg[20 + k]); g_r61Objs[k]->m_pos.y = 3.0f; g_r61Objs[k]->m_pos.z = 0.0f; }
	scene();
	g_modFindHook = r61Find;
	g_r30BoneHook = r61Bone;
	g_findPosHook = r61FindPos;
	g_mcHook = r61Mc;
	g_setStatusHook = r57SetStatus;
	g_distDcHook = r57Dist;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r61Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	RailedTransportDockUpdate *m = new RailedTransportDockUpdate(me, d);
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
		if (o[0] == 2 && o[1] >= 21 && o[1] <= 31) { Object *x = g_r61Objs[o[1] - 20]; x->m_pos.x = r61F(o[2]); x->m_pos.y = 3.0f; x->m_pos.z = 0.0f; }
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
		Object *docker = r61Find((ObjectID)o[1]);
		try {
			switch (o[0]) {
			case 0: g_r61Updating = m; res = (unsigned)m->update(); break;
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
			case 21: res = m->action(docker, r61Find((ObjectID)o[3])) ? 1 : 0; break;
			case 22: xst = runXfer(m, 0, xfer_path, bytes, 4096, &blen, &crc); g_r61Saved.assign(bytes, bytes + blen); break;
			case 23: {
				std::vector<unsigned char> b = g_r61Saved;
				if (b.size() > 4096) b.resize(4096);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 24: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 25: res = m->getDockUpdateInterface() == (DockUpdateInterface *)m ? 1 : 0; break;
			case 26: res = m->isLoadingOrUnloading() ? 1 : 0; break;
			case 27: m->unloadAll(); break;
			case 28: m->unloadSingleObject(docker); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r61Updating = 0;
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
		w.push_back((unsigned)m->m_dockingObjectID);
		w.push_back(realBits(m->m_pullInsideDistancePerFrame));
		w.push_back((unsigned)m->m_unloadingObjectID);
		w.push_back(realBits(m->m_pushOutsideDistancePerFrame));
		w.push_back((unsigned)m->m_unloadCount);
		putU32((unsigned)w.size());
		for (size_t k = 0; k < w.size(); ++k) putU32(w[k]);
		for (int k = 1; k < 12; ++k) {
			const Coord3D *p = g_r61Objs[k]->getPosition();
			putU32(realBits(p->x)); putU32(realBits(p->y)); putU32(realBits(p->z));
		}
		putU32(contain ? (unsigned)contain->m_list.size() : 0xffffffffu);
		if (contain) for (size_t k = 0; k < contain->m_list.size(); ++k) putU32((unsigned)contain->m_list[k]->m_oid);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modFindHook = 0;
	g_r30BoneHook = 0;
	g_findPosHook = 0;
	g_mcHook = 0;
	g_setStatusHook = 0;
	g_distDcHook = 0;
	TheTerrainLogic = 0;
	me->m_behaviors = 0;
	me->m_contain = 0;
	delete m;
	delete d;
	if (contain) { contain->m_list.clear(); delete contain; }
	delete cd;
	for (int k = 0; k < 12; ++k) {
		Object *x = g_r61Objs[k];
		x->m_drawable = 0; x->m_body = 0; x->m_behaviors = 0; x->m_ai = 0;
		delete x;
		g_r61Objs[k] = 0;
	}
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
