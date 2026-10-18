/* Stand-in for GameEngine/Include/GameLogic/Object.h, differential tests only
** (StateMachine build; renamed SmObject by the prelude): what
** StateMachine.cpp calls on an Object is getID() and getPosition()
** (setGoalObject) and, in debug logging only, getTemplate(). */
#ifndef SM_SHIM_OBJECT_H
#define SM_SHIM_OBJECT_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"
#include "Common/ThingTemplate.h"
class Object
{
public:
	ObjectID getID() const { return m_id; }
	const Coord3D *getPosition() const { return &m_position; }
	const ThingTemplate *getTemplate() const { return 0; }
	ObjectID m_id;
	Coord3D m_position;
};
#endif
