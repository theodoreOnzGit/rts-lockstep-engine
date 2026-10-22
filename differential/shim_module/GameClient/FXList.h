/* Stand-in for GameEngine/Include/GameClient/FXList.h, Module build only
** (differential tests, p1w3-modules): FXList as an opaque class with the
** static that UpgradeMuxData::performUpgradeFX calls (defined in
** differential/wrap_module.cpp, where it logs the FXList it is given). The
** store lookup INI::parseFXList calls is the Armor build's, in INI.cpp's own
** names: this unit never calls it. Renamed by the wrapper's macros
** (module_prelude.h). */
#ifndef SHIM_MODULE_FXLIST_H
#define SHIM_MODULE_FXLIST_H
#include "Lib/BaseType.h"
class Object;
class Matrix3D;
class FXList
{
public:
	static void doFXObj(const FXList *fx, const Object *primary, const Object *secondary = 0);
	/* --- p1w12-sonnet --- (FXListDie: defined by differential/l9_dies_j.inc) */
	static void doFXPos(const FXList *fx, const Coord3D *primary, const Matrix3D *primaryMtx = 0, const Real primarySpeed = 0.0f, const Coord3D *secondary = 0, const Real overrideRadius = 0.0f);
	/* --- end p1w12-sonnet --- */
};
/* --- p1w13-sonnet --- (SlowDeathBehavior's parser finds FX lists by name; differential/l9_slow_death_q.inc) */
class FXListStore { public: const FXList *findFXList(const char *name) const; };
extern FXListStore *TheFXListStore;
/* --- end p1w13-sonnet --- */
#endif
