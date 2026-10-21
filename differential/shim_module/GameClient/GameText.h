/* Stand-in for GameEngine/Include/GameClient/GameText.h, Module build only (update batch L): `TheGameText->fetch(label)` of AutoDepositUpdate.cpp answers a
** UnicodeString that remembers the label. Renamed ModTestGameTextInterface / ModTestTheGameText by module_prelude.h. */
#ifndef SHIM_MODULE_GAMETEXT_H
#define SHIM_MODULE_GAMETEXT_H
#include "Common/UnicodeString.h"
class GameTextInterface
{
public:
	UnicodeString fetch(const char *label) { UnicodeString s; s.m_label = label; return s; }
};
extern GameTextInterface *TheGameText;
#endif
