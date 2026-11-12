/* Differential-test wrapper for the client update modules of GitHub #165 (upstream commit 0a05454d), cut verbatim by
** build.rs into OUT_DIR/client_update_gen:
**   cu_psid.inc         GameEngine/Include/GameClient/ParticleSys.h, `enum ParticleSystemID` (to MAX_VOLUME_PARTICLE_DEPTH);
**   cu_shroud.inc       GameEngine/Include/Common/GameCommon.h, `enum ObjectShroudStatus` (to `enum GuardMode`);
**   cu_cells.inc        GameEngine/Include/Common/Radar.h, the RADAR_CELL_* enum and `enum RadarEventType` (to
**                       "// PROTOTYPES");
**   cu_frames.inc       GameCommon.h, the LOGICFRAMES_PER_SECOND enum to SECONDS_PER_LOGICFRAME_REAL (inclusive);
**   cu_breeze.inc       GameEngine/Include/GameLogic/ScriptEngine.h, `struct BreezeInfo` (to "// This could belong
**                       elsewhere");
**   cu_module.inc       GameEngine/Source/Common/Thing/Module.cpp, `Module::crc`, `xfer` and `loadPostProcess` (to
**                       ObjectModule's constructor);
**   cu_drawable_module.inc  Module.cpp, `DrawableModule::crc`, `xfer` and `loadPostProcess` (to the file's end);
**   cu_sway_h.inc, cu_sway.inc   GameEngine/Include/GameClient/Module/SwayClientUpdate.h (the class, to the #endif) and
**                       GameEngine/Source/GameClient/Drawable/Update/SwayClientUpdate.cpp (the constructor to the file's
**                       end: updateSway, clientUpdate, crc, xfer, loadPostProcess);
**   cu_apsb_h.inc, cu_apsb.inc   AnimatedParticleSysBoneClientUpdate.h (the class) and .cpp (the constructor to the end);
**   cu_laser_h.inc, cu_laser_data.inc, cu_laser.inc   GameEngine/Include/GameLogic/Module/LaserUpdate.h (both classes,
**                       to the #endif), GameEngine/Source/GameLogic/Object/Update/LaserUpdate.cpp (the data's constructor,
**                       to buildFieldParse; the module's constructor to the file's end);
**   cu_beacon_h.inc, cu_beacon_data.inc, cu_beacon.inc   BeaconClientUpdate.h (both classes) and .cpp (the data's
**                       constructor, to buildFieldParse; the module's constructor to the file's end, xfer included).
** No edit. They are compiled on stand-ins (namespace cu): the module base classes (the declarations of Module.h's, whose
** crc/xfer/loadPostProcess are the cut's own), one drawable and its objects, the particle system manager, the game
** client, the player list, the script engine, the radar and the logic; on the REAL XferSave / XferLoad (upstream_xfer),
** the real GetGameClientRandomValueReal and InitRandom (RandomValue.cpp, upstream_gameengine), the real Cos (Trig.cpp)
** and WWMath's Matrix3D and Vector3. `cosf`/`sinf` are the Rust's petir functions in the test binary.
**
** Entry points (extern "C", tests/differential_client_update.rs; floats travel as their bits):
**   ref_sway_seq(in, nin, out, cap): in = [seed, has drawable, visible, burned], then steps of 8 words [op, a1..a7]:
**     0 clientUpdate; 1 the breeze = {dir x, dir y, intensity, lean, randomness, period, version} (a1..a7; the version
**     a Short); 2 visible = a1; 3 burned = a1; 4 stopSway; 5 loadPostProcess; 6 has drawable = a1. InitRandom(seed)
**     first. After each step [100, curValue, curAngle, curDelta, curAngleLimit, leanAngle, curVersion, swaying, the
**     instance matrix's 12 floats, setInstanceMatrix calls so far].
**   ref_apsb_seq(in, nin, out, cap): in = [has drawable, n (0..4), then per draw module: has an ObjectDrawInterface, its
**     answer], then steps of 3 words [op, a1, a2]: 0 clientUpdate; 1 module a1 answers a2; 2 has drawable = a1. Each
**     updateBonesForClientParticleSystems records [1, index]; after each step [100, m_life].
**   ref_laser_seq(in, nin, out, cap): in = [frame, muzzle name (0 none, 1 "Flare", 2 "NoSuchSystem"), target name (the
**     same), punch scalar, local player (0 none, else index + 1), creation fails], then 4 objects k = 1..4, 8 words each
**     [has drawable, drawable exists, x, y, z, dead, shroud status, has bone "MUZZLE"], then steps of 16 words
**     [op, a1..a15]: 0 clientUpdate; 1 initLaser(parent a1 (0 NULL, else object a1), target a2, startPos a3 ? (a4..a6),
**     endPos a7 ? (a8..a10), bone a11 (0 "", 1 "MUZZLE", 2 "NOBONE"), sizeDeltaFrames a12); 2 setDecayFrames(a1); 3 the
**     frame = a1; 4 object a1's drawable exists = a2; 5 object a1's position = (a2, a3, a4); 6 object a1 dead = a2;
**     7 object a1's shroud status = a2; 8 the manager loses every system; 9 setDirty(a1); 10 getCurrentLaserRadius with a
**     laser draw module a1 of width a2 (records [9, radius]); 11 the local player = a1 (as in the setup); 12 the
**     destructor (the sequence ends). Object k's drawable id is 10 + k; its bone's transform the identity at
**     (x + 1, y + 2, z + 3). Records: [1, the name's FNV] findTemplate; [2, id] a system made; [3, id, x, y, z] a
**     system's setPosition; [4, id] destroyParticleSystemByID; [5, x, y, z] the drawable's setPosition; [6]
**     destroyDrawable (the sequence ends after the step); after each step [100, start xyz, end xyz, parent, target,
**     dirty, systems, widening, decaying, widen frames, width, decay frames, the bone's FNV]. A step undefined upstream
**     (see LaserUpdate's module docs: the wrapper checks the members before the call) records [999] and ends the run, as
**     does sway's (a new breeze version without a drawable).
**   ref_cu_xfer(kind, mode, in, nin, path, out, cap): kind 0 BeaconClientUpdate, 1 SwayClientUpdate,
**     2 AnimatedParticleSysBoneClientUpdate, 3 LaserUpdate; mode 0 saves the module with the members `in` gives to
**     `path` (XferSave), mode 1 loads `path` (XferLoad) into a fresh module. out = [status (0, or the XferStatus + 1),
**     the members]. Members: beacon [system id, last pulse]; sway [curValue, curAngle, curDelta, curAngleLimit,
**     leanAngle, curVersion, swaying]; bones [life]; laser [start xyz, end xyz, dirty, system, target system, widening,
**     decaying, widen start, widen finish, width, decay start, decay finish, parent, target, bone length, its bytes]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stddef.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Lib/trig.h"
#include "Common/AsciiString.h"
#include "Common/GameType.h"
#include "Common/RandomValue.h"
#include "GameClient/ClientRandomValue.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "matrix3d.h"
#include "vector3.h"

/* Libraries/Include/Lib/BaseType.h:86 (the shim's BaseType.h does not carry it) */
#ifndef PI
#define PI 3.14159265359f
#endif

