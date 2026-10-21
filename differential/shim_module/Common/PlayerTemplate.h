/* Stand-in for GameEngine/Include/Common/PlayerTemplate.h, Module build only (differential tests, p1w12-sonnet, update batch N): OCLUpdate reads the side
** of the controlling player's template and nothing else. Renamed ModTestPlayerTemplate by module_prelude.h. */
#ifndef SHIM_MODULE_PLAYERTEMPLATE_H
#define SHIM_MODULE_PLAYERTEMPLATE_H
#include "Common/AsciiString.h"
class PlayerTemplate
{
public:
	AsciiString getSide() const { return m_side; }
	AsciiString m_side;
};
#endif
