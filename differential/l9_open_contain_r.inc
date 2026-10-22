/* Differential-test driver for OpenContain (p1w13-sonnet, batch R; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/OpenContain.cpp   (#included through OpenContain_edit.cpp: one mechanical edit made by build.rs, the end-iterator read of processDamageToContained) with its header, in wrap_module.cpp's unit on the stand-ins of
**   differential/shim_module: up to four Objects, each with a team and a player, a drawable, an AI, a physics block, a stealth update and a body that log what is
**   done to them, and OpenContain modules on those that have one (the first reads the INI, the others take their data from the scene), the REAL DieMuxData, INI,
**   RandomValue, Matrix3D, Xfer, Object::kill and Object::getRelationship. Every call the container makes on the rest of the world is a line of the effect log
**   (ulog), in the order it is made, in the text the Rust side's cut records, events and probe traces produce. */
#include "Lib/trig.h"   /* Cos, Sin (Trig.cpp, compiled by wrap_trig.cpp) */
extern void (*g_xformHook)(Object *o, const Matrix3D *mx);   /* l9_replace_object.inc */
extern Int (*g_multiBoneHook)(const Object *obj, const char *boneNamePrefix, Int maxBones, Coord3D *positions, Matrix3D *transforms, Bool convertToWorld);   /* l9_active_body_p.inc */
/* OpenContain.cpp calls the Microsoft C runtime's itoa */
static char *ctItoa(int value, char *buffer, int radix)
{
	(void)radix;
	sprintf(buffer, "%d", value);
	return buffer;
}
#define itoa(v, s, r) ctItoa(v, s, r)
#include "OpenContain_edit.cpp"  /* OpenContain.cpp with one mechanical edit made by build.rs */
#undef itoa

namespace {

ModuleData *l9ctRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = OpenContain::friend_newModuleData(&ini);
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

enum { CT_OBJS = 4, CT_PLAYERS = 4, CT_BONES = 8, CT_FP = 4 };

/* the kinds, statuses and model conditions the scene's masks name: local bit i is kCtKinds[i] (the Rust test has the same lists) */
const KindOfType kCtKinds[8] = { KINDOF_STEALTH_GARRISON, KINDOF_STRUCTURE, KINDOF_VEHICLE, KINDOF_MONEY_HACKER, KINDOF_INFANTRY, KINDOF_SELECTABLE, KINDOF_PROJECTILE,
	KINDOF_IMMOBILE };
const ObjectStatusTypes kCtStatus[3] = { OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_STEALTHED };
const ModelConditionFlagType kCtConds[6] = { MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED,
	MODELCONDITION_NIGHT, MODELCONDITION_SNOW };
/* the exit and fire point bones the scene may give an object (the Rust test has the same list) */
const char *const kCtBones[CT_BONES] = { "ExitStart", "ExitEnd", "ExitStart01", "ExitEnd01", "ExitStart02", "ExitEnd02", "ExitStart03", "ExitEnd03" };

struct CtCfg
{
	const unsigned *p;
	size_t i;
	unsigned u() { return p[i++]; }
	int s() { return (int)p[i++]; }
	Real f() { Real r; unsigned v = p[i++]; memcpy(&r, &v, 4); return r; }
};

Real ctF(unsigned v) { Real r; memcpy(&r, &v, 4); return r; }

std::string ctInfoLine(const DamageInfo *i)
{
	char b[160];
	sprintf(b, "%d %d %d %d %08x %08x %08x %d %d %d", (int)i->in.m_sourceID, (int)i->in.m_damageType, (int)i->in.m_deathType, (int)i->in.m_damageFXOverride,
		realBits(i->in.m_amount), realBits(i->out.m_actualDamageDealt), realBits(i->out.m_actualDamageClipped), (int)i->out.m_noEffect, (int)i->in.m_kill,
		(int)i->in.m_damageStatusType);
	return b;
}

/* a body that logs the damage it is given and knows its maximum health (the contained objects' bodies) */
class CtBody : public InactiveBody
{
public:
	CtBody(Thing *t, const ModuleData *d) : InactiveBody(t, d), m_max(0) {}
	virtual Real getMaxHealth() const { return m_max; }
	virtual void attemptDamage(DamageInfo *info) { ulog("bodydamage %d %s", (int)AsObject(const_cast<Thing *>(static_cast<const Thing *>(getObject())))->m_oid, ctInfoLine(info).c_str()); }
	Real m_max;
};

struct CtObj
{
	Object *obj;
	CtBody *body;
	AIUpdateInterface ai;
	StealthUpdate stealth;
	Drawable draw;
	Team team;
	OpenContain *oc;
	OpenContainModuleData *data;
	BehaviorModule *behaviors[3];
	unsigned boobyTrapped;
	unsigned fpCount;
	Matrix3D fp[CT_FP];
	unsigned boneHas[CT_BONES];
	Coord3D bone[CT_BONES];
	unsigned turretHas[CT_FP];
	Matrix3D turret[CT_FP];
	unsigned adjHas;
	Coord3D adj;
	unsigned hasPhys;
	Real mass;
	AsciiString tmplName;
};

CtObj g_ctObjs[CT_OBJS];
int g_ctN = 0;
Player g_ctPlayers[CT_PLAYERS];
Team g_ctNeutralTeam;
Relationship g_ctRel[CT_OBJS][CT_OBJS];
ModTestGameLogic *g_ctLogic = 0;
std::vector<unsigned char> g_ctSaved;
unsigned char g_ctPhysMem[CT_OBJS][sizeof(PhysicsBehavior) + 16];
std::map<const PhysicsBehavior *, Real> g_ctMass;
int g_ctCurrent = 0;
Object *g_ctSavedContainedBy[CT_OBJS];

CtObj *ctFind(unsigned id)
{
	if (id < 1 || (int)id > g_ctN) return 0;
	return &g_ctObjs[id - 1];
}

Object *ctFindObject(ObjectID id) { CtObj *a = ctFind((unsigned)id); return a ? a->obj : 0; }
CtObj *ctOf(const Object *o) { return ctFind((unsigned)o->m_oid); }
void ctDestroyHook(Object *o) { ulog("destroy %d", (int)o->m_oid); }
Relationship ctTeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return g_ctRel[from->m_dummy - 1][to->m_dummy - 1];
}
void ctXformHook(Object *o, const Matrix3D *mx)
{
	char b[400];
	int n = sprintf(b, "xform %d", (int)o->m_oid);
	for (int r = 0; r < 3; ++r) {
		const Vector4 &row = (*mx)[r];
		n += sprintf(b + n, " %08x %08x %08x %08x", realBits(row.X), realBits(row.Y), realBits(row.Z), realBits(row.W));
	}
	ulog("%s", b);
}

