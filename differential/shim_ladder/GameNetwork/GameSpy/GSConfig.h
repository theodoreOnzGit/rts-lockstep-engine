/* Stand-in for GameNetwork/GameSpy/GSConfig.h, differential tests only (differential/wrap_draft_ladder.cpp, #215): the two members of
** `TheGameSpyConfig` that LadderDefs.cpp calls (the wrapper defines the object: the quick-match maps and the leftover config text
** are the test's input). */
#ifndef SHIM_LADDER_GSCONFIG_H
#define SHIM_LADDER_GSCONFIG_H
#include "Common/AsciiString.h"
#include "Common/STLTypedefs.h"
class GameSpyConfigInterface
{
public:
	std::list<AsciiString> getQMMaps(void);
	AsciiString getLeftoverConfig(void);
};
extern GameSpyConfigInterface *TheGameSpyConfig;
#endif
