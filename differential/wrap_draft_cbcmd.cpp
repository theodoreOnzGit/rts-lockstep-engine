/* Differential-test wrapper for the control bar command drafts (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/control_bar_command.rs (populate_inv_data_callback, reset_build_queue_data,
** calculate_veterancy_overlay_for_thing), cut verbatim by build.rs (drafts_guilogic3) from ControlBarCommand.cpp into:
**   cbc_populate.inc  `struct PopulateInvButtonData` .. `populateInvDataCallback` (to `Transports have an extra special manipulation`);
**   cbc_reset.inc     resetBuildQueueData;   cbc_vet.inc  calculateVeterancyOverlayForThing.
** Built on the REAL AsciiString (shim_ascii; classes renamed by build.rs).
** Stand-ins (outside the cuts): `ControlBar` with the members the cuts use (m_containData, m_queueData, m_rankVeteranIcon, m_rankEliteIcon,
** m_rankHeroicIcon, with ContainEntry and QueueEntry as ControlBar.h:934-950 declares them, the union included, and the enums of
** ProductionUpdate.h), `static calculateVeterancyOverlayForObject` (a table lookup by what the test sets: the real one reads the
** object's veterancy level, a different function, not drafted here); `Object` (getID, getTemplate()->getButtonImage(), getTemplate()
** ->getName()); `GameWindow` (an id; winEnable logs); `Image` (an id); GadgetButtonSetEnabledImage and GadgetButtonDrawOverlayImage
** (log); `ThingTemplate::getBehaviorModuleInfo()`, a ModuleInfo of (name, data) with getCount, getNthName, getNthData;
** VeterancyGainCreateModuleData (m_scienceRequired, m_startingLevel); ThePlayerList->getLocalPlayer() (NULL or a Player whose
** hasScience(s) tests bit s of a mask the test sets); SCIENCE_INVALID (-1) and VeterancyLevel as Science.h and GameCommon.h declare them.
** DEBUG_ASSERTCRASH is a no-op (release).
**
** Entry points (extern "C", tests/differential_draft_cbcmd.rs):
**   ref_cb_populate(in, n, out, cap): in = [maxIndex, startIndex, nControls, control ids.., nObjects, (object, buttonImage, overlayImage)..];
**       out = [log: (1 id image: SetEnabledImage) (2 id image: DrawOverlayImage) (3 id enable: winEnable).., -1, currIndex,
**       then m_containData[0..16): control id (0 NULL), object id]. Control ids are non-zero (a NULL control crashes upstream).
**   ref_cb_reset(in, out): in = 9 x (control id, type, pointer P); the union is set through the pointer; out = 9 x (control, type, productionID, pointer).
**   ref_cb_vet(in, out): in = [haveTemplate, havePlayer, nMods, (isVGC, hasData, science, level)..., maskLo, maskHi, veteranIcon, eliteIcon,
**       heroicIcon]; out = [image id (0 NULL)]. */
#include <string.h>
#include <vector>

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theCbAllocator;

#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define NULL 0

enum { MAX_COMMANDS_PER_SET = 16 };
enum { MAX_BUILD_QUEUE_BUTTONS = 9 };
enum ProductionID { PRODUCTIONID_INVALID = 0 };
enum ProductionType { PRODUCTION_INVALID = 0, PRODUCTION_UNIT, PRODUCTION_UPGRADE };
typedef int ObjectID;
enum VeterancyLevel { LEVEL_REGULAR = 0, LEVEL_VETERAN, LEVEL_ELITE, LEVEL_HEROIC, LEVEL_COUNT, LEVEL_INVALID };
enum { SCIENCE_INVALID = -1 };
typedef int ScienceType;