namespace cu {

std::vector<int> g_out;
void rec(const std::vector<int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }
int bits(float f)
{
	int i;
	memcpy(&i, &f, 4);
	return i;
}
float real(int i)
{
	float f;
	memcpy(&f, &i, 4);
	return f;
}
int fnv(const char *s)
{
	unsigned int h = 2166136261u;
	for (; *s; ++s) { h ^= (unsigned char)*s; h *= 16777619u; }
	return (int)h;
}

#include "cu_psid.inc"
#include "cu_shroud.inc"
#include "cu_cells.inc"
#include "cu_frames.inc"
#include "cu_breeze.inc"

#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( cls, name ) public: virtual ~cls();
#define MAKE_STANDARD_MODULE_MACRO( cls ) \
protected: \
	virtual void crc( Xfer *xfer ); \
	virtual void xfer( Xfer *xfer ); \
	virtual void loadPostProcess( void );
#define MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( cls, clsmd ) \
	MAKE_STANDARD_MODULE_MACRO(cls) \
private: \
	const clsmd* get##clsmd() const { return (clsmd*)getModuleData(); }
#define DEBUG_ASSERTCRASH( c, m )
#define DEBUG_CRASH( m )
class MultiIniFieldParse;
class Thing {};
class ModuleData { public: virtual ~ModuleData() {} };
class ClientUpdateModuleData : public ModuleData {};

/* ---- the world the modules see ---- */
struct CuObj { Bool hasDrawable, exists, dead, hasBone; Coord3D pos; Int shroud; };
struct CuWorld
{
	UnsignedInt frame;
	Bool hasDrawable, visible, burned, createFails, lost, destroyed;
	Int localPlayer;          /* 0: NULL, else the index + 1 */
	Int nextId;
	Int instanceSets;
	CuObj obj[5];
	Bool apsbHasDrawable;
	Int nModules, modInterface[4], modAnswer[4];
	Bool laserDraw;
	Real laserWidth;
	/* the beacon's */
	Int color;
};
CuWorld g_w;
BreezeInfo g_breeze;

class ObjectStatusMaskType { public: Bool test( Int ) const { return g_w.burned; } };
enum { OBJECT_STATUS_BURNED = 1 };
class Drawable;
class Object
{
public:
	Int m_k;
	ObjectStatusMaskType getStatusBits() const { return ObjectStatusMaskType(); }
	Bool isEffectivelyDead() const { return g_w.obj[m_k].dead; }
	Drawable *getDrawable() const;
	ObjectShroudStatus getShroudedStatus( Int ) const { return (ObjectShroudStatus)g_w.obj[m_k].shroud; }
	const Coord3D *getPosition() const { return &g_w.obj[m_k].pos; }
	Int getIndicatorColor() const { return g_w.color; }
};
Object g_objects[5];
class LaserDrawInterface { public: Real getLaserTemplateWidth() const { return g_w.laserWidth; } };
LaserDrawInterface g_ldi;
class ObjectDrawInterface
{
public:
	Int m_i;
	Bool updateBonesForClientParticleSystems() { rec({1, m_i}); return g_w.modAnswer[m_i] != 0; }
};
ObjectDrawInterface g_odi[4];
class DrawModule
{
public:
	Int m_i;
	ObjectDrawInterface *getObjectDrawInterface() { return g_w.modInterface[m_i] ? &g_odi[m_i] : NULL; }
	const LaserDrawInterface *getLaserDrawInterface() const { return g_w.laserDraw ? &g_ldi : NULL; }
};
DrawModule g_modules[4];
DrawModule *g_moduleList[5];
class Template { public: AsciiString getName() const { return AsciiString("T"); } };
class Drawable
{
public:
	Int m_k;   /* 0: the module's own drawable; else object k's */
	Matrix3D m_instance;
	DrawableID getID() const { return (DrawableID)(10 + m_k); }
	Object *getObject() const { return &g_objects[m_k]; }
	const Coord3D *getPosition() const { return &g_w.obj[m_k].pos; }
	const Template *getTemplate() const { static Template t; return &t; }
	Bool getCurrentWorldspaceClientBonePositions( const char *bone, Matrix3D &m ) const
	{
		if (!g_w.obj[m_k].hasBone || strcmp(bone, "MUZZLE") != 0) return FALSE;
		const Coord3D &p = g_w.obj[m_k].pos;
		m.Make_Identity();
		m.Set_Translation(Vector3(p.x + 1.0f, p.y + 2.0f, p.z + 3.0f));
		return TRUE;
	}
	DrawModule **getDrawModules() { return g_moduleList; }
	const DrawModule **getDrawModules() const { return (const DrawModule **)g_moduleList; }
	void setPosition( const Coord3D *p ) { rec({5, bits(p->x), bits(p->y), bits(p->z)}); }
	Bool isVisible() { return g_w.visible; }
	const Matrix3D *getInstanceMatrix() const { return &m_instance; }
	void setInstanceMatrix( const Matrix3D *m ) { m_instance = *m; ++g_w.instanceSets; }
	/* the beacon's */
	void setDrawableHidden( Bool ) {}
	void setShadowsEnabled( Bool ) {}
	Bool isDrawableEffectivelyHidden() const { return FALSE; }
};
Drawable g_drawables[5];
Drawable *Object::getDrawable() const { return g_w.obj[m_k].hasDrawable ? &g_drawables[m_k] : NULL; }
class GameClient
{
public:
	Drawable *findDrawableByID( DrawableID id )
	{
		Int k = (Int)id - 10;
		if (k < 1 || k > 4 || !g_w.obj[k].hasDrawable || !g_w.obj[k].exists) return NULL;
		return &g_drawables[k];
	}
	void destroyDrawable( Drawable *d ) { if (d != &g_drawables[0]) abort(); rec({6}); g_w.destroyed = TRUE; }
};
GameClient g_client;
GameClient *TheGameClient = &g_client;
class Player { public: Int m_index; Int getPlayerIndex() const { return m_index; } };
Player g_player;
class PlayerList { public: Player *getLocalPlayer() { if (!g_w.localPlayer) return NULL; g_player.m_index = g_w.localPlayer - 1; return &g_player; } };
PlayerList g_players;
PlayerList *ThePlayerList = &g_players;
class ScriptEngine { public: const BreezeInfo &getBreezeInfo() const { return g_breeze; } };
ScriptEngine g_script;
ScriptEngine *TheScriptEngine = &g_script;
class GameLogic { public: UnsignedInt getFrame() { return g_w.frame; } };
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
class ParticleSystemTemplate { public: Int m_n; };
ParticleSystemTemplate g_tmpl;
class ParticleSystem
{
public:
	ParticleSystemID m_id;
	ParticleSystemID getSystemID() const { return m_id; }
	void setPosition( const Coord3D *p ) { rec({3, (int)m_id, bits(p->x), bits(p->y), bits(p->z)}); }
	void attachToDrawable( const Drawable * ) {}
	void tintAllColors( Int ) {}
	void stop() {}
};
ParticleSystem g_systems[64];
class ParticleSystemManager
{
public:
	const ParticleSystemTemplate *findTemplate( const AsciiString &name ) const
	{
		rec({1, fnv(name.str())});
		return strcmp(name.str(), "Flare") == 0 ? &g_tmpl : NULL;
	}
	ParticleSystem *createParticleSystem( const ParticleSystemTemplate *t, Bool createSlaves = TRUE )
	{
		if (!createSlaves || t == NULL) abort();
		if (g_w.createFails || g_w.nextId >= 63) return NULL;
		ParticleSystem *s = &g_systems[++g_w.nextId];
		s->m_id = (ParticleSystemID)g_w.nextId;
		rec({2, g_w.nextId});
		return s;
	}
	ParticleSystem *findParticleSystem( ParticleSystemID id )
	{
		if (g_w.lost || (Int)id <= 0 || (Int)id > g_w.nextId) return NULL;
		return &g_systems[(Int)id];
	}
	void destroyParticleSystemByID( ParticleSystemID id ) { rec({4, (int)id}); }
};
ParticleSystemManager g_psm;
ParticleSystemManager *TheParticleSystemManager = &g_psm;
class Radar { public: void createEvent( const Coord3D *, RadarEventType, Real = 4.0f ) {} };
Radar g_radar;
Radar *TheRadar = &g_radar;

/* ---- the module bases (Module.h's declarations; the definitions are Module.cpp's cuts) ---- */
class Module
{
public:
	Module( const ModuleData *moduleData ) : m_moduleData(moduleData) {}
	virtual ~Module() {}
	const ModuleData *getModuleData() const { return m_moduleData; }
protected:
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
private:
	const ModuleData *m_moduleData;
};
class DrawableModule : public Module
{
public:
	DrawableModule( Thing *, const ModuleData *moduleData ) : Module(moduleData) {}
	Drawable *getDrawable() const { return g_w.hasDrawable ? &g_drawables[0] : NULL; }
protected:
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
};
class ClientUpdateModule : public DrawableModule
{
public:
	ClientUpdateModule( Thing *thing, const ModuleData *moduleData ) : DrawableModule(thing, moduleData) {}
	virtual void clientUpdate() = 0;
};
#include "cu_module.inc"
#include "cu_drawable_module.inc"

#include "cu_sway_h.inc"
#include "cu_sway.inc"
#include "cu_apsb_h.inc"
#include "cu_apsb.inc"
#include "cu_laser_h.inc"
#include "cu_laser_data.inc"
#include "cu_laser.inc"
#include "cu_beacon_h.inc"
#include "cu_beacon_data.inc"
#include "cu_beacon.inc"

/* the members, through derived classes (they are protected) */
struct SwayPeek : public SwayClientUpdate
{
	static SwayPeek *p(SwayClientUpdate *s) { return (SwayPeek *)s; }
	void put(const int *m) { m_curValue = real(m[0]); m_curAngle = real(m[1]); m_curDelta = real(m[2]); m_curAngleLimit = real(m[3]); m_leanAngle = real(m[4]); m_curVersion = (Short)m[5]; m_swaying = m[6] != 0; }
	void get() { rec({bits(m_curValue), bits(m_curAngle), bits(m_curDelta), bits(m_curAngleLimit), bits(m_leanAngle), (int)m_curVersion, m_swaying ? 1 : 0}); }
	void doXfer(Xfer *x) { xfer(x); }
	void doLoadPostProcess() { loadPostProcess(); }
	Bool swaying() const { return m_swaying; }
	Short version() const { return m_curVersion; }
};
struct ApsbPeek : public AnimatedParticleSysBoneClientUpdate
{
	static ApsbPeek *p(AnimatedParticleSysBoneClientUpdate *s) { return (ApsbPeek *)s; }
	void put(const int *m) { m_life = (UnsignedInt)m[0]; }
	void get() { rec({(int)m_life}); }
	void doXfer(Xfer *x) { xfer(x); }
};
struct LaserPeek : public LaserUpdate
{
	static LaserPeek *p(LaserUpdate *s) { return (LaserPeek *)s; }
	void put(const int *m, int n)
	{
		m_startPos.x = real(m[0]); m_startPos.y = real(m[1]); m_startPos.z = real(m[2]);
		m_endPos.x = real(m[3]); m_endPos.y = real(m[4]); m_endPos.z = real(m[5]);
		m_dirty = m[6] != 0;
		m_particleSystemID = (ParticleSystemID)m[7];
		m_targetParticleSystemID = (ParticleSystemID)m[8];
		m_widening = m[9] != 0; m_decaying = m[10] != 0;
		m_widenStartFrame = (UnsignedInt)m[11]; m_widenFinishFrame = (UnsignedInt)m[12];
		m_currentWidthScalar = real(m[13]);
		m_decayStartFrame = (UnsignedInt)m[14]; m_decayFinishFrame = (UnsignedInt)m[15];
		m_parentID = (DrawableID)m[16]; m_targetID = (DrawableID)m[17];
		std::string b;
		for (int i = 0; i < m[18] && 19 + i < n; ++i) b.push_back((char)m[19 + i]);
		m_parentBoneName = AsciiString(b.c_str());
	}
	void get(bool raw)
	{
		rec({bits(m_startPos.x), bits(m_startPos.y), bits(m_startPos.z), bits(m_endPos.x), bits(m_endPos.y), bits(m_endPos.z), m_dirty ? 1 : 0,
			(int)m_particleSystemID, (int)m_targetParticleSystemID, m_widening ? 1 : 0, m_decaying ? 1 : 0, (int)m_widenStartFrame,
			(int)m_widenFinishFrame, bits(m_currentWidthScalar), (int)m_decayStartFrame, (int)m_decayFinishFrame, (int)m_parentID, (int)m_targetID});
		const char *s = m_parentBoneName.str();
		if (raw) {
			rec({(int)strlen(s)});
			for (; *s; ++s) rec({(int)(unsigned char)*s});
		} else {
			rec({fnv(s)});
		}
	}
	void doXfer(Xfer *x) { xfer(x); }
	Int system() const { return (Int)m_particleSystemID; }
};
struct BeaconPeek : public BeaconClientUpdate
{
	static BeaconPeek *p(BeaconClientUpdate *s) { return (BeaconPeek *)s; }
	void put(const int *m) { m_particleSystemID = (ParticleSystemID)m[0]; m_lastRadarPulse = (UnsignedInt)m[1]; }
	void get() { rec({(int)m_particleSystemID, (int)m_lastRadarPulse}); }
	void doXfer(Xfer *x) { xfer(x); }
};

int copy_out(int *out, int cap)
{
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(out, &g_out[0], n * sizeof(int));
	return (int)g_out.size();
}

void fresh()
{
	memset(&g_w, 0, sizeof g_w);
	memset(&g_breeze, 0, sizeof g_breeze);
	for (int k = 0; k < 5; ++k) {
		g_objects[k].m_k = k;
		g_drawables[k].m_k = k;
		g_drawables[k].m_instance.Make_Identity();
	}
	for (int i = 0; i < 4; ++i) { g_modules[i].m_i = i; g_odi[i].m_i = i; }
	g_out.clear();
}

} /* namespace cu */

