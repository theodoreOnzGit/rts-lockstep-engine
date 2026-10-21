/* The driver of the ObjectCreationList differential (p1w10-sonnet): the members of the stand-ins of ocl_standins_decl.h, the world a program
** describes, the store dump and the operations. Included at the end of differential/wrap_ocl.cpp, after the cut ObjectCreationList.cpp.
**
** A program is a list of 32-bit words, read by `Reader`:
**   seed (the game logic random generator), frame, flags, terrain seed,
**   10 thing templates (T0..T8 and GenericDebris, in this order): flags, mass, max speed, preferred height, damage state,
**   the source object (id 100): template index, position (3), orientation, transform (12), unit direction (2), velocity (3), veterancy,
**   name number, layer, state bits (above terrain, significantly above, off map), indicator colour,
**   the number of operations, then each operation: its kind and its arguments (see doOp).
** Template flags: TF_* below. Program flags: PF_* below.
**
** Every call of a stand-in that has an effect writes one line (Rec); calls that only answer write nothing. After every operation the state of
** every object is written (dumpState). The Rust side (tests/differential_object_creation_list.rs, `Sim`) writes the same lines. */
#ifndef OCL_DRIVER_H
#define OCL_DRIVER_H

#include <signal.h>
#include <setjmp.h>

/* Where upstream is undefined (a null pointer dereferenced) the C++ crashes. A SIGSEGV during an operation is caught here, the lines the
** operation had written are dropped, and the program ends with the line "UNDEF op=<k>"; the Rust writes the same line where it returns an
** `is_undefined()` error, so the two must agree on WHERE upstream is undefined. */
static sigjmp_buf g_jmp;
static volatile sig_atomic_t g_inOp = 0;
static void onCrash(int sig)
{
	if (g_inOp) siglongjmp(g_jmp, 1);
	signal(sig, SIG_DFL);
	raise(sig);
}

enum { PF_DEBRIS_SKIPPED = 1, PF_NEUTRAL_PLAYER = 2, PF_NEUTRAL_TEAM = 4, PF_SRC_PLAYER = 8, PF_SRC_ACTIVE = 16, PF_SRC_TEAM = 32 };
enum
{
	TF_CREATABLE = 1 << 0, TF_PHYSICS = 1 << 1, TF_BODY = 1 << 2, TF_CONTAIN = 1 << 3, TF_ACCEPTS = 1 << 4, TF_DRAWABLE = 1 << 5,
	TF_DEBRIS0 = 1 << 6, TF_DEBRIS1 = 1 << 7, TF_AI = 1 << 8, TF_DPAI = 1 << 9, TF_LIFE = 1 << 10, TF_FLOAT = 1 << 11, TF_RDU = 1 << 12,
	TF_SPCD = 1 << 13, TF_SLAVED0 = 1 << 14, TF_SLAVED1 = 1 << 15, TF_TRAINABLE = 1 << 16, TF_STRUCTURE = 1 << 17, TF_HIDDEN = 1 << 18
};
static const int N_TEMPLATES = 10;      /* T0..T8, GenericDebris */
static const int N_WEAPONS = 6;         /* W0..W5 */
static const int N_PSYS = 4;            /* P0..P3 */

/* ---- the world of a program ------------------------------------------------------------------------------------- */
static GlobalData g_globalData; GlobalData* TheGlobalData = &g_globalData;
static GameLODManager g_lod; GameLODManager* TheGameLODManager = &g_lod;
static ThingTemplate g_tmpls[N_TEMPLATES];
static WeaponTemplate g_wts[N_WEAPONS];
static ParticleSystemTemplate g_pts[N_PSYS];
static ParticleSystem g_psys;
static Team g_teams[2];
static Player g_players[2];
static PlayerList g_playerList; PlayerList* ThePlayerList = &g_playerList;
static GameLogic g_gameLogic; GameLogic* TheGameLogic = &g_gameLogic;
static ThingFactory g_factory; ThingFactory* TheThingFactory = &g_factory;
static TerrainLogic g_terrain; TerrainLogic* TheTerrainLogic = &g_terrain;
static Pathfinder g_pathfinder;
static AI g_ai; AI* TheAI = &g_ai;
static PartitionManager g_partition; PartitionManager* ThePartitionManager = &g_partition;
static WeaponStore g_weaponStore; WeaponStore* TheWeaponStore = &g_weaponStore;
static AudioManager g_audio; AudioManager* TheAudio = &g_audio;
static ScriptEngine g_script; ScriptEngine* TheScriptEngine = &g_script;
static ParticleSystemManager g_psm; ParticleSystemManager* TheParticleSystemManager = &g_psm;
static std::vector<Object*> g_objs;      /* creation order; the source object first */
static int g_nextId = 200;
static unsigned g_tseed = 0;

