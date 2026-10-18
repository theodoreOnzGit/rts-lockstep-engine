/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8, not translated), FiringTracker tests
** only: the members FiringTracker.cpp calls. An object is its id, its weapon bonus word (the tracker
** reads and writes it), its template, a faerie-fire status, and the two effects on the owner that the
** tracker makes (clearAndSetModelConditionFlags, reloadAllAmmo), which the wrapper records. The
** wrapper renames the class (FireTestObject). */
#ifndef SHIM_FIRING_OBJECT_H
#define SHIM_FIRING_OBJECT_H
#include "Common/GameType.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/ModelState.h"
#include "GameLogic/Weapon.h"
class ThingTemplate;
class Object
{
public:
	Object() : m_id(INVALID_ID), m_weaponBonus(0), m_template(NULL), m_faerie(false) {}
	ObjectID getID() const { return m_id; }
	const ThingTemplate *getTemplate() const { return m_template; }
	Bool testWeaponBonusCondition(WeaponBonusConditionType c) const { return ((m_weaponBonus >> c) & 1u) != 0; }
	void setWeaponBonusCondition(WeaponBonusConditionType c);
	void clearWeaponBonusCondition(WeaponBonusConditionType c);
	Bool testStatus(ObjectStatusTypes s) const { return s == OBJECT_STATUS_FAERIE_FIRE && m_faerie; }
	void clearAndSetModelConditionFlags(const ModelConditionFlags &clr, const ModelConditionFlags &set);
	void reloadAllAmmo(Bool now);
	ObjectID m_id;
	UnsignedInt m_weaponBonus;
	const ThingTemplate *m_template;
	Bool m_faerie;
};
#endif
