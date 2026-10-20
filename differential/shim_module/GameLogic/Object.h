/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8), Module build
** only (differential tests, p1w3-modules): an Object that is a Thing and has
** what the module files call: the veterancy level and the status bits
** (DieMuxData::isDieApplicable), and removeUpgrade
** (UpgradeMuxData::muxDataProcessUpgradeRemoval), which is defined in
** differential/wrap_module.cpp and logs the template it is given. The class
** is renamed by the wrapper's macros (module_prelude.h). */
#ifndef SHIM_MODULE_OBJECT_H
#define SHIM_MODULE_OBJECT_H
#include "Common/Thing.h"
#include "Common/GameCommon.h"
#include "Common/ObjectStatusTypes.h"
class UpgradeTemplate;
/* --- p1l9-example --- */
#include "Common/Upgrade.h"
#include "Common/AsciiString.h"
class Player;
/* --- end p1l9-example --- */
class Object : public Thing
{
public:
	Object() : m_level(LEVEL_REGULAR), m_player(0) {}
	VeterancyLevel getVeterancyLevel() const { return m_level; }
	const ObjectStatusMaskType &getStatusBits() const { return m_status; }
	void removeUpgrade(const UpgradeTemplate *t);
	VeterancyLevel m_level;
	ObjectStatusMaskType m_status;
	/* --- p1l9-example --- */
	/* what CommandSetUpgrade reads and writes; m_player is NULL unless a test gives one */
	Player *getControllingPlayer() const { return m_player; }
	UpgradeMaskType getObjectCompletedUpgradeMask() const { return m_objectUpgradesCompleted; }
	void setCommandSetStringOverride(AsciiString s) { m_commandSetStringOverride = s; }
	Player *m_player;
	UpgradeMaskType m_objectUpgradesCompleted;
	AsciiString m_commandSetStringOverride;
	/* --- end p1l9-example --- */
protected:
	virtual Object *asObjectMeth() { return this; }
	virtual const Object *asObjectMeth() const { return this; }
};
#endif
