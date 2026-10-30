/* Differential-test driver for RailroadBehavior (p1w13-sonnet, batch R80; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RailroadGuideAIUpdate.h                (mod_gen/RailroadGuideAIUpdate_r80.h: its include
**     of PhysicsUpdate.h dropped, batch R50's being in already; build.rs)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/RailroadGuideAIUpdate.cpp (mod_gen/RailroadGuideAIUpdate_r80.cpp: its
**     include of the header edited; build.rs)
**   GeneralsMD/Code/GameEngine/Source/GameClient/Statistics.cpp                                 (MuLaw, Normalize, NormalizeToRange, verbatim below)
** in wrap_module.cpp's unit, at its end. RailroadBehavior is a PhysicsBehavior: the class is compiled on batch R50's real PhysicsBehavior
** (R50PhysicsBehavior), by the macros R50's driver uses while it includes PhysicsUpdate.cpp.
**
** The scene, on the module build's stand-ins: six objects (ids 1-6). Objects 1-4 are train cars: 1 always, with the module of the data the test
** reads (block D); 2-4 maybe, with the same data made a carriage (m_isLocomotive FALSE); each maybe with a drawable (its setDrawableHidden and
** model condition calls lines; its position its object's) and a contain (orderAllPassengersToExit a line). Objects 5-6 are what a train hits:
** their kinds, maybe a DemoTrapUpdate (findUpdateModule finds one), under construction, effectively dead, maybe batch R50's PhysicsBehavior
** of the default data (maybe with a bounce sound), maybe an AI (its enter target object 1 or none), maybe contained by object 1, maybe a body.
** Every object: a position, an angle, a geometry, a team of three (each with a player; the relationships the test's), the module build's MBody
** (its attemptDamage a line; the real Object::kill). The waypoints: up to eight, ids 21-28, in the test's list order, at the test's places,
** named WP<n>, WP<n>Tunnel, WP<n>Station, WP<n>Disembark, WP<n>PingPong or WP<n>StationX, each linked to one other (or none). The thing factory:
** CAR_A makes an object with a RailroadBehavior of the carriage data and a body, CAR_B one with a body, ids 7, 8, ... (the new object's team the
** one it is given, at the origin, of the default geometry). The partition manager's getClosestObject: a line, then the first of the test's
** candidates the caller's filters allow. The pathfinder's walls (lines), the terrain's ground (batch O's eight cells, the test's heights; each
** call a line), the audio (addAudioEvent and removeAudioEvent lines, the handles 1, 2, ...; isCurrentlyPlaying the test's answer, a line;
** the silent event, of no name), the game logic (its frame, findObjectByID over the objects, destroyObject a line), the object transform (kept
** as batch R50's: setTransformMatrix keeps the position and the angle), and the REAL UpdateModule, INI, Xfer and logic random values. */

/* Lib/BaseType.h lines 157-163, verbatim (this build's BaseType leaves it out) */
template <typename NUM>
inline int sign(NUM x)
{
	if (x > 0) return 1;
	else if (x < 0) return -1;
	else return 0;
}
/* Statistics.cpp lines 45-62, verbatim (the client's statistics, not otherwise in the build) */
Real MuLaw(Real valueToRun, Real maxValueForVal, Real mu)
{
	Real testVal = (valueToRun - maxValueForVal / 2) / (maxValueForVal / 2);
	return (sign(testVal) * log(1 + mu * fabs(testVal)) /
														 log(1 + mu));
}

// from my head. jkmcd
Real Normalize(Real valueToNormalize, Real minRange, Real maxRange)
{
	return ((valueToNormalize - minRange) / (maxRange - minRange));
}

// from my head again. jkmcd
Real NormalizeToRange(Real valueToNormalize, Real minRange, Real maxRange, Real outRangeMin, Real outRangeMax)
{
	return (Normalize(valueToNormalize, minRange, maxRange) * (outRangeMax - outRangeMin)) + outRangeMin;
}

