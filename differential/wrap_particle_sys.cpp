/* Differential-test wrapper for the particle systems and the FX lists (#46; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp
**   GeneralsMD/Code/GameEngine/Source/Common/INI/INIParticleSys.cpp
**   GeneralsMD/Code/GameEngine/Source/GameClient/FXList.cpp
** compiled with the REAL ParticleSys.h, FXList.h, INI.h, Xfer.h, Snapshot.h, GameCommon.h,
** ClientRandomValue.h, BaseType.h (through differential/shim_psys/Lib) and WWMath's Matrix3D
** and Vector3, on the stand-ins of differential/shim_psys (psys_world.h says what each stands
** in for; their definitions are below).
**
** What is real here (build.rs cuts it VERBATIM into OUT_DIR/psys_gen, each marker asserted to
** occur once):
** - ParticleSys_cut.cpp: ParticleSys.cpp, all of it, but for (1) its include block (from
**   `#include "PreRTS.h"` to `#ifdef _INTERNAL`), replaced by the includes below, (2)
**   ParticleSystemDebugDisplay (the debug display's statistics text, from its comment to
**   angleBetween), which needs the debug display, and (3) ONE mechanical edit for the MSVC dialect,
**   made three times: `for( i=0; i<MAX_KEYFRAMES; i++ )` declares its `int i` (MSVC 6's
**   pre-standard `for` scope let the second loop reuse the first loop's `i`; nothing reads `i`
**   after either loop). Every particle, system, template and manager function, the field parse
**   table, the keyframe parsers, xfer, crc and loadPostProcess are upstream's text.
** - ini_particle_sys.inc: INIParticleSys.cpp from INI::parseParticleSystemDefinition to the end
**   (the function is renamed by shim_psys/PreRTS.h; INI.cpp's block table reaches wrap_ini.cpp's
**   INI::parseParticleSystemDefinition, which forwards through ref_particle_block_hook).
** - psys_save_code.inc: GameState.h's SaveCode enum (the xfer functions throw SC_INVALID_DATA).
** - FXList_cut.cpp: FXList.cpp, all of it, but for its include block (replaced as ParticleSys.cpp's). Its requests
**   reach the stand-ins below, which log them (ref_fx_log); its particle system nugget makes real particle systems.
**   Its INI block reaches FXListStore::parseFXListDefinition through wrap_ini.cpp's ref_fxlist_block_hook.
** - The INI reader and its field parsers (upstream_ini), RandomValue.cpp's client generator
**   (upstream_gameengine: the test sets and reads its state through ref_set_state and
**   ref_get_state), Trig.cpp's Cos, Sin and ACos (upstream_gameengine), the Xfer classes
**   (upstream_xfer), Matrix3D (upstream_wwmath) and the fast_float helpers of BaseType.h
**   (upstream_basetype).
**
** Entry points (extern "C", tests/differential_particle_sys.rs documents the encodings):
** ref_psys_reset, ref_psys_settings, ref_psys_scene, ref_psys_template_dump, ref_psys_create,
** ref_psys_op, ref_psys_update, ref_psys_dump; ref_fx_reset, ref_fx_dump, ref_fx_pos, ref_fx_obj, ref_fx_log. */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <algorithm>
#include <new>
#define private public
#define protected public
#include "PreRTS.h"
#define DEFINE_PARTICLE_SYSTEM_NAMES
#include "Common/INI.h"
#include "Common/Xfer.h"
#include "psys_world.h"
#include "GameClient/ParticleSys.h"
#include "GameClient/FXList.h"
#include "psys_save_code.inc"
#include "ParticleSys_cut.cpp"
#include "ini_particle_sys.inc"
#include "FXList_cut.cpp"
#undef private
#undef protected

/* ------------------------------------------------------------------------- */
/* The stand-ins (psys_world.h)                                               */
/* ------------------------------------------------------------------------- */

