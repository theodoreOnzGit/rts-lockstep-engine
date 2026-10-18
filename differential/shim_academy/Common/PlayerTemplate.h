/* Stand-in for GameEngine/Include/Common/PlayerTemplate.h (layer 5), academy
** tests only: getBaseSide(), the side the advice's checks compare. */
#ifndef SHIM_ACADEMY_PLAYERTEMPLATE_H
#define SHIM_ACADEMY_PLAYERTEMPLATE_H
#include "Common/AsciiString.h"
class PlayerTemplate
{
public:
	AsciiString getBaseSide() const { return m_baseSide; }
	AsciiString m_baseSide;
};
#endif