static unsigned h3(unsigned a, unsigned b, unsigned c)
{
	unsigned x = a * 73856093u ^ b * 19349663u ^ c * 83492791u;
	x ^= x >> 13; x *= 0x5bd1e995u; x ^= x >> 15;
	return x;
}
static int oid(const Object* o) { return o ? (int)o->m_id : 0; }
static std::string qs(const AsciiString& s) { const char* c = s.str(); if (!c) c = ""; char b[16]; snprintf(b, sizeof b, "%d:", (int)strlen(c)); return std::string(b) + c; }
static std::string v3(const Coord3D& c) { char b[48]; snprintf(b, sizeof b, "%08x,%08x,%08x", FB(c.x), FB(c.y), FB(c.z)); return b; }
static std::string m12(const Matrix3D& m)
{
	std::string s;
	for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) { char b[16]; snprintf(b, sizeof b, "%s%08x", (r || c) ? "," : "", FB(m[r][c])); s += b; }
	return s;
}
extern "C" const char* ref_armor_fx_name(const void* fx);
static const char* fxName(const FXList* fx) { return fx ? ref_armor_fx_name(fx) : ""; }

/* ---- the stand-ins' members --------------------------------------------------------------------------------------- */
Real Locomotor::getMaxSpeedForCondition(Int damageState) const { return m_maxSpeed + (Real)damageState * 0.25f; }
Real Locomotor::getPreferredHeight() const { return m_prefHeight; }
Int BodyModuleInterface::getDamageState() const { return m_damageState; }
void BodyModuleInterface::setInitialHealth(Int percent) { Rec("body.health %d %d", oid(m_owner), (int)percent); }

void PhysicsBehavior::applyMotiveForce(const Coord3D* f) { Rec("phys.motive %d %s", oid(m_owner), v3(*f).c_str()); }
void PhysicsBehavior::applyForce(const Coord3D* f) { Rec("phys.force %d %s", oid(m_owner), v3(*f).c_str()); }
void PhysicsBehavior::setMass(Real m) { Rec("phys.mass %d %08x", oid(m_owner), FB(m)); m_mass = m; }
void PhysicsBehavior::setExtraFriction(Real f) { Rec("phys.friction %d %08x", oid(m_owner), FB(f)); }
void PhysicsBehavior::setExtraBounciness(Real b) { Rec("phys.bounciness %d %08x", oid(m_owner), FB(b)); }
void PhysicsBehavior::setAllowBouncing(Bool b) { Rec("phys.allowBounce %d %d", oid(m_owner), (int)b); }
void PhysicsBehavior::setAllowToFall(Bool b) { Rec("phys.allowFall %d %d", oid(m_owner), (int)b); }
void PhysicsBehavior::setBounceSound(const AudioEventRTS* s) { Rec("phys.bounceSound %d %s", oid(m_owner), qs(s->m_eventName).c_str()); }
void PhysicsBehavior::setIgnoreCollisionsWith(const Object* o) { Rec("phys.ignore %d %d", oid(m_owner), oid(o)); }
void PhysicsBehavior::setAngles(Real yaw, Real pitch, Real roll) { Rec("phys.angles %d %08x %08x %08x", oid(m_owner), FB(yaw), FB(pitch), FB(roll)); }
void PhysicsBehavior::setYawRate(Real r) { Rec("phys.yawRate %d %08x", oid(m_owner), FB(r)); }
void PhysicsBehavior::setRollRate(Real r) { Rec("phys.rollRate %d %08x", oid(m_owner), FB(r)); }
void PhysicsBehavior::setPitchRate(Real r) { Rec("phys.pitchRate %d %08x", oid(m_owner), FB(r)); }

Bool ContainModuleInterface::isValidContainerFor(const Object*, Bool) const { return m_accepts; }
void ContainModuleInterface::addToContain(Object* obj) { Rec("contain.add %d %d", oid(m_owner), oid(obj)); }
void SlavedUpdateInterface::onEnslave(const Object* slaver) { Rec("slaved.enslave %d %d", oid(m_owner), oid(slaver)); }
void AIUpdateInterface::aiAttackPosition(const Coord3D* pos, Int maxShots, CommandSourceType cmd) { Rec("ai.attack %d %s %d %d", oid(m_owner), v3(*pos).c_str(), (int)maxShots, (int)cmd); }

