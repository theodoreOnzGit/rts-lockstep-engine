/* Differential-test driver for ActiveBody (p1w13-sonnet, batch P; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/ActiveBody.cpp   (#included, with one mechanical edit: the seven function-local `static` particle
**   system templates lose their `static`, build.rs, mod_gen/ActiveBody_edit.cpp) with its header unchanged, in wrap_module.cpp's unit on the stand-ins of
** differential/shim_module: up to four Objects (each with an ActiveBody, a template with armor sets, a team and a player, a drawable, an AI, a contain module, and
** up to two probe behaviours that are damage modules, die modules, bridge behaviours and projectile interfaces), the REAL Armor (Armor.cpp, in the Armor archive:
** the template's armor and TheArmorStore), the REAL DamageFX functions (DamageFX.cpp's, cut by build.rs into this unit as ModTestDamageFX) and DamageInfo::xfer
** (Damage.cpp, the Armor archive), the REAL ArmorTemplateSetFinder (SparseMatchFinder.h), the REAL Object::getRelationship and Object::kill (Object.cpp, cut by
** build.rs), BridgeBehavior::getBridgeBehaviorInterfaceFromObject (BridgeBehavior.cpp, cut), the REAL INI, BitFlags and Xfer. Every call the body makes on the rest
** of the world is a line of the effect log (ulog), in the order it is made, in the text the Rust side's cut records, events and probe traces produce. */
/* BaseType.h's helpers, which the shim of Lib/BaseType.h leaves out */
template <typename NUM> inline NUM sqr(NUM x) { return x*x; }
#define min(x,y) (((x)<(y)) ? (x) : (y))
#define max(x,y) (((x)>(y)) ? (x) : (y))
#define REAL_TO_INT_CEIL(x) (ref_real_to_int_ceil(x))
extern "C" int ref_real_to_int_ceil(float x);
#include "ActiveBody_edit.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/HighlanderBody.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/ImmortalBody.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/StructureBody.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/InactiveBody.cpp"
#include "UndeadBody_edit.cpp"  /* UndeadBody.cpp with one mechanical edit made by build.rs (the second loop declares its own `update`) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/HiveStructureBody.cpp"
#include "DamageFX_cut.inc"            /* DamageFX::DamageFX, clear, getDamageFXThrottleTime, doDamageFX, getDamageFXList: verbatim from DamageFX.cpp (build.rs) */
#include "BridgeBehaviorFromObject.inc" /* BridgeBehavior::getBridgeBehaviorInterfaceFromObject: verbatim from BridgeBehavior.cpp (build.rs) */
#undef min
#undef max

UnsignedInt (*g_playerFrameHook)() = 0;

