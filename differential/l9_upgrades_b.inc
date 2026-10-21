/* Differential-test driver for six more layer-9 upgrade modules (p1w10-sonnet, upgrade batch B; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/StatusBitsUpgrade.cpp, LocomotorSetUpgrade.cpp, PassengersFireUpgrade.cpp,
**   StealthUpgrade.cpp, SubObjectsUpgrade.cpp, ObjectCreationUpgrade.cpp   (#included, unchanged)
** with their headers unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_upgrades.inc, whose effect log and
** helpers it uses) on the stand-ins of differential/shim_module: the object has the AI, contain and spawn behaviour the test gives it, a
** drawable that logs the sub-objects it is told to show and hide, and the object creation list the test names. The REAL UpgradeCenter and
** INI. Every call a module makes on its object is a line of the effect log (ulog), the object's state is its status bits. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/StatusBitsUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/LocomotorSetUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/PassengersFireUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/StealthUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/SubObjectsUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ObjectCreationUpgrade.cpp"

void AIUpdateInterface::setLocomotorUpgrade(Bool set) { ulog("locoupg %d", (int)set); }
void SpawnBehaviorInterface::giveSlavesStealthUpgrade(Bool giveUpgrade) { ulog("stealthslaves %d", (int)giveUpgrade); }
void Drawable::showSubObject(const AsciiString &name, Bool show) { ulog("subshow %s %d", name.str(), (int)show); }
void Drawable::updateSubObjects() { ulog("subupdate"); }

/* the contain module of an object that has one: every pure virtual of ContainModuleInterface answers "no" (generated from ContainModule.h by
** the script that wrote differential/l9_contain_stub.inc); setPassengerAllowedToFire is the call PassengersFireUpgrade makes */
class UpgContain : public ContainModuleInterface
{
public:
	virtual OpenContain * asOpenContain() { return 0; }
	virtual void containReactToTransformChange() {  }
	virtual Bool isGarrisonable() const { return false; }
	virtual Bool isBustable() const { return false; }
	virtual Bool isSpecialZeroSlotContainer() const { return false; }
	virtual Bool isHealContain() const { return false; }
	virtual Bool isTunnelContain() const { return false; }
	virtual Bool isRiderChangeContain() const { return false; }
	virtual Bool isImmuneToClearBuildingAttacks() const { return false; }
	virtual Bool isSpecialOverlordStyleContainer() const { return false; }
	virtual Bool isAnyRiderAttacking() const { return false; }
	virtual void clientVisibleContainedFlashAsSelected() {  }
	virtual void onObjectWantsToEnterOrExit(Object* obj, ObjectEnterExitType wants) {  }
	virtual Bool hasObjectsWantingToEnterOrExit() const { return false; }
	virtual const Player* getApparentControllingPlayer(const Player* observingPlayer) const { return 0; }
	virtual void recalcApparentControllingPlayer() {  }
	virtual void onContaining(Object *obj, Bool wasSelected) {  }
	virtual void onRemoving(Object *obj) {  }
	virtual void onCapture(Player *oldOwner, Player *newOwner) {  }
	virtual void onSelling() {  }
	virtual Int getContainMax() const { return 0; }
	virtual ExitInterface* getContainExitInterface() { return 0; }
	virtual void orderAllPassengersToExit(CommandSourceType, Bool instantly) {  }
	virtual void orderAllPassengersToIdle(CommandSourceType) {  }
	virtual void orderAllPassengersToHackInternet(CommandSourceType) {  }
	virtual void markAllPassengersDetected() {  }
	virtual Bool isValidContainerFor(const Object* obj, Bool checkCapacity) const { return false; }
	virtual void addToContain(Object *obj) {  }
	virtual void addToContainList(Object *obj) {  }
	virtual void removeFromContain(Object *obj, Bool exposeStealthUnits = FALSE) {  }
	virtual void removeAllContained(Bool exposeStealthUnits = FALSE) {  }
	virtual void killAllContained(void) {  }
	virtual void harmAndForceExitAllContained(DamageInfo *info) {  }
	virtual Bool isEnclosingContainerFor(const Object *obj) const { return false; }
	virtual Bool isPassengerAllowedToFire(ObjectID id = INVALID_ID) const { return false; }
	virtual void setOverrideDestination(const Coord3D *) {  }
	virtual Bool isDisplayedOnControlBar() const { return false; }
	virtual Int getExtraSlotsInUse(void) { return 0; }
	virtual Bool isKickOutOnCapture() { return false; }
	virtual void iterateContained(ContainIterateFunc func, void *userData, Bool reverse) {  }
	virtual UnsignedInt getContainCount() const { return 0; }
	virtual const ContainedItemsList* getContainedItemsList() const { return 0; }
	virtual const Object * friend_getRider() const { return 0; }
	virtual Real getContainedItemsMass() const { return 0; }
	virtual UnsignedInt getStealthUnitsContained() const { return 0; }
	virtual Bool calcBestGarrisonPosition(Coord3D *sourcePos, const Coord3D *targetPos) { return false; }
	virtual Bool attemptBestFirePointPosition(Object *source, Weapon *weapon, Object *victim) { return false; }
	virtual Bool attemptBestFirePointPosition(Object *source, Weapon *weapon, const Coord3D *targetPos) { return false; }
	virtual PlayerMaskType getPlayerWhoEntered(void) const { return 0; }
	virtual void processDamageToContained(Real percentDamage) {  }
	virtual Object* getClosestRider(const Coord3D *pos) { return 0; }
	virtual void enableLoadSounds(Bool enable) {  }
	virtual void setEvacDisposition(EvacDisposition disp) {  }
	virtual Bool isWeaponBonusPassedToPassengers() const { return false; }
	virtual WeaponBonusConditionFlags getWeaponBonusPassedToPassengers() const { return WeaponBonusConditionFlags(); }
	virtual void setPassengerAllowedToFire(Bool permission = TRUE) { ulog("pfire %d", (int)permission); }
};