namespace {
std::map<UnsignedInt, Object> g_objects;
std::map<UnsignedInt, Drawable> g_drawables;
/* an object's drawable: its own, kept beside it (Object::getDrawable) */
std::map<UnsignedInt, Drawable> g_objectDrawables;
GameClient g_client;
GameLogic g_logic;
TerrainLogic g_terrain;
GlobalData g_global;
GameLODManager g_lod;
Display g_display;

class TestParticleSystemManager : public ParticleSystemManager
{
public:
	virtual Int getOnScreenParticleCount( void ) { return 0; }
	virtual void doParticles(RenderInfoClass &) {}
	virtual void queueParticleRender() {}
};

void setMatrix(Matrix3D *m, const float *v)
{
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c)
			(*m)[r][c] = v[r * 4 + c];
}

std::vector<unsigned> g_out;
void w(unsigned x) { g_out.push_back(x); }
void wf(Real r) { unsigned u; memcpy(&u, &r, 4); g_out.push_back(u); }
void wc(const Coord3D &c) { wf(c.x); wf(c.y); wf(c.z); }
void wstr(const AsciiString &s) { w((unsigned)s.getLength()); for (Int i = 0; i < s.getLength(); ++i) w((unsigned char)s.str()[i]); }
void wrv(const GameClientRandomVariable &v) { w((unsigned)v.getDistributionType()); wf(v.getMinimumValue()); wf(v.getMaximumValue()); }
void wmat(const Matrix3D &m) { for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) wf(m[r][c]); }

/* every field of a ParticleSystemInfo, in the Rust struct's order */
void dumpInfo(const ParticleSystemInfo &i)
{
	w(i.m_isOneShot); w(i.m_shaderType); w(i.m_particleType); wstr(i.m_particleTypeName);
	wrv(i.m_angleZ); wrv(i.m_angularRateZ); wrv(i.m_angularDamping); wrv(i.m_velDamping); wrv(i.m_lifetime);
	w(i.m_systemLifetime);
	wrv(i.m_startSize); wrv(i.m_startSizeRate); wrv(i.m_sizeRate); wrv(i.m_sizeRateDamping);
	w(i.m_volumeParticleDepth);
	for (int k = 0; k < MAX_KEYFRAMES; ++k) { wrv(i.m_alphaKey[k].var); w(i.m_alphaKey[k].frame); }
	for (int k = 0; k < MAX_KEYFRAMES; ++k) { wf(i.m_colorKey[k].color.red); wf(i.m_colorKey[k].color.green); wf(i.m_colorKey[k].color.blue); w(i.m_colorKey[k].frame); }
	wrv(i.m_colorScale); wrv(i.m_burstDelay); wrv(i.m_burstCount); wrv(i.m_initialDelay);
	wc(i.m_driftVelocity); wf(i.m_gravity); wstr(i.m_slaveSystemName); wc(i.m_slavePosOffset); wstr(i.m_attachedSystemName);
	w(i.m_emissionVelocityType); w(i.m_priority);
	wrv(i.m_emissionVelocity.ortho.x); wrv(i.m_emissionVelocity.ortho.y); wrv(i.m_emissionVelocity.ortho.z);
	w(i.m_emissionVolumeType);
	wc(i.m_emissionVolume.line.start); wc(i.m_emissionVolume.line.end);
	w(i.m_isEmissionVolumeHollow); w(i.m_isGroundAligned); w(i.m_isEmitAboveGroundOnly); w(i.m_isParticleUpTowardsEmitter);
	w(i.m_windMotion);
	wf(i.m_windAngle); wf(i.m_windAngleChange); wf(i.m_windAngleChangeMin); wf(i.m_windAngleChangeMax);
	wf(i.m_windMotionStartAngle); wf(i.m_windMotionStartAngleMin); wf(i.m_windMotionStartAngleMax);
	wf(i.m_windMotionEndAngle); wf(i.m_windMotionEndAngleMin); wf(i.m_windMotionEndAngleMax);
	w(i.m_windMotionMovingToEndAngle ? 1 : 0);
}

int finish(unsigned *out, int cap)
{
	int n = (int)g_out.size();
	if (n > cap) return -n;
	memcpy(out, g_out.data(), n * sizeof(unsigned));
	return n;
}

void blockHook(INI *ini) { INI::psysParseParticleSystemDefinition(ini); }
void fxBlockHook(INI *ini) { INI::psysParseFXListDefinition(ini); }
} /* namespace */

