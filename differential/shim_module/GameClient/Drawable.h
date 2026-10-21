/* Stand-in for GameEngine/Include/GameClient/Drawable.h, Module build only
** (differential tests, p1w3-modules): the modules name the class and use
** nothing from it. Renamed by the wrapper's macros (module_prelude.h). */
#ifndef SHIM_MODULE_DRAWABLE_H
#define SHIM_MODULE_DRAWABLE_H
/* --- p1w10-sonnet --- (ArmorUpgrade sets the chemical-suit terrain decal; TerrainDecalType is GameClient/Drawable.h lines 257-275 with
** ALLOW_DEMORALIZE undefined, as the shipped game) */
enum TerrainDecalType
{
	TERRAIN_DECAL_DEMORALIZED_OBSOLETE = 0,
	TERRAIN_DECAL_HORDE,
	TERRAIN_DECAL_HORDE_WITH_NATIONALISM_UPGRADE,
	TERRAIN_DECAL_HORDE_VEHICLE,
	TERRAIN_DECAL_HORDE_WITH_NATIONALISM_UPGRADE_VEHICLE,
	TERRAIN_DECAL_CRATE,
	TERRAIN_DECAL_HORDE_WITH_FANATICISM_UPGRADE,
	TERRAIN_DECAL_CHEMSUIT,
	TERRAIN_DECAL_NONE,
	TERRAIN_DECAL_SHADOW_TEXTURE,
	TERRAIN_DECAL_MAX
};
class Drawable { public: void setTerrainDecal(TerrainDecalType t); void setModelConditionState(ModelConditionFlagType a); };
/* --- end p1w10-sonnet --- */
#endif
