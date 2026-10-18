/* Stand-in for GameEngine/Include/GameLogic/Module/UpdateModule.h (layer 9 base, not translated),
** FiringTracker tests only. The sleep constants and the phase enum are upstream's text (UpdateModule.h
** lines 66-75 and 88-92); the rest is the minimum the tracker's header and source use: the module
** base (a Snapshot, so its xfer is reached through the friends of Snapshot, as upstream's
** XferSave reaches a module's), its object, getObject, setWakeFrame and the pure
** getDisabledTypesToProcess. The wrapper renames the class and defines the base's methods
** (wrap_firing_tracker.cpp). Module.h's MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA declares
** crc, xfer and loadPostProcess protected; this stand-in's macro does the same. */
#ifndef SHIM_FIRING_UPDATEMODULE_H
#define SHIM_FIRING_UPDATEMODULE_H
#include "Common/GameType.h"
#include "Common/Snapshot.h"
#include "Common/DisabledTypes.h"
#include "Common/GameMemory.h"
class Thing;
class Object;
class ModuleData
{
public:
	virtual ~ModuleData() {}
};
enum UpdateSleepTime
{
	UPDATE_SLEEP_INVALID	= 0,
	UPDATE_SLEEP_NONE			= 1,
	UPDATE_SLEEP_FOREVER	= 0x3fffffff
};
#define UPDATE_SLEEP(numFrames)				((UpdateSleepTime)(numFrames))
enum SleepyUpdatePhase
{
	PHASE_INITIAL				= 0,
	PHASE_PHYSICS				= 1,
	PHASE_NORMAL				= 2,
	PHASE_FINAL					= 3
};
/* MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA, as Module.h declares its crc/xfer/loadPostProcess */
#define MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA(cls, clsmd) \
protected: \
	virtual void crc( Xfer *xfer ); \
	virtual void xfer( Xfer *xfer ); \
	virtual void loadPostProcess( void );
class Thing
{
public:
	Object *m_object;
};
class UpdateModule : public MemoryPoolObject, public Snapshot
{
public:
	UpdateModule(Thing *thing, const ModuleData *modData);
	virtual ~UpdateModule() {}
	Object *getObject() const { return m_object; }
	void setWakeFrame(Object *obj, UpdateSleepTime wakeDelay);
	virtual DisabledMaskType getDisabledTypesToProcess() const = 0;
	virtual SleepyUpdatePhase getUpdatePhase() const { return PHASE_NORMAL; }
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
	Object *m_object;
};
#endif
