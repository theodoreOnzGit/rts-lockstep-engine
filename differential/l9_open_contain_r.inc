/* Differential-test driver for OpenContain (p1w13-sonnet, batch R; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/OpenContain.cpp   (#included through OpenContain_edit.cpp: one mechanical edit made by build.rs, the end-iterator read of processDamageToContained) with its header, in wrap_module.cpp's unit on the stand-ins of
**   differential/shim_module: up to four Objects, each with a team and a player, a drawable, an AI, a physics block, a stealth update and a body that log what is
**   done to them, and OpenContain modules on those that have one (the first reads the INI, the others take their data from the scene), the REAL DieMuxData, INI,
**   RandomValue, Matrix3D, Xfer, Object::kill and Object::getRelationship. Every call the container makes on the rest of the world is a line of the effect log
**   (ulog), in the order it is made, in the text the Rust side's cut records, events and probe traces produce. */
#include <time.h>
#include <unistd.h>
#include <sys/syscall.h>
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
/* --- p1w13-sonnet --- (batch R2: TransportContain) */
#include "TransportContain_edit.cpp"  /* TransportContain.cpp with its two `while( *it )` loops ending at end() (build.rs) */
/* batch R3: the two derived transports, unchanged */
/* RailedTransportContain.cpp includes RailedTransportDockUpdate.h for one class: that header's guard is defined here and the class is copied
** verbatim (RailedTransportDockUpdate.h:57-66), so the rest of DockUpdate (not translated yet) stays out of this unit */
#define __RAILED_TRANSPORT_DOCK_UPDATE_H_
class RailedTransportDockUpdateInterface
{

public:

	virtual Bool isLoadingOrUnloading( void ) = 0;
	virtual void unloadAll( void ) = 0;
	virtual void unloadSingleObject( Object *obj ) = 0;

};
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/InternetHackContain.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/RailedTransportContain.cpp"
/* --- end p1w13-sonnet --- */
#undef itoa

