/* The stand-ins of the units ResourceGatheringManager.cpp calls into (p1w7-sonnet): the objects of a test world
** (id, AI, modules by name, dock interface), TheGameLogic, the partition manager's distance, ActionManager's
** canTransferSuppliesAt, the supply-truck AI interface and the dock interface. The class names are renamed by
** macros in wrap_resource_gathering.cpp. */
#ifndef SHIM_RGM_WORLD_H
#define SHIM_RGM_WORLD_H
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/GameType.h"

/* NAMEKEY("...") : the wrapper's table of the two names the file looks up (the real generator is verified by
** differential_name_key.rs; its header is the real one, for NameKeyType) */
#include "Common/NameKeyGenerator.h"
inline NameKeyType rgmNameKey(const char *s)
{
	if (strcmp(s, "SupplyWarehouseDockUpdate") == 0) return (NameKeyType)1;
	if (strcmp(s, "SupplyCenterDockUpdate") == 0) return (NameKeyType)2;
	return NAMEKEY_INVALID;
}
#undef NAMEKEY
#define NAMEKEY(str) rgmNameKey(str)

class Object;
enum DistanceCalculationType { FROM_CENTER_2D = 0, FROM_CENTER_3D, FROM_BOUNDINGSPHERE_2D, FROM_BOUNDINGSPHERE_3D };

class UpdateModule { public: int m_key; };
class SupplyWarehouseDockUpdate : public UpdateModule {};
class DockUpdateInterface { public: virtual Bool isClearToApproach(Object const *docker) const; Bool m_clear; };
class SupplyTruckAIInterface
{
public:
	virtual Real getWarehouseScanDistance() const { return m_scan; }
	virtual ObjectID getPreferredDockID() const { return m_preferred; }
	Real m_scan; ObjectID m_preferred;
};
class AIUpdateInterface { public: SupplyTruckAIInterface *getSupplyTruckAIInterface() { return m_truck; } SupplyTruckAIInterface *m_truck; };
class Object
{
public:
	Object() : m_id(INVALID_ID), m_ai(NULL), m_dock(NULL), m_hasWarehouseUpdate(false), m_hasCenterUpdate(false), m_exists(true) { m_pos[0] = m_pos[1] = m_pos[2] = 0; }
	ObjectID getID() const { return m_id; }
	AIUpdateInterface *getAI() const { return m_ai; }
	UpdateModule *findUpdateModule(NameKeyType key) const
	{
		if ((int)key == 1 && m_hasWarehouseUpdate) return const_cast<UpdateModule *>(&m_warehouseModule);
		if ((int)key == 2 && m_hasCenterUpdate) return const_cast<UpdateModule *>(&m_centerModule);
		return NULL;
	}
	DockUpdateInterface *getDockUpdateInterface() { return m_dock; }
	ObjectID m_id; AIUpdateInterface *m_ai; DockUpdateInterface *m_dock; Bool m_hasWarehouseUpdate, m_hasCenterUpdate, m_exists;
	UpdateModule m_warehouseModule, m_centerModule; Real m_pos[3];
	AIUpdateInterface m_aiStorage; SupplyTruckAIInterface m_truckStorage; DockUpdateInterface m_dockStorage;
};
class GameLogic { public: Object *findObjectByID(ObjectID id); };
extern GameLogic *TheGameLogic;
class PartitionManager { public: Real getDistanceSquared(const Object *obj, const Object *other, DistanceCalculationType dc, void *vec = NULL) const; };
extern PartitionManager *ThePartitionManager;
class ActionManager { public: Bool canTransferSuppliesAt(const Object *obj, const Object *transferObj); };
extern ActionManager *TheActionManager;
#endif