class CtAudio : public AudioManager { public: virtual void stopAudio(AudioAffect) {} };
class CtUi : public InGameUI {};

/* the tile of the terrain tables, as the Rust side's terrain_cell (and l9_height_die_o.inc's hdCell) */
unsigned ctCell(Real x, Real y) { return hdCell(x, y); }

/* getMultiLogicalBonePosition("FIREPOINT", max, NULL, transforms, TRUE): the object's fire points, when it has a drawable (the Rust side's cut) */
Int ctMultiBone(const Object *obj, const char *prefix, Int maxBones, Coord3D *, Matrix3D *transforms, Bool)
{
	ulog("bonen %d %s %d", (int)obj->m_oid, prefix, (int)maxBones);
	CtObj *a = ctOf(obj);
	if (!obj->m_drawable || !a) return 0;
	Int n = (Int)a->fpCount;
	if (n > maxBones) n = maxBones;
	if (n > CT_FP) n = CT_FP;
	for (Int i = 0; i < n; ++i) transforms[i] = a->fp[i];
	return n;
}

}  // namespace

/* ---- the calls the container makes on the rest of the world ---- */
void StealthUpdate::markAsDetected(UnsignedInt) { ulog("markdetected %d", (int)m_owner->m_oid); }
void Drawable::setDrawableHidden(Bool hidden) { ulog("hidden %d %d", m_ownerObject ? (int)m_ownerObject->m_oid : 0, (int)hidden); }
void Object::setLayer(PathfindLayerEnum layer) { ulog("setlayer %d %d", (int)m_oid, (int)layer); }
void Object::setOrientation(Real angle) { ulog("setorient %d %08x", (int)m_oid, realBits(angle)); }
void Object::leaveGroup() { ulog("leavegroup %d", (int)m_oid); }
Bool Object::checkAndDetonateBoobyTrap(const Object *victim)
{
	CtObj *v = victim ? ctOf(victim) : 0;
	Bool r = v ? v->boobyTrapped != 0 : false;
	ulog("booby %d %d %d", (int)m_oid, victim ? (int)victim->m_oid : 0, (int)r);
	return r;
}
void Object::onContainedBy(Object *containedBy) { ulog("oncontainedby %d %d", (int)m_oid, (int)containedBy->m_oid); m_containedBy = containedBy; }
void Object::onRemovedFrom(Object *removedFrom) { ulog("onremovedfrom %d %d", (int)m_oid, (int)removedFrom->m_oid); m_containedBy = 0; }
void Object::clearAndSetModelConditionState(ModelConditionFlagType clr, ModelConditionFlagType set)
{
	if (m_drawable) m_drawable->clearAndSetModelConditionState(clr, set);
}
Bool Object::getSingleLogicalBonePosition(const char *boneName, Coord3D *position, Matrix3D *) const
{
	ulog("bone1 %d %s", (int)m_oid, boneName);
	CtObj *a = ctOf(this);
	for (int i = 0; i < CT_BONES; ++i) {
		if (a && a->boneHas[i] && strcmp(boneName, kCtBones[i]) == 0) { if (position) *position = a->bone[i]; return true; }
	}
	if (position) *position = m_pos;
	return false;
}
Bool Object::getSingleLogicalBonePositionOnTurret(WhichTurretType, const char *boneName, Coord3D *, Matrix3D *transform) const
{
	ulog("boneturret %d %s", (int)m_oid, boneName);
	CtObj *a = ctOf(this);
	for (int i = 0; i < CT_FP; ++i) {
		char nm[16];
		sprintf(nm, "FIREPOINT%02d", i + 1);
		if (a && a->turretHas[i] && strcmp(boneName, nm) == 0) { if (transform) *transform = a->turret[i]; return true; }
	}
	return false;
}
Real PhysicsBehavior::getMass() const
{
	std::map<const PhysicsBehavior *, Real>::const_iterator it = g_ctMass.find(this);
	return it == g_ctMass.end() ? 0.0f : it->second;
}
void AIUpdateInterface::aiExit(Object *objectToExit, CommandSourceType cmdSource) { ulog("aiexit %d %d %d", (int)m_owner->m_oid, (int)objectToExit->m_oid, (int)cmdSource); }
void AIUpdateInterface::aiExitInstantly(Object *objectToExit, CommandSourceType cmdSource) { ulog("aiexitnow %d %d %d", (int)m_owner->m_oid, (int)objectToExit->m_oid, (int)cmdSource); }
void AIUpdateInterface::aiHackInternet(CommandSourceType cmdSource) { ulog("aihack %d %d", (int)m_owner->m_oid, (int)cmdSource); }
void AIUpdateInterface::aiMoveToPosition(const Coord3D *pos, CommandSourceType cmdSource)
{
	ulog("aimove %d %08x %08x %08x %d", (int)m_owner->m_oid, realBits(pos->x), realBits(pos->y), realBits(pos->z), (int)cmdSource);
}
void AIUpdateInterface::ignoreObstacle(const Object *obj) { ulog("aiignore %d %d", (int)m_owner->m_oid, obj ? (int)obj->m_oid : 0); }
void AIUpdateInterface::setIgnoreCollisionTime(Int frames) { ulog("aiignorecoll %d %d", (int)m_owner->m_oid, (int)frames); }
static std::string ctPath(const std::vector<Coord3D> *path)
{
	std::string s;
	for (size_t i = 0; i < path->size(); ++i) {
		char b[40];
		sprintf(b, "%s%08x,%08x,%08x", i ? ";" : "", realBits((*path)[i].x), realBits((*path)[i].y), realBits((*path)[i].z));
		s += b;
	}
	return s;
}
void AIUpdateInterface::aiFollowPath(const std::vector<Coord3D> *path, Object *ignoreObject, CommandSourceType cmdSource)
{
	ulog("aipath %d %d %d %s", (int)m_owner->m_oid, ignoreObject ? (int)ignoreObject->m_oid : 0, (int)cmdSource, ctPath(path).c_str());
}
void AIUpdateInterface::doQuickExit(const std::vector<Coord3D> *path) { ulog("aiquick %d %s", (int)m_owner->m_oid, ctPath(path).c_str()); }
void Pathfinder::removeUnitFromPathfindMap(Object *obj) { ulog("pathremoveunit %d", (int)obj->m_oid); }
void Pathfinder::updatePos(Object *obj, const Coord3D *newPos) { ulog("pathupdatepos %d %08x %08x %08x", (int)obj->m_oid, realBits(newPos->x), realBits(newPos->y), realBits(newPos->z)); }
void Pathfinder::updateGoal(Object *obj, const Coord3D *newGoal, PathfindLayerEnum layer)
{
	ulog("pathupdategoal %d %08x %08x %08x %d", (int)obj->m_oid, realBits(newGoal->x), realBits(newGoal->y), realBits(newGoal->z), (int)layer);
}
Bool Pathfinder::adjustToPossibleDestination(Object *obj, const LocomotorSet *, Coord3D *dest)
{
	ulog("pathadjust %d %08x %08x %08x", (int)obj->m_oid, realBits(dest->x), realBits(dest->y), realBits(dest->z));
	CtObj *a = ctOf(obj);
	if (a && a->adjHas) *dest = a->adj;
	return true;
}
PathfindLayerEnum TerrainLogic::getLayerForDestination(const Coord3D *pos)
{
	ulog("destlayer %08x %08x %08x", realBits(pos->x), realBits(pos->y), realBits(pos->z));
	return (PathfindLayerEnum)g_hdLayer[hdCell(pos->x, pos->y)];
}
Real PartitionManager::getDistanceSquared(const Object *obj, const Coord3D *pos, DistanceCalculationType, Coord3D *) const
{
	ulog("cdist %d %08x %08x %08x", (int)obj->m_oid, realBits(pos->x), realBits(pos->y), realBits(pos->z));
	Real dx = obj->m_pos.x - pos->x, dy = obj->m_pos.y - pos->y;
	return dx * dx + dy * dy;
}
void PartitionManager::registerObject(Object *obj) { ulog("pregister %d", (int)obj->m_oid); }
void PartitionManager::unRegisterObject(Object *obj) { ulog("punregister %d", (int)obj->m_oid); }