/* ---- FXList.cpp's stand-ins */
namespace {
std::vector<unsigned> g_fxLog;
std::map<std::string, ThingTemplate> g_thingTemplates;
std::vector<Drawable *> g_tracers;
std::vector<DrawModule *> g_tracerModules;
ThingFactory g_thingFactory;
GameAudio g_audio;
PlayerList g_players;
PartitionManager g_partition;
View g_view;
NameKeyGenerator g_names;
void fxClear()
{
	for (size_t i = 0; i < g_tracers.size(); ++i) { delete g_tracers[i]; delete g_tracerModules[2 * i]; }
	g_tracers.clear(); g_tracerModules.clear();
}
}
void psysLog(unsigned x) { g_fxLog.push_back(x); }
void psysLogReal(Real r) { unsigned u; memcpy(&u, &r, 4); g_fxLog.push_back(u); }
void psysLogCoord(const Coord3D *c) { psysLogReal(c->x); psysLogReal(c->y); psysLogReal(c->z); }
void psysLogName(const char *s) { size_t n = strlen(s); psysLog((unsigned)n); for (size_t i = 0; i < n; ++i) psysLog((unsigned char)s[i]); }

ThingFactory *TheThingFactory = &g_thingFactory;
GameAudio *TheAudio = &g_audio;
PlayerList *ThePlayerList = &g_players;
PartitionManager *ThePartitionManager = &g_partition;
View *TheTacticalView = &g_view;
NameKeyGenerator *TheNameKeyGenerator = &g_names;

/* the requests, as the Rust test writes its FxRequest list: 1 sound, 2 light pulse, 3 view shake, 4 scorch,
** 5 tracer, 6 ray effect */
void GameAudio::addAudioEvent(const AudioEventRTS *e)
{
	psysLog(1); psysLogName(e->m_name.str()); psysLog(e->m_hasPos ? 1 : 0); if (e->m_hasPos) psysLogCoord(&e->m_pos);
}
void Display::createLightPulse(const Coord3D *pos, const RGBColor *color, Real, Real attenuationWidth, UnsignedInt inc, UnsignedInt dec)
{
	psysLog(2); psysLogCoord(pos); psysLogReal(color->red); psysLogReal(color->green); psysLogReal(color->blue);
	psysLogReal(attenuationWidth); psysLog(inc); psysLog(dec);
}
void View::shake(const Coord3D *epicenter, CameraShakeType shakeType) { psysLog(3); psysLogCoord(epicenter); psysLog((unsigned)shakeType); }
void GameClient::addScorch(const Coord3D *pos, Real radius, Scorches type) { psysLog(4); psysLogCoord(pos); psysLogReal(radius); psysLog((unsigned)type); }
void GameClient::createRayEffectByTemplate(const Coord3D *start, const Coord3D *end, const ThingTemplate *tmpl)
{
	psysLog(6); psysLogName(tmpl->m_name.c_str()); psysLogCoord(start); psysLogCoord(end);
}
const ThingTemplate *ThingFactory::findTemplate(const AsciiString &name, Bool)
{
	ThingTemplate &t = g_thingTemplates[name.str()];
	t.m_name = name.str();
	return &t;
}
Drawable *ThingFactory::newDrawable(const ThingTemplate *tmpl)
{
	Drawable *d = new Drawable();
	memset(d->m_tracer, 0, sizeof(d->m_tracer));
	d->m_template = tmpl;
	DrawModule *m = new DrawModule();
	m->m_tracer.m_owner = d;
	g_tracers.push_back(d);
	g_tracerModules.push_back(m);
	g_tracerModules.push_back(NULL);
	return d;
}
DrawModule **Drawable::getDrawModules()
{
	for (size_t i = 0; i < g_tracers.size(); ++i) if (g_tracers[i] == this) return &g_tracerModules[2 * i];
	abort();
}
void TracerDrawInterface::setTracerParms(Real speed, Real length, Real width, const RGBColor &color, Real)
{
	Real *v = m_owner->m_tracer;
	v[0] = speed; v[1] = length; v[2] = width; v[3] = color.red; v[4] = color.green; v[5] = color.blue;
}
void Drawable::setExpirationDate(UnsignedInt frame)
{
	psysLog(5); psysLogName(m_template->m_name.c_str()); psysLogCoord(&m_pos);
	for (int i = 0; i < 6; ++i) psysLogReal(m_tracer[i]);
	psysLog(frame - TheGameLogic->getFrame());
}
Player *Object::getControllingPlayer() const { return &g_players.m_local; }

