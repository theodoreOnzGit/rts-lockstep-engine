/* Stand-in for GameEngine/Include/GameLogic/ObjectCreationList.h, Weapon tests only: the one lookup
** `WeaponTemplate::postProcessLoad` calls. The wrapper answers an OCL for the names that start with "OCL_". */
#ifndef SHIM_WEAPON_OCL_H
#define SHIM_WEAPON_OCL_H
#include "Common/AsciiString.h"
class ObjectCreationList;
class ObjectCreationListStore { public: const ObjectCreationList *findObjectCreationList(const char *name) const; };
extern ObjectCreationListStore *TheObjectCreationListStore;
#endif
