/* Stand-in for GameEngine/Include/GameClient/Drawable.h, Module build only
** (differential tests, p1w3-modules): the modules name the class and use
** nothing from it. Renamed by the wrapper's macros (module_prelude.h). */
#ifndef SHIM_MODULE_DRAWABLE_H
#define SHIM_MODULE_DRAWABLE_H
#include "Common/AsciiString.h"
#include "matrix3d.h"   /* p1w13-sonnet: FloatUpdate */
class Object;   /* p1w13-sonnet: batch P */
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
	/* --- p1w12-sonnet --- (PowerPlantUpdate, RadarUpdate: l9_updates_e.inc) */
	void clearModelConditionFlags(const ModelConditionFlags &clr); void clearAndSetModelConditionState(ModelConditionFlagType clr, ModelConditionFlagType set);
	void flashAsSelected(const RGBColor *color = 0);   /* the crates' sabotage feedback */
	/* --- end p1w12-sonnet --- */
	/* --- p1w12-sonnet --- (update batch N) */
	void reactToGeometryChange();
	void clearModelConditionState(ModelConditionFlagType a);   /* update batch N: l9_flammable_n.inc */
	/* --- end p1w12-sonnet --- */
	/* --- p1w13-sonnet --- (update batch O: FloatUpdate; differential/l9_float_o.inc) */
	const Matrix3D *getInstanceMatrix() const;
	void setInstanceMatrix(const Matrix3D *instance);
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch P: ActiveBody; differential/l9_active_body_p.inc) */
	void reactToBodyDamageStateChange( int newState );   /* a BodyDamageType */
	Object *getObject() const { return m_ownerObject; }
	Object *m_ownerObject = 0;
	void setShadowsEnabled(Bool enable);   /* SlowDeathBehavior */
	void setTerrainDecalFadeTarget(Real target, Real rate);
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R: OpenContain; differential/l9_open_contain_r.inc) */
	Bool isSelected() const { return m_selected; }
	void setDrawableHidden(Bool hidden);
	const ModelConditionFlags &getModelConditionFlags() const { return m_condFlags; }
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	/* the transport's exit bone: one named ExitBoneT, at a world position, if the scene gives the art one */
	Int getPristineBonePositions(const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const
	{
		/* batch R8: and the parachute's three bones (ParachuteContain) */
		static const char *const kPara[3] = { "PARA_COG", "PARA_ATTCH", "PARA_MAN" };
		for (int i = 0; i < 3; ++i) {
			if (strcmp(boneNamePrefix, kPara[i]) != 0) continue;
			if (!m_paraBoneHas[i] || maxBones < 1) return 0;
			if (positions) positions[0] = m_paraBone[i];
			return 1;
		}
		if (!m_exitBoneHas || strcmp(boneNamePrefix, "ExitBoneT") != 0 || maxBones < 1) return 0;
		if (positions) positions[0] = m_exitBone;
		return 1;
	}
	Bool m_paraBoneHas[3] = { false, false, false };
	Coord3D m_paraBone[3];
	const class GeometryInfo &getDrawableGeometryInfo() const;   /* batch R8: the owner's geometry (differential/l9_open_contain_r.inc) */
	Bool m_exitBoneHas = false;
	Coord3D m_exitBone;
	/* --- end p1w13-sonnet --- */
	Bool m_selected = false;
	ModelConditionFlags m_condFlags;
	/* --- end p1w13-sonnet --- */ };
/* --- end p1w10-sonnet --- */
#endif
