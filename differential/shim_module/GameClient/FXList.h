/* Stand-in for GameEngine/Include/GameClient/FXList.h, Module build only
** (differential tests, p1w3-modules): FXList as an opaque class with the
** static that UpgradeMuxData::performUpgradeFX calls (defined in
** differential/wrap_module.cpp, where it logs the FXList it is given). The
** store lookup INI::parseFXList calls is the Armor build's, in INI.cpp's own
** names: this unit never calls it. Renamed by the wrapper's macros
** (module_prelude.h). */
#ifndef SHIM_MODULE_FXLIST_H
#define SHIM_MODULE_FXLIST_H
class Object;
class FXList
{
public:
	static void doFXObj(const FXList *fx, const Object *primary, const Object *secondary = 0);
};
#endif
