/* Stand-in for GameEngine/Include/Common/Dict.h, handicap tests only: the
** one method Handicap::readFromDict calls, over a table of NameKeyTypes that
** the wrapper fills (the real Dict is compiled elsewhere under another name;
** see build.rs). */
#ifndef SHIM_MONEY_DICT_H
#define SHIM_MONEY_DICT_H
#include "Common/NameKeyGenerator.h"
class Dict
{
public:
	Real getReal(NameKeyType key, Bool *exists = NULL) const;
};
#endif