static std::string decalStr(const RadiusDecalTemplate& d)
{
	char b[160];
	snprintf(b, sizeof b, "shadow=%d min=%08x max=%08x throb=%u color=%08x only=%d", (int)d.m_shadowType, FB(d.m_minOpacity), FB(d.m_maxOpacity), (unsigned)d.m_opacityThrobTime, (unsigned)d.m_color, (int)d.m_onlyVisibleToOwningPlayer);
	return qs(d.m_name) + " " + b;
}
static std::string dataStr(const DeliverPayloadData& d)
{
	char b[512];
	snprintf(b, sizeof b, "dist=%08x preopen=%08x attempts=%d dropOff=%s dropVar=%s dropDelay=%u fire=%d selfDestruct=%d bones=%d diveStart=%08x diveEnd=%08x slot=%d perInterval=%d inherit=%d parachute=%d exitPitch=%08x strafeLen=%08x decalRadius=%08x weapon=%d",
		FB(d.m_distToTarget), FB(d.m_preOpenDistance), (int)d.m_maxAttempts, v3(d.m_dropOffset).c_str(), v3(d.m_dropVariance).c_str(), (unsigned)d.m_dropDelay,
		(int)d.m_fireWeapon, (int)d.m_selfDestructObject, (int)d.m_visibleNumBones, FB(d.m_diveStartDistance), FB(d.m_diveEndDistance), (int)d.m_strafingWeaponSlot,
		(int)d.m_visibleItemsDroppedPerInterval, (int)d.m_inheritTransportVelocity, (int)d.m_isParachuteDirectly, FB(d.m_exitPitchRate), FB(d.m_strafeLength),
		FB(d.m_deliveryDecalRadius), d.m_visiblePayloadWeaponTemplate ? d.m_visiblePayloadWeaponTemplate->m_idx : 0);
	return qs(d.m_visibleDropBoneName) + " " + qs(d.m_visibleSubObjectName) + " " + qs(d.m_visiblePayloadTemplateName) + " fx=" + qs(AsciiString(fxName(d.m_strafeFX))) + " " + b + " decal=" + decalStr(d.m_deliveryDecalTemplate);
}
void DeliverPayloadAIUpdate::deliverPayload(const Coord3D* moveToPos, const Coord3D* targetPos, const DeliverPayloadData* data)
{
	Rec("dpai.deliver %d move=%s target=%s %s", oid(m_owner), v3(*moveToPos).c_str(), v3(*targetPos).c_str(), dataStr(*data).c_str());
}
void LifetimeUpdate::setLifetimeRange(UnsignedInt minFrames, UnsignedInt maxFrames) { Rec("life.range %d %u %u", oid(m_owner), (unsigned)minFrames, (unsigned)maxFrames); }
void FloatUpdate::setEnabled(Bool b) { Rec("float.enable %d %d", oid(m_owner), (int)b); }
void RadiusDecalUpdate::createRadiusDecal(const RadiusDecalTemplate& tmpl, Real radius, const Coord3D& pos) { Rec("rdu.create %d %s r=%08x pos=%s", oid(m_owner), decalStr(tmpl).c_str(), FB(radius), v3(pos).c_str()); }
void RadiusDecalUpdate::killWhenNoLongerAttacking(Bool b) { Rec("rdu.kill %d %d", oid(m_owner), (int)b); }
void SpecialPowerCompletionDie::setCreator(ObjectID id) { Rec("spcd.creator %d %d", oid(m_owner), (int)id); }
void ExperienceTracker::setVeterancyLevel(VeterancyLevel level, Bool) { Rec("exp.vet %d %d", oid(m_owner), (int)level); }
void DebrisDrawInterface::setModelName(AsciiString name, Color color, ShadowType t) { Rec("debris.model %d %d %s color=%08x shadow=%d", oid(m_owner), (int)m_idx, qs(name).c_str(), (unsigned)color, (int)t); }
void DebrisDrawInterface::setAnimNames(AsciiString initial, AsciiString flying, AsciiString final, const FXList* finalFX)
{
	Rec("debris.anim %d %d %s %s %s fx=%s", oid(m_owner), (int)m_idx, qs(initial).c_str(), qs(flying).c_str(), qs(final).c_str(), qs(AsciiString(fxName(finalFX))).c_str());
}
void Drawable::setDrawableHidden(Bool h) { Rec("draw.hidden %d %d", oid(m_owner), (int)h); m_hidden = h; }
void Drawable::fadeIn(UnsignedInt frames) { Rec("draw.fadeIn %d %u", oid(m_owner), (unsigned)frames); }
void Drawable::fadeOut(UnsignedInt frames) { Rec("draw.fadeOut %d %u", oid(m_owner), (unsigned)frames); }

void* Object::findUpdateModule(NameKeyType key)
{
	if (key == TheNameKeyGenerator->nameToKey("RadiusDecalUpdate")) return m_hasRdu ? (void*)&m_rdu : NULL;
	if (key == TheNameKeyGenerator->nameToKey("DeliverPayloadAIUpdate")) return m_hasDpai ? (void*)&m_dpai : NULL;
	if (key == TheNameKeyGenerator->nameToKey("LifetimeUpdate")) return m_hasLife ? (void*)&m_life : NULL;
	if (key == TheNameKeyGenerator->nameToKey("FloatUpdate")) return m_hasFloat ? (void*)&m_float : NULL;
	return NULL;
}
void Object::setProducer(const Object* obj) { Rec("o.producer %d %d", oid(this), oid(obj)); }
void Object::setPosition(const Coord3D* pos) { Rec("o.pos %d %s", oid(this), v3(*pos).c_str()); m_pos = *pos; }
void Object::setOrientation(Real angle) { Rec("o.orient %d %08x", oid(this), FB(angle)); m_orient = angle; }
void Object::setTransformMatrix(const Matrix3D* mtx) { Rec("o.xfrm %d %s", oid(this), m12(*mtx).c_str()); m_xfrm = *mtx; }
void Object::setLayer(PathfindLayerEnum layer) { Rec("o.layer %d %d", oid(this), (int)layer); m_layer = layer; }
void Object::setScriptStatus(ObjectScriptStatusBit bit, Bool set) { Rec("o.script %d %d %d", oid(this), (int)bit, (int)set); }
void Object::setDisabledUntil(DisabledType type, UnsignedInt frame) { Rec("o.disabled %d %d %u", oid(this), (int)type, (unsigned)frame); }
void Object::setWeaponLock(WeaponSlotType slot, WeaponLockType lock) { Rec("o.weaponLock %d %d %d", oid(this), (int)slot, (int)lock); }
void Object::goInvulnerable(UnsignedInt frames) { Rec("o.invulnerable %d %u", oid(this), (unsigned)frames); }
void Object::kill() { Rec("o.kill %d", oid(this)); }
void Object::attemptDamage(DamageInfo* info)
{
	Rec("o.damage %d type=%d death=%d src=%d amount=%08x", oid(this), (int)info->in.m_damageType, (int)info->in.m_deathType, (int)info->in.m_sourceID, FB(info->in.m_amount));
}