class UpgradeTemplate;
struct Image { int id; };
static std::vector<int> theLog;
class GameWindow
{
public:
	int id;
	void winEnable(Bool e) { theLog.push_back(3); theLog.push_back(id); theLog.push_back(e ? 1 : 0); }
};
static void GadgetButtonSetEnabledImage(GameWindow *w, const Image *i) { theLog.push_back(1); theLog.push_back(w->id); theLog.push_back(i ? i->id : 0); }
static void GadgetButtonDrawOverlayImage(GameWindow *w, const Image *i) { theLog.push_back(2); theLog.push_back(w->id); theLog.push_back(i ? i->id : 0); }

static Image theButtonImage, theOverlayImage, theVetIcon, theEliteIcon, theHeroicIcon;
static bool haveButtonImage, haveOverlayImage;
class ThingTemplate
{
public:
	const Image *getButtonImage() const { return haveButtonImage ? &theButtonImage : NULL; }
	AsciiString getName() const { return AsciiString("t"); }
};
class Object
{
public:
	ObjectID getID() const { return m_id; }
	const ThingTemplate *getTemplate() const { static ThingTemplate t; return &t; }
	ObjectID m_id;
};

struct VeterancyGainCreateModuleData { ScienceType m_scienceRequired; VeterancyLevel m_startingLevel; };
struct ModuleEntry { const char *name; const VeterancyGainCreateModuleData *data; };
class ModuleInfo
{
public:
	Int getCount() const { return (Int)m_e.size(); }
	AsciiString getNthName(Int i) const { return AsciiString(m_e[i].name); }
	const VeterancyGainCreateModuleData *getNthData(Int i) const { return m_e[i].data; }
	std::vector<ModuleEntry> m_e;
};
static ModuleInfo theModuleInfo;
class ThingTemplateForVet
{
public:
	const ModuleInfo &getBehaviorModuleInfo() const { return theModuleInfo; }
};

class Player { public: Bool hasScience(ScienceType s) const { return (m_mask >> s) & 1; } unsigned long long m_mask; };
static Player thePlayer;
static bool haveLocalPlayer;
class PlayerList { public: Player *getLocalPlayer() { return haveLocalPlayer ? &thePlayer : NULL; } };
static PlayerList thePlayerListObject;
static PlayerList *ThePlayerList = &thePlayerListObject;

class ControlBar
{
public:
	struct ContainEntry
	{
	  GameWindow *control;
		ObjectID objectID;
	};
	static ContainEntry m_containData[ MAX_COMMANDS_PER_SET ];
	struct QueueEntry
	{
		GameWindow *control;
		ProductionType type;
		union
		{
			ProductionID productionID;
			const UpgradeTemplate *upgradeToResearch;
		};
	};
	QueueEntry m_queueData[ MAX_BUILD_QUEUE_BUTTONS ];
	static const Image *m_rankVeteranIcon;
	static const Image *m_rankEliteIcon;
	static const Image *m_rankHeroicIcon;
	static void populateInvDataCallback( Object *obj, void *userData );
	void resetBuildQueueData( void );
	static const Image* calculateVeterancyOverlayForThing( const ThingTemplateForVet *thingTemplate );
	static const Image* calculateVeterancyOverlayForObject( const Object *obj ) { return haveOverlayImage ? &theOverlayImage : NULL; }
};
ControlBar::ContainEntry ControlBar::m_containData[ MAX_COMMANDS_PER_SET ];
const Image *ControlBar::m_rankVeteranIcon = NULL;
const Image *ControlBar::m_rankEliteIcon = NULL;
const Image *ControlBar::m_rankHeroicIcon = NULL;

/* the cuts name the template class `ThingTemplate` in the signatures and, for the one the veterancy function takes, `const ThingTemplate *` */
#define ThingTemplate ThingTemplateForVet
#include "cbc_populate.inc"
#include "cbc_reset.inc"
#include "cbc_vet.inc"
#undef ThingTemplate

