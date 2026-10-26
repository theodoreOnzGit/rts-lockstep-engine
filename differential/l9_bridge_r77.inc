/* Differential-test driver for BridgeBehavior (p1w13-sonnet, batch R77; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeBehavior.cpp   (#included, unchanged)
** with its header unchanged apart from the forward enum and the interface the module build's stand-in header already has (build.rs:
** mod_gen/BridgeBehavior_r77.h), compiled under R77 names (BridgeBehavior is R77BridgeBehavior while the two files are included: the stand-in
** header's class of that name is the module build's), in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the bridge (id 1:
** maybe a team of three, the module build's MBody (attemptDamage and attemptHealing lines; its damage state the test's), a position), four
** towers (ids 2-5: BRIDGE_TOWER or not, maybe an MBody), four objects that may stand on the bridge (ids 6-9: their kinds, physics (its fall
** flag), body, layer and position the test's; Object::setLayer sets the layer), the terrain bridge (batch R70's table: present or not, its
** corners, template name and layer; findBridgeAt is batch R70's line; the ids set on it lines), the terrain roads (two bridge templates: their
** effect names, sounds, height, count and scaffold names the test's), the thing factory (R77Scaffold and R77Support: their geometry the test's;
** newObject makes ids 10, 11, ... with the REAL BridgeScaffoldBehavior), the partition scan (batch R33's line; the candidates the test's), the
** object's bones (batch R74's hook: BoneA, if the test has it), the FX and OCL stores (FX_A, FX_B; OCL_A, OCL_B: their lines), the audio, radar,
** pathfinder and terrain logic calls (lines), the REAL Object::kill and attemptHealing, PointInsideArea2D (Line2D.cpp), UpdateModule, INI,
** RandomValue (the logic's) and Xfer; the game logic (its frame). */
#ifndef TWO_PI
#define TWO_PI 6.28318530718f   /* BaseType.h line 87, verbatim (the module build's BaseType stand-in has none) */
#endif
#define BridgeBehavior R77BridgeBehavior
#include "BridgeBehavior_r77.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeBehavior.cpp"
#undef BridgeBehavior

void Bridge::setBridgeObjectID( ObjectID id ) { ulog("setbridgeid %d", (int)id); }
void Bridge::setTowerObjectID( ObjectID id, BridgeTowerType which ) { ulog("settowerid %d %d", (int)id, (int)which); }
void TerrainLogic::updateBridgeDamageStates( void ) { ulog("bridgestates"); }
void Radar::queueTerrainRefresh( void ) { ulog("radarqueue"); }
void Pathfinder::changeBridgeState( PathfindLayerEnum layer, Bool repaired ) { ulog("bridgestate %d %d", (int)layer, repaired ? 1 : 0); }
TerrainRoadType g_r77Roads[2];
const char *const kR77RoadNames[2] = { "R77Road", "R77Other" };
TerrainRoadType *TerrainRoadCollection::findBridge( AsciiString name )
{
	for (int i = 0; i < 2; ++i) if (name == kR77RoadNames[i]) return &g_r77Roads[i];
	return 0;
}
TerrainRoadCollection g_r77RoadCollection;
TerrainRoadCollection *TheTerrainRoads = &g_r77RoadCollection;
/* the thing templates' geometry (batch R29's getTemplateGeometryInfo asks the hook first) */
extern const GeometryInfo *(*g_r77GeomHook)(const ThingTemplate *);