const ThingTemplate* ThingFactory::findTemplate(const AsciiString& name, Bool)
{
	for (int i = 0; i < N_TEMPLATES; ++i) if (strcmp(name.str() ? name.str() : "", g_tmpls[i].m_name.str()) == 0) return &g_tmpls[i];
	return NULL;
}

static void initObject(Object* o, const ThingTemplate* t, Team* team)
{
	unsigned f = t->m_flags;
	o->m_tmpl = t;
	o->m_indicatorColor = (Color)(0x11000000u + (unsigned)o->m_id * 257u);
	o->m_team = team;
	o->m_player = team ? team->m_player : NULL;
	o->m_xfrm = Matrix3D(true);
	o->m_layer = LAYER_GROUND;
	o->m_structure = (f & TF_STRUCTURE) != 0;
	o->m_dir2d.x = 0.6f; o->m_dir2d.y = 0.8f; o->m_dir2d.z = 0.25f;
	o->m_hasPhys = (f & TF_PHYSICS) != 0; o->m_phys.m_owner = o; o->m_phys.m_mass = t->m_mass;
	o->m_hasBody = (f & TF_BODY) != 0; o->m_body.m_owner = o; o->m_body.m_damageState = t->m_damageState;
	o->m_hasContain = (f & TF_CONTAIN) != 0; o->m_contain.m_owner = o; o->m_contain.m_accepts = (f & TF_ACCEPTS) != 0;
	o->m_hasDraw = (f & TF_DRAWABLE) != 0; o->m_draw.m_owner = o; o->m_draw.m_hidden = (f & TF_HIDDEN) != 0;
	for (int i = 0; i < 2; ++i)
	{
		o->m_draw.m_modules[i].m_owner = o;
		o->m_draw.m_modules[i].m_hasDebris = (f & (i ? TF_DEBRIS1 : TF_DEBRIS0)) != 0;
		o->m_draw.m_modules[i].m_debris.m_owner = o;
		o->m_draw.m_modules[i].m_debris.m_idx = i;
		o->m_draw.m_modPtrs[i] = &o->m_draw.m_modules[i];
		o->m_mods[i].m_owner = o;
		o->m_mods[i].m_hasSlaved = (f & (i ? TF_SLAVED1 : TF_SLAVED0)) != 0;
		o->m_mods[i].m_slaved.m_owner = o;
		o->m_modPtrs[i] = &o->m_mods[i];
	}
	o->m_draw.m_modPtrs[2] = NULL;
	o->m_modPtrs[2] = NULL;
	o->m_exp.m_owner = o; o->m_exp.m_trainable = (f & TF_TRAINABLE) != 0;
	o->m_hasAi = (f & TF_AI) != 0; o->m_ai.m_owner = o;
	o->m_hasDpai = (f & TF_DPAI) != 0; o->m_dpai.m_owner = o; o->m_dpai.m_loco.m_owner = o;
	o->m_dpai.m_loco.m_maxSpeed = t->m_maxSpeed; o->m_dpai.m_loco.m_prefHeight = t->m_prefHeight;
	o->m_hasLife = (f & TF_LIFE) != 0; o->m_life.m_owner = o;
	o->m_hasFloat = (f & TF_FLOAT) != 0; o->m_float.m_owner = o;
	o->m_hasRdu = (f & TF_RDU) != 0; o->m_rdu.m_owner = o;
	o->m_hasSpcd = (f & TF_SPCD) != 0; o->m_spcd.m_owner = o;
	g_objs.push_back(o);
}
Object* ThingFactory::newObject(const ThingTemplate* tmplate, Team* team)
{
	if (!tmplate || !(tmplate->m_flags & TF_CREATABLE)) return NULL;
	Object* o = new Object();
	o->m_id = (ObjectID)g_nextId++;
	initObject(o, tmplate, team);
	Rec("new %d tmpl=%s team=%d", oid(o), tmplate->m_name.str(), team ? team->m_id : 0);
	return o;
}

