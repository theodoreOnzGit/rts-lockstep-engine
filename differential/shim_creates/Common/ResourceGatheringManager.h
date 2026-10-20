/* Stand-in for GameEngine/Include/Common/ResourceGatheringManager.h (layer 7), create-module build only
** (p1l9-creates): the two adders the create modules call, logging through crt_log. Renamed to CrtRGM. */
#ifndef SHIM_CREATES_RGM_H
#define SHIM_CREATES_RGM_H
class Object;
class ResourceGatheringManager
{
public:
	void addSupplyCenter(Object *newCenter);
	void addSupplyWarehouse(Object *newWarehouse);
	int m_id;
};
#endif