namespace {

/* --- p1w13-sonnet --- (batch R3) the class the data block names: 0 OpenContain (D), 1 TransportContain (T), 2 InternetHackContain (I),
** 3 RailedTransportContain (R) */
unsigned g_ctBlockCls = 0;
/* --- end p1w13-sonnet --- */

ModuleData *l9ctRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			/* --- p1w13-sonnet --- (batches R2, R3) */
			const char c0 = currentLine.str()[0];
			g_ctBlockCls = c0 == 'T' ? 1 : c0 == 'I' ? 2 : c0 == 'R' ? 3 : 0;
			if (g_ctBlockCls == 1) data = TransportContain::friend_newModuleData(&ini);
			else if (g_ctBlockCls == 2) data = InternetHackContain::friend_newModuleData(&ini);
			else if (g_ctBlockCls == 3) data = RailedTransportContain::friend_newModuleData(&ini);
			else data = OpenContain::friend_newModuleData(&ini);
			/* --- end p1w13-sonnet --- */
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

enum { CT_OBJS = 4, CT_PLAYERS = 4, CT_BONES = 14, CT_FP = 12 };

/* the kinds, statuses and model conditions the scene's masks name: local bit i is kCtKinds[i] (the Rust test has the same lists) */
const KindOfType kCtKinds[8] = { KINDOF_STEALTH_GARRISON, KINDOF_STRUCTURE, KINDOF_VEHICLE, KINDOF_MONEY_HACKER, KINDOF_INFANTRY, KINDOF_SELECTABLE, KINDOF_PROJECTILE,
	KINDOF_IMMOBILE };
const ObjectStatusTypes kCtStatus[3] = { OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_STEALTHED };
const ModelConditionFlagType kCtConds[6] = { MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED,
	MODELCONDITION_NIGHT, MODELCONDITION_SNOW };
/* the exit and fire point bones the scene may give an object (the Rust test has the same list) */
const char *const kCtBones[CT_BONES] = { "ExitStart", "ExitEnd", "ExitStart01", "ExitEnd01", "ExitStart02", "ExitEnd02", "ExitStart03", "ExitEnd03", "ExitStart09", "ExitEnd09", "ExitStart9", "ExitEnd9", "ExitStart10", "ExitEnd10" };

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
/* --- p1w13-sonnet --- (batch R3: RailedTransportContain) a behaviour with the dock interface: open or closed, and setDockOpen logs */
struct CtDock : public TestBehavior, public DockUpdateInterface
{
	CtDock(Thing *t, const ModuleData *d) : TestBehavior(t, d), m_open(false) {}
	virtual DockUpdateInterface *getDockUpdateInterface() { return this; }
	virtual Bool isDockOpen(void) { return m_open; }
	virtual void setDockOpen(Bool open) { ulog("docksetopen %d %d", (int)AsObject(const_cast<Thing *>(static_cast<const Thing *>(getObject())))->m_oid, open ? 1 : 0); m_open = open; }
	/* the rest of the interface: no contain module calls it */
	virtual Bool isClearToApproach(Object const *) const { return false; }
	virtual Bool reserveApproachPosition(Object *, Coord3D *, Int *) { return false; }
	virtual Bool advanceApproachPosition(Object *, Coord3D *, Int *) { return false; }
	virtual Bool isClearToEnter(Object const *) const { return false; }
	virtual Bool isClearToAdvance(Object const *, Int) const { return false; }
	virtual void getEnterPosition(Object *, Coord3D *) {}
	virtual void getDockPosition(Object *, Coord3D *) {}
	virtual void getExitPosition(Object *, Coord3D *) {}
	virtual void onApproachReached(Object *) {}
	virtual void onEnterReached(Object *) {}
	virtual void onDockReached(Object *) {}
	virtual void onExitReached(Object *) {}
	virtual Bool action(Object *, Object *) { return false; }
	virtual void cancelDock(Object *) {}
	virtual void setDockCrippled(Bool) {}
	virtual Bool isAllowPassthroughType() { return false; }
	virtual Bool isRallyPointAfterDockType() { return false; }
	Bool m_open;
};
/* --- end p1w13-sonnet --- */

class CtBody : public InactiveBody
{
public:
	CtBody(Thing *t, const ModuleData *d) : InactiveBody(t, d), m_max(0), m_health(0) {}
	virtual Real getMaxHealth() const { return m_max; }
	/* --- p1w13-sonnet --- (batch R2: TransportContain heals its riders) the health the scene gives, and a heal that logs and fills up to the maximum */
	virtual Real getHealth() const { return m_health; }
	virtual void attemptHealing(DamageInfo *info)
	{
		ulog("heal %d %08x %d", (int)AsObject(const_cast<Thing *>(static_cast<const Thing *>(getObject())))->m_oid, realBits(info->in.m_amount), (int)info->in.m_sourceID);
		Real room = m_max - m_health;
		Real healed = (info->in.m_damageType == DAMAGE_HEALING && info->in.m_amount < room) ? info->in.m_amount : room;
		m_health += healed;
		info->out.m_actualDamageDealt = healed;
	}
	Real m_health;
	/* --- end p1w13-sonnet --- */
	virtual void attemptDamage(DamageInfo *info) { ulog("bodydamage %d %s", (int)AsObject(const_cast<Thing *>(static_cast<const Thing *>(getObject())))->m_oid, ctInfoLine(info).c_str()); }
	Real m_max;
};

struct CtObj
{
	Object *obj;
	CtBody *body;
	struct CtDock *dock;   /* p1w13-sonnet, batch R3 */
	AIUpdateInterface ai;
	StealthUpdate stealth;
	Drawable draw;
	Team team;
	OpenContain *oc;
	OpenContainModuleData *data;
	BehaviorModule *behaviors[4];   /* p1w13-sonnet, batch R3: body, contain, dock, NULL */
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
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	unsigned cls;   /* 0 OpenContain, 1 TransportContain */
	Weapon weap[3];
	unsigned char physData[sizeof(PhysicsBehaviorModuleData) + 16];
	unsigned validTerrain;
	/* --- end p1w13-sonnet --- */
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
/* --- p1w13-sonnet --- (batch R2: TransportContain) */
int ctPhysOwner(const PhysicsBehavior *p) { for (int k = 0; k < g_ctN; ++k) if ((const void *)g_ctPhysMem[k] == (const void *)p) return k + 1; return 0; }
void PhysicsBehavior::applyMotiveForce(const Coord3D *f) { ulog("pmotive %d %08x %08x %08x", ctPhysOwner(this), realBits(f->x), realBits(f->y), realBits(f->z)); }
void PhysicsBehavior::setPitchRate(Real pitch) { ulog("ppitch %d %08x", ctPhysOwner(this), realBits(pitch)); }
void AIUpdateInterface::setAttitude(AttitudeType tude) { ulog("aiattitude %d %d", (int)m_owner->m_oid, (int)tude); }
void AIUpdateInterface::wakeUpAndAttemptToTarget(void) { ulog("aiwake %d", (int)m_owner->m_oid); }
Bool Pathfinder::validMovementTerrain(PathfindLayerEnum layer, const Locomotor *locomotor, const Coord3D *pos)
{
	for (int k = 0; k < g_ctN; ++k) if ((const void *)&g_ctObjs[k].ai == (const void *)locomotor) return g_ctObjs[k].validTerrain != 0;
	return false;
}
/* Object.cpp:3065-3073: the flag cleared, the weapon set told, the drawable's model condition (the line setWeaponSetFlag's stand-in writes, l9_upgrades.inc) */
void Object::clearWeaponSetFlag(WeaponSetType wst)
{
	m_weaponSetFlags[wst / 64] &= ~(1ULL << (wst % 64));
	ulog("wset");
	if (m_drawable) m_drawable->clearModelConditionState(TheWeaponSetTypeToModelConditionTypeMap[wst]);
}
void Weapon::transferNextShotStatsFrom(const Weapon &weapon) { ulog("shotxfer %d %d", (int)m_ownerId, (int)weapon.m_ownerId); }
/* Object.cpp:5855-5867, verbatim (batch R3) */
DockUpdateInterface *Object::getDockUpdateInterface( void )
{
	DockUpdateInterface *dock = NULL;

	for( BehaviorModule **u = m_behaviors; *u; ++u )
	{
		if( (dock = (*u)->getDockUpdateInterface()) != NULL )
			return dock;
	}

	return NULL;

}  // end getDockUpdateInterface
/* Object.cpp:724-741, verbatim */
Int Object::getTransportSlotCount() const
{
	Int count = getTemplate()->getRawTransportSlotCount();
	ContainModuleInterface* contain = getContain();
	if ( contain && contain->isSpecialZeroSlotContainer() )
	{
		count = 0;
		const ContainedItemsList* items = contain->getContainedItemsList();
		if (items)
		{
			for (ContainedItemsList::const_iterator it = items->begin(); it != items->end(); ++it)
			{
				count += (*it)->getTransportSlotCount();
			}
		}
	}
	return count;
}
/* the payload: the factory finds the scene's templates by name and makes nothing (the call is a line of the log) */
const ThingTemplate *ctFindTemplate(const AsciiString &name)
{
	for (int k = 0; k < g_ctN; ++k) if (g_ctObjs[k].obj->m_tmpl.m_name.compare(name) == 0) return &g_ctObjs[k].obj->m_tmpl;
	return 0;
}
Object *ctNewObject(const ThingTemplate *tmplate, Team *team) { ulog("newobject %s %d", tmplate->getName().str(), team ? (int)team->m_dummy : -1); return 0; }
/* --- end p1w13-sonnet --- */
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
		putU32(dynamic_cast<TransportContain *>(oc) ? 2 : 1);   /* --- p1w13-sonnet --- (batch R2): 2 marks a transport */
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
		/* --- p1w13-sonnet --- (batch R2: TransportContain) */
		if (TransportContain *tc = dynamic_cast<TransportContain *>(oc)) {
			putU32(tc->m_payloadCreated ? 1 : 0);
			putU32((unsigned)tc->m_extraSlotsInUse);
			putU32(tc->m_frameExitNotBusy);
		}
		/* --- end p1w13-sonnet --- */
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
		/* --- p1w13-sonnet --- (batch R2: TransportContain) */
		const TransportContainModuleData *td = dynamic_cast<const TransportContainModuleData *>(d);
		putU32(td ? g_ctBlockCls : 0u);   /* p1w13-sonnet, batch R3: the class of the block */
		if (td) {
			putU32((unsigned)td->m_slotCapacity);
			putU32(realBits(td->m_exitPitchRate));
			putStr(td->m_exitBone.str(), td->m_exitBone.getLength());
			putStr(td->m_initialPayload.name.str(), td->m_initialPayload.name.getLength());
			putU32((unsigned)td->m_initialPayload.count);
			putU32(realBits(td->m_healthRegen));
			putU32(td->m_exitDelay);
			putU32((td->m_scatterNearbyOnExit ? 1u : 0u) | (td->m_orientLikeContainerOnExit ? 2u : 0u) | (td->m_keepContainerVelocityOnExit ? 4u : 0u) |
				(td->m_goAggressiveOnExit ? 8u : 0u) | (td->m_armedRidersUpgradeWeaponSet ? 16u : 0u) | (td->m_resetMoodCheckTimeOnExit ? 32u : 0u) |
				(td->m_destroyRidersWhoAreNotFreeToExit ? 64u : 0u) | (td->m_isDelayExitInAir ? 128u : 0u));
		}
		/* --- end p1w13-sonnet --- */
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
		/* --- p1w13-sonnet --- (batch R2: TransportContain) */
		unsigned cls; int rawSlots; unsigned airborne; int freeToExit; unsigned hasLoco, validTerrain, com, vel[3], weapons, unmanned, extraKinds, exitBoneHas, exitBone[3];
		int slotCap; unsigned regen, exitDelay, tflags, pitch; int payloadCount; unsigned payloadName, exitBoneName, health, hasDock, dockOpen;
		/* --- end p1w13-sonnet --- */
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
		/* --- p1w13-sonnet --- (batch R2: TransportContain) */
		o.cls = c.u(); o.rawSlots = c.s(); o.airborne = c.u(); o.freeToExit = c.s(); o.hasLoco = c.u(); o.validTerrain = c.u(); o.com = c.u();
		for (int i = 0; i < 3; ++i) o.vel[i] = c.u();
		o.weapons = c.u(); o.unmanned = c.u(); o.extraKinds = c.u(); o.exitBoneHas = c.u(); for (int i = 0; i < 3; ++i) o.exitBone[i] = c.u();
		o.slotCap = c.s(); o.regen = c.u(); o.exitDelay = c.u(); o.tflags = c.u(); o.pitch = c.u(); o.payloadCount = c.s(); o.payloadName = c.u(); o.exitBoneName = c.u(); o.health = c.u(); o.hasDock = c.u(); o.dockOpen = c.u();
		/* --- end p1w13-sonnet --- */
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
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	g_findTemplateHook = ctFindTemplate;
	g_newObjectHook = ctNewObject;
	/* --- end p1w13-sonnet --- */
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
		/* --- p1w13-sonnet --- (batch R2: TransportContain) */
		a.cls = k == 0 ? g_ctBlockCls : o.cls;   /* batch R3: the first container's class is the block's */
		a.validTerrain = o.validTerrain;
		obj->m_tmpl.m_rawTransportSlotCount = o.rawSlots;
		obj->m_airborneLoco = o.airborne != 0;
		a.ai.m_freeToExit = o.freeToExit;
		a.ai.m_hasLoco = o.hasLoco != 0;
		memset(a.physData, 0, sizeof(a.physData));
		((PhysicsBehaviorModuleData *)a.physData)->m_centerOfMassOffset = ctF(o.com);
		phys->m_moduleData = (const ModuleData *)a.physData;
		phys->m_vel.x = ctF(o.vel[0]); phys->m_vel.y = ctF(o.vel[1]); phys->m_vel.z = ctF(o.vel[2]);
		for (int s = 0; s < 3; ++s) {
			a.weap[s] = Weapon();
			a.weap[s].m_tmplObj.m_contact = (o.weapons >> (3 * s + 1) & 1) != 0;
			a.weap[s].m_damageW = (o.weapons >> (3 * s + 2) & 1) != 0;
			a.weap[s].m_ownerId = k + 1;
			obj->m_slotWeapon[s] = (o.weapons >> (3 * s) & 1) ? &a.weap[s] : 0;
		}
		if (o.unmanned) obj->m_disabledBits |= 1u << (unsigned)DISABLED_UNMANNED;
		if (o.extraKinds & 1) obj->m_kindOf.set(KINDOF_CLIFF_JUMPER);
		if (o.extraKinds & 2) obj->m_kindOf.set(KINDOF_HERO);
		if (o.extraKinds & 4) obj->m_kindOf.set(KINDOF_SALVAGER);
		a.draw.m_exitBoneHas = o.exitBoneHas != 0;
		a.draw.m_exitBone.x = ctF(o.exitBone[0]); a.draw.m_exitBone.y = ctF(o.exitBone[1]); a.draw.m_exitBone.z = ctF(o.exitBone[2]);
		/* --- end p1w13-sonnet --- */
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
		a.dock = 0;   /* p1w13-sonnet, batch R3 */
		a.oc = 0;
		a.data = 0;
		obj->m_body = 0;
		obj->m_contain = 0;
		obj->m_behaviors = a.behaviors;
		a.behaviors[0] = a.behaviors[1] = a.behaviors[2] = a.behaviors[3] = 0;
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
			a.body->m_health = ctF(o.health);   /* --- p1w13-sonnet --- (batch R2) */
			a.obj->m_effDead = o.effDead != 0;   /* InactiveBody::InactiveBody made its object effectively dead: the scene says whether it is */
			a.obj->m_body = a.body;
			a.behaviors[idx++] = a.body;
		}
		if (o.hasContain) {
			TransportContainModuleData *tdata = 0;   /* --- p1w13-sonnet --- (batch R2: TransportContain) */
			if (k == 0) a.data = (OpenContainModuleData *)d;
			else {
				if (o.cls) {
					tdata = new TransportContainModuleData;
					tdata->m_slotCapacity = o.slotCap;
					tdata->m_healthRegen = ctF(o.regen);
					tdata->m_exitDelay = o.exitDelay;
					tdata->m_orientLikeContainerOnExit = (o.tflags & 1) != 0;
					tdata->m_keepContainerVelocityOnExit = (o.tflags & 2) != 0;
					tdata->m_goAggressiveOnExit = (o.tflags & 4) != 0;
					tdata->m_armedRidersUpgradeWeaponSet = (o.tflags & 8) != 0;
					tdata->m_resetMoodCheckTimeOnExit = (o.tflags & 16) != 0;
					tdata->m_destroyRidersWhoAreNotFreeToExit = (o.tflags & 32) != 0;
					tdata->m_isDelayExitInAir = (o.tflags & 64) != 0;
					tdata->m_exitPitchRate = ctF(o.pitch);
					tdata->m_initialPayload.count = o.payloadCount;
					static const char *const kPayload[4] = { "", "T1", "T2", "Nope" };
					tdata->m_initialPayload.name = kPayload[o.payloadName % 4];
					static const char *const kExitBone[3] = { "", "ExitBoneT", "Other" };
					tdata->m_exitBone = kExitBone[o.exitBoneName % 3];
					a.data = tdata;
				} else
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
			try {
				/* --- p1w13-sonnet --- (batch R2: TransportContain) */
				/* the memory pool's zero fill: the constructor leaves m_payloadCreated (batch R3: the derived transports by class) */
				const unsigned cls = k == 0 ? g_ctBlockCls : o.cls;
				ModuleData *td = k == 0 ? d : (ModuleData *)tdata;
				if (cls == 1) a.oc = new(ShimZeroFill{}) TransportContain(a.obj, td);
				else if (cls == 2) a.oc = new(ShimZeroFill{}) InternetHackContain(a.obj, td);
				else if (cls == 3) a.oc = new(ShimZeroFill{}) RailedTransportContain(a.obj, td);
				else
				/* --- end p1w13-sonnet --- */
				a.oc = new OpenContain(a.obj, a.data);
			}
			CATCH_ALL(status, code, msg, msgCap)
			if (status) { failed = true; break; }
			a.obj->m_contain = a.oc;
			a.behaviors[idx++] = a.oc;
		}
		/* --- p1w13-sonnet --- (batch R3) the dock, after the contain (the Rust side's dock probe is the last module too) */
		if (o.hasDock) {
			a.dock = new CtDock(a.obj, new ModuleData);
			a.dock->m_open = o.dockOpen != 0;
			a.behaviors[idx++] = a.dock;
		}
		/* --- end p1w13-sonnet --- */
	}
	if (failed) {
		TheGameLogic = 0; TheAudio = 0; TheInGameUI = 0; TheAI = 0; ThePartitionManager = 0; TheTerrainLogic = 0;
		g_modFindHook = 0; g_modDestroyHook = 0; g_teamRelHook = 0; g_xformHook = 0; g_multiBoneHook = 0;
		g_findTemplateHook = 0; g_newObjectHook = 0;   /* --- p1w13-sonnet --- (batch R2: TransportContain) */
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
				case 25: if (oc2) result = oc2->isPassengerAllowedToFire((o[3] & 1) ? r2->getID() : INVALID_ID) ? 1 : 0; break;   /* p1w13-sonnet, batch R2: the rider's id when the fourth word is odd */
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
				case 46: t->obj->m_containedBy = ((o[3] & 1) || r2 == t->obj) ? 0 : r2; break;
				case 47: if (oc2) for (unsigned j = 0; j < 1 + o[3] % 12; ++j) { if (o[4] & 1) oc2->exitObjectInAHurry(r2); else oc2->exitObjectViaDoor(r2, DOOR_1); } break;
				case 48: if (oc2) for (unsigned j = 0; j < 1 + o[3] % 12; ++j) oc2->putObjAtNextFirePoint(r2); break;
				/* --- p1w13-sonnet --- (batch R2: TransportContain) */
				case 49: if (oc2) result = (unsigned)oc2->reserveDoorForExit(0, (o[3] & 1) ? 0 : r2); break;
				case 50: if (oc2) result = oc2->isExitBusy() ? 1 : 0; break;
				case 51: if (oc2) result = (unsigned)oc2->getContainMax(); break;
				case 52: if (oc2) result = (unsigned)oc2->getExtraSlotsInUse(); break;
				case 53: if (oc2) oc2->onCapture(o[3] % 5 < 4 ? &g_ctPlayers[o[3] % 5] : 0, o[4] % 5 < 4 ? &g_ctPlayers[o[4] % 5] : 0); break;
				case 54: if (oc2) oc2->killRidersWhoAreNotFreeToExit(); break;
				case 55: if (oc2 && t->cls) result = ((TransportContain *)oc2)->isSpecificRiderFreeToExit((o[3] & 1) ? 0 : r2) ? 1 : 0; break;
				/* --- end p1w13-sonnet --- */
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
	g_findTemplateHook = 0; g_newObjectHook = 0;   /* --- p1w13-sonnet --- (batch R2: TransportContain) */
	return 0;
}

int ref_l9ct_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	/* A planted Rust bug can steer past the guard that makes a ring of containers undefined (the Rust side records it and the case is never sent), and the C++
	** then walks the ring for ever (isPassengerAllowedToFire). An alarm turns that into the same status 99 as a crash: a mismatch, never a hang. A real case
	** takes milliseconds; the unplanted run has none that reaches the alarm. */
	static bool timedOut = false;   /* once one case has hung, the run is already a mismatch: the rest are not walked */
	if (timedOut) { *len = 0; return 99; }
	struct sigaction sa, old;
	memset(&sa, 0, sizeof sa);
	sa.sa_handler = l9tCrashHandler;
	sigemptyset(&sa.sa_mask);
	sigaction(SIGALRM, &sa, &old);
	/* the timer's signal goes to THIS thread: a process-directed alarm could land on another, and the jump would leave its stack */
	struct sigevent ev;
	memset(&ev, 0, sizeof ev);
	ev.sigev_notify = SIGEV_THREAD_ID;
	ev.sigev_signo = SIGALRM;
	ev._sigev_un._tid = (int)syscall(SYS_gettid);
	timer_t timer;
	bool armed = timer_create(CLOCK_MONOTONIC, &ev, &timer) == 0;
	if (armed) { struct itimerspec its; memset(&its, 0, sizeof its); its.it_value.tv_sec = 5; timer_settime(timer, 0, &its, 0); }
	int rc = l9TrappedRun(l9ctRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
	if (armed) timer_delete(timer);
	sigaction(SIGALRM, &old, 0);
	if (rc == 99) timedOut = true;   /* (a segmentation fault ends the walk the same way) */
	return rc;
}

}  /* extern "C" */
