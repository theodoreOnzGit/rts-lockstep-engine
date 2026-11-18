/* Differential-test wrapper for the PropagandaCenterBehavior draft (#215, upstream commit 0a05454d),
** src/drafts/logic_common/propaganda_center_behavior.rs:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PropagandaCenterBehavior.cpp from
**   `PropagandaCenterBehavior::PropagandaCenterBehavior( Thing *thing` to `void PropagandaCenterBehavior::crc` (the constructor and
**   destructor, onDelete, update, onRemoving), cut by build.rs into propaganda.inc with ONE mechanical edit (asserted once): in `update`
**   the loop variable `it` is read after the `for` that declares it (VC6's for-scope), so the copy declares `it` before the loop. (Upstream's file is under
**   `#ifdef ALLOW_SURRENDER`, which the shipped game does not define; the cut is compiled as if it were.)
**
** Stand-ins (everything the cut calls outside itself; the engine's classes of these names are not read): the class
** PropagandaCenterBehavior (its three members and the functions the cut defines, as the header), on a PrisonBehavior base whose
** onDelete, update and onRemoving record their call and whose getContainList, reserveDoorForExit and exitObjectViaDoor are the
** test's; the module data (m_brainwashDuration); Object (getID, getTemplate, getControllingPlayer, getAIUpdateInterface,
** setTemporaryTeam, restoreOriginalTeam, each recording its call); Player::getDefaultTeam; TheGameLogic (findObjectByID over
** the ids 1 to 8 the test says exist; getFrame); AIUpdateInterface::setSurrendered; UPDATE_SLEEP_NONE; DOOR_NONE_AVAILABLE.
** DEBUG_ASSERTCRASH is the shim's release-build no-op.
**
** Entry point (extern "C", tests/differential_draft_propaganda.rs):
**   ref_propaganda_run(duration, ops, nops, out) runs the ops on one behaviour (constructed first). Each op is 12 words
**     [kind, frame, exist mask (bit i = object i+1), ai mask, has_player, door_available, n_contained, contained x4 ids, remove id]; kind 0 =
**     update(), 1 = onRemoving(remove id), 2 = onDelete(). Events as words: [20] base update, [21] base onDelete, [22 id] base
**     onRemoving, [11 id] reserveDoorForExit, [12 id] setTemporaryTeam, [13 id] setSurrendered, [14 id] exitObjectViaDoor,
**     [10 id] restoreOriginalTeam; after each op [30, subject, start frame, list size, the list's ids newest first]. Returns the
**     number of words. */
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include <list>
#include <vector>
#define NULL 0

