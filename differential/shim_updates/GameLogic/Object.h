/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8), p1l9-updates build only (differential/wrap_l9updates.cpp).
** It is differential/shim_module/GameLogic/Object.h with the same members in the same order (no data member is added, so
** the layout the module harness of wrap_module.cpp compiles against is unchanged) and three declarations: isKindOf and
** kill, which the l9 update modules call and wrap_l9updates.cpp defines. The class is renamed by module_prelude.h. */
#ifndef SHIM_UPDATES_OBJECT_H
#define SHIM_UPDATES_OBJECT_H
#include "Common/Thing.h"
#include "Common/GameCommon.h"
#include "Common/ObjectStatusTypes.h"
class UpgradeTemplate;
class BodyModuleInterface;
class Object : public Thing
{
public:
	Object() : m_level(LEVEL_REGULAR) {}
	VeterancyLevel getVeterancyLevel() const { return m_level; }
	const ObjectStatusMaskType &getStatusBits() const { return m_status; }
	void removeUpgrade(const UpgradeTemplate *t);
	/* p1l9-updates: defined in wrap_l9updates.cpp */
	Bool isKindOf(KindOfType t) const;
	void kill();
	Bool testStatus(ObjectStatusTypes bit) const;
	void attemptHealing(Real amount, const Object *source);
	BodyModuleInterface *getBodyModule();
	const Coord3D *getPosition() const;
	void setPosition(const Coord3D *pos);
	Bool isSignificantlyAboveTerrain() const;
	VeterancyLevel m_level;
	ObjectStatusMaskType m_status;
protected:
	virtual Object *asObjectMeth() { return this; }
	virtual const Object *asObjectMeth() const { return this; }
};
#endif
