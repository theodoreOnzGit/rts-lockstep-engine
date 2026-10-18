/* Stand-in for GameEngine/Include/GameClient/GameText.h (client), academy tests
** only: fetch(label), which the advice text comes from. The wrapper's fetch
** returns the label itself, so the test reads the chosen tips as labels. */
#ifndef SHIM_ACADEMY_GAMETEXT_H
#define SHIM_ACADEMY_GAMETEXT_H
#include "Common/UnicodeString.h"
class GameTextInterface
{
public:
	UnicodeString fetch(const char *label) const;
};
extern GameTextInterface *TheGameText;
#endif
