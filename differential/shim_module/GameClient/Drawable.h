/* Stand-in for GameEngine/Include/GameClient/Drawable.h, Module build only
** (differential tests, p1w3-modules): the modules name the class and use
** nothing from it. Renamed by the wrapper's macros (module_prelude.h). */
#ifndef SHIM_MODULE_DRAWABLE_H
#define SHIM_MODULE_DRAWABLE_H
#include "Common/AsciiString.h"
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
/* GameClient/Drawable.h lines 241-249, verbatim */
enum TintStatus
{
	TINT_STATUS_DISABLED		= 0x00000001,
	TINT_STATUS_IRRADIATED	= 0x00000002,
	TINT_STATUS_POISONED		= 0x00000004,
	TINT_STATUS_GAINING_SUBDUAL_DAMAGE		= 0x00000008,
	TINT_STATUS_FRENZY			= 0x00000010,
};
class Drawable { public: void setTerrainDecal(TerrainDecalType t); void setModelConditionState(ModelConditionFlagType a);
	void setTintStatus(TintStatus statusBits); void clearTintStatus(TintStatus statusBits);
	/* SubObjectsUpgrade (batch B) */
	void showSubObject(const AsciiString &name, Bool show); void updateSubObjects();
	/* --- p1w11-sonnet --- (PowerPlantUpdate, RadarUpdate: l9_updates_e.inc) */
	void clearModelConditionFlags(const ModelConditionFlags &clr); void clearAndSetModelConditionState(ModelConditionFlagType clr, ModelConditionFlagType set);
	void flashAsSelected(const RGBColor *color = 0);   /* the crates' sabotage feedback */
	/* --- end p1w11-sonnet --- */ };
/* --- end p1w10-sonnet --- */
#endif
