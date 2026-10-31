/* Stand-in for Common/PlayerTemplate.h, differential tests only (differential/wrap_game_info.cpp): the template count and a template's display name. */
#ifndef SHIM_GINFO_PLAYERTEMPLATE_H
#define SHIM_GINFO_PLAYERTEMPLATE_H
#include "Common/UnicodeString.h"
class PlayerTemplate { public: UnicodeString m_name; UnicodeString getDisplayName() const { return m_name; } };
class PlayerTemplateStore { public: Int m_count; PlayerTemplate m_t[64]; Int getPlayerTemplateCount() { return m_count; } const PlayerTemplate *getNthPlayerTemplate(Int i) { return &m_t[i]; } };
extern PlayerTemplateStore *ThePlayerTemplateStore;
#endif