/* The file keeps its name keys in function-local statics, made the first time each function runs; the test's name key generator starts
** over in most cases, so a key made in one case names nothing (or another name) in the next. While the file is included NAMEKEY gives the
** two names the stand-ins answer to keys of their own, the same in every case. */
const NameKeyType kR80KeyRailroad = (NameKeyType)0x7F790001;
const NameKeyType kR80KeyDemoTrap = (NameKeyType)0x7F790002;
inline NameKeyType r80NameKey(const char *name)
{
	if (strcmp(name, "RailroadBehavior") == 0) return kR80KeyRailroad;
	if (strcmp(name, "DemoTrapUpdate") == 0) return kR80KeyDemoTrap;
	return TheNameKeyGenerator->nameToKey(name);
}
#define NAMEKEY r80NameKey
#define PhysicsBehavior R50PhysicsBehavior
#define PhysicsBehaviorModuleData R50PhysicsBehaviorModuleData
#define getPhysicsBehaviorModuleData getR50PhysicsBehaviorModuleData
#define getPhysics r50GetPhysics
#define destroyObject r50DestroyObject
#include "RailroadGuideAIUpdate_r80.cpp"
#undef destroyObject
#undef getPhysics
#undef getPhysicsBehaviorModuleData
#undef PhysicsBehaviorModuleData
#undef PhysicsBehavior
#undef NAMEKEY

const Coord3D *Drawable::getPosition() const { return m_ownerObject->getPosition(); }
const AudioEventRTS *AudioManager::getValidSilentAudioEvent() const { static AudioEventRTS silent; return &silent; }
Waypoint *g_r80First = 0;
Waypoint *TerrainLogic::getFirstWaypoint(void) { return g_r80First; }

/* a contain whose orderAllPassengersToExit is a line */
class R80Contain : public UpgContain
{
public:
	Object *m_r80Owner = 0;
	virtual void orderAllPassengersToExit(CommandSourceType s, Bool instantly) { ulog("exitall %d %d %d", (int)m_r80Owner->m_oid, (int)s, (int)instantly); }
};

