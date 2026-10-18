/* Stand-in for GameLogic/ScriptEngine.h in the sides/scripts build. The real
** ScriptEngine (layer 11) is the interpreter; Scripts.cpp needs only
** what it asks of it: the condition and action templates
** (ScriptEngine.cpp lines 7694-7712: an index out of range is 0) and
** signalUIInteract (SignalUIInteraction, which nothing here calls), and
** TScriptListReadInfo (ScriptEngine.h lines 79-83), the struct
** ScriptList::ParseScriptsDataChunk fills. The tables are installed by
** differential/wrap_sides.cpp. */
#ifndef SHIM_SIDES_SCRIPTENGINE_H
#define SHIM_SIDES_SCRIPTENGINE_H
#include "GameLogic/Scripts.h"
#include "Common/SubsystemInterface.h"
struct TScriptListReadInfo
{
	Int numLists;
	ScriptList *readLists[MAX_PLAYER_COUNT];
};
class ScriptEngine
{
public:
	virtual const ActionTemplate *getActionTemplate(Int ndx);
	virtual const ConditionTemplate *getConditionTemplate(Int ndx);
	virtual void signalUIInteract(const AsciiString &hookName);
};
extern ScriptEngine *TheScriptEngine;
#endif