namespace {

ModuleData *l9r77Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = R77BridgeBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r77Saved;
Object *g_r77Objs[9];
std::vector<Object *> g_r77Made;
std::vector<BridgeScaffoldBehavior *> g_r77Scaffolds;
std::vector<BehaviorModule **> g_r77MadeLists;
unsigned g_r77NextId = 10;
ThingTemplate g_r77Templates[2];
const char *const kR77TmplNames[2] = { "R77Scaffold", "R77Support" };
GeometryInfo g_r77Geom[2] = { GeometryInfo(GEOMETRY_BOX, false, 10.0f, 10.0f, 10.0f), GeometryInfo(GEOMETRY_BOX, false, 10.0f, 10.0f, 10.0f) };
ModuleData *g_r77ScaffoldData = 0;
unsigned g_r77Bones = 0;
/* the test's tables, by index (the same lists in the Rust test) */
const Real kR77Max[4] = { 100.0f, 250.0f, 1000.0f, 40.0f };
const char *const kR77Ocl[4] = { "", "OCL_A", "OCL_B", "None" };
const char *const kR77Fx[4] = { "", "FX_A", "FX_B", "None" };
const char *const kR77Sound[4] = { "", "R77SndA", "R77SndB", "R77SndC" };
const char *const kR77Names[4] = { "R77Scaffold", "R77Support", "Nope", "" };
const char *const kR77Road[4] = { "R77Road", "R77Other", "Nope", "" };
const Real kR77Radius[4] = { 5.0f, 8.0f, 12.0f, 20.0f };
const Real kR77ScaffoldHeight[4] = { 3.0f, 6.0f, 10.0f, 0.0f };
const Real kR77SupportHeight[4] = { 4.0f, 6.0f, 10.0f, 25.0f };
Real r77F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
Real r77B(unsigned word, int byte) { return (Real)(signed char)(word >> (8 * byte) & 255); }
Object *r77Find(ObjectID id)
{
	if ((int)id >= 1 && (int)id <= 9) return g_r77Objs[(int)id - 1];
	for (size_t i = 0; i < g_r77Made.size(); ++i) if (g_r77Made[i]->m_oid == id) return g_r77Made[i];
	return 0;
}
int r77TmplIndex(const ThingTemplate *t) { for (int i = 0; i < 2; ++i) if (t == &g_r77Templates[i]) return i; return -1; }
const ThingTemplate *r77FindTemplate(const AsciiString &name)
{
	for (int i = 0; i < 2; ++i) if (strcmp(name.str(), kR77TmplNames[i]) == 0) return &g_r77Templates[i];
	return 0;
}
const GeometryInfo *r77Geom(const ThingTemplate *t) { int i = r77TmplIndex(t); return i < 0 ? 0 : &g_r77Geom[i]; }
Object *r77NewObject(const ThingTemplate *tmplate, Team *team)
{
	/* (a support template of no height makes supports for ever, which the Rust side excludes as undefined: stop it here) */
	if (g_r77Made.size() >= 4000) throw ERROR_BUG;
	Object *o = newObject();
	o->m_oid = (ObjectID)g_r77NextId++;
	o->m_team = team;
	o->m_player = team ? team->m_controller : 0;
	o->m_tmplShared = tmplate;
	BridgeScaffoldBehavior *s = new BridgeScaffoldBehavior(o, g_r77ScaffoldData);
	BehaviorModule **list = new BehaviorModule *[2];
	list[0] = s;
	list[1] = 0;
	o->m_behaviors = list;
	g_r77Made.push_back(o);
	g_r77Scaffolds.push_back(s);
	g_r77MadeLists.push_back(list);
	return o;
}
int r77Bone(const Object *obj, const char *name, Coord3D *position, Matrix3D *transform)
{
	if (obj == g_r77Objs[0] && (g_r77Bones & 1) && strcmp(name, "BoneA") == 0) {
		if (position) { position->x = 7.0f; position->y = -3.0f; position->z = 2.0f; }
		return 1;
	}
	if (position) *position = obj->m_pos;
	return 0;
}
void r77SetLayer(Object *obj, PathfindLayerEnum layer) { obj->m_layer = layer; }
/* a bridge template from six words: the damage OCLs, FXs, the repair OCLs, FXs (two bits a slot, state-major), the sounds (two bits each: the
** four damage sounds, then the four repair sounds), the rest (bits 0-7 the effects' height, 8-9 their count, 10-11 and 12-13 the scaffold and
** support names) */
void r77Road(TerrainRoadType &t, const unsigned *w)
{
	for (int s = 0; s < 4; ++s) {
		for (int i = 0; i < 3; ++i) {
			int k = 2 * (3 * s + i);
			t.m_damageToOCLString[s][i] = AsciiString(kR77Ocl[w[0] >> k & 3]);
			t.m_damageToFXString[s][i] = AsciiString(kR77Fx[w[1] >> k & 3]);
			t.m_repairedToOCLString[s][i] = AsciiString(kR77Ocl[w[2] >> k & 3]);
			t.m_repairedToFXString[s][i] = AsciiString(kR77Fx[w[3] >> k & 3]);
		}
		t.m_damageToSoundString[s] = AsciiString(kR77Sound[w[4] >> (2 * s) & 3]);
		t.m_repairedToSoundString[s] = AsciiString(kR77Sound[w[4] >> (8 + 2 * s) & 3]);
	}
	t.m_transitionEffectsHeight = (Real)(w[5] & 255);
	t.m_numFXPerType = (Int)(w[5] >> 8 & 3);
	t.m_scaffoldObjectName = AsciiString(kR77Names[w[5] >> 10 & 3]);
	t.m_scaffoldSupportObjectName = AsciiString(kR77Names[w[5] >> 12 & 3]);
}
/* the codes of the resolved effects (the same in the Rust test) */
unsigned r77OclCode(const ObjectCreationList *ocl) { return !ocl ? 0u : (ocl == &g_upOcls[0] ? 1u : 2u); }

}  // namespace