namespace {

/* which class's data the first block of the file was: 0 ActiveBody (block D), 1 UndeadBody (U), 2 HiveStructureBody (H) */
int g_abDataKind = 0;

ModuleData *l9abRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			const char *b = ini.m_buffer;
			while (*b == ' ' || *b == '\t') ++b;
			g_abDataKind = *b == 'U' ? 1 : *b == 'H' ? 2 : 0;
			if (g_abDataKind == 1) data = UndeadBody::friend_newModuleData(&ini);
			else if (g_abDataKind == 2) data = HiveStructureBody::friend_newModuleData(&ini);
			else data = ActiveBody::friend_newModuleData(&ini);
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

enum { AB_OBJS = 4, AB_PLAYERS = 4, AB_PS_CLASSES = 7, AB_ARMORS = 3, AB_FXS = 2, AB_TEMPLATES = 3, AB_SETS = 4, AB_INFOS = 12, AB_FXLISTS = 8 };

/* the kinds and statuses the scene's masks name: local bit i is kAbKinds[i], kAbStatus[i] (the Rust test has the same lists) */
const KindOfType kAbKinds[13] = { KINDOF_VEHICLE, KINDOF_STRUCTURE, KINDOF_PROJECTILE, KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER, KINDOF_CAN_BE_REPULSED, KINDOF_DRONE,
	KINDOF_CANNOT_RETALIATE, KINDOF_IMMOBILE, KINDOF_FS_INTERNET_CENTER, KINDOF_INFANTRY, KINDOF_FS_FACTORY, KINDOF_SELECTABLE };
const ObjectStatusTypes kAbStatus[8] = { OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_IS_USING_ABILITY,
	OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_AFLAME, OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_REPULSOR };

struct AbCfg
{
	const unsigned *p;
	size_t i;
	unsigned u() { return p[i++]; }
	int s() { return (int)p[i++]; }
	Real f() { Real r; unsigned v = p[i++]; memcpy(&r, &v, 4); return r; }
};

Real abF(unsigned v) { Real r; memcpy(&r, &v, 4); return r; }

std::string abInfoLine(const DamageInfo *i)
{
	char b[160];
	sprintf(b, "%d %d %d %d %08x %08x %08x %d %d %d", (int)i->in.m_sourceID, (int)i->in.m_damageType, (int)i->in.m_deathType, (int)i->in.m_damageFXOverride,
		realBits(i->in.m_amount), realBits(i->out.m_actualDamageDealt), realBits(i->out.m_actualDamageClipped), (int)i->out.m_noEffect, (int)i->in.m_kill,
		(int)i->in.m_damageStatusType);
	return b;
}

/* the scene */
struct AbObj;
AbObj *abFind(unsigned id);

class AbContain : public UpgContain
{
public:
	std::vector<Object *> m_items;
	bool m_garrisonable, m_immune, m_riderChange;
	Object *m_owner;
	mutable ContainedItemsList m_list;
	Object *m_closestRider;
	AbContain() : m_garrisonable(false), m_immune(false), m_riderChange(false), m_owner(0), m_closestRider(0) {}
	virtual Object *getClosestRider(const Coord3D *pos)
	{
		ulog("closestrider %d %08x %08x %08x", (int)m_owner->m_oid, realBits(pos->x), realBits(pos->y), realBits(pos->z));
		return m_closestRider;
	}
	virtual UnsignedInt getContainCount() const { return (UnsignedInt)m_items.size(); }
	virtual Bool isGarrisonable() const { return m_garrisonable; }
	virtual Bool isImmuneToClearBuildingAttacks() const { return m_immune; }
	virtual Bool isRiderChangeContain() const { return m_riderChange; }
	virtual const ContainedItemsList *getContainedItemsList() const
	{
		m_list.clear();
		for (size_t i = 0; i < m_items.size(); ++i) m_list.push_back(m_items[i]);
		return &m_list;
	}
	virtual void orderAllPassengersToIdle(CommandSourceType) { ulog("idleall %d", (int)m_owner->m_oid); }
	virtual void orderAllPassengersToHackInternet(CommandSourceType) { ulog("hackall %d", (int)m_owner->m_oid); }
};

/* a behaviour module with the damage, die, bridge and projectile interfaces (the Rust side's ProbeModule) */
class AbProbe : public BehaviorModule, public DamageModuleInterface, public DieModuleInterface, public BridgeBehaviorInterface, public ProjectileUpdateInterface,
	public SlowDeathBehaviorInterface
{
public:
	AbProbe(Thing *t, const ModuleData *d, int index) : BehaviorModule(t, d), m_index(index), m_bridge(false), m_projectile(false), m_slow(false), m_slowMask(0), m_slowProb(0)
	{
		for (int i = 0; i < 4; ++i) m_towers[i] = INVALID_ID;
	}
	virtual SlowDeathBehaviorInterface *getSlowDeathBehaviorInterface() { return m_slow ? this : 0; }
	/* SlowDeathBehaviorInterface (UndeadBody's second life asks every behaviour of the object) */
	virtual void beginSlowDeath(const DamageInfo *info) { ulog("slowbegin %d %s", (int)getObject()->m_oid, abInfoLine(info).c_str()); }
	virtual Int getProbabilityModifier(const DamageInfo *) const { return m_slowProb; }
	virtual Bool isDieApplicable(const DamageInfo *info) const
	{
		const int t = (int)info->in.m_damageType;
		return t >= 0 && t < 64 && ((m_slowMask >> t) & 1) != 0;
	}
	virtual NameKeyType getModuleNameKey() const { return NAMEKEY_INVALID; }
	virtual DamageModuleInterface *getDamage() { return this; }
	virtual DieModuleInterface *getDie() { return this; }
	virtual BridgeBehaviorInterface *getBridgeBehaviorInterface() { return m_bridge ? this : 0; }
	virtual ProjectileUpdateInterface *getProjectileUpdateInterface() { return m_projectile ? this : 0; }
	int me() const { return (int)AsObject(const_cast<Thing *>(static_cast<const Thing *>(getObject())))->m_oid; }
	/* DamageModuleInterface */
	virtual void onDamage(DamageInfo *info) { ulog("ondamage %d %d %s", (int)getObject()->m_oid, m_index, abInfoLine(info).c_str()); }
	virtual void onHealing(DamageInfo *info) { ulog("onhealing %d %d %s", (int)getObject()->m_oid, m_index, abInfoLine(info).c_str()); }
	virtual void onBodyDamageStateChange(const DamageInfo *info, BodyDamageType oldState, BodyDamageType newState)
	{
		ulog("bodystate %d %d %d %d %s", (int)getObject()->m_oid, m_index, (int)oldState, (int)newState, abInfoLine(info).c_str());
	}
	/* DieModuleInterface (the body's onDie is the Object's, which does not run the die modules here) */
	virtual void onDie(const DamageInfo *) {}
	/* BridgeBehaviorInterface */
	virtual void setTower(BridgeTowerType, Object *) {}
	virtual ObjectID getTowerID(BridgeTowerType t) { return m_towers[(int)t]; }
	virtual void createScaffolding(void) {}
	virtual void removeScaffolding(void) {}
	virtual Bool isScaffoldInMotion(void) { return false; }
	virtual Bool isScaffoldPresent(void) { return false; }
	/* ProjectileUpdateInterface */
	virtual void projectileLaunchAtObjectOrPosition(const Object *, const Coord3D *, const Object *, WeaponSlotType, Int, const WeaponTemplate *, const ParticleSystemTemplate *) {}
	virtual void projectileFireAtObjectOrPosition(const Object *, const Coord3D *, const WeaponTemplate *, const ParticleSystemTemplate *) {}
	virtual Bool projectileIsArmed() const { return false; }
	virtual ObjectID projectileGetLauncherID() const { return INVALID_ID; }
	virtual Bool projectileHandleCollision(Object *) { return false; }
	virtual void setFramesTillCountermeasureDiversionOccurs(UnsignedInt) {}
	virtual void projectileNowJammed() { ulog("jammed %d", (int)getObject()->m_oid); }
	int m_index;
	bool m_bridge, m_projectile, m_slow;
	unsigned long long m_slowMask;
	int m_slowProb;
	ObjectID m_towers[4];
};

struct AbObj
{
	Object *obj;
	BodyModule *body; /* the body, of whatever class (cls 4 is an InactiveBody, which is no ActiveBody) */
	unsigned cls;
	ActiveBodyModuleData *data;
	AbProbe *probe[2];
	int nProbes;
	BehaviorModule *behaviors[4];
	AbContain contain;
	SpawnBehaviorInterface spawn;
	AIUpdateInterface ai;
	Drawable draw;
	Team team;
};

AbObj g_abObjs[AB_OBJS];
int g_abN = 0;
Player g_abPlayers[AB_PLAYERS];
Team g_abNeutralTeam;
Relationship g_abRel[AB_OBJS][AB_OBJS];
int g_abAble[AB_OBJS][AB_OBJS];
Real g_abDist[AB_OBJS][AB_OBJS];
std::vector<Object *> g_abFriends;
std::map<std::pair<int, std::string>, std::vector<Coord3D> > g_abBones;
unsigned g_abDeadMod = 0, g_abDeadRem = 0;
std::vector<int> g_abSelection;
ArmorTemplate g_abArmors[AB_ARMORS];
ModTestDamageFX g_abDfx[AB_FXS];
FXList g_abFxLists[AB_FXLISTS];
ArmorStore g_abArmorStore;
std::vector<unsigned char> g_abSaved;
ParticleSystem g_abPsAny;
GameLogic *g_abLogic = 0;

AbObj *abFind(unsigned id)
{
	if (id < 1 || (int)id > g_abN) return 0;
	return &g_abObjs[id - 1];
}

Object *abFindObject(ObjectID id) { AbObj *a = abFind((unsigned)id); return a ? a->obj : 0; }
UnsignedInt abFrame() { return g_abLogic ? g_abLogic->m_frame : 0; }
void abFxHook(const Object *primary, const Object *secondary) { ulog("fxobj %d %d", primary ? (int)primary->m_oid : 0, secondary ? (int)secondary->m_oid : 0); }
void abSetStatusHook(Object *o, ObjectStatusMaskType mask, Bool set)
{
	/* the status mask as the Rust side's SetStatus record prints it: the two 32-bit words */
	unsigned w[2] = { 0, 0 };
	for (int i = 0; i < OBJECT_STATUS_COUNT && i < 64; ++i) if (mask.test(i)) w[i / 32] |= 1u << (i % 32);
	ulog("setstatus %d %08x%08x %d", (int)o->m_oid, w[1], w[0], (int)set);
}
void abSetTeamHook(Object *o, Team *team) { ulog("setteam %d", team ? team->m_dummy : -1); }
Relationship abTeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return g_abRel[from->m_dummy - 1][to->m_dummy - 1];
}