ParticleSystemTemplate* ParticleSystemManager::findTemplate(const AsciiString& name) const
{
	for (int i = 0; i < N_PSYS; ++i) { char b[8]; snprintf(b, sizeof b, "P%d", i); if (strcmp(name.str() ? name.str() : "", b) == 0) return &g_pts[i]; }
	return NULL;
}
ParticleSystem* ParticleSystemManager::createParticleSystem(const ParticleSystemTemplate* tmp) { g_psys.m_tmplIdx = tmp->m_idx; return &g_psys; }
void ParticleSystem::attachToObject(Object* obj) { Rec("psys.attach %d tmpl=%d", oid(obj), m_tmplIdx); }
void WeaponStore::createAndFireTempWeapon(const WeaponTemplate* wt, const Object* source, const Coord3D* pos) { Rec("weapon.fire w=%d src=%d pos=%s", wt ? wt->m_idx : 0, oid(source), v3(*pos).c_str()); }
void AudioManager::addAudioEvent(const AudioEventRTS* e) { Rec("audio %s obj=%d", qs(e->m_eventName).c_str(), (int)e->m_obj); }
void ScriptEngine::transferObjectName(const AsciiString& name, Object* obj) { Rec("script.transfer %s %d", qs(name).c_str(), oid(obj)); }
void GameLogic::destroyObject(Object* obj) { Rec("destroy %d", oid(obj)); }

static Real groundAt(Real x, Real y) { return (Real)(h3(FB(x), FB(y), g_tseed) % 400u) * 0.5f; }
Real TerrainLogic::getGroundHeight(Real x, Real y, Coord3D*) const { return groundAt(x, y); }
PathfindLayerEnum TerrainLogic::getHighestLayerForDestination(const Coord3D* pos) const
{
	unsigned k = h3(FB(pos->x), FB(pos->y), g_tseed ^ 0x55u ^ (pos->z > 1000.0f ? 0x1000u : 0u)) % 8u;
	return k <= 4 ? LAYER_GROUND : k == 5 ? (PathfindLayerEnum)2 : k == 6 ? (PathfindLayerEnum)3 : LAYER_WALL;
}
Real TerrainLogic::getLayerHeight(Real x, Real y, PathfindLayerEnum layer) const { return groundAt(x, y) + (Real)(int)layer * 0.5f; }
void TerrainLogic::flattenTerrain(Object* obj) { Rec("terrain.flatten %d", oid(obj)); }
Bool TerrainLogic::isUnderwater(Real x, Real y, Real* waterZ, Real* terrainZ) const
{
	unsigned h = h3(FB(x), FB(y), g_tseed ^ 0x77u);
	if (h % 5u != 0) return false;
	if (waterZ) *waterZ = groundAt(x, y) + (Real)(h / 5u % 16u) * 0.5f;
	if (terrainZ) *terrainZ = groundAt(x, y);
	return true;
}
PathfindCell* Pathfinder::getCell(PathfindLayerEnum layer, Int x, Int y)
{
	static PathfindCell cell;
	unsigned h = h3((unsigned)layer * 7919u + (unsigned)x, (unsigned)y, g_tseed ^ 0x99u) % 11u;
	if (h == 10u) return NULL;
	cell.m_type = (PathfindCell::CellType)(h % 7u);
	return &cell;
}
void Pathfinder::addObjectToPathfindMap(Object* obj) { Rec("pathfind.add %d", oid(obj)); }
Pathfinder* AI::pathfinder() { return &g_pathfinder; }
void PartitionManager::findPositionAround(const Coord3D* c, const FindPositionOptions* o, Coord3D* r)
{
	r->x = c->x + o->minRadius * 0.5f + o->maxRadius * 0.25f;
	r->y = c->y - o->minRadius * 0.25f;
	r->z = c->z + ((o->flags & FPF_USE_HIGHEST_LAYER) ? 1.0f : 0.0f);
}

/* ---- reading a program ------------------------------------------------------------------------------------------- */
struct Reader
{
	const unsigned* w; int n; int p;
	unsigned u() { return p < n ? w[p++] : 0u; }
	int i() { return (int)u(); }
	float f() { unsigned x = u(); float r; memcpy(&r, &x, 4); return r; }
};

static void setupWorld(Reader& r)
{
	for (int i = 0; i < N_WEAPONS; ++i) g_wts[i].m_idx = i + 1;
	for (int i = 0; i < N_PSYS; ++i) g_pts[i].m_idx = i + 1;
	unsigned seed = r.u();
	g_gameLogic.m_frame = r.u();
	unsigned flags = r.u();
	g_tseed = r.u();
	ref_init_game_logic_random(seed);
	g_lod.m_debrisSkipped = (flags & PF_DEBRIS_SKIPPED) != 0;
	for (int i = 0; i < 2; ++i) { g_teams[i].m_id = i + 1; g_teams[i].m_player = &g_players[i]; g_players[i].m_active = true; g_players[i].m_team = &g_teams[i]; }
	g_players[0].m_team = (flags & PF_NEUTRAL_TEAM) ? &g_teams[0] : NULL;
	g_players[1].m_team = (flags & PF_SRC_TEAM) ? &g_teams[1] : NULL;
	g_players[1].m_active = (flags & PF_SRC_ACTIVE) != 0;
	g_playerList.m_neutral = (flags & PF_NEUTRAL_PLAYER) ? &g_players[0] : NULL;
	for (int i = 0; i < N_TEMPLATES; ++i)
	{
		ThingTemplate& t = g_tmpls[i];
		t.m_idx = i;
		char b[16];
		if (i == N_TEMPLATES - 1) snprintf(b, sizeof b, "GenericDebris"); else snprintf(b, sizeof b, "T%d", i);
		t.m_name = AsciiString(b);
		t.m_flags = r.u(); t.m_mass = r.f(); t.m_maxSpeed = r.f(); t.m_prefHeight = r.f(); t.m_damageState = r.i();
	}
	/* the source object */
	Object* s = new Object();
	s->m_id = (ObjectID)100;
	unsigned ti = r.u() % N_TEMPLATES;
	initObject(s, &g_tmpls[ti], &g_teams[1]);
	s->m_pos.x = r.f(); s->m_pos.y = r.f(); s->m_pos.z = r.f();
	s->m_orient = r.f();
	for (int a = 0; a < 3; ++a) for (int c = 0; c < 4; ++c) s->m_xfrm[a][c] = r.f();
	s->m_dir2d.x = r.f(); s->m_dir2d.y = r.f(); s->m_dir2d.z = 0.0f;
	s->m_phys.m_vel.x = r.f(); s->m_phys.m_vel.y = r.f(); s->m_phys.m_vel.z = r.f();
	s->m_veterancy = (VeterancyLevel)(r.u() % 4u);
	unsigned nameNo = r.u();
	if (nameNo) { char b[16]; snprintf(b, sizeof b, "N%u", nameNo); s->m_name = AsciiString(b); }
	s->m_layer = (PathfindLayerEnum)r.i();
	unsigned st = r.u();
	s->m_aboveTerrain = (st & 1) != 0; s->m_significantlyAbove = (st & 2) != 0; s->m_offMap = (st & 4) != 0;
	s->m_indicatorColor = (Color)r.u();
	s->m_player = (flags & PF_SRC_PLAYER) ? &g_players[1] : NULL;
}