extern "C" int ref_cb_populate(const int *in, int nin, int *out, int cap)
{
	TheDynamicMemoryAllocator = &theCbAllocator;
	theLog.clear();
	memset(ControlBar::m_containData, 0, sizeof ControlBar::m_containData);
	int at = 0;
	PopulateInvButtonData data;
	data.maxIndex = in[at++];
	data.currIndex = in[at++];
	int nc = in[at++];
	static GameWindow wins[MAX_COMMANDS_PER_SET];
	static GameWindow *ctls[MAX_COMMANDS_PER_SET];
	for (int i = 0; i < nc; ++i) { wins[i].id = in[at++]; ctls[i] = &wins[i]; }
	data.controls = ctls;
	data.transport = NULL;
	int no = in[at++];
	for (int i = 0; i < no; ++i)
	{
		Object o;
		o.m_id = in[at++];
		haveButtonImage = in[at] != 0; theButtonImage.id = in[at]; at++;
		haveOverlayImage = in[at] != 0; theOverlayImage.id = in[at]; at++;
		ControlBar::populateInvDataCallback(&o, &data);
	}
	std::vector<int> o = theLog;
	o.push_back(-1);
	o.push_back(data.currIndex);
	for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i)
	{
		o.push_back(ControlBar::m_containData[i].control ? ControlBar::m_containData[i].control->id : 0);
		o.push_back(ControlBar::m_containData[i].objectID);
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); ++i) out[i] = o[i];
	return (int)o.size();
}

extern "C" int ref_cb_reset(const int *in, int nin, int *out, int cap)
{
	ControlBar cb;
	static GameWindow wins[MAX_BUILD_QUEUE_BUTTONS];
	for (int i = 0; i < MAX_BUILD_QUEUE_BUTTONS; ++i)
	{
		wins[i].id = in[i * 3];
		cb.m_queueData[i].control = in[i * 3] ? &wins[i] : NULL;
		cb.m_queueData[i].type = (ProductionType)in[i * 3 + 1];
		cb.m_queueData[i].upgradeToResearch = (const UpgradeTemplate *)(unsigned long)(unsigned)in[i * 3 + 2];
	}
	cb.resetBuildQueueData();
	int n = 0;
	for (int i = 0; i < MAX_BUILD_QUEUE_BUTTONS; ++i)
	{
		out[n++] = cb.m_queueData[i].control ? cb.m_queueData[i].control->id : 0;
		out[n++] = cb.m_queueData[i].type;
		out[n++] = (int)cb.m_queueData[i].productionID;
		out[n++] = (int)(unsigned long)cb.m_queueData[i].upgradeToResearch;
	}
	return n;
}

extern "C" int ref_cb_vet(const int *in, int nin, int *out, int cap)
{
	TheDynamicMemoryAllocator = &theCbAllocator;
	static VeterancyGainCreateModuleData datas[32];
	static ThingTemplateForVet tt;
	int at = 0;
	bool haveT = in[at++] != 0;
	haveLocalPlayer = in[at++] != 0;
	int nm = in[at++];
	theModuleInfo.m_e.clear();
	for (int i = 0; i < nm; ++i)
	{
		bool isV = in[at++] != 0, hasD = in[at++] != 0;
		datas[i].m_scienceRequired = in[at++];
		datas[i].m_startingLevel = (VeterancyLevel)in[at++];
		ModuleEntry e;
		e.name = isV ? "VeterancyGainCreate" : "OtherModule";
		e.data = hasD ? &datas[i] : NULL;
		theModuleInfo.m_e.push_back(e);
	}
	thePlayer.m_mask = (unsigned long long)(unsigned)in[at] | ((unsigned long long)(unsigned)in[at + 1] << 32);
	at += 2;
	theVetIcon.id = in[at++]; theEliteIcon.id = in[at++]; theHeroicIcon.id = in[at++];
	ControlBar::m_rankVeteranIcon = theVetIcon.id ? &theVetIcon : NULL;
	ControlBar::m_rankEliteIcon = theEliteIcon.id ? &theEliteIcon : NULL;
	ControlBar::m_rankHeroicIcon = theHeroicIcon.id ? &theHeroicIcon : NULL;
	const Image *r = ControlBar::calculateVeterancyOverlayForThing(haveT ? &tt : NULL);
	out[0] = r ? r->id : 0;
	return 1;
}