/* every stand-in and the cut are in an anonymous namespace: other differential wrappers define classes of these names (Object, Player,
** Team ...) with other inline bodies, which must not be merged with these at link time */
namespace {

typedef int ObjectID;
static const ObjectID INVALID_ID = 0;
static std::vector<unsigned> theLog;
static void ev(unsigned a) { theLog.push_back(a); }
static void ev(unsigned a, unsigned b) { theLog.push_back(a); theLog.push_back(b); }

struct Team { int unused; };
static Team theTeam;
struct Player { Team *getDefaultTeam() { return &theTeam; } };
static Player thePlayer;
struct ThingTemplate { int unused; };
static ThingTemplate theTemplate;
struct AIUpdateInterface { ObjectID id; void setSurrendered(void *, Bool) { ev(13, id); } };
struct Object
{
	ObjectID id; bool hasPlayer; AIUpdateInterface *ai;
	ObjectID getID() const { return id; }
	const ThingTemplate *getTemplate() const { return &theTemplate; }
	Player *getControllingPlayer() const { return hasPlayer ? &thePlayer : 0; }
	AIUpdateInterface *getAIUpdateInterface() const { return ai; }
	void setTemporaryTeam(Team *) { ev(12, id); }
	void restoreOriginalTeam() { ev(10, id); }
};
static Object theObjects[8];
static AIUpdateInterface theAIs[8];
static unsigned theExistMask = 0;
static unsigned theFrame = 0;
struct GameLogicStandIn
{
	Object *findObjectByID(ObjectID id) { return (id >= 1 && id <= 8 && (theExistMask & (1u << (id - 1)))) ? &theObjects[id - 1] : 0; }
	UnsignedInt getFrame() const { return theFrame; }
};
static GameLogicStandIn theGL;
static GameLogicStandIn *TheGameLogic = &theGL;

enum UpdateSleepTime { UPDATE_SLEEP_NONE = 0 };
enum ExitDoorType { DOOR_1_COUNT = 1, DOOR_NONE_AVAILABLE = -1 };
struct Thing {};
struct ModuleData {};
struct PropagandaCenterBehaviorModuleData : public ModuleData { UnsignedInt m_brainwashDuration; };
static Object *theUs = 0;
static std::list<Object *> theContain;
static bool theDoorAvailable = false;

struct PrisonBehavior
{
	PrisonBehavior(Thing *, const ModuleData *) {}
	virtual ~PrisonBehavior() {}
	virtual void onDelete() { ev(21); }
	virtual UpdateSleepTime update() { ev(20); return UPDATE_SLEEP_NONE; }
	virtual void onRemoving(Object *obj) { ev(22, obj->getID()); }
	Object *getObject() const { return theUs; }
	std::list<Object *> &getContainList() { return theContain; }
	ExitDoorType reserveDoorForExit(const ThingTemplate *, Object *obj) { ev(11, obj->getID()); return theDoorAvailable ? DOOR_1_COUNT : DOOR_NONE_AVAILABLE; }
	void exitObjectViaDoor(Object *obj, ExitDoorType) { ev(14, obj->getID()); }
};

typedef std::list< ObjectID > BrainwashedIDList;
typedef BrainwashedIDList::const_iterator BrainwashedIDListContIterator;
typedef BrainwashedIDList::iterator BrainwashedIDListIterator;

class PropagandaCenterBehavior : public PrisonBehavior
{
public:
	PropagandaCenterBehavior( Thing *thing, const ModuleData *moduleData );
	virtual ~PropagandaCenterBehavior( void );
	virtual void onDelete( void );
	virtual UpdateSleepTime update( void );
	virtual void onRemoving( Object *obj );
	const PropagandaCenterBehaviorModuleData *md;
	const PropagandaCenterBehaviorModuleData *getPropagandaCenterBehaviorModuleData() const { return md; }
	ObjectID m_brainwashingSubjectID;
	UnsignedInt m_brainwashingSubjectStartFrame;
	BrainwashedIDList m_brainwashedList;
};

#include "propaganda.inc"

}

extern "C" int ref_propaganda_run(unsigned duration, const unsigned *ops, int nops, unsigned *out)
{
	theLog.clear();
	PropagandaCenterBehaviorModuleData md;
	md.m_brainwashDuration = duration;
	Thing thing; ModuleData mdata;
	PropagandaCenterBehavior b(&thing, &mdata);
	b.md = &md;
	Object us; us.id = 100; us.ai = 0; us.hasPlayer = true;
	theUs = &us;
	for (int k = 0; k < nops; ++k) {
		const unsigned *o = ops + 12 * k;
		theFrame = o[1];
		theExistMask = o[2];
		for (int i = 0; i < 8; ++i) {
			theObjects[i].id = i + 1;
			theObjects[i].hasPlayer = true;
			theAIs[i].id = i + 1;
			theObjects[i].ai = (o[3] & (1u << i)) ? &theAIs[i] : 0;
		}
		us.hasPlayer = o[4] != 0;
		theDoorAvailable = o[5] != 0;
		theContain.clear();
		for (unsigned i = 0; i < o[6]; ++i) theContain.push_back(&theObjects[o[7 + i] - 1]);
		if (o[0] == 0) b.update();
		else if (o[0] == 1) b.onRemoving(&theObjects[o[11] - 1]);
		else b.onDelete();
		ev(30, b.m_brainwashingSubjectID); ev(b.m_brainwashingSubjectStartFrame); ev((unsigned)b.m_brainwashedList.size());
		for (BrainwashedIDListIterator it = b.m_brainwashedList.begin(); it != b.m_brainwashedList.end(); ++it) ev(*it);
	}
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}