static void clearWorld()
{
	for (size_t i = 0; i < g_objs.size(); ++i) delete g_objs[i];
	g_objs.clear();
	g_nextId = 200;
}

static std::vector<std::string> oclNames()
{
	std::vector<std::string> v;
	for (ObjectCreationListStore::ObjectCreationListMap::const_iterator it = TheObjectCreationListStore->m_ocls.begin(); it != TheObjectCreationListStore->m_ocls.end(); ++it)
		v.push_back(TheNameKeyGenerator->keyToName(it->first).str());
	std::sort(v.begin(), v.end());
	return v;
}
static const ObjectCreationList* pickOcl(unsigned k)
{
	std::vector<std::string> v = oclNames();
	if (v.empty()) return NULL;
	return TheObjectCreationListStore->findObjectCreationList(v[k % v.size()].c_str());
}
static Object* sel(unsigned k) { return (k == 0 || g_objs.empty()) ? NULL : g_objs[(k - 1) % g_objs.size()]; }

static void dumpState()
{
	for (size_t i = 0; i < g_objs.size(); ++i)
	{
		const Object* o = g_objs[i];
		Rec("st %d pos=%s or=%08x layer=%d vet=%d mass=%08x hidden=%d xfrm=%s", oid(o), v3(o->m_pos).c_str(), FB(o->m_orient), (int)o->m_layer, (int)o->m_veterancy,
			FB(o->m_phys.m_mass), (int)o->m_draw.m_hidden, m12(o->m_xfrm).c_str());
	}
}

static void doOp(unsigned op, Reader& r)
{
	switch (op)
	{
	case 0:
	{
		unsigned oi = r.u(), ps = r.u(), ss = r.u(), life = r.u();
		const ObjectCreationList* ocl = pickOcl(oi);
		if (!ocl) { Rec("skip"); break; }
		Object* first = ObjectCreationList::create(ocl, sel(ps), sel(ss), (UnsignedInt)life);
		Rec("ret obj first=%d", oid(first));
		break;
	}
	case 1: case 2:
	{
		unsigned oi = r.u(), ps = r.u(), hasP = r.u(); Coord3D p; p.x = r.f(); p.y = r.f(); p.z = r.f();
		unsigned hasS = r.u(); Coord3D s; s.x = r.f(); s.y = r.f(); s.z = r.f();
		unsigned flagOrAngle = r.u(); Real angle = r.f(); unsigned life = r.u();
		const ObjectCreationList* ocl = pickOcl(oi);
		if (!ocl) { Rec("skip"); break; }
		const Coord3D* pp = (hasP & 1) ? &p : NULL;
		const Coord3D* sp = (hasS & 1) ? &s : NULL;
		Object* first;
		if (op == 1) { Bool owner = (flagOrAngle & 1) != 0; first = ObjectCreationList::create(ocl, sel(ps), pp, sp, owner, (UnsignedInt)life); }
		else { first = ObjectCreationList::create(ocl, sel(ps), pp, sp, angle, (UnsignedInt)life); }
		Rec("ret %s first=%d", op == 1 ? "owner" : "angle", oid(first));
		break;
	}
	case 3:
	{
		unsigned ni = r.u();
		std::vector<std::string> v = oclNames();
		v.push_back("None"); v.push_back("none"); v.push_back("Nope"); v.push_back("t0");
		const std::string& name = v[ni % v.size()];
		const ObjectCreationList* o = TheObjectCreationListStore->findObjectCreationList(name.c_str());
		Rec("ret find %s found=%d nuggets=%d", name.c_str(), o ? 1 : 0, o ? (int)o->m_nuggets.size() : 0);
		break;
	}
	default: Rec("ret nop");
	}
}