GameClient *TheGameClient = &g_client;
GameLogic *TheGameLogic = &g_logic;
TerrainLogic *TheTerrainLogic = &g_terrain;
GlobalData *TheGlobalData = &g_global;
GameLODManager *TheGameLODManager = &g_lod;
Display *TheDisplay = &g_display;

Drawable *GameClient::findDrawableByID(DrawableID id)
{
	std::map<UnsignedInt, Drawable>::iterator it = g_drawables.find((UnsignedInt)id);
	return it == g_drawables.end() ? NULL : &it->second;
}

Object *GameLogic::findObjectByID(ObjectID id)
{
	std::map<UnsignedInt, Object>::iterator it = g_objects.find((UnsignedInt)id);
	return it == g_objects.end() ? NULL : &it->second;
}

/* INI::parseFXList (INI.cpp, in upstream_ini; FXListAtBonePos's `FX`) looks the list up through the global
** TheFXListStore and FXListStore::findFXList under their own names, which are, in a test binary that reads FXList
** fields, the Armor build's stand-ins (wrap_armor.cpp: a distinct pointer for every name, NULL for "None" in any case;
** ref_armor_fx_name maps a pointer back to its name), not this unit's renamed store. So the nugget keeps the name it
** was given, as the Rust does (client::fx_list: FxListAtBonePos names its list); upstream's lookup when the field is
** read (NULL for a list not yet made) is not reproduced. The lists are never played through it: the stand-in
** drawables have no bones. */
extern "C" const char *ref_armor_fx_name(const void *fx);
static void psysBindFXListStore() {}

