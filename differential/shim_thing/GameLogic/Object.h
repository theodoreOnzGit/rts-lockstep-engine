/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8, not
** translated), Thing and ObjectIter tests only: an object is the template it
** was made from. The wrapper renames the class (wrap_thing.cpp,
** wrap_object_iter.cpp). */
#ifndef SHIM_THING_OBJECT_H
#define SHIM_THING_OBJECT_H
#include "Common/ThingTemplate.h"
class Object
{
public:
	explicit Object(const ThingTemplate *t) : m_template(t) {}
	const ThingTemplate *getTemplate() const { return m_template; }
private:
	const ThingTemplate *m_template;
};
#endif