/* ---- the dump of the store ---------------------------------------------------------------------------------------- */
static std::string dumpText;
static void D(const char* fmt, ...)
{
	char buf[4096];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	dumpText += buf;
	dumpText += '\n';
}
static std::string dumpNugget(const ObjectCreationNugget* n)
{
	char b[2048];
	if (const FireWeaponNugget* f = dynamic_cast<const FireWeaponNugget*>(n))
	{
		snprintf(b, sizeof b, "FireWeapon weapon=%d", f->m_weapon ? f->m_weapon->m_idx : 0);
		return b;
	}
	if (const AttackNugget* a = dynamic_cast<const AttackNugget*>(n))
	{
		snprintf(b, sizeof b, "Attack radius=%08x shots=%d slot=%d decal=", FB(a->m_deliveryDecalRadius), (int)a->m_numberOfShots, (int)a->m_weaponSlot);
		return b + decalStr(a->m_deliveryDecalTemplate);
	}
	if (const ApplyRandomForceNugget* a = dynamic_cast<const ApplyRandomForceNugget*>(n))
	{
		snprintf(b, sizeof b, "Force spin=%08x mag=%08x,%08x pitch=%08x,%08x", FB(a->m_spinRate), FB(a->m_minMag), FB(a->m_maxMag), FB(a->m_minPitch), FB(a->m_maxPitch));
		return b;
	}
	if (const DeliverPayloadNugget* d = dynamic_cast<const DeliverPayloadNugget*>(n))
	{
		snprintf(b, sizeof b, "Deliver spacing=%08x convergence=%08x error=%08x delay=%u size=%u startPref=%d startMax=%d payloads=%d",
			FB(d->m_formationSpacing), FB(d->m_convergenceFactor), FB(d->m_errorRadius), (unsigned)d->m_delayDeliveryFramesMax, (unsigned)d->m_formationSize,
			(int)d->m_startAtPreferredHeight, (int)d->m_startAtMaxSpeed, (int)d->m_payload.size());
		std::string s = std::string(b) + " transport=" + qs(d->m_transportName) + " putIn=" + qs(d->m_putInContainerName);
		for (size_t i = 0; i < d->m_payload.size(); ++i) { char c[32]; snprintf(c, sizeof c, " #%d", (int)d->m_payload[i].m_payloadCount); s += " " + qs(d->m_payload[i].m_payloadName) + c; }
		return s + " data=" + dataStr(d->m_data);
	}
	if (const GenericObjectCreationNugget* g = dynamic_cast<const GenericObjectCreationNugget*>(n))
	{
		std::string s;
		snprintf(b, sizeof b, "Generic areObjects=%d names=%d", (int)g->m_nameAreObjects, (int)g->m_names.size());
		s = b;
		for (size_t i = 0; i < g->m_names.size(); ++i) s += " " + qs(g->m_names[i]);
		snprintf(b, sizeof b, "%d", (int)g->m_animSets.size());
		s += " container=" + qs(g->m_putInContainer) + " anims=" + b;
		for (size_t i = 0; i < g->m_animSets.size(); ++i) s += " " + qs(g->m_animSets[i].m_animInitial) + " " + qs(g->m_animSets[i].m_animFlying) + " " + qs(g->m_animSets[i].m_animFinal);
		s += " fxFinal=" + qs(AsciiString(fxName(g->m_fxFinal))) + " psys=" + qs(g->m_particleSysName) + " fadeSound=" + qs(g->m_fadeSoundName) + " bounce=" + qs(g->m_bounceSound.m_eventName);
		snprintf(b, sizeof b, " count=%d mass=%08x bounciness=%08x friction=%08x offset=%s disposition=%u intensity=%08x spin=%08x yaw=%08x roll=%08x pitch=%08x"
			" mag=%08x,%08x pitchRange=%08x,%08x frames=%u,%u shadow=%d lod=%d invuln=%u health=%08x,%08x fadeFrames=%u dist=%08x,%08x,%08x objectCount=%d"
			" flags=%d%d%d%d%d%d%d%d%d%d%d%d",
			(int)g->m_debrisToGenerate, FB(g->m_mass), FB(g->m_extraBounciness), FB(g->m_extraFriction), v3(g->m_offset).c_str(), (unsigned)g->m_disposition,
			FB(g->m_dispositionIntensity), FB(g->m_spinRate), FB(g->m_yawRate), FB(g->m_rollRate), FB(g->m_pitchRate), FB(g->m_minMag), FB(g->m_maxMag),
			FB(g->m_minPitch), FB(g->m_maxPitch), (unsigned)g->m_minFrames, (unsigned)g->m_maxFrames, (int)g->m_shadowType, (int)g->m_minLODRequired,
			(unsigned)g->m_invulnerableTime, FB(g->m_minHealth), FB(g->m_maxHealth), (unsigned)g->m_fadeFrames, FB(g->m_minDistanceAFormation),
			FB(g->m_minDistanceBFormation), FB(g->m_maxDistanceFormation), (int)g->m_objectCount,
			(int)g->m_requiresLivePlayer, (int)g->m_containInsideSourceObject, (int)g->m_preserveLayer, (int)g->m_okToChangeModelColor, (int)g->m_orientInForceDirection,
			(int)g->m_spreadFormation, (int)g->m_fadeIn, (int)g->m_fadeOut, (int)g->m_ignorePrimaryObstacle, (int)g->m_inheritsVeterancy, (int)g->m_diesOnBadLand,
			(int)g->m_skipIfSignificantlyAirborne);
		return s + b;
	}
	return "Unknown";
}

