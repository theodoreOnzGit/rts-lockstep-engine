/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8), academy tests
** only: isKindOf (the kind bits the wrapper sets), getContain() (the container's
** tunnel answer), getCommandSetString() (what findDozerCommandSet passes on). */
#ifndef SHIM_ACADEMY_OBJECT_H
#define SHIM_ACADEMY_OBJECT_H
#include "Common/KindOf.h"
#include "Common/AsciiString.h"
#include "GameLogic/Module/ContainModule.h"
class Object
{
public:
	Object() : m_contain(NULL) {}
	Bool isKindOf(KindOfType t) const { return m_kindOf.test(t); }
	ContainModuleInterface *getContain() const { return m_contain; }
	AsciiString getCommandSetString() const { return AsciiString("CommandSetDozer"); }
	KindOfMaskType m_kindOf;
	ContainModuleInterface *m_contain;
};
#endif
