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
/* --- p1w13-sonnet --- (batch R26: StealthUpdate; GameClient/Drawable.h lines 216-226, verbatim) */
enum StealthLookType
{
	STEALTHLOOK_NONE,								///< unit is not stealthed at all
	STEALTHLOOK_VISIBLE_FRIENDLY,		///< unit is stealthed-but-visible due to friendly status
	STEALTHLOOK_DISGUISED_ENEMY,		///< we can have units that are disguised (instead of invisible)
	STEALTHLOOK_VISIBLE_DETECTED,		///< unit is stealthed and invisible, but a second material pass
																						///< is added to reveal the invisible unit as with heat vision
	STEALTHLOOK_VISIBLE_FRIENDLY_DETECTED,		///< unit is stealthed-but-visible due to being detected, 
																						///< and rendered in heatvision effect second material pass
	STEALTHLOOK_INVISIBLE						///< unit is stealthed-and-invisible
};
/* --- end p1w13-sonnet --- */
class Drawable { public: void setTerrainDecal(TerrainDecalType t); void setModelConditionState(ModelConditionFlagType a); void setAnimationLoopDuration(UnsignedInt numFrames);   /* batch R53 (l9_missile_launcher_r53.inc) */

	void setTintStatus(TintStatus statusBits); void clearTintStatus(TintStatus statusBits);
	/* batch R33: EMPUpdate (differential/l9_emp_r33.inc) */
	void setInstanceScale(Real scale); void colorTint(const RGBColor *color); void colorFlash(const RGBColor *color, UnsignedInt decayFrames, UnsignedInt attackFrames, UnsignedInt sustainAtPeak);
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
	void setTerrainDecalSize(Real x, Real y);   /* batch R14c: CreateCrateDie (differential/l9_dies_r14.inc) */
	/* batch R25: HordeUpdate reads the decal it set (Drawable.h line 527); setTerrainDecal keeps it (l9_upgrades.inc) */
	TerrainDecalType getTerrainDecalType( void ) const { return m_terrainDecalType; }
	TerrainDecalType m_terrainDecalType = TERRAIN_DECAL_NONE;
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R: OpenContain; differential/l9_open_contain_r.inc) */
	Bool isSelected() const { return m_selected; }
	void setDrawableHidden(Bool hidden);
	const ModelConditionFlags &getModelConditionFlags() const { return m_condFlags; }
	void updateDrawableSupplyStatus( Int maxSupply, Int currentSupply );   /* p1w13-sonnet, batch R44: a line (differential/l9_supply_docks_r44.inc) */
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	/* the transport's exit bone: one named ExitBoneT, at a world position, if the scene gives the art one */
	Int getPristineBonePositions(const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const
	{
		/* batch R30: BoneFXUpdate's bones, when the test gives them (differential/l9_bonefx_r30.inc) */
		extern const Coord3D *(*g_r30BoneHook)(const char *);
		if (g_r30BoneHook) {
			/* batch R31: and a prefix's numbered bones, from startIndex while the art has them (W3DModelDraw's loop) */
			Int count = 0;
			for (Int i = startIndex; i <= (startIndex == 0 ? 0 : 99) && count < maxBones; ++i) {
				char name[256];
				if (i == 0) snprintf(name, sizeof(name), "%s", boneNamePrefix);
				else snprintf(name, sizeof(name), "%s%02d", boneNamePrefix, (int)i);
				const Coord3D *p = g_r30BoneHook(name);
				if (!p) break;
				if (positions) positions[count] = *p;
				/* batch R45: and the bones' transforms, when the test gives them (differential/l9_exits_r45.inc) */
				extern const Matrix3D *(*g_boneXformHook)(const char *);
				if (transforms && g_boneXformHook) { const Matrix3D *t = g_boneXformHook(name); if (t) transforms[count] = *t; }
				++count;
			}
			return count;
		}
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
	void setIndicatorColor(Color color);   /* batch R11: CaveContain (never reached there: see cave_contain.rs); batch R12: a line (l9_open_contain_r.inc) */
	/* batch R12: GarrisonContain's muzzle flashes (a line each: differential/l9_open_contain_r.inc), the shroud and the flags put back */
	void setPosition(const Coord3D *pos);
	void setOrientation(Real angle);
	void setFullyObscuredByShroud(Bool fully);
	Bool getFullyObscuredByShroud() const { return m_obscured; }
	void replaceModelConditionFlags(const ModelConditionFlags &flags);
	void clearAndSetModelConditionFlags(const ModelConditionFlags &clr, const ModelConditionFlags &set);   /* p1w13-sonnet, batch R14b: differential/l9_dies_r14.inc */
	DrawableID getID() const { return m_effectId; }
	Bool m_obscured = false;
	DrawableID m_effectId = (DrawableID)0;
	unsigned m_boneCond = 0;   /* the condition clearAndSetModelConditionFlags set last (0 after replaceModelConditionFlags): the contain test's bones rise by it */
	Bool m_exitBoneHas = false;
	Coord3D m_exitBone;
	/* --- end p1w13-sonnet --- */
	Bool m_selected = false;
	ModelConditionFlags m_condFlags;
	/* --- p1w13-sonnet --- (batch R26: StealthUpdate; the calls are lines of the effect log, differential/l9_stealth_r26.inc) */
	void setEffectiveOpacity( Real pulseFactor, Real explicitOpacity = -1.0f );
	void setSecondMaterialPassOpacity( Real op );
	void setStealthLook( StealthLookType look );
	Bool isDrawableEffectivelyHidden() const { return m_hiddenR26; }
	void setModelConditionFlags( const ModelConditionFlags &set );
	void updateDrawable();
	Bool m_hiddenR26 = false;
	void stopAmbientSound();   /* batch R48: HelicopterSlowDeathBehavior (a line: differential/l9_heli_death_r48.inc) */
	void convertBonePosToWorldPos(const Coord3D *bonePos, const Matrix3D *boneTransform, Coord3D *worldPos, Matrix3D *worldTransform) const;   /* batch R48 */
	class ClientUpdateModule **getClientUpdateModules();   /* batch R28: ToppleUpdate stops the sway (a line, and none: differential/l9_topple_r28.inc) */
	class ClientUpdateModule *findClientUpdateModule( NameKeyType key ) { return m_r59Laser; }   /* batch R59: the laser object's LaserUpdate, the test's */
	class ClientUpdateModule *m_r59Laser = 0;
	/* --- end p1w13-sonnet --- */
	/* --- end p1w13-sonnet --- */ };
/* --- end p1w10-sonnet --- */
#endif
