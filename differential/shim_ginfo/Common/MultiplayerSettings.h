/* Stand-in for Common/MultiplayerSettings.h, differential tests only (differential/wrap_game_info.cpp): the colour count, the three show-random flags and the observer colour. */
#ifndef SHIM_GINFO_MPSETTINGS_H
#define SHIM_GINFO_MPSETTINGS_H
class MultiplayerColorDefinition { public: Int m_color; Int getColor() const { return m_color; } };
class MultiplayerSettings
{
public:
	Int m_numColors; Bool m_randomTemplate, m_randomColor, m_randomStart;
	MultiplayerColorDefinition m_observer;
	Int getNumColors() { return m_numColors; }
	Bool showRandomPlayerTemplate() const { return m_randomTemplate; }
	Bool showRandomColor() const { return m_randomColor; }
	Bool showRandomStartPos() const { return m_randomStart; }
	MultiplayerColorDefinition *getColor(Int) { return &m_observer; }
};
extern MultiplayerSettings *TheMultiplayerSettings;
#endif