extern "C" {

int ref_sway_seq(const int *in, int nin, int *out, int cap)
{
	using namespace cu;
	fresh();
	InitRandom((UnsignedInt)in[0]);
	g_w.hasDrawable = in[1] != 0;
	g_w.visible = in[2] != 0;
	g_w.burned = in[3] != 0;
	ModuleData data;
	Thing thing;
	SwayClientUpdate *s = new SwayClientUpdate(&thing, &data);
	for (int st = 4; st + 7 < nin; st += 8) {
		const int *a = in + st;
		switch (a[0]) {
		case 0:
			/* undefined upstream: a new breeze version without a drawable reads the instance matrix through NULL */
			if (!g_w.hasDrawable && SwayPeek::p(s)->swaying() && SwayPeek::p(s)->version() != g_breeze.m_breezeVersion) {
				rec({999});
				return copy_out(out, cap);
			}
			s->clientUpdate();
			break;
		case 1:
			g_breeze.m_directionVec.x = real(a[1]); g_breeze.m_directionVec.y = real(a[2]);
			g_breeze.m_intensity = real(a[3]); g_breeze.m_lean = real(a[4]); g_breeze.m_randomness = real(a[5]);
			g_breeze.m_breezePeriod = (Short)a[6]; g_breeze.m_breezeVersion = (Short)a[7];
			break;
		case 2: g_w.visible = a[1] != 0; break;
		case 3: g_w.burned = a[1] != 0; break;
		case 4: s->stopSway(); break;
		case 5: SwayPeek::p(s)->doLoadPostProcess(); break;
		default: g_w.hasDrawable = a[1] != 0; break;
		}
		rec({100});
		SwayPeek::p(s)->get();
		const Matrix3D &m = g_drawables[0].m_instance;
		for (int r = 0; r < 3; ++r)
			for (int c = 0; c < 4; ++c)
				rec({bits(m[r][c])});
		rec({g_w.instanceSets});
	}
	delete s;
	return copy_out(out, cap);
}

int ref_apsb_seq(const int *in, int nin, int *out, int cap)
{
	using namespace cu;
	fresh();
	g_w.hasDrawable = in[0] != 0;
	g_w.nModules = in[1];
	for (int i = 0; i < g_w.nModules; ++i) {
		g_w.modInterface[i] = in[2 + 2 * i];
		g_w.modAnswer[i] = in[3 + 2 * i];
		g_moduleList[i] = &g_modules[i];
	}
	g_moduleList[g_w.nModules] = NULL;
	ModuleData data;
	Thing thing;
	AnimatedParticleSysBoneClientUpdate *a = new AnimatedParticleSysBoneClientUpdate(&thing, &data);
	for (int st = 2 + 2 * g_w.nModules; st + 2 < nin; st += 3) {
		const int *s = in + st;
		switch (s[0]) {
		case 0: a->clientUpdate(); break;
		case 1: g_w.modAnswer[s[1]] = s[2]; break;
		default: g_w.hasDrawable = s[1] != 0; break;
		}
		rec({100});
		ApsbPeek::p(a)->get();
	}
	delete a;
	return copy_out(out, cap);
}

static const char *laser_name(int k) { return k == 1 ? "Flare" : k == 2 ? "NoSuchSystem" : ""; }

int ref_laser_seq(const int *in, int nin, int *out, int cap)
{
	using namespace cu;
	fresh();
	g_w.frame = (UnsignedInt)in[0];
	LaserUpdateModuleData defaults;
	LaserUpdateModuleData data;
	data.m_particleSystemName = AsciiString(laser_name(in[1]));
	data.m_targetParticleSystemName = AsciiString(laser_name(in[2]));
	data.m_punchThroughScalar = real(in[3]);
	g_w.localPlayer = in[4];
	g_w.createFails = in[5] != 0;
	g_w.hasDrawable = TRUE;
	for (int k = 1; k <= 4; ++k) {
		const int *o = in + 6 + 8 * (k - 1);
		CuObj &c = g_w.obj[k];
		c.hasDrawable = o[0] != 0; c.exists = o[1] != 0;
		c.pos.x = real(o[2]); c.pos.y = real(o[3]); c.pos.z = real(o[4]);
		c.dead = o[5] != 0; c.shroud = o[6]; c.hasBone = o[7] != 0;
	}
	g_moduleList[0] = &g_modules[0];
	g_moduleList[1] = NULL;
	rec({200, bits(defaults.m_punchThroughScalar), (int)defaults.m_particleSystemName.getLength(), (int)defaults.m_targetParticleSystemName.getLength()});
	Thing thing;
	LaserUpdate *l = new LaserUpdate(&thing, &data);
	bool gone = false;
	for (int st = 38; st + 15 < nin && !gone; st += 16) {
		const int *a = in + st;
		switch (a[0]) {
		case 0: l->clientUpdate(); break;
		case 1: {
			/* undefined upstream: NULL read through (the local player with a parent, before the systems are made;
			** posToUse.add(endPos) without a parent, with a startPos and a target, and no endPos) */
			if ((a[1] && (a[2] || a[7]) && !g_w.localPlayer && LaserPeek::p(l)->system() == 0) || (!a[1] && a[3] && a[2] && !a[7])) {
				rec({999});
				return copy_out(out, cap);
			}
			const Object *parent = a[1] ? &g_objects[a[1]] : NULL;
			const Object *target = a[2] ? &g_objects[a[2]] : NULL;
			Coord3D sp, ep;
			sp.x = real(a[4]); sp.y = real(a[5]); sp.z = real(a[6]);
			ep.x = real(a[8]); ep.y = real(a[9]); ep.z = real(a[10]);
			const char *bones[3] = { "", "MUZZLE", "NOBONE" };
			l->initLaser(parent, target, a[3] ? &sp : NULL, a[7] ? &ep : NULL, AsciiString(bones[a[11]]), a[12]);
			break;
		}
		case 2: l->setDecayFrames((UnsignedInt)a[1]); break;
		case 3: g_w.frame = (UnsignedInt)a[1]; break;
		case 4: g_w.obj[a[1]].exists = a[2] != 0; break;
		case 5: g_w.obj[a[1]].pos.x = real(a[2]); g_w.obj[a[1]].pos.y = real(a[3]); g_w.obj[a[1]].pos.z = real(a[4]); break;
		case 6: g_w.obj[a[1]].dead = a[2] != 0; break;
		case 7: g_w.obj[a[1]].shroud = a[2]; break;
		case 8: g_w.lost = TRUE; break;
		case 9: l->setDirty(a[1] != 0); break;
		case 10:
			g_w.laserDraw = a[1] != 0;
			g_w.laserWidth = real(a[2]);
			rec({9, bits(l->getCurrentLaserRadius())});
			break;
		case 11: g_w.localPlayer = a[1]; break;
		default:
			delete l;
			l = NULL;
			gone = true;
			break;
		}
		if (g_w.destroyed)
			gone = true;
		if (l) {
			rec({100});
			LaserPeek::p(l)->get(false);
		}
	}
	if (l) delete l;
	return copy_out(out, cap);
}

int ref_cu_xfer(int kind, int mode, const int *in, int nin, const char *path, int *out, int cap)
{
	using namespace cu;
	fresh();
	ModuleData data;
	LaserUpdateModuleData laserData;
	BeaconClientUpdateModuleData beaconData;
	Thing thing;
	g_w.frame = 7;
	SwayClientUpdate *sway = NULL;
	AnimatedParticleSysBoneClientUpdate *apsb = NULL;
	LaserUpdate *laser = NULL;
	BeaconClientUpdate *beacon = NULL;
	switch (kind) {
	case 0: beacon = new BeaconClientUpdate(&thing, &beaconData); if (mode == 0) BeaconPeek::p(beacon)->put(in); break;
	case 1: sway = new SwayClientUpdate(&thing, &data); if (mode == 0) SwayPeek::p(sway)->put(in); break;
	case 2: apsb = new AnimatedParticleSysBoneClientUpdate(&thing, &data); if (mode == 0) ApsbPeek::p(apsb)->put(in); break;
	default: laser = new LaserUpdate(&thing, &laserData); if (mode == 0) LaserPeek::p(laser)->put(in, nin); break;
	}
	int status = 0;
	XferSave xs;
	XferLoad xl;
	Xfer *x = mode == 0 ? (Xfer *)&xs : (Xfer *)&xl;
	try {
		x->open(AsciiString(path));
		switch (kind) {
		case 0: BeaconPeek::p(beacon)->doXfer(x); break;
		case 1: SwayPeek::p(sway)->doXfer(x); break;
		case 2: ApsbPeek::p(apsb)->doXfer(x); break;
		default: LaserPeek::p(laser)->doXfer(x); break;
		}
		x->close();
	} catch (XferStatus s) {
		status = (int)s + 1;
		try { x->close(); } catch (...) {}
	}
	g_out.clear();
	rec({status});
	switch (kind) {
	case 0: BeaconPeek::p(beacon)->get(); delete beacon; break;
	case 1: SwayPeek::p(sway)->get(); delete sway; break;
	case 2: ApsbPeek::p(apsb)->get(); delete apsb; break;
	default: {
		/* the destructor's destroyParticleSystemByID calls are not part of the load's words */
		LaserPeek::p(laser)->get(true);
		size_t n = g_out.size();
		delete laser;
		g_out.resize(n);
		break;
	}
	}
	return copy_out(out, cap);
}

} /* extern "C" */