extern Object *(*g_newObjectHook)(const ThingTemplate *tmplate, Team *team);
extern const ThingTemplate *(*g_findTemplateHook)(const AsciiString &name);
extern int (*g_r74BoneHook)(const Object *, const char *, Coord3D *, Matrix3D *);
extern void (*g_setLayerHook)(Object *, PathfindLayerEnum);
extern bool g_r28ThingPose;

extern "C" {

/* Read `filename` (block `D`) as the data of a BridgeBehavior and make the module on the bridge (id 1) at frame cfg[0], the logic's random
** seed cfg[1]. `cfg` (40 words; 2 and 6-33 also settable by op 2): 2 the bridge: bits 0-1 its team (0 none), 2-3 its body's maximum (of
** kR77Max; read once), 4-5 its damage state, 6 a body; 3-5 its position (float bits); 6-9 the towers (ids 2-5): bit 0 BRIDGE_TOWER, 1 a body,
** 2-3 its maximum (read once); 10-13 the objects 6-9: bit 0 BRIDGE, 1 BRIDGE_TOWER, 2 physics, 3 a body, 4-5 the layer, 6 the physics' fall
** flag (set by the scene), bytes 1-3 the position (signed); 14 the terrain bridge: bit 0 present, 1-2 its template name (of kR77Road), 3-4 its
** layer; 15-18 its corners fromLeft, fromRight, toLeft, toRight (bytes 0-2: x, y, z, signed); 19-24 and 25-30 the two bridge templates
** (r77Road); 31 the geometry: bits 0-1 the scaffold's radius, 2-3 its height, 4-5 the support's radius, 6-7 its height; 32 the partition
** scan's candidates (nibbles, ids, 0 ends); 33 bit 0 the bridge has BoneA. The data dump goes first (the speeds, the FX and OCL lists).
** Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3 onDamage (amount a, damage type b & 255, death
** type b >> 8, source c)  4 onHealing (amount a, source c)  5 onBodyDamageStateChange(a, b)  6 onDie  7 setTower(a, object b)
** 8 getTowerID(a)  9 createScaffolding  10 removeScaffolding  11 isScaffoldInMotion | isScaffoldPresent << 1  12 onDelete  13 xfer save
** 14 xfer load of the last save  15 xfer CRC  16 each scaffold object's update(). Each op appends: the CATCH_ALL status, the result, the
** effect log, the xfer status, bytes and CRC, the state (the towers, the flags, the death frame, the scaffold list), the resolved effects,
** the objects (health, layer, fall flag) and the objects made (template, team, position, orientation, scaffold state). */
static int l9r77RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r77Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r77Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[40];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const BridgeBehaviorModuleData *bd = (const BridgeBehaviorModuleData *)d;
		putU32(realBits(bd->m_lateralScaffoldSpeed));
		putU32(realBits(bd->m_verticalScaffoldSpeed));
		putU32((unsigned)bd->m_fx.size());
		for (BridgeFXList::const_iterator it = bd->m_fx.begin(); it != bd->m_fx.end(); ++it) {
			putU32((unsigned)fxId(it->fx));
			putU32(it->timeAndLocationInfo.delay);
			putStr(it->timeAndLocationInfo.boneName.str(), it->timeAndLocationInfo.boneName.getLength());
		}
		putU32((unsigned)bd->m_ocl.size());
		for (BridgeOCLList::const_iterator it = bd->m_ocl.begin(); it != bd->m_ocl.end(); ++it) {
			putU32(r77OclCode(it->ocl));
			putU32(it->timeAndLocationInfo.delay);
			putStr(it->timeAndLocationInfo.boneName.str(), it->timeAndLocationInfo.boneName.getLength());
		}
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitRandom(cfg[1]);
	InitGameLogicRandom(cfg[1]);
	AbAudio audio;
	TheAudio = &audio;
	g_audioHandleLast = 0;
	Radar radar;
	TheRadar = &radar;
	AI ai;
	TheAI = &ai;
	TerrainLogic terrain;
	TheTerrainLogic = &terrain;
	g_r77NextId = 10;
	if (!g_r77ScaffoldData) g_r77ScaffoldData = BridgeScaffoldBehavior::friend_newModuleData(0);
	Player players[3];
	Team teams[3];
	for (int p = 0; p < 3; ++p) {
		players[p].m_idx = p;
		teams[p].m_dummy = p + 1;
		teams[p].m_controller = &players[p];
	}
	for (int i = 0; i < 2; ++i) g_r77Templates[i].m_name = AsciiString(kR77TmplNames[i]);
	MBody bodies[9];
	unsigned char physMem[9][sizeof(PhysicsBehavior) + 16];
	BehaviorModule *behaviors[9][2];
	ThingTemplate bridgeTemplate;
	bridgeTemplate.m_name = AsciiString("R77Bridge");
	for (int k = 0; k < 9; ++k) {
		g_r77Objs[k] = newObject();
		g_r77Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r77Objs[k]->m_tmplShared = &bridgeTemplate;
		bodies[k].m_owner = g_r77Objs[k];
		unsigned maxIdx = k == 0 ? (cfg[2] >> 2 & 3) : (k <= 4 ? (cfg[5 + k] >> 2 & 3) : 0);
		bodies[k].m_max = kR77Max[maxIdx];
		bodies[k].m_health = bodies[k].m_max * 0.5f;
		memset(physMem[k], 0, sizeof(physMem[k]));
		behaviors[k][0] = 0;
		behaviors[k][1] = 0;
	}
	Object *me = g_r77Objs[0];
	R77BridgeBehavior *m = 0;
	try { m = newInstance(R77BridgeBehavior)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	behaviors[0][0] = m;
	auto scene = [&]() {
		for (int k = 0; k < 9; ++k) {
			Object *o = g_r77Objs[k];
			o->m_kindOf = KindOfMaskType();
			o->m_physics = 0;
			o->m_body = 0;
			if (k == 0) {
				unsigned f = cfg[2];
				o->m_kindOf.set(KINDOF_BRIDGE);
				unsigned t = f & 3;
				o->m_team = t ? &teams[t - 1] : 0;
				o->m_player = t ? &players[t - 1] : 0;
				bodies[k].m_state = (BodyDamageType)(f >> 4 & 3);
				if (f >> 6 & 1) o->m_body = &bodies[k];
				o->m_pos.x = r77F(cfg[3]); o->m_pos.y = r77F(cfg[4]); o->m_pos.z = r77F(cfg[5]);
			} else if (k <= 4) {
				unsigned f = cfg[5 + k];
				if (f & 1) o->m_kindOf.set(KINDOF_BRIDGE_TOWER);
				if (f >> 1 & 1) o->m_body = &bodies[k];
			} else {
				unsigned f = cfg[5 + k];
				if (f & 1) o->m_kindOf.set(KINDOF_BRIDGE);
				if (f >> 1 & 1) o->m_kindOf.set(KINDOF_BRIDGE_TOWER);
				PhysicsBehavior *phys = (PhysicsBehavior *)physMem[k];
				if (f >> 2 & 1) { o->m_physics = phys; phys->setAllowToFall((f >> 6 & 1) != 0); }
				if (f >> 3 & 1) o->m_body = &bodies[k];
				o->m_layer = (PathfindLayerEnum)(f >> 4 & 3);
				o->m_pos.x = r77B(f, 1); o->m_pos.y = r77B(f, 2); o->m_pos.z = r77B(f, 3);
			}
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_behaviors = behaviors[k];
		}
		unsigned b = cfg[14];
		for (int c = 0; c < 8; ++c) {
			Bridge &br = g_r70Bridges[c];
			br.m_cell = (b & 1) ? c : -1;
			br.m_templateName = AsciiString(kR77Road[b >> 1 & 3]);
			br.m_layer = (PathfindLayerEnum)(b >> 3 & 3);
			Coord3D *cs[4] = { &br.m_bridgeInfo.fromLeft, &br.m_bridgeInfo.fromRight, &br.m_bridgeInfo.toLeft, &br.m_bridgeInfo.toRight };
			for (int i = 0; i < 4; ++i) { cs[i]->x = r77B(cfg[15 + i], 0); cs[i]->y = r77B(cfg[15 + i], 1); cs[i]->z = r77B(cfg[15 + i], 2); }
		}
		r77Road(g_r77Roads[0], &cfg[19]);
		r77Road(g_r77Roads[1], &cfg[25]);
		g_r77Roads[0].m_name = AsciiString(kR77RoadNames[0]);
		g_r77Roads[1].m_name = AsciiString(kR77RoadNames[1]);
		unsigned g = cfg[31];
		g_r77Geom[0] = GeometryInfo(GEOMETRY_BOX, false, kR77ScaffoldHeight[g >> 2 & 3], kR77Radius[g & 3], kR77Radius[g & 3]);
		g_r77Geom[1] = GeometryInfo(GEOMETRY_BOX, false, kR77SupportHeight[g >> 6 & 3], kR77Radius[g >> 4 & 3], kR77Radius[g >> 4 & 3]);
		g_ahRange.clear();
		for (int i = 0; i < 8; ++i) {
			unsigned id = cfg[32] >> (4 * i) & 15;
			if (id == 0) break;
			Object *o = r77Find((ObjectID)id);
			if (o) g_ahRange.push_back(o);
		}
		g_r77Bones = cfg[33];
	};
	scene();
	g_modFindHook = r77Find;
	g_findTemplateHook = r77FindTemplate;
	g_newObjectHook = r77NewObject;
	g_r74BoneHook = r77Bone;
	g_ahRangeHook = r33ScanHook;
	g_setLayerHook = r77SetLayer;
	g_r77GeomHook = r77Geom;
	g_r28ThingPose = true;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			if (g_events[j].kind != 100) continue;
			const std::string &l = g_ulog[(size_t)g_events[j].p];
			if (l.compare(0, 10, "setorient ") == 0 || l.compare(0, 9, "setlayer ") == 0 || l.compare(0, 8, "destroy ") == 0) continue;
			log.push_back(l);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	auto putState = [&]() {
		for (int i = 0; i < BRIDGE_MAX_TOWERS; ++i) putU32((unsigned)m->m_towerID[i]);
		putU32(m->m_fxResolved ? 1u : 0u);
		putU32(m->m_scaffoldPresent ? 1u : 0u);
		putU32(m->m_deathFrame);
		putU32((unsigned)m->m_scaffoldObjectIDList.size());
		for (ObjectIDList::const_iterator it = m->m_scaffoldObjectIDList.begin(); it != m->m_scaffoldObjectIDList.end(); ++it) putU32((unsigned)*it);
		/* the resolved effects: per state, damaged-to then repaired-to: three OCLs, three FXs, the sound's name and object */
		for (int s = 0; s < BODYDAMAGETYPE_COUNT; ++s) {
			for (int r = 0; r < 2; ++r) {
				for (int i = 0; i < MAX_BRIDGE_BODY_FX; ++i) putU32(r77OclCode(r ? m->m_repairToOCL[s][i] : m->m_damageToOCL[s][i]));
				for (int i = 0; i < MAX_BRIDGE_BODY_FX; ++i) putU32((unsigned)fxId(r ? m->m_repairToFX[s][i] : m->m_damageToFX[s][i]));
				const AudioEventRTS &a = r ? m->m_repairToSound[s] : m->m_damageToSound[s];
				putStr(a.getEventName().str(), a.getEventName().getLength());
				putU32((unsigned)a.m_objectID);
			}
		}
		for (int k = 0; k < 9; ++k) {
			Object *o = g_r77Objs[k];
			putU32(realBits(bodies[k].m_health));
			putU32((unsigned)o->m_layer);
			putU32(o->m_physics ? (o->m_physics->getAllowToFall() ? 1u : 0u) : 2u);
		}
		putU32((unsigned)g_r77Made.size());
		for (size_t i = 0; i < g_r77Made.size(); ++i) {
			Object *o = g_r77Made[i];
			BridgeScaffoldBehavior *s = g_r77Scaffolds[i];
			putU32((unsigned)r77TmplIndex(o->m_tmplShared));
			putU32(o->m_team ? (unsigned)o->m_team->m_dummy : 0u);
			putU32(realBits(o->m_pos.x)); putU32(realBits(o->m_pos.y)); putU32(realBits(o->m_pos.z));
			putU32(realBits(o->m_orientation));
			putU32((unsigned)s->m_targetMotion);
			const Coord3D *cs[4] = { &s->m_createPos, &s->m_riseToPos, &s->m_buildPos, &s->m_targetPos };
			for (int c = 0; c < 3; ++c) { putU32(realBits(cs[c]->x)); putU32(realBits(cs[c]->y)); putU32(realBits(cs[c]->z)); }
			putU32(realBits(s->m_lateralSpeed));
			putU32(realBits(s->m_verticalSpeed));
			putU32(realBits(cs[3]->x)); putU32(realBits(cs[3]->y)); putU32(realBits(cs[3]->z));
		}
	};
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && ((o[1] >= 2 && o[1] <= 33) && !(o[1] >= 3 && o[1] <= 5))) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[2048];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: case 4: {
				DamageInfo di;
				di.in.m_amount = r77F(o[1]);
				di.in.m_damageType = o[0] == 4 ? DAMAGE_HEALING : (DamageType)(o[2] & 255);
				di.in.m_deathType = o[0] == 4 ? DEATH_NONE : (DeathType)(o[2] >> 8);
				di.in.m_sourceID = (ObjectID)o[3];
				if (o[0] == 3) m->onDamage(&di);
				else m->onHealing(&di);
				break;
			}
			case 5: { DamageInfo di; m->onBodyDamageStateChange(&di, (BodyDamageType)o[1], (BodyDamageType)o[2]); break; }
			case 6: { DamageInfo di; m->onDie(&di); break; }
			case 7: m->setTower((BridgeTowerType)(Int)o[1], r77Find((ObjectID)o[2])); break;
			case 8: res = (unsigned)m->getTowerID((BridgeTowerType)(Int)o[1]); break;
			case 9: m->createScaffolding(); break;
			case 10: m->removeScaffolding(); break;
			case 11: res = (m->isScaffoldInMotion() ? 1u : 0u) | (m->isScaffoldPresent() ? 2u : 0u); break;
			case 12: m->onDelete(); break;
			case 13: xst = abRunXfer(m, 0, xfer_path, bytes, 2048, &blen, &crc); g_r77Saved.assign(bytes, bytes + blen); break;
			case 14: {
				std::vector<unsigned char> b = g_r77Saved;
				if (b.size() > 2048) b.resize(2048);
				xst = abRunXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 15: xst = abRunXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 16: for (size_t k = 0; k < g_r77Scaffolds.size(); ++k) g_r77Scaffolds[k]->update(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		if (st) res = 0;
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putState();
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_r74BoneHook = 0;
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_setLayerHook = 0;
	g_r77GeomHook = 0;
	g_r28ThingPose = false;
	delete m;   /* (its destructor destroys the towers it finds: none, with the find hook gone) */
	delete d;
	for (int k = 0; k < 9; ++k) {
		Object *x = g_r77Objs[k];
		x->m_body = 0; x->m_behaviors = 0; x->m_physics = 0; x->m_team = 0; x->m_player = 0; x->m_tmplShared = 0;
		delete x;
		g_r77Objs[k] = 0;
	}
	for (size_t i = 0; i < g_r77Made.size(); ++i) {
		delete g_r77Scaffolds[i];
		g_r77Made[i]->m_behaviors = 0; g_r77Made[i]->m_team = 0; g_r77Made[i]->m_player = 0; g_r77Made[i]->m_tmplShared = 0;
		delete g_r77Made[i];
		delete[] g_r77MadeLists[i];
	}
	g_r77Made.clear();
	g_r77Scaffolds.clear();
	g_r77MadeLists.clear();
	TheAudio = 0;
	TheRadar = 0;
	TheAI = 0;
	TheTerrainLogic = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r77_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r77RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