/* the object creation lists INI::parseObjectCreationList finds: OCL_A and OCL_B (case matters); `None` and any other name are NULL */
static ObjectCreationList g_upOcls[2];
const ObjectCreationList *ObjectCreationListStore::findObjectCreationList(const char *name) const
{
	if (strcmp(name, "OCL_A") == 0) return &g_upOcls[0];
	if (strcmp(name, "OCL_B") == 0) return &g_upOcls[1];
	return 0;
}
/* ObjectCreationList::create(ocl, primary, NULL) reaches this */
Object *ObjectCreationList::createInternal(const Object *primary, const Object *secondary, UnsignedInt lifetimeFrames) const
{
	ulog("ocl %d", this == &g_upOcls[0] ? 0 : 1);
	return 0;
}

namespace {

ModuleData *l9upbRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			switch (kind) {
			case 0: data = StatusBitsUpgrade::friend_newModuleData(&ini); break;
			case 1: data = LocomotorSetUpgrade::friend_newModuleData(&ini); break;
			case 2: data = PassengersFireUpgrade::friend_newModuleData(&ini); break;
			case 3: data = StealthUpgrade::friend_newModuleData(&ini); break;
			case 4: data = SubObjectsUpgrade::friend_newModuleData(&ini); break;
			default: data = ObjectCreationUpgrade::friend_newModuleData(&ini); break;
			}
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

/* a status mask as the indices of its set bits */
void putStatus(const ObjectStatusMaskType &m)
{
	std::vector<int> bits;
	for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) if (m.test((ObjectStatusTypes)i)) bits.push_back(i);
	putU32((unsigned)bits.size());
	for (size_t i = 0; i < bits.size(); ++i) putI32(bits[i]);
}

}  // namespace

struct SubProbe : public SubObjectsUpgrade {
	SubProbe(Thing *t, const ModuleData *d) : SubObjectsUpgrade(t, d) {}
	bool probe() { return isSubObjectsUpgrade(); }
};