extern "C" {

extern void (*ref_particle_block_hook)(INI *);

/* A new manager (the old one deleted, with its systems and templates), an empty scene, frames 0,
** ground 0, the settings of ParticleSettings::default() (useFX, scale 1, max 0, field 30, LOD
** priorities 1, not skipped); the INI block hook installed. */
void ref_psys_reset(void)
{
	if (TheParticleSystemManager) delete TheParticleSystemManager;
	TheParticleSystemManager = new TestParticleSystemManager;
	g_objects.clear(); g_drawables.clear(); g_objectDrawables.clear();
	g_client.m_frame = 0; g_logic.m_frame = 0; g_terrain.m_ground = 0.0f;
	g_global.m_useFX = TRUE; g_global.m_particleScale = 1.0f; g_global.m_maxParticleCount = 0; g_global.m_maxFieldParticleCount = 30;
	g_lod.m_minPriority = PARTICLE_PRIORITY_LOWEST; g_lod.m_minSkipPriority = PARTICLE_PRIORITY_LOWEST; g_lod.m_numParticleGenerations = 0; g_lod.m_dynamicParticleSkipMask = 0;
	ref_particle_block_hook = blockHook;
	/* computeParticleVelocity's `static Coord3D newVel` outlives the manager (OUTWARD velocity with a volume type it has
	** no case for keeps it): one call by a system with no velocity type (its default branch) zeroes it, so that every
	** case starts as a new ParticleSystemManager does in the Rust (last_new_vel zero). The system's constructor draws
	** three client random values; the tests set the generator's state after this. */
	ParticleSystemTemplate *t = newInstance(ParticleSystemTemplate)(AsciiString(""));
	ParticleSystem *s = TheParticleSystemManager->createParticleSystem(t, FALSE);
	Coord3D zero; zero.zero();
	s->computeParticleVelocity(&zero);
	s->deleteInstance();
	t->deleteInstance();
	TheParticleSystemManager->m_uniqueSystemID = INVALID_PARTICLE_SYSTEM_ID;
}

/* (#160: `skipMask` and `generations` are the LOD manager's m_dynamicParticleSkipMask and m_numParticleGenerations; the
** count after an update is read back by ref_psys_generations) */
void ref_psys_settings(int useFX, float scale, int maxCount, int maxField, int minPri, int minSkipPri, int skipMask, unsigned fieldCount, int generations)
{
	g_global.m_useFX = useFX != 0; g_global.m_particleScale = scale; g_global.m_maxParticleCount = maxCount;
	g_global.m_maxFieldParticleCount = maxField;
	g_lod.m_minPriority = minPri; g_lod.m_minSkipPriority = minSkipPri; g_lod.m_dynamicParticleSkipMask = skipMask; g_lod.m_numParticleGenerations = generations;
	TheParticleSystemManager->m_fieldParticleCount = fieldCount;
}

/* (#160) the LOD manager's particle count after the last update */
int ref_psys_generations(void) { return g_lod.m_numParticleGenerations; }

/* The scene. kind 0: object `id` (pos 3, transform 12, the drawable's transform 12, the bounding circle radius; iv:
** shroud status, has drawable); kind 1: drawable `id` (pos 3, transform 12, obscured); kind 2: remove object `id`;
** kind 3: remove drawable `id`; kind 4: the ground height (v[0]). */
void ref_psys_scene(int kind, unsigned id, const float *v, const int *iv)
{
	switch (kind) {
	case 0: {
		Object &o = g_objects[id];
		o.m_id = (ObjectID)id; o.m_pos.x = v[0]; o.m_pos.y = v[1]; o.m_pos.z = v[2]; setMatrix(&o.m_transform, v + 3);
		o.m_shroud = (ObjectShroudStatus)iv[0];
		o.m_geometry.m_boundingCircleRadius = v[27];
		if (iv[1]) {
			Drawable &d = g_objectDrawables[id];
			d.m_id = (DrawableID)id; d.m_pos = o.m_pos; setMatrix(&d.m_transform, v + 15); d.m_obscured = FALSE;
			o.m_drawable = &d;
		} else {
			o.m_drawable = NULL;
		}
		break;
	}
	case 1: {
		Drawable &d = g_drawables[id];
		d.m_id = (DrawableID)id; d.m_pos.x = v[0]; d.m_pos.y = v[1]; d.m_pos.z = v[2]; setMatrix(&d.m_transform, v + 3);
		d.m_obscured = iv[0] != 0;
		break;
	}
	case 2: g_objects.erase(id); break;
	case 3: g_drawables.erase(id); break;
	case 4: g_terrain.m_ground = v[0]; break;
	default: abort();
	}
}

/* Every template, in name order: the count, then each name and dumpInfo. */
int ref_psys_template_dump(unsigned *out, int cap)
{
	g_out.clear();
	ParticleSystemManager::TemplateMap &m = TheParticleSystemManager->m_templateMap;
	w((unsigned)m.size());
	for (ParticleSystemManager::TemplateMap::iterator it = m.begin(); it != m.end(); ++it) {
		wstr(it->second->m_name);
		dumpInfo(*it->second);
	}
	return finish(out, cap);
}

/* createParticleSystem(findTemplate(name), createSlaves) at client frame `frame`, attached to
** object `attachTo` (createAttachedParticleSystemID) when it is not 0: the id (0 without the
** template). */
unsigned ref_psys_create(const char *name, int createSlaves, unsigned frame, unsigned attachTo)
{
	g_client.m_frame = frame;
	const ParticleSystemTemplate *t = TheParticleSystemManager->findTemplate(AsciiString(name));
	if (attachTo) {
		Object *o = TheGameLogic->findObjectByID((ObjectID)attachTo);
		return (unsigned)TheParticleSystemManager->createAttachedParticleSystemID(t, o, createSlaves != 0);
	}
	ParticleSystem *s = TheParticleSystemManager->createParticleSystem(t, createSlaves != 0);
	return s ? (unsigned)s->getSystemID() : 0;
}

/* One operation on system `id` (op 12: on object `id`). Returns 0, or 1 when there is no such
** system. */
int ref_psys_op(int op, unsigned id, const float *v, int iv)
{
	if (op == 12) {
		Object *o = TheGameLogic->findObjectByID((ObjectID)id);
		TheParticleSystemManager->destroyAttachedSystems(o);
		return 0;
	}
	if (op == 11) { TheParticleSystemManager->destroyParticleSystemByID((ParticleSystemID)id); return 0; }
	ParticleSystem *s = TheParticleSystemManager->findParticleSystem((ParticleSystemID)id);
	if (!s) return 1;
	switch (op) {
	case 0: { Coord3D c; c.x = v[0]; c.y = v[1]; c.z = v[2]; s->setPosition(&c); break; }
	case 1: { Matrix3D m(true); setMatrix(&m, v); s->setLocalTransform(&m); break; }
	case 2: s->rotateLocalTransformX(v[0]); break;
	case 3: s->rotateLocalTransformY(v[0]); break;
	case 4: s->rotateLocalTransformZ(v[0]); break;
	case 5: s->attachToDrawable(iv ? TheGameClient->findDrawableByID((DrawableID)iv) : NULL); break;
	case 6: s->attachToObject(iv ? TheGameLogic->findObjectByID((ObjectID)iv) : NULL); break;
	case 7: s->start(); break;
	case 8: s->stop(); break;
	case 9: { Coord3D c; c.x = v[0]; c.y = v[1]; c.z = v[2]; s->setVelocityMultiplier(&c); break; }
	case 10: s->setBurstCountMultiplier(v[0]); s->setBurstDelayMultiplier(v[1]); s->setSizeMultiplier(v[2]); break;
	case 13: s->setInitialDelay((UnsignedInt)iv); break;
	case 14: s->setEmissionVolumeSphereRadius(v[0]); break;
	case 15: s->setEmissionVolumeCylinderRadius(v[0]); break;
	case 16: s->tintAllColors((ParticleSystemInfo::Color)iv); break;
	case 17: s->setSkipParentXfrm(iv != 0); break;
	default: abort();
	}
	return 0;
}

/* TheParticleSystemManager->update() at logic frame `logic` and client frame `client`. */
void ref_psys_update(unsigned logic, unsigned client)
{
	g_logic.m_frame = logic;
	g_client.m_frame = client;
	TheParticleSystemManager->update();
}

/* The manager, every system in list order with its particles in its list order, and the overall
** lists of the priorities 1..13 (see the Rust test's `rust_dump` for the words). */
int ref_psys_dump(unsigned *out, int cap)
{
	g_out.clear();
	ParticleSystemManager *m = TheParticleSystemManager;
	w(m->m_particleCount); w((unsigned)m->m_uniqueSystemID); w(m->m_particleSystemCount); w(m->m_lastLogicFrameUpdate);
	w(m->m_fieldParticleCount);
	w((unsigned)m->m_allParticleSystemList.size());
	for (ParticleSystemManager::ParticleSystemListIt it = m->m_allParticleSystemList.begin(); it != m->m_allParticleSystemList.end(); ++it) {
		ParticleSystem *s = *it;
		w((unsigned)s->m_systemID); wstr(s->m_template->m_name);
		dumpInfo(*s);
		w((unsigned)s->m_attachedToDrawableID); w((unsigned)s->m_attachedToObjectID);
		wmat(s->m_localTransform); wmat(s->m_transform);
		w(s->m_burstDelayLeft); w(s->m_delayLeft); w(s->m_startTimestamp); w(s->m_systemLifetimeLeft); w(s->m_personalityStore);
		wf(s->m_accumulatedSizeBonus); wc(s->m_velCoeff); wf(s->m_countCoeff); wf(s->m_delayCoeff); wf(s->m_sizeCoeff);
		wc(s->m_pos); wc(s->m_lastPos);
		w(s->m_slaveSystem ? (unsigned)s->m_slaveSystem->m_systemID : 0);
		w(s->m_masterSystem ? (unsigned)s->m_masterSystem->m_systemID : 0);
		if (s->m_controlParticle) { w(1); w((unsigned)s->m_controlParticle->m_system->m_systemID); w(s->m_controlParticle->m_personality); }
		else { w(0); w(0); w(0); }
		w(s->m_isLocalIdentity); w(s->m_isIdentity); w(s->m_isForever); w(s->m_isStopped); w(s->m_isDestroyed);
		w(s->m_isFirstPos); w(s->m_isSaveable); w(s->m_skipParentXfrm);
		w(s->m_particleCount);
		for (Particle *p = s->m_systemParticlesHead; p; p = p->m_systemNext) {
			w(p->m_personality);
			wc(p->m_vel); wc(p->m_pos); wc(p->m_emitterPos);
			wf(p->m_velDamping); wf(p->m_angleZ); wf(p->m_angularRateZ); wf(p->m_angularDamping);
			w(p->m_lifetime); wf(p->m_size); wf(p->m_sizeRate); wf(p->m_sizeRateDamping);
			for (int k = 0; k < MAX_KEYFRAMES; ++k) { wf(p->m_alphaKey[k].value); w(p->m_alphaKey[k].frame); }
			for (int k = 0; k < MAX_KEYFRAMES; ++k) { wf(p->m_colorKey[k].color.red); wf(p->m_colorKey[k].color.green); wf(p->m_colorKey[k].color.blue); w(p->m_colorKey[k].frame); }
			wf(p->m_colorScale); wf(p->m_windRandomness); w(p->m_particleUpTowardsEmitter);
			wc(p->m_accel); wc(p->m_lastPos); w(p->m_lifetimeLeft); w(p->m_createTimestamp);
			wf(p->m_alpha); wf(p->m_alphaRate); w((unsigned)p->m_alphaTargetKey);
			wf(p->m_color.red); wf(p->m_color.green); wf(p->m_color.blue);
			wf(p->m_colorRate.red); wf(p->m_colorRate.green); wf(p->m_colorRate.blue); w((unsigned)p->m_colorTargetKey);
			w(p->m_isCulled);
			w(p->m_systemUnderControl ? (unsigned)p->m_systemUnderControl->m_systemID : 0);
		}
	}
	for (int pr = PARTICLE_PRIORITY_LOWEST; pr < NUM_PARTICLE_PRIORITIES; ++pr) {
		unsigned n = 0;
		for (Particle *p = m->m_allParticlesHead[pr]; p; p = p->m_overallNext) ++n;
		w(n);
		for (Particle *p = m->m_allParticlesHead[pr]; p; p = p->m_overallNext) { w((unsigned)p->m_system->m_systemID); w(p->m_personality); }
	}
	return finish(out, cap);
}

extern void (*ref_fxlist_block_hook)(INI *);

/* A new FXListStore (TheFXListStore), the name keys, the tracers and the request log emptied; the INI block hook
** installed. (After ref_psys_reset, whose scene and manager it uses.) */
void ref_fx_reset(void)
{
	if (TheFXListStore) delete TheFXListStore;
	TheFXListStore = new FXListStore;
	g_names.m_keys.clear(); g_names.m_names.clear();
	fxClear();
	g_fxLog.clear();
	g_players.m_local.m_index = 0;
	ref_fxlist_block_hook = fxBlockHook;
	psysBindFXListStore();
}

/* Every FX list in name order: the count, then each name, its nugget count and each nugget (1 sound, 2 ray effect,
** 3 tracer, 4 light pulse, 5 view shake, 6 terrain scorch, 7 particle system, 8 FX list at bone positions: the
** Rust test's `fx_dump` for the fields). */
int ref_fx_dump(unsigned *out, int cap)
{
	g_out.clear();
	std::map<std::string, const FXList *> byName;
	for (FXListStore::FXListMap::iterator it = TheFXListStore->m_fxmap.begin(); it != TheFXListStore->m_fxmap.end(); ++it)
		byName[g_names.m_names[it->first]] = &it->second;
	w((unsigned)byName.size());
	for (std::map<std::string, const FXList *>::iterator it = byName.begin(); it != byName.end(); ++it) {
		wstr(AsciiString(it->first.c_str()));
		const FXList *l = it->second;
		w((unsigned)l->m_nuggets.size());
		for (FXList::FXNuggetList::const_iterator n = l->m_nuggets.begin(); n != l->m_nuggets.end(); ++n) {
			FXNugget *g = *n;
			if (SoundFXNugget *s = dynamic_cast<SoundFXNugget *>(g)) { w(1); wstr(s->m_soundName); }
			else if (RayEffectFXNugget *s = dynamic_cast<RayEffectFXNugget *>(g)) { w(2); wstr(s->m_templateName); wc(s->m_primaryOffset); wc(s->m_secondaryOffset); }
			else if (TracerFXNugget *s = dynamic_cast<TracerFXNugget *>(g)) {
				w(3); wstr(s->m_tracerName); wstr(s->m_boneName); wf(s->m_speed); wf(s->m_decayAt); wf(s->m_length); wf(s->m_width);
				wf(s->m_color.red); wf(s->m_color.green); wf(s->m_color.blue); wf(s->m_probability);
			}
			else if (LightPulseFXNugget *s = dynamic_cast<LightPulseFXNugget *>(g)) {
				w(4); wf(s->m_color.red); wf(s->m_color.green); wf(s->m_color.blue); wf(s->m_radius); wf(s->m_boundingCirclePct);
				w(s->m_increaseFrames); w(s->m_decreaseFrames);
			}
			else if (ViewShakeFXNugget *s = dynamic_cast<ViewShakeFXNugget *>(g)) { w(5); w((unsigned)s->m_shake); }
			else if (TerrainScorchFXNugget *s = dynamic_cast<TerrainScorchFXNugget *>(g)) { w(6); w((unsigned)s->m_scorch); wf(s->m_radius); }
			else if (ParticleSystemFXNugget *s = dynamic_cast<ParticleSystemFXNugget *>(g)) {
				w(7); wstr(s->m_name); w((unsigned)s->m_count); wc(s->m_offset); wrv(s->m_radius); wrv(s->m_height); wrv(s->m_delay);
				wf(s->m_rotateX); wf(s->m_rotateY); wf(s->m_rotateZ);
				w(s->m_orientToObject); w(s->m_attachToObject); w(s->m_createAtGroundHeight); w(s->m_useCallersRadius); w(s->m_ricochet);
			}
			else if (FXListAtBonePosFXNugget *s = dynamic_cast<FXListAtBonePosFXNugget *>(g)) {
				/* the list INI::parseFXList found, by its name ("" for none): see psysBindFXListStore */
				const char *fx = s->m_fx ? ref_armor_fx_name(s->m_fx) : "";
				w(8); wstr(AsciiString(fx)); wstr(s->m_boneName); w(s->m_orientToBone);
			}
			else abort();
		}
	}
	return finish(out, cap);
}

/* FXList::doFXPos(TheFXListStore->findFXList(name), primary, mtx or NULL, speed, secondary or NULL, overrideRadius) at
** logic frame `logic` and client frame `client`. */
void ref_fx_pos(const char *name, const float *primary, const float *mtx, float speed, const float *secondary, float overrideRadius, unsigned logic, unsigned client)
{
	g_logic.m_frame = logic; g_client.m_frame = client;
	Coord3D p, s;
	Matrix3D m(true);
	p.x = primary[0]; p.y = primary[1]; p.z = primary[2];
	if (secondary) { s.x = secondary[0]; s.y = secondary[1]; s.z = secondary[2]; }
	if (mtx) setMatrix(&m, mtx);
	FXList::doFXPos(TheFXListStore->findFXList(name), &p, mtx ? &m : NULL, speed, secondary ? &s : NULL, overrideRadius);
}

/* FXList::doFXObj(TheFXListStore->findFXList(name), object `primary` or NULL (0, or no such object), the same for
** `secondary`). */
void ref_fx_obj(const char *name, unsigned primary, unsigned secondary, unsigned logic, unsigned client)
{
	g_logic.m_frame = logic; g_client.m_frame = client;
	FXList::doFXObj(TheFXListStore->findFXList(name), primary ? TheGameLogic->findObjectByID((ObjectID)primary) : NULL,
		secondary ? TheGameLogic->findObjectByID((ObjectID)secondary) : NULL);
}

/* The requests logged since the last call (then emptied). */
int ref_fx_log(unsigned *out, int cap)
{
	g_out = g_fxLog;
	g_fxLog.clear();
	return finish(out, cap);
}

} /* extern "C" */

