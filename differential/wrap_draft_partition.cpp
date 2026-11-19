/* Differential-test wrapper for the partition manager drafts (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/partition_manager.rs (get_controlling_player_index, friend_set_shroudedness_previous,
** reset_contact_list), cut verbatim by build.rs (drafts_guilogic3) from PartitionManager.cpp into:
**   pm_ctl.inc     PartitionData::getControllingPlayerIndex (to the `friend_setShroudednessPrevious` comment);
**   pm_shroud.inc  PartitionData::friend_setShroudednessPrevious (to getShroudedStatus);
**   pm_reset.inc   PartitionContactList::resetContactList (to processContactList).
** Stand-ins (outside the cuts): `PartitionData` with the members the cuts read (m_shroudednessPrevious, m_everSeenByPlayer, m_shroudedness,
** declared as PartitionManager.h:399-401 does) and getObject()->getControllingPlayer()->getPlayerIndex() (a Player whose index the test sets,
** or none); `PartitionContactListNode` (m_nextHash, m_next, m_obj, m_other, m_hashValue as PartitionManager.cpp:987-991 declares them;
** `deleteInstance` counts and deletes) and `PartitionContactList` (m_contactHash and m_contactList and the declaration of resetContactList; the
** real class has more members, which the cut does not touch); ObjectShroudStatus and ERROR_BUG as GameCommon.h and Errors.h declare them.
** `DISABLE_INVALID_PREVENTION` is defined as PartitionManager.cpp:109 defines it, so the `#ifndef` block in friend_setShroudednessPrevious
** is not compiled, as in the shipped build. DEBUG_CRASH and DEBUG_ASSERTCRASH are no-ops (release).
**
** Entry points (extern "C", tests/differential_draft_partition.rs):
**   ref_pm_ctl(has_player, index) -> getControllingPlayerIndex, or -999 when it throws ERROR_BUG (no player);
**   ref_pm_shroud(in, n, out, cap): in = (playerIndex, status).. applied in order; out = the 16 m_shroudednessPrevious then the 16 m_everSeenByPlayer;
**   ref_pm_reset(nodes, hashmask, out): builds `nodes` list nodes and a hash table whose socket i holds a node when bit i of the mask (first 32 sockets)
**       is set; resetContactList(); out = [nodes deleted, non-NULL sockets after, m_contactList NULL as 1/0]. */
#include <string.h>

#define DEBUG_CRASH(x) ((void)0)
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define DISABLE_INVALID_PREVENTION
#define NULL 0
typedef int Int;
typedef int Bool;
#define TRUE 1
#define FALSE 0

enum ObjectShroudStatus
{
	OBJECTSHROUD_INVALID,
	OBJECTSHROUD_CLEAR,
	OBJECTSHROUD_PARTIAL_CLEAR,
	OBJECTSHROUD_FOGGED,
	OBJECTSHROUD_SHROUDED,
	OBJECTSHROUD_INVALID_BUT_PREVIOUS_VALID,
	OBJECTSHROUD_COUNT
};
enum { MAX_PLAYER_COUNT = 16 };
enum { ERROR_BASE = 0x0000, ERROR_BUG = (ERROR_BASE + 0x0000) };

class Player { public: Int getPlayerIndex() const { return m_index; } Int m_index; };
static Player thePlayer;
static bool haveControllingPlayer;
class Object { public: const Player *getControllingPlayer() const { return haveControllingPlayer ? &thePlayer : NULL; } };
static Object theObject;

class PartitionData
{
public:
	Object *getObject() const { return &theObject; }
	Int getControllingPlayerIndex() const;
	void friend_setShroudednessPrevious(Int playerIndex, ObjectShroudStatus status);
	ObjectShroudStatus					m_shroudedness[MAX_PLAYER_COUNT];
	ObjectShroudStatus					m_shroudednessPrevious[MAX_PLAYER_COUNT];
	Bool												m_everSeenByPlayer[MAX_PLAYER_COUNT];
};

static int theDeleted;
class PartitionContactListNode
{
public:
	void deleteInstance() { ++theDeleted; delete this; }
	PartitionContactListNode*			m_nextHash;
	PartitionContactListNode*			m_next;
	PartitionData*								m_obj;
	PartitionData*								m_other;
	Int														m_hashValue;
};

class PartitionContactList
{
public:
	enum { PartitionContactList_SOCKET_COUNT = 5381 };
	PartitionContactListNode* m_contactHash[PartitionContactList_SOCKET_COUNT];
	PartitionContactListNode* m_contactList;
	void resetContactList();
};

#include "pm_ctl.inc"
#include "pm_shroud.inc"
#include "pm_reset.inc"

extern "C" int ref_pm_ctl(int has_player, int index)
{
	haveControllingPlayer = has_player != 0;
	thePlayer.m_index = index;
	PartitionData pd;
	try { return pd.getControllingPlayerIndex(); }
	catch (...) { return -999; }
}

extern "C" int ref_pm_shroud(const int *in, int nin, int *out, int cap)
{
	PartitionData pd;
	for (int i = 0; i < MAX_PLAYER_COUNT; ++i) { pd.m_shroudedness[i] = OBJECTSHROUD_INVALID; pd.m_shroudednessPrevious[i] = OBJECTSHROUD_INVALID; pd.m_everSeenByPlayer[i] = FALSE; }
	for (int at = 0; at + 1 < nin; at += 2) pd.friend_setShroudednessPrevious(in[at], (ObjectShroudStatus)in[at + 1]);
	if (cap < 2 * MAX_PLAYER_COUNT) return -1;
	for (int i = 0; i < MAX_PLAYER_COUNT; ++i) { out[i] = pd.m_shroudednessPrevious[i]; out[MAX_PLAYER_COUNT + i] = pd.m_everSeenByPlayer[i]; }
	return 2 * MAX_PLAYER_COUNT;
}

extern "C" int ref_pm_reset(int nodes, unsigned mask, int *out)
{
	static PartitionContactList list;
	memset(list.m_contactHash, 0, sizeof(list.m_contactHash));
	list.m_contactList = NULL;
	theDeleted = 0;
	for (int i = 0; i < nodes; ++i)
	{
		PartitionContactListNode *n = new PartitionContactListNode;
		n->m_nextHash = NULL; n->m_obj = NULL; n->m_other = NULL; n->m_hashValue = 0;
		n->m_next = list.m_contactList;
		list.m_contactList = n;
		if (i < 32 && ((mask >> i) & 1)) list.m_contactHash[i] = n;
	}
	/* sockets that name a node outside bit 0..31: the last socket too, so that a loop that stops early is seen */
	if (nodes > 0) list.m_contactHash[PartitionContactList::PartitionContactList_SOCKET_COUNT - 1] = list.m_contactList;
	/* the hash entries point into nodes that resetContactList deletes: they are cleared by the function itself */
	list.resetContactList();
	int nn = 0;
	for (int i = 0; i < PartitionContactList::PartitionContactList_SOCKET_COUNT; ++i) if (list.m_contactHash[i]) ++nn;
	out[0] = theDeleted; out[1] = nn; out[2] = list.m_contactList == NULL;
	return 3;
}