extern "C" {

/* Read `filename` (block `D`) as the data of upgrade module `kind` (0 StatusBits, 1 LocomotorSet, 2 PassengersFire, 3 Stealth, 4 SubObjects,
** 5 ObjectCreation) and make the module on an Object whose controlling player is a Player with completed upgrades `player_mask` (none if
** `has_player` is 0) and whose own are `obj_mask`. `cfg`: [has drawable, has AI, has contain, has spawn behaviour, KINDOF_SPAWNS_ARE_THE_WEAPONS].
** The data dump goes first (the mux, then the module's own fields). Then `n` ops of 5 words [op, a, b, c, d]: 0 wouldUpgrade  1 attemptUpgrade
** 2 resetUpgrade  3 forceRefreshUpgrade  4 isAlreadyUpgraded  (m = a..d as 128 bits)  5 the object's completed upgrades = m  6 the player's
** completed upgrades = m  7 the object's status bits = a | b << 32. Each op appends: the CATCH_ALL status, the Bool result, isAlreadyUpgraded(), the
** effect log (count, then each as a string), the status bits of the object (the indices of the set bits). */
int ref_l9upb_run(int kind, const char *filename, int has_player, const unsigned *player_mask, const unsigned *obj_mask, const unsigned *cfg,
	int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9upbRead(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	putMux(((UpgradeModuleData *)d)->m_upgradeMuxData);
	switch (kind) {
	case 0: putStatus(((StatusBitsUpgradeModuleData *)d)->m_statusToSet); putStatus(((StatusBitsUpgradeModuleData *)d)->m_statusToClear); break;
	case 4:
	{
		const SubObjectsUpgradeModuleData *s = (SubObjectsUpgradeModuleData *)d;
		putU32((unsigned)s->m_showSubObjectNames.size());
		for (size_t i = 0; i < s->m_showSubObjectNames.size(); ++i) putAscii(s->m_showSubObjectNames[i]);
		putU32((unsigned)s->m_hideSubObjectNames.size());
		for (size_t i = 0; i < s->m_hideSubObjectNames.size(); ++i) putAscii(s->m_hideSubObjectNames[i]);
		break;
	}
	case 5:
	{
		const ObjectCreationList *o = ((ObjectCreationUpgradeModuleData *)d)->m_ocl;
		putI32(o == 0 ? 0 : (o == &g_upOcls[0] ? 1 : 2));
		break;
	}
	default: break;
	}
	Player *player = has_player ? new Player : 0;
	if (player) player->m_upgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, player_mask);
	Object *obj = newObject();
	obj->m_player = player;
	obj->m_objectUpgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, obj_mask);
	if (cfg[0]) obj->m_drawable = new Drawable;
	AIUpdateInterface ai;
	UpgContain contain;
	SpawnBehaviorInterface spawn;
	if (cfg[1]) obj->m_ai = &ai;
	if (cfg[2]) obj->m_contain = &contain;
	if (cfg[3]) obj->m_spawn = &spawn;
	if (cfg[4]) obj->m_kindOf.set(KINDOF_SPAWNS_ARE_THE_WEAPONS);
	obj->m_xp = new ExperienceTracker;
	UpgradeModule *m = 0;
	switch (kind) {
	case 0: m = new StatusBitsUpgrade(obj, d); break;
	case 1: m = new LocomotorSetUpgrade(obj, d); break;
	case 2: m = new PassengersFireUpgrade(obj, d); break;
	case 3: m = new StealthUpgrade(obj, d); break;
	case 4: m = new SubProbe(obj, d); break;
	default: m = new ObjectCreationUpgrade(obj, d); break;
	}
	if (kind == 4) g_dump.push_back(((SubProbe *)m)->probe() ? 1 : 0);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		g_events.clear();
		g_ulog.clear();
		int st = 0, res = 0;
		unsigned c2 = 0;
		char m2[16];
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		try {
			switch (o[0]) {
			case 0: res = m->wouldUpgrade(mask) ? 1 : 0; break;
			case 1: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 2: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 3: m->forceRefreshUpgrade(); break;
			case 4: res = m->isAlreadyUpgraded() ? 1 : 0; break;
			case 5: obj->m_objectUpgradesCompleted = mask; break;
			case 6: if (player) player->m_upgradesCompleted = mask; break;
			case 7:
				obj->m_status.clear(obj->m_status);
				for (int b = 0; b < 64 && b < OBJECT_STATUS_COUNT; ++b) {
					unsigned long long w = (unsigned long long)o[1] | ((unsigned long long)o[2] << 32);
					if ((w >> b) & 1) obj->m_status.set((ObjectStatusTypes)b);
				}
				break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		g_dump.push_back((unsigned char)res);
		g_dump.push_back(m->isAlreadyUpgraded() ? 1 : 0);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putStatus(obj->m_status);
	}
	*len = finish(buf, cap);
	obj->m_ai = 0; obj->m_contain = 0; obj->m_spawn = 0;
	delete m;
	delete d;
	delete obj;
	delete player;
	return 0;
}

}  /* extern "C" */