namespace {

ModuleData *l9r80Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = RailroadBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r80Saved;
Object *g_r80Objs[6];
std::vector<Object *> g_r80Made;
std::vector<MBody *> g_r80MadeBodies;
std::vector<Drawable *> g_r80MadeDraws;
std::vector<BehaviorModule **> g_r80MadeLists;
std::map<const Object *, RailroadBehavior *> g_r80Mods;
const ModuleData *g_r80CarData = 0;
bool g_r80Trap[2] = { false, false };
Waypoint g_r80Ways[8];
int g_r80NumWays = 0;
Team g_r80Teams[3];
Player g_r80Players[3];
int g_r80Rel[3][3];
std::vector<Object *> g_r80Candidates;
ThingTemplate g_r80Templates[2];
const Real kR80Radii[8] = { 1.0f, 2.5f, 5.0f, 8.0f, 12.0f, 0.5f, 3.0f, 6.0f };
const Real kR80Grounds[8] = { 0.0f, 1.0f, -2.0f, 4.0f, -5.0f, 0.5f, 8.0f, -10.0f };
const char *const kR80Suffix[6] = { "", "Tunnel", "Station", "Disembark", "PingPong", "StationX" };
const KindOfType kR80Kinds[9] = { KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_HUGE_VEHICLE, KINDOF_STRUCTURE, KINDOF_FS_POWER, KINDOF_FS_FACTORY,
	KINDOF_FS_BASE_DEFENSE, KINDOF_FS_TECHNOLOGY, KINDOF_REBUILD_HOLE };

Object *r80Find(ObjectID id)
{
	if ((int)id >= 1 && (int)id <= 6) return g_r80Objs[(int)id - 1];
	size_t k = (size_t)((int)id - 7);
	if ((int)id >= 7 && k < g_r80Made.size()) return g_r80Made[k];
	return 0;
}
RailroadBehavior *r80Module(const Object *o)
{
	std::map<const Object *, RailroadBehavior *>::const_iterator it = g_r80Mods.find(o);
	return it == g_r80Mods.end() ? 0 : it->second;
}
UpdateModule *r80FindUpdate(const Object *obj, NameKeyType key)
{
	static char trap[16];
	if (key == kR80KeyRailroad) return r80Module(obj);
	if (key == kR80KeyDemoTrap) {
		for (int k = 0; k < 2; ++k) if (obj == g_r80Objs[4 + k] && g_r80Trap[k]) return (UpdateModule *)(void *)trap;
	}
	return 0;
}
Relationship r80TeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return (Relationship)g_r80Rel[from->m_dummy - 1][to->m_dummy - 1];
}
Waypoint *r80WaypointById(UnsignedInt id)
{
	for (int k = 0; k < g_r80NumWays; ++k) if ((UnsignedInt)g_r80Ways[k].m_id == id) return &g_r80Ways[k];
	return NULL;
}
Object *r80Closest(PartitionFilter **filters)
{
	for (size_t i = 0; i < g_r80Candidates.size(); ++i) {
		Object *c = g_r80Candidates[i];
		bool ok = true;
		for (PartitionFilter **f = filters; f && *f; ++f) if (!(*f)->allow(c)) { ok = false; break; }
		if (ok) return c;
	}
	return 0;
}
const ThingTemplate *r80FindTemplate(const AsciiString &name)
{
	for (int k = 0; k < 2; ++k) if (g_r80Templates[k].m_name.compare(name) == 0) return &g_r80Templates[k];
	return 0;
}
UpdateModule *g_r80Updating = 0;
/* GameLogic::friend_awakenUpdateModule as batch R50's driver applies it */
void r80Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r80Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
/* the Rust world schedules an update module when it makes the object (the frame, or 1 at frame 0) for a constructor that sets no wake frame */
void r80Schedule(UpdateModule *u)
{
	if (u->friend_getNextCallFrame() == 0) u->friend_setNextCallFrame(TheGameLogic->getFrame() ? TheGameLogic->getFrame() : 1);
}
Object *r80NewObject(const ThingTemplate *tmplate, Team *team)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)(7 + g_r80Made.size());
	o->m_team = team;
	o->m_player = team ? team->m_controller : 0;
	o->poseMatrix();
	MBody *b = new MBody;
	b->m_owner = o;
	o->m_body = b;
	Drawable *dr = new Drawable;   /* (ThingFactory::newObject makes the object's drawable) */
	dr->m_ownerObject = o;
	o->m_drawable = dr;
	g_r80MadeDraws.push_back(dr);
	BehaviorModule **list = new BehaviorModule *[2];
	list[0] = list[1] = 0;
	if (tmplate == &g_r80Templates[0]) {
		RailroadBehavior *rb = newInstance(RailroadBehavior)(o, g_r80CarData);
		r80Schedule(rb);
		list[0] = rb;
		g_r80Mods[o] = rb;
	}
	o->m_behaviors = list;
	g_r80Made.push_back(o);
	g_r80MadeBodies.push_back(b);
	g_r80MadeLists.push_back(list);
	return o;
}
Real r80F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
/* FNV-1a over words */
unsigned r80Hash(const std::vector<unsigned> &w)
{
	unsigned h = 2166136261u;
	for (size_t i = 0; i < w.size(); ++i) for (int b = 0; b < 4; ++b) { h ^= (w[i] >> (8 * b)) & 255u; h *= 16777619u; }
	return h;
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a RailroadBehavior (and again, made a carriage, as the data of objects 2-4 and CAR_A's), make the
** scene and the module on object 1. `cfg` (32 words; * settable by op 1): 0 the frame, 1 the random seed, 2 the number of waypoints (1-8),
** 3-10 the waypoints (bits 0-7 x / 8 and 8-15 y / 8, signed bytes; 16-19 z * 2; 20-22 the name's suffix, of kR80Suffix; 23-26 the waypoint it
** links to, 1-8, or 0 none), 11-16* objects 1-6's x, 17-22* their y (float bits; z 0), 23* their angles (nibbles: 0.4 k - 3.0), 24 objects 1-4
** (a byte each: 0 a RailroadBehavior (object 1's always), 1 a drawable, 2 a contain, 3-4 the team, 0 none, 5-7 the radius, of kR80Radii), 25-26
** objects 5-6 (bits 0-8 the kinds of kR80Kinds, 9 a DemoTrapUpdate, 10* under construction, 11* effectively dead, 12 a PhysicsBehavior, 13 an
** AI, 14* its enter target object 1, 15* contained by object 1, 16-17 the team, 18 a bounce sound, 19-21 the radius, 22 a body), 27* the
** candidates (nibbles of object numbers, 0 ends), 28 the teams' relationships (2 bits each, row-major), 29* bit 0 the audio is playing,
** bits 1-24 the eight ground cells' heights (of kR80Grounds). The data dump goes first. Then `n` ops of 5 words [op, a, b, c, d] ("object a's
** module" its RailroadBehavior; none, nothing): 0 its update()  1 cfg[a] = b  2 the frame = a  3 xfer save (object 1's)  4 xfer load of the
** last save, then loadPostProcess  5 xfer CRC  6 its onCollide(object b or NULL, d bit 0: NULL else b's position, d bit 1: NULL else (0, 0, 1))
** 7 its isRailroad()  8 its setHeld(b)  9 its destroyTheWholeTrainNow()  10 its disembark()  11 its makeAWallOutOfThisTrain(b)
** 12 its hasEverBeenHitched(). Each op appends: the CATCH_ALL status, the result, the effect log (the settransform and partition lines left
** out), the xfer status, bytes and CRC; then the number of objects and, per object: its transform and angle, its producer, its RailroadBehavior
** (0, or 1 then its 57 state words and its track: whether it has one, then if so whether it loops, its length, its points and their hash), its
** PhysicsBehavior (objects 5-6: 0, or 1 then its velocity and the three rates). */
static int l9r80RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r80Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	ModuleData *cd = 0;
	try { d = l9r80Read(filename); cd = l9r80Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { delete d; return status; }
	((RailroadBehaviorModuleData *)cd)->m_isLocomotive = FALSE;
	g_r80CarData = cd;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const RailroadBehaviorModuleData *rd = (const RailroadBehaviorModuleData *)d;
		putU32((unsigned)rd->m_carriageTemplateNameData.size());
		for (size_t k = 0; k < rd->m_carriageTemplateNameData.size(); ++k) putStr(rd->m_carriageTemplateNameData[k].str(), (int)rd->m_carriageTemplateNameData[k].getLength());
		putStr(rd->m_pathPrefixName.str(), (int)rd->m_pathPrefixName.getLength());
		putStr(rd->m_CrashFXTemplateName.str(), (int)rd->m_CrashFXTemplateName.getLength());
		putU32(rd->m_isLocomotive ? 1 : 0);
		const Real reals[7] = { rd->m_runningGarrisonSpeedMax, rd->m_killSpeedMin, rd->m_speedMax, rd->m_acceleration, rd->m_braking, rd->m_friction,
			rd->m_mass };
		for (int k = 0; k < 7; ++k) putU32(realBits(reals[k]));
		putU32(rd->m_waitAtStationTime);
		const AudioEventRTS *sounds[6] = { &rd->m_runningSound, &rd->m_clicketyClackSound, &rd->m_bigMetalImpactDefaultSound,
			&rd->m_smallMetalImpactDefaultSound, &rd->m_meatyImpactDefaultSound, &rd->m_whistleSound };
		for (int k = 0; k < 6; ++k) putStr(sounds[k]->getEventName().str(), (int)sounds[k]->getEventName().getLength());
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[1]);
	R34Audio audio;
	TheAudio = &audio;
	g_audioHandleLast = 0;
	AI ai;
	TheAI = &ai;
	ThePartitionManager = &g_ahPm;
	TheTerrainLogic = &g_hdTerrain;
	g_modDestroyHook = r50Destroy;
	g_xformHook = r50Xform;
	g_objXformSyncPos = true;
	g_modFindHook = r80Find;
	g_findUpdateHook = r80FindUpdate;
	g_teamRelHook = r80TeamRel;
	g_findTemplateHook = r80FindTemplate;
	g_newObjectHook = r80NewObject;
	g_waypointByIdHook = r80WaypointById;
	g_closestAtHook = r80Closest;
	gl.m_awakenHook = r80Awaken;
	g_r80Made.clear();
	g_r80MadeBodies.clear();
	g_r80MadeLists.clear();
	g_r80MadeDraws.clear();
	g_r80Mods.clear();
	g_r80Templates[0].m_name = AsciiString("CAR_A");
	g_r80Templates[1].m_name = AsciiString("CAR_B");
	for (int t = 0; t < 3; ++t) {
		g_r80Players[t].m_idx = t;
		g_r80Teams[t].m_dummy = t + 1;
		g_r80Teams[t].m_controller = &g_r80Players[t];
	}
	for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r80Rel[a][b] = (int)(cfg[28] >> (2 * (3 * a + b)) & 3);
	/* the waypoints, in list order */
	g_r80NumWays = (int)(cfg[2] & 255);
	if (g_r80NumWays > 8) g_r80NumWays = 8;
	for (int k = 0; k < 8; ++k) {
		Waypoint &w = g_r80Ways[k];
		unsigned c = cfg[3 + k];
		w.m_id = 21 + k;
		w.m_location.x = (Real)(signed char)(c & 255) * 8.0f;
		w.m_location.y = (Real)(signed char)(c >> 8 & 255) * 8.0f;
		w.m_location.z = (Real)(c >> 16 & 15) * 0.5f;
		char name[32];
		sprintf(name, "WP%d%s", k + 1, kR80Suffix[(c >> 20 & 7) % 6]);
		w.m_name = AsciiString(name);
		for (int l = 0; l <= Waypoint::MAX_LINKS; ++l) w.m_links[l] = 0;
		w.m_numLinks = 0;
		unsigned link = c >> 23 & 15;
		if (k < g_r80NumWays && link >= 1 && (int)link <= g_r80NumWays && (int)link != k + 1) {   /* (TerrainLogic::addWaypointLink links no waypoint to itself) */ w.m_links[0] = &g_r80Ways[link - 1]; w.m_numLinks = 1; }
		w.m_pNext = (k + 1 < g_r80NumWays) ? &g_r80Ways[k + 1] : 0;
	}
	g_r80First = g_r80NumWays > 0 ? &g_r80Ways[0] : 0;
	/* the objects */
	MBody bodies[6];
	Drawable draws[4];
	R80Contain contains[4];
	AIUpdateInterface ais[2];
	BehaviorModule *lists[6][2] = { { 0, 0 }, { 0, 0 }, { 0, 0 }, { 0, 0 }, { 0, 0 }, { 0, 0 } };
	R50PhysicsBehaviorModuleData physData;
	R50PhysicsBehavior *phys[2] = { 0, 0 };
	AudioEventRTS bounce(AsciiString("SND_B"), INVALID_ID);
	for (int k = 0; k < 6; ++k) {
		Object *o = g_r80Objs[k] = newObject();
		o->m_oid = (ObjectID)(k + 1);
		o->m_behaviors = lists[k];
		bodies[k].m_owner = o;
		unsigned f = k < 4 ? (cfg[24] >> (8 * k) & 255) : cfg[21 + k];
		unsigned team = k < 4 ? (f >> 3 & 3) : (f >> 16 & 3);
		o->m_team = team ? &g_r80Teams[team - 1] : 0;
		o->m_player = team ? &g_r80Players[team - 1] : 0;
		Real r = k < 4 ? kR80Radii[f >> 5 & 7] : kR80Radii[f >> 19 & 7];
		o->m_geometry = GeometryInfo(GEOMETRY_SPHERE, false, r, r, r);
		o->m_kindOf.clear();
		if (k < 4) {
			o->m_body = &bodies[k];
			draws[k].m_ownerObject = o;
			o->m_drawable = (f >> 1 & 1) ? &draws[k] : 0;
			contains[k].m_r80Owner = o;
			o->m_contain = (f >> 2 & 1) ? &contains[k] : 0;
		} else {
			for (int b = 0; b < 9; ++b) if (f >> b & 1) o->m_kindOf.set(kR80Kinds[b]);
			g_r80Trap[k - 4] = (f >> 9 & 1) != 0;
			o->m_body = (f >> 22 & 1) ? &bodies[k] : 0;
			ais[k - 4].m_owner = o;
			o->m_ai = (f >> 13 & 1) ? &ais[k - 4] : 0;
		}
		o->m_tmpl.m_kindOf = o->m_kindOf;
		o->m_pos.x = r80F(cfg[11 + k]);
		o->m_pos.y = r80F(cfg[17 + k]);
		o->m_pos.z = 0.0f;
		o->m_orientation = (Real)(cfg[23] >> (4 * k) & 15) * 0.4f - 3.0f;
		o->poseMatrix();
	}
	Object *me = g_r80Objs[0];
	/* the words op 1 may set */
	auto apply = [&](unsigned a) {
		if (a >= 11 && a <= 23) {
			for (int k = 0; k < 6; ++k) {
				Object *o = g_r80Objs[k];
				o->m_pos.x = r80F(cfg[11 + k]);
				o->m_pos.y = r80F(cfg[17 + k]);
				o->m_pos.z = 0.0f;
				o->m_orientation = (Real)(cfg[23] >> (4 * k) & 15) * 0.4f - 3.0f;
				o->poseMatrix();
			}
		} else if (a == 25 || a == 26) {
			int k = (int)a - 21;
			Object *o = g_r80Objs[k];
			unsigned f = cfg[a];
			o->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (f >> 10 & 1) ? 1 : 0);
			o->m_effDead = (f >> 11 & 1) != 0;
			ais[k - 4].m_enterTarget = (f >> 14 & 1) ? me : 0;
			o->m_containedBy = (f >> 15 & 1) ? me : 0;
		} else if (a == 27) {
			g_r80Candidates.clear();
			for (int k = 0; k < 8; ++k) { unsigned id = cfg[27] >> (4 * k) & 15; if (id == 0) break; if (id <= 6) g_r80Candidates.push_back(g_r80Objs[id - 1]); }
		} else if (a == 29) {
			g_r34Playing = (cfg[29] & 1) != 0;
			for (int c = 0; c < 8; ++c) g_hdGround[c] = kR80Grounds[cfg[29] >> (1 + 3 * c) & 7];
		}
	};
	for (unsigned a = 25; a <= 29; ++a) apply(a);
	RailroadBehavior *m = 0;
	try {
		m = newInstance(RailroadBehavior)(me, d);
		g_r80Mods[me] = m;
		lists[0][0] = m;
		for (int k = 1; k < 4; ++k) {
			if (cfg[24] >> (8 * k) & 1) {
				RailroadBehavior *rb = newInstance(RailroadBehavior)(g_r80Objs[k], cd);
				g_r80Mods[g_r80Objs[k]] = rb;
				lists[k][0] = rb;
			}
		}
		for (int k = 0; k < 2; ++k) {
			if (cfg[25 + k] >> 12 & 1) {
				phys[k] = newInstance(R50PhysicsBehavior)(g_r80Objs[4 + k], &physData);
				if (cfg[25 + k] >> 18 & 1) phys[k]->setBounceSound(&bounce);
				lists[4 + k][0] = phys[k];
				g_r80Objs[4 + k]->m_r50Physics = phys[k];
			}
		}
	}
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	for (std::map<const Object *, RailroadBehavior *>::iterator it = g_r80Mods.begin(); it != g_r80Mods.end(); ++it) r80Schedule(it->second);
	for (int k = 0; k < 2; ++k) if (phys[k]) r80Schedule(phys[k]);
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			if (g_events[j].kind != 100) continue;
			const std::string &l = g_ulog[(size_t)g_events[j].p];
			if (l.compare(0, 13, "settransform ") == 0 || l == "partition") continue;   /* the transforms are the dump's */
			log.push_back(l);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	auto putModule = [&](RailroadBehavior *rb) {
		const unsigned base[23] = { realBits(rb->m_yawRate), realBits(rb->m_rollRate), realBits(rb->m_pitchRate), realBits(rb->m_accel.x),
			realBits(rb->m_accel.y), realBits(rb->m_accel.z), realBits(rb->m_prevAccel.x), realBits(rb->m_prevAccel.y), realBits(rb->m_prevAccel.z),
			realBits(rb->m_vel.x), realBits(rb->m_vel.y), realBits(rb->m_vel.z), (unsigned)(Int)rb->m_turning, (unsigned)rb->m_ignoreCollisionsWith,
			(unsigned)rb->m_flags, realBits(rb->m_mass), (unsigned)rb->m_currentOverlap, (unsigned)rb->m_previousOverlap, (unsigned)rb->m_lastCollidee,
			rb->m_motiveForceExpires, realBits(rb->m_extraBounciness), realBits(rb->m_extraFriction), realBits(rb->m_velMag) };
		for (int j = 0; j < 23; ++j) putU32(base[j]);
		putU32((unsigned)rb->m_nextStationTask);
		putU32((unsigned)rb->m_trailerID);
		const RailroadBehavior::PullInfo *pis[2] = { &rb->conductorPullInfo, &rb->m_pullInfo };
		for (int p = 0; p < 2; ++p) {
			const RailroadBehavior::PullInfo &pi = *pis[p];
			putU32(realBits(pi.m_direction)); putU32(realBits(pi.speed)); putU32(realBits(pi.trackDistance));
			putU32(realBits(pi.towHitchPosition.x)); putU32(realBits(pi.towHitchPosition.y)); putU32(realBits(pi.towHitchPosition.z));
			putU32((unsigned)pi.m_mostRecentSpecialPointHandle); putU32(pi.previousWaypoint); putU32(pi.currentWaypoint);
		}
		putU32((unsigned)rb->m_currentPointHandle);
		putU32((unsigned)rb->m_waitAtStationTimer);
		const Bool fl[10] = { rb->m_carriagesCreated, rb->m_hasEverBeenHitched, rb->m_trackDataLoaded, rb->m_waitingInWings, rb->m_endOfLine,
			rb->m_isLocomotive, rb->m_isLeadCarraige, rb->m_disembark, rb->m_inTunnel, rb->m_held };
		unsigned flags = 0;
		for (int j = 0; j < 10; ++j) if (fl[j]) flags |= 1u << j;
		putU32(flags);
		putU32((unsigned)rb->m_wantsToBeLeadCarraige);
		putU32((unsigned)rb->m_conductorState);
		putU32((unsigned)rb->m_anchorWaypointID);
		putU32(rb->m_whistleSound.getPlayingHandle());
		putU32(rb->m_clicketyClackSound.getPlayingHandle());
		putU32(rb->m_runningSound.getPlayingHandle());
		const AudioEventRTS &cc = rb->m_clicketyClackSound;
		putU32(cc.m_hasPos ? realBits(cc.m_pos.x) : 0); putU32(cc.m_hasPos ? realBits(cc.m_pos.y) : 0); putU32(cc.m_hasPos ? realBits(cc.m_pos.z) : 0);
		putU32(cc.m_hasPos ? 1 : 0);
		putU32(cc.m_hasVolume ? realBits(cc.m_volume) : 0);
		TrainTrack *tr = rb->m_track;
		putU32(tr ? 1 : 0);
		if (tr) {
			putU32(tr->m_isLooping ? 1 : 0);
			putU32(realBits(tr->m_length));
			const TrackPointList *pl = tr->getPointList();
			std::vector<unsigned> w;
			for (TrackPointList::const_iterator it = pl->begin(); it != pl->end(); ++it) {
				const TrackPoint &p = *it;
				const unsigned pw[10] = { realBits(p.m_position.x), realBits(p.m_position.y), realBits(p.m_position.z), realBits(p.m_distanceFromPrev),
					realBits(p.m_distanceFromFirst), (unsigned)p.m_handle,
					(unsigned)((p.m_isFirstPoint ? 1 : 0) | (p.m_isLastPoint ? 2 : 0) | (p.m_isTunnelOrBridge ? 4 : 0) | (p.m_isStation ? 8 : 0)
						| (p.m_isDisembark ? 16 : 0) | (p.m_isPingPong ? 32 : 0)), 0, 0, 0 };
				w.insert(w.end(), pw, pw + 7);
			}
			putU32((unsigned)pl->size());
			putU32(r80Hash(w));
		}
	};
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 1 && ((o[1] >= 11 && o[1] <= 23) || (o[1] >= 25 && o[1] <= 27) || o[1] == 29)) { cfg[o[1]] = o[2]; apply(o[1]); }
		if (o[0] == 2) gl.m_frame = o[1];
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[1024];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		Object *objA = r80Find((ObjectID)o[1]);
		RailroadBehavior *ma = objA ? r80Module(objA) : 0;
		Object *objB = (o[2] >= 1) ? r80Find((ObjectID)o[2]) : 0;
		try {
			switch (o[0]) {
			case 0: if (ma) { g_r80Updating = ma; res = (unsigned)ma->update(); } break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 1024, &blen, &crc); g_r80Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r80Saved;
				if (b.size() > 1024) b.resize(1024);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 6: {
				Coord3D loc = objB ? *objB->getPosition() : Coord3D();
				if (!objB) loc.x = loc.y = loc.z = 0.0f;
				Coord3D normal;
				normal.x = 0.0f; normal.y = 0.0f; normal.z = 1.0f;
				if (ma) ma->onCollide(objB, (o[4] & 1) ? 0 : &loc, (o[4] & 2) ? 0 : &normal);
				break;
			}
			case 7: if (ma) res = ma->isRailroad() ? 1 : 0; break;
			case 8: if (ma) ma->setHeld(o[2] != 0); break;
			case 9: if (ma) ma->destroyTheWholeTrainNow(); break;
			case 10: if (ma) ma->disembark(); break;
			case 11: if (ma) ma->makeAWallOutOfThisTrain(o[2] != 0); break;
			case 12: if (ma) res = ma->hasEverBeenHitched() ? 1 : 0; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r80Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32((unsigned)(6 + g_r80Made.size()));
		for (size_t k = 0; k < 6 + g_r80Made.size(); ++k) {
			Object *x = k < 6 ? g_r80Objs[k] : g_r80Made[k - 6];
			const Matrix3D &mx = *x->getTransformMatrix();
			for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) putU32(realBits(mx[r][c]));
			putU32(realBits(x->m_orientation));
			putU32((unsigned)x->getProducerID());
			RailroadBehavior *rb = r80Module(x);
			putU32(rb ? 1 : 0);
			if (rb) putModule(rb);
			if (k == 4 || k == 5) {
				R50PhysicsBehavior *p = phys[k - 4];
				putU32(p ? 1 : 0);
				if (p) {
					const unsigned pw[6] = { realBits(p->m_vel.x), realBits(p->m_vel.y), realBits(p->m_vel.z), realBits(p->m_yawRate), realBits(p->m_pitchRate),
						realBits(p->m_rollRate) };
					for (int j = 0; j < 6; ++j) putU32(pw[j]);
					putU32((unsigned)p->m_flags);
				}
			}
		}
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modDestroyHook = 0;
	g_xformHook = 0;
	g_objXformSyncPos = false;
	g_modFindHook = 0;
	g_findUpdateHook = 0;
	g_teamRelHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_waypointByIdHook = 0;
	g_closestAtHook = 0;
	g_r80First = 0;
	g_r80Candidates.clear();
	g_r34Playing = false;
	for (std::map<const Object *, RailroadBehavior *>::iterator it = g_r80Mods.begin(); it != g_r80Mods.end(); ++it) delete it->second;
	g_r80Mods.clear();
	for (int k = 0; k < 2; ++k) delete phys[k];
	for (int k = 0; k < 6; ++k) {
		Object *x = g_r80Objs[k];
		x->m_body = 0; x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_drawable = 0; x->m_contain = 0; x->m_ai = 0; x->m_containedBy = 0;
		x->m_r50Physics = 0;
		delete x;
		g_r80Objs[k] = 0;
	}
	for (size_t k = 0; k < g_r80Made.size(); ++k) {
		Object *x = g_r80Made[k];
		x->m_body = 0; x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_drawable = 0;
		delete x;
		delete g_r80MadeBodies[k];
		delete g_r80MadeDraws[k];
		delete[] g_r80MadeLists[k];
	}
	g_r80Made.clear();
	g_r80MadeBodies.clear();
	g_r80MadeLists.clear();
	g_r80MadeDraws.clear();
	delete d;
	delete cd;
	g_r80CarData = 0;
	TheGameLogic = 0;
	TheAudio = 0;
	TheAI = 0;
	ThePartitionManager = 0;
	TheTerrainLogic = 0;
	return 0;
}

int ref_l9r80_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r80RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
