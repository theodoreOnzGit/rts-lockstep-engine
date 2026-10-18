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
class Object : public Thing
{
public:
	Object() : m_level(LEVEL_REGULAR) {}
	VeterancyLevel getVeterancyLevel() const { return m_level; }
	const ObjectStatusMaskType &getStatusBits() const { return m_status; }
	void removeUpgrade(const UpgradeTemplate *t);
	VeterancyLevel m_level;
	ObjectStatusMaskType m_status;
protected:
	virtual Object *asObjectMeth() { return this; }
	virtual const Object *asObjectMeth() const { return this; }
};
#endif
