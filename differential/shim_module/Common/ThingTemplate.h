/* Stand-in for GameEngine/Include/Common/ThingTemplate.h, Module build only (differential tests, p1w3-modules): Module.cpp includes it and uses nothing
** from it. */
/* --- p1w12-sonnet --- (die batch D: EjectPilotDie asks its object's template for the per-unit sounds VoiceEject and SoundEject; renamed
** ModTestThingTemplate by module_prelude.h, getPerUnitSound answered by differential/l9_dies_d.inc) */
#ifndef SHIM_MODULE_THINGTEMPLATE_H
#define SHIM_MODULE_THINGTEMPLATE_H
#include "Common/AsciiString.h"
#include "Common/AudioEventRTS.h"
/* --- p1w13-sonnet --- (batch P: ActiveBody finds its armor set in the template; the name is the FIRST member, as the Armor build's stand-in ThingTemplate has it, because
** Damage.cpp's DamageInfoInput::xfer (that build) reads the name of the template this build hands it) */
#include "GameLogic/ArmorSet.h"
/* --- end p1w13-sonnet --- */
#include "Common/KindOf.h"   /* p1w13-sonnet, batch R13c */
/* --- p1w13-sonnet --- (batch R13d: ConvertToCarBombCrateCollide asks the template for its best weapon set; the real WeaponSet.h needs the weapon
** layer, so this is upstream WeaponSet.h:124-170's matching interface of WeaponTemplateSet, and the finder is the real SparseMatchFinder) */
#include "GameLogic/WeaponSetFlags.h"
#include "Common/SparseMatchFinder.h"
#include <vector>
class WeaponTemplateSet
{
public:
	WeaponSetFlags m_types;
	Bool testWeaponSetFlag( WeaponSetType wst ) const { return m_types.test( wst ); }
	inline Int getConditionsYesCount() const { return 1; }
	inline const WeaponSetFlags& getNthConditionsYes(Int i) const { return m_types; }
	inline AsciiString getDescription() const { return AsciiString("ArmorTemplateSet"); }
};
typedef std::vector<WeaponTemplateSet> WeaponTemplateSetVector;
typedef SparseMatchFinder<WeaponTemplateSet, WeaponSetFlags> WeaponTemplateSetFinder;
/* --- end p1w13-sonnet --- */
class ThingTemplate
{
public:
	/* --- p1w13-sonnet --- */
	AsciiString m_name;
	const AsciiString &getName() const { return m_name; }
	/* --- end p1w13-sonnet --- */
	const AudioEventRTS *getPerUnitSound(const AsciiString &soundNameToGet) const;
	/* p1w13-sonnet, batch R26: StealthUpdate's two template sounds (the test's templates have none: an empty event, as the Rust test's) */
	const AudioEventRTS *getSoundStealthOn() const { return &m_soundStealthOn; }
	const AudioEventRTS *getSoundStealthOff() const { return &m_soundStealthOff; }
	AudioEventRTS m_soundStealthOn, m_soundStealthOff;
	/* p1w13-sonnet, batch R13c: SalvageCrateCollide asks the template (the test keeps it equal to the object's kinds) */
	KindOfMaskType m_kindOf;
	Bool isKindOf(KindOfType t) const { return m_kindOf.test(t); }
	Bool isKindOfMulti(const KindOfMaskType &mustBeSet, const KindOfMaskType &mustBeClear) const { return m_kindOf.testSetAndClear(mustBeSet, mustBeClear); }   /* batch R14c */
	/* batch R13d: ThingTemplate.h's findWeaponTemplateSet, verbatim */
	WeaponTemplateSetVector m_weaponTemplateSets;
	mutable WeaponTemplateSetFinder m_weaponTemplateSetFinder;
	const WeaponTemplateSet* findWeaponTemplateSet(const WeaponSetFlags& t) const { return m_weaponTemplateSetFinder.findBestInfo(m_weaponTemplateSets, t); }
	AudioEventRTS m_voiceEject;
	AudioEventRTS m_soundEject;
	/* --- p1w13-sonnet --- (batch P) */
	ArmorTemplateSetVector m_armorTemplateSets;
	mutable ArmorTemplateSetFinder m_armorTemplateSetFinder;
	const ArmorTemplateSet* findArmorTemplateSet(const ArmorSetFlags& t) const { return m_armorTemplateSetFinder.findBestInfo(m_armorTemplateSets, t); }
	Byte m_structureRubbleHeight = 0;
	Real getStructureRubbleHeight() const { return (Real)m_structureRubbleHeight; }
	AudioEventRTS m_soundOnDamaged, m_soundOnReallyDamaged, m_voiceFear, m_soundPromotedVeteran, m_soundPromotedElite, m_soundPromotedHero;
	const AudioEventRTS *getSoundOnDamaged() const { return &m_soundOnDamaged; }
	const AudioEventRTS *getSoundOnReallyDamaged() const { return &m_soundOnReallyDamaged; }
	const AudioEventRTS *getVoiceFear() const { return &m_voiceFear; }
	const AudioEventRTS *getSoundPromotedVeteran() const { return &m_soundPromotedVeteran; }
	const AudioEventRTS *getSoundPromotedElite() const { return &m_soundPromotedElite; }
	const AudioEventRTS *getSoundPromotedHero() const { return &m_soundPromotedHero; }
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R: OpenContain) */
	AudioEventRTS m_soundEnter, m_soundExit, m_soundFalling;
	const AudioEventRTS *getSoundEnter() const { return &m_soundEnter; }
	const AudioEventRTS *getSoundExit() const { return &m_soundExit; }
	const AudioEventRTS *getSoundFalling() const { return &m_soundFalling; }
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	Int m_rawTransportSlotCount = 0;
	Int getRawTransportSlotCount() const { return m_rawTransportSlotCount; }
	UnsignedInt getOcclusionDelay() const { return m_occlusionDelay; }   /* batch R10: TunnelContain */
	UnsignedInt m_occlusionDelay = 0;
	Bool isEquivalentTo(const ThingTemplate *tt) const;   /* p1w13-sonnet, batch R5: differential/l9_open_contain_r.inc */
	/* --- end p1w13-sonnet --- */
	/* --- end p1w13-sonnet --- */
};
#endif
/* --- end p1w12-sonnet --- */