/* ---- the entry points ------------------------------------------------------------------------------------------- */
extern "C" const void* (*ref_weapon_lookup_hook)(const char*);
static const void* weaponHook(const char* name)
{
	for (int i = 0; i < N_WEAPONS; ++i) { char b[8]; snprintf(b, sizeof b, "W%d", i); if (strcmp(name, b) == 0) { g_wts[i].m_idx = i + 1; return &g_wts[i]; } }
	return NULL;
}

extern "C" {

/* an empty store, the parse-time global data, the name key generator (one for the life of the process: the statics of the cut file cache keys) */
void ref_ocl_reset(int preloadAssets)
{
	static NameKeyGenerator* gen = NULL;
	if (!gen) { gen = new NameKeyGenerator; gen->reset(); }
	TheNameKeyGenerator = gen;
	delete TheObjectCreationListStore;
	TheObjectCreationListStore = new ObjectCreationListStore;
	debrisModelNamesGlobalHack.clear();
	g_globalData.m_preloadAssets = preloadAssets != 0;
	ref_weapon_lookup_hook = weaponHook;
}

/* the store: the global hack list, every nugget in store order, then every list by name with the indices of its nuggets */
int ref_ocl_dump(char* out, int cap)
{
	dumpText.clear();
	const ObjectCreationListStore* st = TheObjectCreationListStore;
	D("store nuggets=%d hack=%d", (int)st->m_nuggets.size(), (int)debrisModelNamesGlobalHack.size());
	for (size_t i = 0; i < debrisModelNamesGlobalHack.size(); ++i) D("hack %s", qs(debrisModelNamesGlobalHack[i]).c_str());
	for (size_t i = 0; i < st->m_nuggets.size(); ++i) D("nugget %d %s", (int)i, dumpNugget(st->m_nuggets[i]).c_str());
	/* the lists by name (a list named "None" cannot be found by name: findObjectCreationList answers NULL for it) */
	std::vector<std::pair<std::string, const ObjectCreationList*> > lists;
	for (ObjectCreationListStore::ObjectCreationListMap::const_iterator it = st->m_ocls.begin(); it != st->m_ocls.end(); ++it)
		lists.push_back(std::make_pair(std::string(TheNameKeyGenerator->keyToName(it->first).str()), &it->second));
	std::sort(lists.begin(), lists.end());
	for (size_t i = 0; i < lists.size(); ++i)
	{
		const std::string& listName = lists[i].first;
		const ObjectCreationList* o = lists[i].second;
		std::string idx;
		for (size_t k = 0; k < o->m_nuggets.size(); ++k)
		{
			int at = -1;
			for (size_t j = 0; j < st->m_nuggets.size(); ++j) if (st->m_nuggets[j] == o->m_nuggets[k]) at = (int)j;
			char b[16]; snprintf(b, sizeof b, " %d", at);
			idx += b;
		}
		D("ocl %s n=%d%s", qs(AsciiString(listName.c_str())).c_str(), (int)o->m_nuggets.size(), idx.c_str());
	}
	if ((int)dumpText.size() <= cap) memcpy(out, dumpText.data(), dumpText.size());
	return (int)dumpText.size();
}

/* The ObjectCreationList differential (tests/differential_object_creation_list.rs): runs a program (the words) against the store as parsed, writes
** the trace lines, each ended by '\n'. Returns the size of the text (written only if it fits `cap`). */
int ref_ocl_run(const unsigned* words, int n, char* out, int cap)
{
	std::vector<std::string> lines;
	g_lines = &lines;
	Reader r; r.w = words; r.n = n; r.p = 0;
	setupWorld(r);
	int nOps = r.i();
	struct sigaction sa, oldSegv, oldBus;
	memset(&sa, 0, sizeof sa);
	sa.sa_handler = onCrash;
	sigemptyset(&sa.sa_mask);
	sa.sa_flags = SA_NODEFER;
	sigaction(SIGSEGV, &sa, &oldSegv);
	sigaction(SIGBUS, &sa, &oldBus);
	for (int k = 0; k < nOps; ++k)
	{
		size_t before = lines.size();
		unsigned op = r.u();
		if (sigsetjmp(g_jmp, 1) == 0)
		{
			g_inOp = 1;
			doOp(op, r);
			g_inOp = 0;
			dumpState();
		}
		else
		{
			g_inOp = 0;
			lines.resize(before);
			Rec("UNDEF op=%d", k);
			break;
		}
	}
	sigaction(SIGSEGV, &oldSegv, NULL);
	sigaction(SIGBUS, &oldBus, NULL);
	std::string all;
	for (size_t i = 0; i < lines.size(); ++i) { all += lines[i]; all += '\n'; }
	clearWorld();
	g_lines = NULL;
	if ((int)all.size() <= cap) memcpy(out, all.data(), all.size());
	return (int)all.size();
}

}  /* extern "C" */

#endif
