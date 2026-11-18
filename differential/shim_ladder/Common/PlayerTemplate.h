/* Stand-in for Common/PlayerTemplate.h, differential tests only (differential/wrap_draft_ladder.cpp, #215): the player template store
** as LadderDefs.cpp reads it (all side strings, the template count, the nth template with `isPlayableSide` and `getSide`). */
#ifndef SHIM_LADDER_PLAYERTEMPLATE_H
#define SHIM_LADDER_PLAYERTEMPLATE_H
#include "Common/AsciiString.h"
#include "Common/STLTypedefs.h"
class PlayerTemplate
{
public:
	AsciiString m_side; Bool m_playable;
	Bool isPlayableSide() const { return m_playable; }
	AsciiString getSide() const { return m_side; }
};
class PlayerTemplateStore
{
public:
	void getAllSideStrings(AsciiStringList *outStringList);
	Int getPlayerTemplateCount() const;
	const PlayerTemplate *getNthPlayerTemplate(Int i) const;
};
extern PlayerTemplateStore *ThePlayerTemplateStore;
#endif
