/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8), score keeper
** tests only: getTemplate(), getControllingPlayer(), and testStatus for the
** one status ScoreKeeper.cpp tests (OBJECT_STATUS_UNDER_CONSTRUCTION, real
** enum from ObjectStatusTypes.h). */
#ifndef SHIM_SCORE_OBJECT_H
#define SHIM_SCORE_OBJECT_H
#include "Common/ObjectStatusTypes.h"
#include "Common/Player.h"
#include "Common/ThingTemplate.h"
class Object
{
public:
	Object() : m_template(NULL), m_controller(NULL), m_underConstruction(false) {}
	const ThingTemplate *getTemplate() const { return m_template; }
	Player *getControllingPlayer() const { return m_controller; }
	Bool testStatus(ObjectStatusTypes status) const { return status == OBJECT_STATUS_UNDER_CONSTRUCTION && m_underConstruction; }
	const ThingTemplate *m_template;
	Player *m_controller;
	Bool m_underConstruction;
};
#endif