namespace {

void ctPutLog()
{
	std::vector<std::string> log;
	for (size_t j = 0; j < g_events.size(); ++j) {
		const Event &e = g_events[j];
		if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
	}
	putU32((unsigned)log.size());
	for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
}

/* the state of every object: the container's words (OpenContain::state_words on the Rust side), then the object's own */
void ctPutObject(CtObj *a)
{
	if (a->oc) {
		OpenContain *oc = a->oc;
		putU32(1);
		putU32(oc->m_containListSize);
		putU32((unsigned)oc->m_containList.size());
		for (ContainedItemsList::const_iterator it = oc->m_containList.begin(); it != oc->m_containList.end(); ++it) putU32((unsigned)(*it)->m_oid);
		putU32((unsigned)oc->m_playerEnteredMask);
		putU32(oc->m_lastUnloadSoundFrame);
		putU32(oc->m_lastLoadSoundFrame);
		putU32(oc->m_stealthUnitsContained);
		putU32(oc->m_doorCloseCountdown);
		/* the condition state: its bits, as 128 flags in four words */
		for (int w = 0; w < 4; ++w) {
			unsigned m = 0;
			for (int b = 0; b < 32; ++b) if (w * 32 + b < MODELCONDITION_COUNT && oc->m_conditionState.test(w * 32 + b)) m |= 1u << b;
			putU32(m);
		}
		/* the fire points, folded */
		unsigned h = 0;
		for (int i = 0; i < OpenContain::MAX_FIRE_POINTS; ++i) {
			for (int r = 0; r < 3; ++r) {
				const Vector4 &row = oc->m_firePoints[i][r];
				h = h * 31u + realBits(row.X); h = h * 31u + realBits(row.Y); h = h * 31u + realBits(row.Z); h = h * 31u + realBits(row.W);
			}
		}
		putU32(h);
		putU32((unsigned)oc->m_firePointStart);
		putU32((unsigned)oc->m_firePointNext);
		putU32((unsigned)oc->m_firePointSize);
		putU32(oc->m_noFirePointsInArt ? 1 : 0);
		putU32(realBits(oc->m_rallyPoint.x));
		putU32(realBits(oc->m_rallyPoint.y));
		putU32(realBits(oc->m_rallyPoint.z));
		putU32(oc->m_rallyPointExists ? 1 : 0);
		putU32((unsigned)oc->m_objectEnterExitInfo.size());
		for (OpenContain::ObjectEnterExitMap::const_iterator it = oc->m_objectEnterExitInfo.begin(); it != oc->m_objectEnterExitInfo.end(); ++it) {
			putU32((unsigned)it->first);
			putU32((unsigned)it->second);
		}
		putU32((unsigned)oc->m_whichExitPath);
		putU32(oc->m_passengerAllowedToFire ? 1 : 0);
		putU32(oc->m_loadSoundsEnabled ? 1 : 0);
	} else {
		putU32(0);
	}
	putU32(a->obj->m_containedBy ? (unsigned)a->obj->m_containedBy->m_oid : 0u);
	putU32(realBits(a->obj->m_pos.x));
	putU32(realBits(a->obj->m_pos.y));
	putU32(realBits(a->obj->m_pos.z));
	unsigned w[2] = { 0, 0 };
	for (int i = 0; i < OBJECT_STATUS_COUNT && i < 64; ++i) if (a->obj->m_status.test(i)) w[i / 32] |= 1u << (i % 32);
	putU32(w[0]);
	putU32(w[1]);
	putU32(a->obj->m_effDead ? 1 : 0);
	putU32(a->ai.m_dead ? 1 : 0);
	putU32(a->obj->m_physics ? (a->obj->m_physics->getAllowToFall() ? 1u : 0u) : 2u);
	putU32(realBits(a->obj->m_geometry.getBoundingCircleRadius()));
	putU32(realBits(a->obj->m_orientation));
	putU32((unsigned)a->obj->m_layer);
	/* the drawable's model conditions (the read the container makes of the client) and whether it is selected */
	for (int w = 0; w < 4; ++w) {
		unsigned m = 0;
		for (int b = 0; b < 32; ++b) if (w * 32 + b < MODELCONDITION_COUNT && a->draw.m_condFlags.test(w * 32 + b)) m |= 1u << b;
		putU32(m);
	}
	putU32(a->draw.m_selected && a->obj->m_drawable ? 1 : 0);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of an OpenContain for object 1 of the scene `cfg` describes (see tests/differential_contain_open.rs, the one place the
** layout is written), and make the containers. The data dump goes first (the fifteen fields). Then `n` ops of 5 words [op, a, b, c, d] (the test's table). Each op,
** and the construction, appends: the CATCH_ALL status, a result word, the effect log (count, then each line as a string), the xfer status, bytes and CRC, then the
** state of every object. */
static int l9ctRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_events.clear();
	g_ulog.clear();
	g_ctSaved.clear();
	g_ctMass.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9ctRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const OpenContainModuleData *od = (const OpenContainModuleData *)d;
		putU32((unsigned)od->m_containMax);
		putStr(od->m_enterSound.getEventName().str(), od->m_enterSound.getEventName().getLength());
		putStr(od->m_exitSound.getEventName().str(), od->m_exitSound.getEventName().getLength());
		putU32(od->m_passengersAllowedToFire);
		putU32(od->m_passengersInTurret);
		putU32((unsigned)od->m_numberOfExitPaths);
		putU32(realBits(od->m_damagePercentageToUnits));
		putU32(od->m_isBurnedDeathToUnits);
		putU32(od->m_doorOpenTime);
		unsigned am = 0, fm = 0;
		for (int i = 0; i < 8; ++i) { if (od->m_allowInsideKindOf.test(kCtKinds[i])) am |= 1u << i; if (od->m_forbidInsideKindOf.test(kCtKinds[i])) fm |= 1u << i; }
		putU32(am);
		putU32(fm);
		putU32(od->m_weaponBonusPassedToPassengers);
		putU32((od->m_allowAlliesInside ? 1u : 0u) | (od->m_allowEnemiesInside ? 2u : 0u) | (od->m_allowNeutralInside ? 4u : 0u));
		putU32(od->m_dieMuxData.m_deathTypes);
		putU32(od->m_dieMuxData.m_veterancyLevels);
		unsigned ex = 0, rq = 0;
		for (int i = 0; i < 3; ++i) { if (od->m_dieMuxData.m_exemptStatus.test(kCtStatus[i])) ex |= 1u << i; if (od->m_dieMuxData.m_requiredStatus.test(kCtStatus[i])) rq |= 1u << i; }
		putU32(ex);
		putU32(rq);
	}
	initKindOfMasks();

	CtCfg c = { cfg, 0 };
	ModTestGameLogic gl;
	gl.m_frame = c.u();
	g_ctLogic = &gl;
	TheGameLogic = &gl;
	const unsigned logicSeed = c.u(), clientSeed = c.u();
	g_ctN = (int)c.u();
	struct ObjCfg {
		unsigned snd[3]; unsigned kinds, status; int player; unsigned pos[3]; unsigned orient; unsigned layer; unsigned radius; unsigned drawable, selected, cond;
		unsigned hasAi, aiIdle, enterTarget, hasPhys, mass, canFall, hasStealth, hasBody, maxHealth, booby, effDead, weaponBonus, containedBy, hasContain;
		int containMax; unsigned enterSnd, exitSnd, allowFire, inTurret; int exitPaths; unsigned damagePct, burned, doorOpen, allowMode, allowMask, forbidMask,
			weaponPass, allowRel, deathMask, vetMask, exemptMask, requiredMask;
		unsigned adjHas, adj[3]; unsigned fpCount; unsigned fp[CT_FP][3]; unsigned boneHas[CT_BONES]; unsigned bone[CT_BONES][3]; unsigned turretHas[CT_FP];
		unsigned turret[CT_FP][3];
	} oc[CT_OBJS];
	for (int k = 0; k < g_ctN; ++k) {
		ObjCfg &o = oc[k];
		for (int i = 0; i < 3; ++i) o.snd[i] = c.u();
		o.kinds = c.u(); o.status = c.u(); o.player = c.s();
		for (int i = 0; i < 3; ++i) o.pos[i] = c.u();
		o.orient = c.u(); o.layer = c.u(); o.radius = c.u(); o.drawable = c.u(); o.selected = c.u(); o.cond = c.u();
		o.hasAi = c.u(); o.aiIdle = c.u(); o.enterTarget = c.u(); o.hasPhys = c.u(); o.mass = c.u(); o.canFall = c.u(); o.hasStealth = c.u(); o.hasBody = c.u();
		o.maxHealth = c.u(); o.booby = c.u(); o.effDead = c.u(); o.weaponBonus = c.u(); o.containedBy = c.u(); o.hasContain = c.u();
		o.containMax = c.s(); o.enterSnd = c.u(); o.exitSnd = c.u(); o.allowFire = c.u(); o.inTurret = c.u(); o.exitPaths = c.s(); o.damagePct = c.u(); o.burned = c.u();
		o.doorOpen = c.u(); o.allowMode = c.u(); o.allowMask = c.u(); o.forbidMask = c.u(); o.weaponPass = c.u(); o.allowRel = c.u(); o.deathMask = c.u(); o.vetMask = c.u();
		o.exemptMask = c.u(); o.requiredMask = c.u();
		o.adjHas = c.u(); for (int i = 0; i < 3; ++i) o.adj[i] = c.u();
		o.fpCount = c.u();
		for (int j = 0; j < CT_FP; ++j) for (int i = 0; i < 3; ++i) o.fp[j][i] = c.u();
		for (int j = 0; j < CT_BONES; ++j) { o.boneHas[j] = c.u(); for (int i = 0; i < 3; ++i) o.bone[j][i] = c.u(); }
		for (int j = 0; j < CT_FP; ++j) { o.turretHas[j] = c.u(); for (int i = 0; i < 3; ++i) o.turret[j][i] = c.u(); }
	}
	for (int i = 0; i < g_ctN; ++i) for (int j = 0; j < g_ctN; ++j) g_ctRel[i][j] = (Relationship)c.s();
	for (int i = 0; i < 8; ++i) g_hdLayer[i] = c.s();
	for (int i = 0; i < 8; ++i) g_hdLayerHeight[i] = c.f();

	/* build the world */
	CtAudio audio;
	TheAudio = &audio;
	CtUi ui;
	TheInGameUI = &ui;
	AI ai;
	TheAI = &ai;
	ModTestPartitionManager pm;
	ThePartitionManager = &pm;
	ModTestTerrainLogic terrain;
	TheTerrainLogic = &terrain;
	g_modFindHook = ctFindObject;
	g_modDestroyHook = ctDestroyHook;
	g_teamRelHook = ctTeamRel;
	g_xformHook = ctXformHook;
	g_multiBoneHook = ctMultiBone;
	TheGlobalData = TheGlobalData;
	g_ctNeutralTeam.m_dummy = 0;
	for (int p = 0; p < CT_PLAYERS; ++p) {
		g_ctPlayers[p] = Player();
		g_ctPlayers[p].m_idx = p;
		g_ctPlayers[p].m_defaultTeam = &g_ctNeutralTeam;
	}
	for (int k = 0; k < g_ctN; ++k) {
		CtObj &a = g_ctObjs[k];
		const ObjCfg &o = oc[k];
		a.obj = new Object;
		a.obj->m_level = (VeterancyLevel)(1 + (k % 3));   /* never LEVEL_REGULAR: getVeterancyLevelFlag shifts by -1 for it (undefined) */
		Object *obj = a.obj;
		obj->m_oid = (ObjectID)(k + 1);
		obj->m_pos.x = ctF(o.pos[0]); obj->m_pos.y = ctF(o.pos[1]); obj->m_pos.z = ctF(o.pos[2]);
		obj->m_orientation = ctF(o.orient);
		obj->m_layer = (PathfindLayerEnum)o.layer;
		obj->m_geometry = GeometryInfo(GEOMETRY_SPHERE, false, 1.0f, ctF(o.radius), ctF(o.radius));
		for (int i = 0; i < 8; ++i) if (o.kinds >> i & 1) obj->m_kindOf.set(kCtKinds[i]);
		for (int i = 0; i < 3; ++i) if (o.status >> i & 1) obj->m_status.set(kCtStatus[i]);
		obj->m_effDead = o.effDead != 0;
		obj->m_weaponBonusCondition = o.weaponBonus;
		a.team.m_dummy = k + 1;
		obj->m_team = &a.team;
		obj->m_player = o.player >= 0 ? &g_ctPlayers[o.player] : 0;
		a.tmplName.format("T%d", k + 1);
		obj->m_tmpl.m_name = a.tmplName;
		AudioEventRTS *ev[3] = { &obj->m_tmpl.m_soundEnter, &obj->m_tmpl.m_soundExit, &obj->m_tmpl.m_soundFalling };
		for (int i = 0; i < 3; ++i) { if (o.snd[i]) { char sn[16]; sprintf(sn, "snd%u", o.snd[i]); ev[i]->setEventName(AsciiString(sn)); } }
		a.draw.m_ownerObject = obj;
		a.draw.m_selected = o.selected != 0;
		a.draw.m_condFlags.clear();
		for (int i = 0; i < 6; ++i) if (o.cond >> i & 1) a.draw.m_condFlags.set(kCtConds[i]);
		obj->m_drawable = o.drawable ? &a.draw : 0;
		a.ai.m_owner = obj;
		a.ai.m_idle = o.aiIdle != 0;
		a.ai.m_enterTarget = 0;
		obj->m_ai = o.hasAi ? &a.ai : 0;
		a.stealth.m_owner = obj;
		obj->m_stealth = o.hasStealth ? &a.stealth : 0;
		a.hasPhys = o.hasPhys;
		memset(g_ctPhysMem[k], 0, sizeof(g_ctPhysMem[k]));
		PhysicsBehavior *phys = (PhysicsBehavior *)g_ctPhysMem[k];
		obj->m_physics = o.hasPhys ? phys : 0;
		if (o.hasPhys) { g_ctMass[phys] = ctF(o.mass); phys->setAllowToFall(o.canFall != 0); }
		a.boobyTrapped = o.booby;
		a.adjHas = o.adjHas;
		a.adj.x = ctF(o.adj[0]); a.adj.y = ctF(o.adj[1]); a.adj.z = ctF(o.adj[2]);
		a.fpCount = o.fpCount;
		for (unsigned j = 0; j < o.fpCount && j < CT_FP; ++j) {
			a.fp[j].Make_Identity();
			a.fp[j].Set_Translation(Vector3(ctF(o.fp[j][0]), ctF(o.fp[j][1]), ctF(o.fp[j][2])));
		}
		for (int j = 0; j < CT_BONES; ++j) { a.boneHas[j] = o.boneHas[j]; a.bone[j].x = ctF(o.bone[j][0]); a.bone[j].y = ctF(o.bone[j][1]); a.bone[j].z = ctF(o.bone[j][2]); }
		for (int j = 0; j < CT_FP; ++j) {
			a.turretHas[j] = o.turretHas[j];
			a.turret[j].Make_Identity();
			a.turret[j].Set_Translation(Vector3(ctF(o.turret[j][0]), ctF(o.turret[j][1]), ctF(o.turret[j][2])));
		}
		a.body = 0;
		a.oc = 0;
		a.data = 0;
		obj->m_body = 0;
		obj->m_contain = 0;
		obj->m_behaviors = a.behaviors;
		a.behaviors[0] = a.behaviors[1] = a.behaviors[2] = 0;
		g_ctSavedContainedBy[k] = 0;
	}
	/* the modules, once every object they may name exists */
	bool failed = false;
	for (int k = 0; k < g_ctN && !failed; ++k) {
		CtObj &a = g_ctObjs[k];
		const ObjCfg &o = oc[k];
		a.obj->m_containedBy = o.containedBy ? ctFind(o.containedBy)->obj : 0;
		a.ai.m_enterTarget = o.enterTarget ? ctFind(o.enterTarget)->obj : 0;
		int idx = 0;
		if (o.hasBody) {
			ModuleData *bd = new ModuleData;
			a.body = new CtBody(a.obj, bd);
			a.body->m_max = ctF(o.maxHealth);
			a.obj->m_effDead = o.effDead != 0;   /* InactiveBody::InactiveBody made its object effectively dead: the scene says whether it is */
			a.obj->m_body = a.body;
			a.behaviors[idx++] = a.body;
		}
		if (o.hasContain) {
			if (k == 0) a.data = (OpenContainModuleData *)d;
			else {
				a.data = new OpenContainModuleData;
				a.data->m_containMax = o.containMax;
				if (o.enterSnd) { char sn[16]; sprintf(sn, "snd%u", o.enterSnd); a.data->m_enterSound.setEventName(AsciiString(sn)); }
				if (o.exitSnd) { char sn[16]; sprintf(sn, "snd%u", o.exitSnd); a.data->m_exitSound.setEventName(AsciiString(sn)); }
				a.data->m_passengersAllowedToFire = o.allowFire != 0;
				a.data->m_passengersInTurret = o.inTurret != 0;
				a.data->m_numberOfExitPaths = o.exitPaths;
				a.data->m_damagePercentageToUnits = ctF(o.damagePct);
				a.data->m_isBurnedDeathToUnits = o.burned != 0;
				a.data->m_doorOpenTime = o.doorOpen;
				if (o.allowMode != 0) {
					a.data->m_allowInsideKindOf.clear();
					for (int i = 0; i < 8; ++i) if (o.allowMask >> i & 1) a.data->m_allowInsideKindOf.set(kCtKinds[i]);
				}
				for (int i = 0; i < 8; ++i) if (o.forbidMask >> i & 1) a.data->m_forbidInsideKindOf.set(kCtKinds[i]);
				a.data->m_weaponBonusPassedToPassengers = o.weaponPass != 0;
				a.data->m_allowAlliesInside = (o.allowRel & 1) != 0;
				a.data->m_allowEnemiesInside = (o.allowRel & 2) != 0;
				a.data->m_allowNeutralInside = (o.allowRel & 4) != 0;
				a.data->m_dieMuxData.m_deathTypes = o.deathMask;
				a.data->m_dieMuxData.m_veterancyLevels = o.vetMask;
				for (int i = 0; i < 3; ++i) { if (o.exemptMask >> i & 1) a.data->m_dieMuxData.m_exemptStatus.set(kCtStatus[i]); if (o.requiredMask >> i & 1) a.data->m_dieMuxData.m_requiredStatus.set(kCtStatus[i]); }
			}
			try { a.oc = new OpenContain(a.obj, a.data); }
			CATCH_ALL(status, code, msg, msgCap)
			if (status) { failed = true; break; }
			a.obj->m_contain = a.oc;
			a.behaviors[idx++] = a.oc;
		}
	}
	if (failed) {
		TheGameLogic = 0; TheAudio = 0; TheInGameUI = 0; TheAI = 0; ThePartitionManager = 0; TheTerrainLogic = 0;
		g_modFindHook = 0; g_modDestroyHook = 0; g_teamRelHook = 0; g_xformHook = 0; g_multiBoneHook = 0;
		return 5;
	}
	for (int k = 0; k < g_ctN; ++k) {
		CtObj &a = g_ctObjs[k];
		const ObjCfg &o = oc[k];
		/* the objects' places in the contain lists named by `containedBy`: the container lists them (the scene's `contained` objects) */
		(void)o;
		if (a.obj->m_containedBy) {
			CtObj *cont = ctOf(a.obj->m_containedBy);
			if (cont && cont->oc) cont->oc->addToContainList(a.obj);
		}
	}

	/* the random generators start after the objects are made (the Rust side's object creation may draw) */
	InitRandom(clientSeed);
	InitGameLogicRandom(logicSeed);

	putU32((unsigned)g_ctN);
	for (int phase = 0; phase < 1 + n; ++phase) {
		const unsigned *o = phase ? ops + 5 * (phase - 1) : 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[8192];
		int blen = 0;
		unsigned result = 0;
		int xst = -2;
		g_events.clear();
		g_ulog.clear();
		if (phase) {
			CtObj *t = ctFind(o[1] % (unsigned)g_ctN + 1);
			OpenContain *oc2 = t->oc;
			Object *r2 = ctFind(o[2] % (unsigned)g_ctN + 1)->obj;   /* the rider an op names (the unsigned word modulo the object count) */
			DamageInfo infoOp;
			if (o[0] == 4 || o[0] == 6) {
				infoOp.in.m_sourceID = (ObjectID)(o[3] % 5);
				infoOp.in.m_damageType = (DamageType)(o[2] % 8);
				infoOp.in.m_deathType = (DeathType)(o[4] % 5 + 1);
				infoOp.in.m_amount = 10.0f;
			}
			try {
				switch (o[0]) {
				case 0: if (oc2) oc2->addToContain(r2); break;
				case 1: if (oc2) oc2->removeFromContain(r2, o[3] != 0); break;
				case 2: if (oc2) oc2->removeAllContained(o[2] != 0); break;
				case 3: if (oc2) oc2->killAllContained(); break;
				case 4: if (oc2) { oc2->harmAndForceExitAllContained(&infoOp); result = realBits(infoOp.in.m_amount); } break;
				case 5: if (oc2) oc2->onCollide(r2, 0, 0); break;
				case 6: if (oc2) { result = oc2->getOpenContainModuleData()->m_dieMuxData.isDieApplicable(t->obj, &infoOp) ? 1 : 0; oc2->onDie(&infoOp); } break;
				case 7: if (oc2) result = (unsigned)oc2->update(); break;
				case 8: if (oc2) result = oc2->isValidContainerFor(r2, o[3] != 0) ? 1 : 0; break;
				case 9: if (oc2) oc2->exitObjectViaDoor(r2, DOOR_1); break;
				case 10: if (oc2) oc2->exitObjectInAHurry(r2); break;
				case 11: if (oc2) oc2->orderAllPassengersToExit(CMD_FROM_AI, o[2] != 0); break;
				case 12: if (oc2) oc2->orderAllPassengersToIdle(CMD_FROM_AI); break;
				case 13: if (oc2) oc2->orderAllPassengersToHackInternet(CMD_FROM_AI); break;
				case 14: if (oc2) oc2->markAllPassengersDetected(); break;
				case 15: if (oc2) oc2->processDamageToContained(ctF(o[2])); break;
				case 16: if (oc2) oc2->onObjectWantsToEnterOrExit(r2, (ObjectEnterExitType)(o[3] % 3)); break;
				case 17: if (oc2) { Coord3D p; p.x = ctF(o[2]); p.y = ctF(o[3]); p.z = ctF(o[4]); oc2->setRallyPoint(&p); } break;
				case 18: if (oc2) {
					const Coord3D *p = oc2->getRallyPoint();
					result = p ? realBits(p->x) * 31u * 31u + realBits(p->y) * 31u + realBits(p->z) : 0xdead0000u;
				} break;
				case 19: if (oc2) { Coord3D p; oc2->getNaturalRallyPoint(p, o[2] != 0); result = realBits(p.x) * 31u * 31u + realBits(p.y) * 31u + realBits(p.z); } break;
				case 20: if (oc2) { Coord3D p; p.x = ctF(o[2]); p.y = ctF(o[3]); p.z = ctF(o[4]); Object *c3 = oc2->getClosestRider(&p); result = c3 ? (unsigned)c3->m_oid : 0u; } break;
				case 21: if (oc2) result = oc2->isAnyRiderAttacking() ? 1 : 0; break;
				case 22: if (oc2) oc2->containReactToTransformChange(); break;
				case 23: if (oc2) oc2->onSelling(); break;
				case 24: if (oc2) oc2->setPassengerAllowedToFire(o[2] != 0); break;
				case 25: if (oc2) result = oc2->isPassengerAllowedToFire() ? 1 : 0; break;
				case 26: if (oc2) {
					unsigned h = realBits(oc2->getContainedItemsMass());
					h = h * 31u + oc2->getStealthUnitsContained();
					h = h * 31u + (unsigned)oc2->getPlayerWhoEntered();
					h = h * 31u + oc2->getContainCount();
					h = h * 31u + (oc2->isWeaponBonusPassedToPassengers() ? 1u : 0u);
					h = h * 31u + (unsigned)oc2->getWeaponBonusPassedToPassengers();
					h = h * 31u + (unsigned)oc2->getContainMax();
					h = h * 31u + (oc2->isEnclosingContainerFor(r2) ? 1u : 0u);
					h = h * 31u + (oc2->isDisplayedOnControlBar() ? 1u : 0u);
					h = h * 31u + (unsigned)oc2->getExtraSlotsInUse();
					h = h * 31u + (oc2->isKickOutOnCapture() ? 1u : 0u);
					h = h * 31u + (oc2->hasObjectsWantingToEnterOrExit() ? 1u : 0u);
					h = h * 31u + (oc2->isExitBusy() ? 1u : 0u);
					h = h * 31u + (oc2->isImmuneToClearBuildingAttacks() ? 1u : 0u);
					result = h;
				} break;
				case 27: if (oc2) oc2->scatterToNearbyPosition(r2); break;
				case 28: if (oc2) oc2->enableLoadSounds(o[2] != 0); break;
				case 29: if (oc2) oc2->onDelete(); break;
				case 30: if (oc2) { std::vector<unsigned char> b; xst = abRunXfer(oc2, 0, xfer_path, bytes, (int)sizeof(bytes), &blen, &crc); g_ctSaved.assign(bytes, bytes + blen); } break;
				case 31: if (oc2) {
					std::vector<unsigned char> b = g_ctSaved;
					xst = abRunXfer(oc2, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				} break;
				case 32: if (oc2) xst = abRunXfer(oc2, 2, "crc", bytes, 0, &blen, &crc); break;
				case 33: if (oc2) { xst = 0; try { oc2->loadPostProcess(); } catch (SaveCode sc) { xst = sc == SC_INVALID_DATA ? (int)XFER_LIST_NOT_EMPTY : -1; } } break;
				case 34: gl.m_frame = o[2]; break;
				case 35: t->draw.m_selected = o[2] != 0; break;
				case 36: t->draw.m_condFlags.clear(); for (int i = 0; i < 6; ++i) if (o[2] >> i & 1) t->draw.m_condFlags.set(kCtConds[i]); break;
				case 37: t->obj->m_status.clear(); for (int i = 0; i < 3; ++i) if (o[2] >> i & 1) t->obj->m_status.set(kCtStatus[i]); break;
				case 38: t->obj->m_pos.x = ctF(o[2]); t->obj->m_pos.y = ctF(o[3]); t->obj->m_pos.z = ctF(o[4]); break;
				case 39: t->obj->m_effDead = o[2] != 0; break;
				case 40: t->boobyTrapped = o[2]; break;
				case 41: t->ai.m_idle = (o[2] & 1) != 0; t->ai.m_enterTarget = (o[2] & 2) ? r2 : 0; break;
				case 42: if (oc2) oc2->onContaining(r2, o[3] != 0); break;
				case 43: if (oc2) oc2->onRemoving(r2); break;
				case 44: if (oc2) oc2->redeployOccupants(); break;
				case 45: if (oc2) oc2->putObjAtNextFirePoint(r2); break;
				default: break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
		}
		putI32(st);
		putU32(result);
		ctPutLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		for (int k = 0; k < g_ctN; ++k) ctPutObject(&g_ctObjs[k]);
	}
	*len = finish(buf, cap);
	for (int k = 0; k < g_ctN; ++k) {
		CtObj &a = g_ctObjs[k];
		/* the contain lists were filled by hand and the objects are gone with the scene: the destructor's list is not asked about */
		if (a.oc) a.oc->m_containList.clear();
		delete a.oc;
		delete a.body;
		if (k != 0) delete a.data;
		delete a.obj;
	}
	delete d;
	TheGameLogic = 0; TheAudio = 0; TheInGameUI = 0; TheAI = 0; ThePartitionManager = 0; TheTerrainLogic = 0;
	g_modFindHook = 0; g_modDestroyHook = 0; g_teamRelHook = 0; g_xformHook = 0; g_multiBoneHook = 0;
	return 0;
}

int ref_l9ct_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9ctRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