void abRangeHook(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	const PartitionFilterPlayerAffiliation *f = (const PartitionFilterPlayerAffiliation *)filters[0];
	ulog("friends %08x %08x %08x %08x %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), f->m_player ? (int)((const Player *)f->m_player)->m_idx : -1);
}

class AbAudio : public AudioManager { public: virtual void stopAudio(AudioAffect) {} };
class AbUi : public InGameUI {};

}  // namespace

/* ---- the calls the body makes on the rest of the world ---- */
void Object::scoreTheKill(const Object *victim) { ulog("score %d %d", (int)m_oid, (int)victim->m_oid); }
void Object::onDie(DamageInfo *damageInfo) { ulog("ondie %d %s", (int)m_oid, abInfoLine(damageInfo).c_str()); }
void Object::setDisabled(DisabledType type) { ulog("setdisabled %d %d", (int)m_oid, (int)type); }
void Object::doStatusDamage(ObjectStatusTypes status, Real duration) { ulog("statusdamage %d %d %d", (int)m_oid, (int)status, (int)duration); }
void Object::notifySubdualDamage(Real amount) { ulog("subdual %d %08x", (int)m_oid, realBits(amount)); }
void Object::setGeometryInfoZ(Real newZ)
{
	m_geometry.setMaxHeightAbovePosition(newZ);
	if (m_drawable) m_drawable->reactToGeometryChange();
}
ProjectileUpdateInterface *Object::getProjectileUpdateInterface() const
{
	for (BehaviorModule **m = m_behaviors; *m; ++m) {
		ProjectileUpdateInterface *p = (*m)->getProjectileUpdateInterface();
		if (p) return p;
	}
	return 0;
}
Int Object::getMultiLogicalBonePosition(const char *boneNamePrefix, Int maxBones, Coord3D *positions, Matrix3D *transforms, Bool convertToWorld) const
{
	ulog("bones %d %s %d", (int)m_oid, boneNamePrefix, (int)maxBones);
	if (!m_drawable) return 0;
	std::map<std::pair<int, std::string>, std::vector<Coord3D> >::const_iterator it = g_abBones.find(std::make_pair((int)m_oid, std::string(boneNamePrefix)));
	if (it == g_abBones.end()) return 0;
	Int n = (Int)it->second.size();
	if (n > maxBones) n = maxBones;
	for (Int i = 0; i < n; ++i) positions[i] = it->second[(size_t)i];
	return n;
}
Object *SpawnBehaviorInterface::getClosestSlave(const Coord3D *pos)
{
	ulog("closestslave %d %08x %08x %08x", m_ownerId, realBits(pos->x), realBits(pos->y), realBits(pos->z));
	return m_closestSlave;
}
Bool Object::isFactionStructure() const { return isAnyKindOf(KINDOFMASK_FS); }
CanAttackResult Object::getAbleToAttackSpecificObject(AbleToAttackType, const Object *target, CommandSourceType, WeaponSlotType) const
{
	ulog("canattack %d %d", (int)m_oid, (int)target->m_oid);
	return (CanAttackResult)g_abAble[m_oid - 1][target->m_oid - 1];
}
void AIUpdateInterface::aiIdle(CommandSourceType) { ulog("aiidle %d", (int)m_owner->m_oid); }
void AIUpdateInterface::aiEvacuateInstantly(Bool exposeStealthUnits, CommandSourceType) { ulog("aievac %d %d", (int)m_owner->m_oid, (int)exposeStealthUnits); }
void AIUpdateInterface::aiGuardRetaliate(Object *victim, const Coord3D *pos, Int, CommandSourceType)
{
	ulog("guard %d %d %08x %08x %08x", (int)m_owner->m_oid, (int)victim->m_oid, realBits(pos->x), realBits(pos->y), realBits(pos->z));
}
void PowerAcademyStats::recordVehicleSniped() { ulog("sniped"); }
void PowerAcademyStats::recordClearedGarrisonedBuilding() { ulog("garrisoncleared %d", (int)((Player *)(void *)this)->m_idx); }
void GameLogic::deselectObject(Object *objToDeselect, PlayerMaskType playerMask, Bool affectClient)
{
	ulog("deselect %d %u %d", (int)objToDeselect->m_oid, (unsigned)playerMask, (int)affectClient);
}
void Drawable::reactToBodyDamageStateChange(int newState) { ulog("react %d %d", m_ownerObject ? (int)m_ownerObject->m_oid : 0, newState); }
Real PartitionManager::getDistanceSquared(const Object *obj, const Object *otherObj, DistanceCalculationType, Coord3D *) const
{
	ulog("distsq %d %d", (int)obj->m_oid, (int)otherObj->m_oid);
	return g_abDist[obj->m_oid - 1][otherObj->m_oid - 1];
}
void ParticleSystem::attachToObject(const Object *obj) { ulog("psattach %u %d", m_id, (int)obj->m_oid); }
void ParticleSystem::destroy(void) { ulog("psdestroy %u", m_id); }
ParticleSystem *ParticleSystemManager::findParticleSystem(ParticleSystemID id)
{
	if (g_abDeadMod != 0 && id % g_abDeadMod == g_abDeadRem) return 0;
	g_abPsAny.m_id = id;
	return &g_abPsAny;
}
Drawable *InGameUI::getFirstSelectedDrawable() { return g_abSelection.empty() ? 0 : &g_abObjs[g_abSelection[0]].draw; }
Int InGameUI::getSelectCount() { return (Int)g_abSelection.size(); }

namespace {

/* XferStatus thrown, or SC_INVALID_DATA (which the Rust side reports as XferStatus::ListNotEmpty) */
int abRunXfer(Snapshot *s, int kind, const char *path, unsigned char *bytes, int cap, int *len, unsigned *crc)
{
	int st = 0;
	*len = 0;
	if (kind == 0) {
		{
			XferSave x;
			try { x.open(AsciiString(path)); x.xferSnapshot(s); } catch (XferStatus e) { st = (int)e; } catch (SaveCode c) { st = c == SC_INVALID_DATA ? (int)XFER_LIST_NOT_EMPTY : -1; } catch (...) { st = -1; }
			try { x.close(); } catch (...) {}
		}
		if (FILE *f = fopen(path, "rb")) { *len = (int)fread(bytes, 1, cap, f); fclose(f); }
	} else if (kind == 1) {
		if (FILE *f = fopen(path, "wb")) { if (cap > 0) fwrite(bytes, 1, cap, f); fclose(f); }
		{
			XferLoad x;
			try { x.open(AsciiString(path)); x.xferSnapshot(s); } catch (XferStatus e) { st = (int)e; } catch (SaveCode c) { st = c == SC_INVALID_DATA ? (int)XFER_LIST_NOT_EMPTY : -1; } catch (...) { st = -1; }
			try { x.close(); } catch (...) {}
		}
	} else {
		XferCRC x;
		try { x.open(AsciiString("crc")); x.xferSnapshot(s); x.close(); } catch (XferStatus e) { st = (int)e; } catch (SaveCode c) { st = c == SC_INVALID_DATA ? (int)XFER_LIST_NOT_EMPTY : -1; } catch (...) { st = -1; }
		*crc = x.getCRC();
	}
	return st;
}

void abPutDamageInfo(const DamageInfo *i)
{
	putU32(i->in.m_sourceID);
	putU32(i->in.m_sourcePlayerMask);
	putI32((int)i->in.m_damageType);
	putI32((int)i->in.m_damageStatusType);
	putI32((int)i->in.m_damageFXOverride);
	putI32((int)i->in.m_deathType);
	putU32(realBits(i->in.m_amount));
	putU32(i->in.m_kill ? 1 : 0);
	putU32(realBits(i->in.m_shockWaveVector.x));
	putU32(realBits(i->in.m_shockWaveVector.y));
	putU32(realBits(i->in.m_shockWaveVector.z));
	putU32(realBits(i->in.m_shockWaveAmount));
	putU32(realBits(i->in.m_shockWaveRadius));
	putU32(realBits(i->in.m_shockWaveTaperOff));
	const char *tn = i->in.m_sourceTemplate ? i->in.m_sourceTemplate->getName().str() : "";
	putStr(tn, (int)strlen(tn));
	putU32(realBits(i->out.m_actualDamageDealt));
	putU32(realBits(i->out.m_actualDamageClipped));
	putU32(i->out.m_noEffect ? 1 : 0);
}

/* the state of an object: the body's words (ActiveBody::state_words on the Rust side), its last damage, then the object's status bits, effectively-dead flag and
** geometry height */
void abPutObject(AbObj *a)
{
	if (a->cls == 4) {
		/* an InactiveBody has no fields of the ActiveBody layout: its virtuals, in the same 18 words (the Rust side asks the module the same questions) */
		BodyModule *ib = a->body;
		putU32(realBits(ib->getHealth()));
		putU32(realBits(ib->getPreviousHealth()));
		putU32(realBits(ib->getMaxHealth()));
		putU32(realBits(ib->getInitialHealth()));
		putU32(realBits(ib->getCurrentSubdualDamageAmount()));
		putU32((unsigned)ib->getDamageState());
		putU32(ib->getSubdualDamageHealRate());
		putU32((ib->hasAnySubdualDamage() ? 1u : 0u) ^ realBits(ib->getSubdualDamageHealAmount()));
		putU32(ib->getLastDamageTimestamp());
		putU32(ib->getLastHealingTimestamp());
		putU32(ib->getFrontCrushed() ? 1 : 0);
		putU32(ib->getBackCrushed() ? 1 : 0);
		putU32((unsigned)ib->getClearableLastAttacker());
		putU32(ib->isIndestructible() ? 1 : 0);
		unsigned fl = 0;
		for (int i = 0; i < ARMORSET_COUNT; ++i) if (ib->testArmorSetFlag((ArmorSetType)i)) fl |= 1u << i;
		putU32(fl);
		putU32(realBits(ib->getDamageScalar()));
		putU32(ib->getLastDamageInfo() ? 1u : 0u);
		putU32(0);
		DamageInfo none;
		abPutDamageInfo(&none);
	} else {
	ActiveBody *b = (ActiveBody *)a->body;
	putU32(realBits(b->m_currentHealth));
	putU32(realBits(b->m_prevHealth));
	putU32(realBits(b->m_maxHealth));
	putU32(realBits(b->m_initialHealth));
	putU32(realBits(b->m_currentSubdualDamage));
	putU32((unsigned)b->m_curDamageState);
	putU32(b->m_nextDamageFXTime);
	putU32((unsigned)b->m_lastDamageFXDone);
	putU32(b->m_lastDamageTimestamp);
	putU32(b->m_lastHealingTimestamp);
	putU32(b->m_frontCrushed ? 1 : 0);
	putU32(b->m_backCrushed ? 1 : 0);
	putU32(b->m_lastDamageCleared ? 1 : 0);
	putU32(b->m_indestructible ? 1 : 0);
	unsigned flags = 0;
	for (int i = 0; i < ARMORSET_COUNT; ++i) if (b->m_curArmorSetFlags.test(i)) flags |= 1u << i;
	putU32(flags);
	putU32(realBits(b->getDamageScalar()));
	unsigned setIdx = 0xffffffffu;
	if (b->m_curArmorSet) setIdx = (unsigned)(b->m_curArmorSet - &a->obj->m_tmpl.m_armorTemplateSets[0]);
	putU32(setIdx);
	unsigned n = 0;
	for (BodyParticleSystem *s = b->m_particleSystems; s; s = s->m_next) ++n;
	putU32(n);
	for (BodyParticleSystem *s = b->m_particleSystems; s; s = s->m_next) putU32(s->m_particleSystemID);
	abPutDamageInfo(&b->m_lastDamageInfo);
	}
	unsigned w[2] = { 0, 0 };
	for (int i = 0; i < OBJECT_STATUS_COUNT && i < 64; ++i) if (a->obj->m_status.test(i)) w[i / 32] |= 1u << (i % 32);
	putU32(w[0]);
	putU32(w[1]);
	putU32(a->obj->m_effDead ? 1 : 0);
	putU32(realBits(a->obj->m_geometry.getMaxHeightAbovePosition()));
	/* the structure's constructor object, the undead body's second life flag */
	putU32(a->cls == 3 || a->cls == 6 ? (unsigned)((StructureBody *)a->body)->getConstructorObjectID() : a->cls == 5 ? (unsigned)((UndeadBody *)a->body)->m_isSecondLife : 0u);
}

/* the effect log of one op: the events in order (the FX lists, the UI refresh) and the ulog lines */
void abPutLog()
{
	std::vector<std::string> log;
	for (size_t j = 0; j < g_events.size(); ++j) {
		const Event &e = g_events[j];
		if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		else if (e.kind == 1) {
			/* FXList::doFXObj of the real header does nothing for a NULL list; the stand-in logs it (and the hook's line follows): drop both */
			if (!e.p) { if (j + 1 < g_events.size() && g_events[j + 1].kind == 100 && strncmp(g_ulog[(size_t)g_events[j + 1].p].c_str(), "fxobj", 5) == 0) ++j; continue; }
			char b[32]; sprintf(b, "fx %d", fxId((const FXList *)e.p)); log.push_back(b);
		}
		else if (e.kind == 3) log.push_back("uidirty");
	}
	putU32((unsigned)log.size());
	for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of an ActiveBody for object 1 of the scene `cfg` describes (see tests/differential_body_active.rs, the one place the
** layout is written), and make the bodies. The data dump goes first (the five fields). Then `n` ops of 5 words [op, a, b, c, d] (the test's table). Each op, and the
** construction, appends: the CATCH_ALL status, the op's result word, the effect log (count, then each as a string), the xfer status, bytes and CRC, the state of every
** object (abPutObject) and of every player (the attacked-by bits and frame). */
static int l9abRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_abSaved.clear();
	g_ahPsNames.clear();
	g_ahPsTemplates.clear();
	g_ahPsNext = 0;
	g_ahPsLastId = 0;
	g_abBones.clear();
	g_abFriends.clear();
	g_abSelection.clear();
	g_abDeadMod = g_abDeadRem = 0;
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9abRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const ActiveBodyModuleData *ad = (const ActiveBodyModuleData *)d;
		putU32((unsigned)g_abDataKind);
		if (g_abDataKind == 1) putU32(realBits(((const UndeadBodyModuleData *)d)->m_secondLifeMaxHealth));
		if (g_abDataKind == 2) {
			const HiveStructureBodyModuleData *hd = (const HiveStructureBodyModuleData *)d;
			unsigned long long pm = 0, sm = 0;
			for (int t = 0; t < 38; ++t) { if (hd->m_damageTypesToPropagateToSlaves.test(t)) pm |= 1ull << t; if (hd->m_damageTypesToSwallow.test(t)) sm |= 1ull << t; }
			putU32((unsigned)pm); putU32((unsigned)(pm >> 32)); putU32((unsigned)sm); putU32((unsigned)(sm >> 32));
		}
		putU32(realBits(ad->m_maxHealth));
		putU32(realBits(ad->m_initialHealth));
		putU32(realBits(ad->m_subdualDamageCap));
		putU32(ad->m_subdualDamageHealRate);
		putU32(realBits(ad->m_subdualDamageHealAmount));
	}
	initKindOfMasks();

	AbCfg c = { cfg, 0 };
	GameLogic gl;
	gl.m_frame = c.u();
	g_abLogic = &gl;
	TheGameLogic = &gl;
	const unsigned logicSeed = c.u(), clientSeed = c.u();
	/* the globals */
	TheGlobalData->m_unitDamagedThresh = c.f();
	TheGlobalData->m_unitReallyDamagedThresh = c.f();
	TheGlobalData->m_defaultStructureRubbleHeight = c.f();
	for (int i = 0; i < 4; ++i) TheGlobalData->m_healthBonus[i] = c.f();
	AI ai;
	ai.m_aiData.m_enableRepulsors = c.u() != 0;
	ai.m_aiData.m_retaliateFriendsRadius = c.f();
	ai.m_aiData.m_maxRetaliateDistance = c.f();
	{
		/* the auto particle classes: fire small, medium, large, smoke small, medium, large, aflame */
		AsciiString *prefix[AB_PS_CLASSES] = { &TheGlobalData->m_autoFireParticleSmallPrefix, &TheGlobalData->m_autoFireParticleMediumPrefix, &TheGlobalData->m_autoFireParticleLargePrefix,
			&TheGlobalData->m_autoSmokeParticleSmallPrefix, &TheGlobalData->m_autoSmokeParticleMediumPrefix, &TheGlobalData->m_autoSmokeParticleLargePrefix,
			&TheGlobalData->m_autoAflameParticlePrefix };
		AsciiString *system[AB_PS_CLASSES] = { &TheGlobalData->m_autoFireParticleSmallSystem, &TheGlobalData->m_autoFireParticleMediumSystem, &TheGlobalData->m_autoFireParticleLargeSystem,
			&TheGlobalData->m_autoSmokeParticleSmallSystem, &TheGlobalData->m_autoSmokeParticleMediumSystem, &TheGlobalData->m_autoSmokeParticleLargeSystem,
			&TheGlobalData->m_autoAflameParticleSystem };
		Int *max[AB_PS_CLASSES] = { &TheGlobalData->m_autoFireParticleSmallMax, &TheGlobalData->m_autoFireParticleMediumMax, &TheGlobalData->m_autoFireParticleLargeMax,
			&TheGlobalData->m_autoSmokeParticleSmallMax, &TheGlobalData->m_autoSmokeParticleMediumMax, &TheGlobalData->m_autoSmokeParticleLargeMax,
			&TheGlobalData->m_autoAflameParticleMax };
		for (int k = 0; k < AB_PS_CLASSES; ++k) {
			char b[32];
			sprintf(b, "BONE%d", k);
			*prefix[k] = b;
			*max[k] = c.s();
			unsigned nm = c.u();
			if (nm == 0) *system[k] = "None";
			else { sprintf(b, "PS%u", nm); *system[k] = b; }
		}
	}
	const int neutralTeam = c.s();
	/* the armor store and the damage FX */
	{
		unsigned nArmors = c.u();
		for (unsigned a = 0; a < nArmors; ++a) {
			g_abArmors[a].clear();
			for (int t = 0; t < DAMAGE_NUM_TYPES; ++t) g_abArmors[a].m_damageCoefficient[t] = c.f();
		}
		unsigned nFx = c.u();
		for (unsigned f = 0; f < nFx; ++f) {
			g_abDfx[f].clear();
			DamageFX::DFX def;
			def.m_amountForMajorFX = c.f();
			unsigned maj = c.u(), mnr = c.u();
			def.m_majorDamageFXList = maj ? &g_abFxLists[maj - 1] : 0;
			def.m_minorDamageFXList = mnr ? &g_abFxLists[mnr - 1] : 0;
			def.m_damageFXThrottleTime = c.u();
			for (int t = 0; t < DAMAGE_NUM_TYPES; ++t) for (int v = LEVEL_FIRST; v <= LEVEL_LAST; ++v) g_abDfx[f].m_dfx[t][v] = def;
			unsigned nOver = c.u();
			for (unsigned o = 0; o < nOver; ++o) {
				int t = c.s(), v = c.s();
				DamageFX::DFX x;
				x.m_amountForMajorFX = c.f();
				unsigned mj = c.u(), mn = c.u();
				x.m_majorDamageFXList = mj ? &g_abFxLists[mj - 1] : 0;
				x.m_minorDamageFXList = mn ? &g_abFxLists[mn - 1] : 0;
				x.m_damageFXThrottleTime = c.u();
				g_abDfx[f].m_dfx[t][v] = x;
			}
		}
	}
	/* the templates */
	struct TmplCfg { Byte rubble; unsigned audio[6]; unsigned nSets; unsigned mask[AB_SETS], armor[AB_SETS], fx[AB_SETS]; } tmpls[AB_TEMPLATES];
	unsigned nTmpl = c.u();
	for (unsigned t = 0; t < nTmpl; ++t) {
		tmpls[t].rubble = (Byte)c.s();
		for (int i = 0; i < 6; ++i) tmpls[t].audio[i] = c.u();
		tmpls[t].nSets = c.u();
		for (unsigned s = 0; s < tmpls[t].nSets; ++s) { tmpls[t].mask[s] = c.u(); tmpls[t].armor[s] = c.u(); tmpls[t].fx[s] = c.u(); }
	}
	/* the players */
	for (int p = 0; p < AB_PLAYERS; ++p) {
		g_abPlayers[p] = Player();
		g_abPlayers[p].m_idx = p;
		g_abPlayers[p].m_type = c.u() ? PLAYER_COMPUTER : PLAYER_HUMAN;
		g_abPlayers[p].m_retaliation = c.u() != 0;
		g_abPlayers[p].m_defaultTeam = &g_abNeutralTeam;
	}
	g_abNeutralTeam.m_dummy = neutralTeam;
	g_abNeutralTeam.m_controller = &g_abPlayers[0];
	/* the objects */
	g_abN = (int)c.u();
	struct ObjCfg { unsigned tmpl, kinds, status; int player; int vet; unsigned pos[3]; unsigned containedBy; unsigned drawable, hasAi, moving, idle, hasContain, garrisonable, immune, riderChange, nProbes;
		unsigned nItems; unsigned items[3]; unsigned probeBridge[2], probeProj[2]; unsigned towers[4]; unsigned bodyData[5]; unsigned cls;
		unsigned secondLife, propLo, propHi, swallowLo, swallowHi, spawn, closestSlave, closestRider; unsigned probeSlow[2], slowLo[2], slowHi[2]; int slowProb[2]; } oc[AB_OBJS];
	for (int k = 0; k < g_abN; ++k) {
		ObjCfg &o = oc[k];
		o.tmpl = c.u(); o.kinds = c.u(); o.status = c.u(); o.player = c.s(); o.vet = c.s();
		for (int i = 0; i < 3; ++i) o.pos[i] = c.u();
		o.containedBy = c.u(); o.drawable = c.u(); o.hasAi = c.u(); o.moving = c.u(); o.idle = c.u(); o.hasContain = c.u(); o.garrisonable = c.u(); o.immune = c.u(); o.riderChange = c.u();
		o.nProbes = c.u();
		o.nItems = c.u();
		for (int i = 0; i < 3; ++i) o.items[i] = c.u();
		for (int i = 0; i < 2; ++i) { o.probeBridge[i] = c.u(); o.probeProj[i] = c.u(); }
		for (int i = 0; i < 4; ++i) o.towers[i] = c.u();
		for (int i = 0; i < 5; ++i) o.bodyData[i] = c.u();
		o.cls = c.u();
		o.secondLife = c.u(); o.propLo = c.u(); o.propHi = c.u(); o.swallowLo = c.u(); o.swallowHi = c.u();
		o.spawn = c.u(); o.closestSlave = c.u(); o.closestRider = c.u();
		for (int i = 0; i < 2; ++i) { o.probeSlow[i] = c.u(); o.slowLo[i] = c.u(); o.slowHi[i] = c.u(); o.slowProb[i] = c.s(); }
	}
	for (int i = 0; i < g_abN; ++i) for (int j = 0; j < g_abN; ++j) g_abRel[i][j] = (Relationship)c.s();
	/* the damage infos */
	DamageInfo infos[AB_INFOS];
	unsigned nInfos = c.u();
	for (unsigned i = 0; i < nInfos; ++i) {
		DamageInfo &di = infos[i];
		di.in.m_sourceID = (ObjectID)c.u();
		di.in.m_damageType = (DamageType)c.s();
		di.in.m_deathType = (DeathType)c.s();
		di.in.m_damageFXOverride = (DamageType)c.s();
		di.in.m_amount = c.f();
		di.in.m_kill = c.u() != 0;
		di.in.m_damageStatusType = (ObjectStatusTypes)c.s();
	}
	/* the tables: attack results, distances, bones, dead particle systems */
	for (int i = 0; i < g_abN; ++i) for (int j = 0; j < g_abN; ++j) g_abAble[i][j] = c.s();
	for (int i = 0; i < g_abN; ++i) for (int j = 0; j < g_abN; ++j) g_abDist[i][j] = c.f();
	{
		unsigned nb = c.u();
		for (unsigned i = 0; i < nb; ++i) {
			int ob = c.s(), cls = c.s();
			unsigned cnt = c.u();
			char name[16];
			sprintf(name, "BONE%d", cls);
			std::vector<Coord3D> v;
			for (unsigned j = 0; j < cnt; ++j) { Coord3D p; p.x = c.f(); p.y = c.f(); p.z = c.f(); v.push_back(p); }
			g_abBones[std::make_pair(ob, std::string(name))] = v;
		}
	}
	g_abDeadMod = c.u();
	g_abDeadRem = c.u();
	const unsigned nameCount = 8;
	(void)nameCount;

	/* build the world */
	AbAudio audio;
	TheAudio = &audio;
	AbUi ui;
	TheInGameUI = &ui;
	ControlBar controlBar;
	TheControlBar = &controlBar;
	TheAI = &ai;
	PlayerList playerList;
	ThePlayerList = &playerList;
	g_tbNeutral = &g_abPlayers[0];
	g_playerFrameHook = abFrame;
	g_modFindHook = abFindObject;
	g_fxObjHook = abFxHook;
	g_setStatusHook = abSetStatusHook;
	g_setTeamHook = abSetTeamHook;
	TheArmorStore = &g_abArmorStore;
	ParticleSystemManager psm;
	TheParticleSystemManager = &psm;
	ThePartitionManager = &g_ahPm;
	g_ahRange.clear();
	g_ahRangeHook = abRangeHook;
	g_teamRelHook = abTeamRel;
	InitRandom(clientSeed);
	InitGameLogicRandom(logicSeed);
	g_events.clear();
	g_ulog.clear();

	bool failed = false;
	for (int k = 0; k < g_abN && !failed; ++k) {
		AbObj &a = g_abObjs[k];
		const ObjCfg &o = oc[k];
		a.obj = newObject();
		Object *obj = a.obj;
		obj->m_oid = k + 1;
		obj->m_pos.x = abF(o.pos[0]); obj->m_pos.y = abF(o.pos[1]); obj->m_pos.z = abF(o.pos[2]);
		obj->m_level = (VeterancyLevel)o.vet;
		for (int i = 0; i < 13; ++i) if (o.kinds >> i & 1) obj->m_kindOf.set(kAbKinds[i]);
		for (int i = 0; i < 8; ++i) if (o.status >> i & 1) obj->m_status.set(kAbStatus[i]);
		a.team.m_dummy = k + 1;
		a.team.m_controller = o.player >= 0 ? &g_abPlayers[o.player] : 0;
		obj->m_team = &a.team;   /* an object always has a team; a team need not have a player */
		obj->m_player = o.player >= 0 ? &g_abPlayers[o.player] : 0;
		{
			ModTestThingTemplate &tt = obj->m_tmpl;
			char nm[16];
			sprintf(nm, "T%d", k + 1);
			tt.m_name = nm;
			const TmplCfg &tc = tmpls[o.tmpl];
			tt.m_structureRubbleHeight = tc.rubble;
			AudioEventRTS *ev[6] = { &tt.m_soundOnDamaged, &tt.m_soundOnReallyDamaged, &tt.m_voiceFear, &tt.m_soundPromotedVeteran, &tt.m_soundPromotedElite, &tt.m_soundPromotedHero };
			for (int i = 0; i < 6; ++i) { if (tc.audio[i]) { char sn[16]; sprintf(sn, "snd%u", tc.audio[i]); ev[i]->setEventName(AsciiString(sn)); } }
			tt.m_armorTemplateSets.clear();
			for (unsigned s = 0; s < tc.nSets; ++s) {
				ArmorTemplateSet set;
				for (int i = 0; i < ARMORSET_COUNT; ++i) if (tc.mask[s] >> i & 1) set.m_types.set(i, 1);
				set.m_template = tc.armor[s] ? &g_abArmors[tc.armor[s] - 1] : 0;
				set.m_fx = tc.fx[s] ? &g_abDfx[tc.fx[s] - 1] : 0;
				tt.m_armorTemplateSets.push_back(set);
			}
		}
		a.draw.m_ownerObject = obj;
		obj->m_drawable = o.drawable ? &a.draw : 0;
		a.ai.m_owner = obj;
		a.ai.m_moving = o.moving != 0;
		a.ai.m_idle = o.idle != 0;
		obj->m_ai = o.hasAi ? &a.ai : 0;
		a.contain.m_owner = obj;
		a.contain.m_garrisonable = o.garrisonable != 0;
		a.contain.m_immune = o.immune != 0;
		a.contain.m_riderChange = o.riderChange != 0;
		a.contain.m_items.clear();
		obj->m_contain = o.hasContain ? &a.contain : 0;
		/* the spawn behaviour is the one of probe (spawn - 1), when the object has that many */
		a.spawn.m_ownerId = k + 1;
		a.spawn.m_closestSlave = 0;
		obj->m_spawn = (o.spawn != 0 && o.spawn <= o.nProbes) ? &a.spawn : 0;
		/* the body */
		if (k == 0) a.data = (ActiveBodyModuleData *)d;
		else {
			if (o.cls == 5) {
				UndeadBodyModuleData *ud = new UndeadBodyModuleData;
				ud->m_secondLifeMaxHealth = abF(o.secondLife);
				a.data = ud;
			} else if (o.cls == 6) {
				HiveStructureBodyModuleData *hd = new HiveStructureBodyModuleData;
				for (int t = 0; t < 38; ++t) {
					if ((t < 32 ? o.propLo >> t : o.propHi >> (t - 32)) & 1) hd->m_damageTypesToPropagateToSlaves.set(t, 1);
					if ((t < 32 ? o.swallowLo >> t : o.swallowHi >> (t - 32)) & 1) hd->m_damageTypesToSwallow.set(t, 1);
				}
				a.data = hd;
			} else a.data = new ActiveBodyModuleData;
			a.data->m_maxHealth = abF(o.bodyData[0]);
			a.data->m_initialHealth = abF(o.bodyData[1]);
			a.data->m_subdualDamageCap = abF(o.bodyData[2]);
			a.data->m_subdualDamageHealRate = o.bodyData[3];
			a.data->m_subdualDamageHealAmount = abF(o.bodyData[4]);
		}
		a.nProbes = (int)o.nProbes;
		ModuleData *probeData = new BehaviorModuleData;
		for (int i = 0; i < a.nProbes; ++i) {
			a.probe[i] = new AbProbe(obj, probeData, i + 1);
			a.probe[i]->m_bridge = o.probeBridge[i] != 0;
			a.probe[i]->m_projectile = o.probeProj[i] != 0;
			a.probe[i]->m_slow = o.probeSlow[i] != 0;
			a.probe[i]->m_slowMask = ((unsigned long long)o.slowHi[i] << 32) | o.slowLo[i];
			a.probe[i]->m_slowProb = o.slowProb[i];
			for (int t = 0; t < 4; ++t) a.probe[i]->m_towers[t] = (ObjectID)o.towers[t];
		}
		a.body = 0;
		obj->m_body = 0;
		obj->m_behaviors = a.behaviors;
		for (int i = 0; i < 4; ++i) a.behaviors[i] = 0;
	}
	/* what names another object, once every object exists */
	for (int k = 0; k < g_abN; ++k) {
		AbObj &a = g_abObjs[k];
		const ObjCfg &o = oc[k];
		a.obj->m_containedBy = o.containedBy ? abFind(o.containedBy)->obj : 0;
		for (unsigned i = 0; i < o.nItems; ++i) if (o.items[i]) a.contain.m_items.push_back(abFind(o.items[i])->obj);
		a.spawn.m_closestSlave = o.closestSlave ? abFind(o.closestSlave)->obj : 0;
		a.contain.m_closestRider = o.closestRider ? abFind(o.closestRider)->obj : 0;
	}
	/* the bodies, once every object they may name exists (the constructor reads its object's template) */
	for (int k = 0; k < g_abN && !failed; ++k) {
		AbObj &a = g_abObjs[k];
		status = 0;
		a.cls = oc[k].cls;
		try {
			switch (a.cls) {
			case 1: a.body = new HighlanderBody(a.obj, a.data); break;
			case 2: a.body = new ImmortalBody(a.obj, a.data); break;
			case 3: a.body = new StructureBody(a.obj, a.data); break;
			case 4: a.body = new InactiveBody(a.obj, a.data); break;
			case 5: a.body = new UndeadBody(a.obj, a.data); break;
			case 6: a.body = new HiveStructureBody(a.obj, a.data); break;
			default: a.body = new ActiveBody(a.obj, a.data); break;
			}
		}
		CATCH_ALL(status, code, msg, msgCap)
		if (status) { failed = true; break; }
		a.obj->m_body = a.body;
		a.behaviors[0] = a.body;
		for (int i = 0; i < a.nProbes; ++i) a.behaviors[1 + i] = a.probe[i];
		a.behaviors[1 + a.nProbes] = 0;
	}
	if (failed) {
		TheGameLogic = 0; TheAudio = 0; TheInGameUI = 0; TheControlBar = 0; TheAI = 0; ThePlayerList = 0; TheParticleSystemManager = 0; ThePartitionManager = 0;
		g_modFindHook = 0; g_fxObjHook = 0; g_setStatusHook = 0; g_setTeamHook = 0; g_playerFrameHook = 0; g_tbNeutral = 0; TheArmorStore = 0; g_ahRangeHook = 0; g_teamRelHook = 0;
		return 5;
	}

	putU32((unsigned)g_abN);
	for (int phase = 0; phase < 1 + n; ++phase) {
		const unsigned *o = phase ? ops + 5 * (phase - 1) : 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[2048];
		int blen = 0;
		int xst = -2;
		unsigned result = 0;
		g_events.clear();
		g_ulog.clear();
		if (phase) {
			AbObj *t = (o[0] <= 16 || (o[0] >= 29 && o[0] <= 36)) && (int)o[1] < g_abN ? &g_abObjs[o[1]] : 0;
			try {
				switch (o[0]) {
				case 0: { DamageInfo di = infos[o[2]]; t->obj->attemptDamage(&di); break; }
				case 1: { DamageInfo di = infos[o[2]]; t->body->attemptHealing(&di); break; }
				case 2: { DamageInfoInput in = infos[o[2]].in; result = realBits(t->body->estimateDamage(in)); break; }
				case 3: t->body->setDamageState((BodyDamageType)o[2]); break;
				case 4: t->body->setInitialHealth((Int)o[2]); break;
				case 5: { Real h; memcpy(&h, &o[2], 4); if (o[3] == 4) t->body->setMaxHealth(h); else t->body->setMaxHealth(h, (MaxHealthChangeType)o[3]); break; }
				case 6: { Real dl; memcpy(&dl, &o[2], 4); t->body->internalChangeHealth(dl); break; }
				case 7: t->body->setIndestructible(o[2] != 0); break;
				case 8: {
					t->obj->m_status.set(OBJECT_STATUS_AFLAME, o[2] != 0);
					t->body->setAflame(o[2] != 0);
					break;
				}
				case 9: t->body->onVeterancyLevelChanged((VeterancyLevel)o[2], (VeterancyLevel)o[3], o[4] != 0); break;
				case 10:
					if (o[2] == 0) t->body->setArmorSetFlag((ArmorSetType)o[3]);
					else if (o[2] == 1) t->body->clearArmorSetFlag((ArmorSetType)o[3]);
					else result = t->body->testArmorSetFlag((ArmorSetType)o[3]) ? 1 : 0;
					break;
				case 11: if (o[2] == 0) t->body->setFrontCrushed(o[3] != 0); else t->body->setBackCrushed(o[3] != 0); break;
				case 12: t->body->clearLastAttacker(); break;
				case 13: t->body->evaluateVisualCondition(); break;
				case 14: t->body->updateBodyParticleSystems(); break;
				case 15: t->body->onDelete(); break;
				case 16: { Real s; memcpy(&s, &o[2], 4); t->body->applyDamageScalar(s); break; }
				case 17: gl.m_frame = o[1]; break;
				case 18: {
					AbObj *x = &g_abObjs[o[1]];
					for (int i = 0; i < 8; ++i) if (o[2] >> i & 1) { x->obj->m_status.set(kAbStatus[i], o[3] != 0); }
					break;
				}
				case 19: {
					AbObj *x = &g_abObjs[o[1]];
					x->obj->m_kindOf.clear();
					for (int i = 0; i < 13; ++i) if (o[2] >> i & 1) x->obj->m_kindOf.set(kAbKinds[i]);
					break;
				}
				case 21: g_abPlayers[o[1] & 3].m_retaliation = o[2] != 0; g_abPlayers[o[1] & 3].m_type = o[3] ? PLAYER_COMPUTER : PLAYER_HUMAN; break;
				case 22: g_abAble[o[1] & 3][o[2] & 3] = (int)o[3]; break;
				case 23: { Real r; memcpy(&r, &o[3], 4); g_abDist[o[1] & 3][o[2] & 3] = r; break; }
				case 24: {
					g_abFriends.clear();
					g_ahRange.clear();
					for (unsigned i = 0; i < o[1] && i < 3; ++i) { AbObj *x = abFind(o[2 + i]); if (x) g_ahRange.push_back(x->obj); }
					break;
				}
				case 26: {
					AbObj *x = &g_abObjs[o[1] & 3];
					switch (o[2]) {
					case 0: x->ai.m_moving = o[3] != 0; break;
					case 1: x->ai.m_idle = o[3] != 0; break;
					case 2: x->contain.m_immune = o[3] != 0; break;
					case 3: x->contain.m_riderChange = o[3] != 0; break;
					case 4: x->contain.m_garrisonable = o[3] != 0; break;
					default:
						x->contain.m_items.clear();
						for (unsigned i = 0; i < o[3] && i < 3; ++i) { AbObj *y = abFind((o[4] >> (4 * i)) & 15); if (y) x->contain.m_items.push_back(y->obj); }
						break;
					}
					break;
				}
				case 27: g_abSelection.clear(); for (unsigned i = 0; i < o[1] && i < 3; ++i) { int x = (int)((o[2] >> (4 * i)) & 3); if (x < g_abN) g_abSelection.push_back(x); } break;
				case 28: g_abObjs[o[1] & 3].obj->m_containedBy = o[2] ? abFind(o[2])->obj : 0; break;
				case 29: xst = abRunXfer(t->body, 0, xfer_path, bytes, 2048, &blen, &crc); g_abSaved.assign(bytes, bytes + blen); break;
				case 30: {
					std::vector<unsigned char> b = g_abSaved;
					if (b.size() > 2048) b.resize(2048);
					xst = abRunXfer(t->body, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				case 31: xst = abRunXfer(t->body, 2, "crc", bytes, 0, &blen, &crc); break;
				case 32: t->obj->m_effDead = o[2] != 0; break;
				case 33: t->obj->m_drawable = o[2] ? &t->draw : 0; break;
				case 34: t->obj->m_level = (VeterancyLevel)o[2]; break;
				case 35: if (t->cls != 4) { ActiveBody *ab = (ActiveBody *)t->body; ab->m_particleSystems = ab->m_particleSystems; } break;
				case 36: if (t->cls == 3 || t->cls == 6) ((StructureBody *)t->body)->setConstructorObject(o[2] ? abFind(o[2])->obj : 0); break;
				default: break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
		}
		putI32(st);
		putU32(result);
		abPutLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		for (int k = 0; k < g_abN; ++k) abPutObject(&g_abObjs[k]);
		for (int p = 0; p < AB_PLAYERS; ++p) {
			unsigned m = 0;
			for (int i = 0; i < 16; ++i) if (g_abPlayers[p].m_attackedBy[i]) m |= 1u << i;
			putU32(m);
			putU32(g_abPlayers[p].m_attackedFrame);
		}
	}
	*len = finish(buf, cap);
	for (int k = 0; k < g_abN; ++k) {
		AbObj &a = g_abObjs[k];
		delete a.body;
		for (int i = 0; i < a.nProbes; ++i) delete a.probe[i];
		if (k != 0) delete a.data;
		delete a.obj;
	}
	delete d;
	TheGameLogic = 0; TheAudio = 0; TheInGameUI = 0; TheControlBar = 0; TheAI = 0; ThePlayerList = 0; TheParticleSystemManager = 0; ThePartitionManager = 0;
	g_modFindHook = 0; g_fxObjHook = 0; g_setStatusHook = 0; g_setTeamHook = 0; g_playerFrameHook = 0; g_tbNeutral = 0; TheArmorStore = 0; g_ahRangeHook = 0; g_teamRelHook = 0;
	return 0;
}

int ref_l9ab_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9abRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
