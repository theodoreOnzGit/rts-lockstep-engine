/* Stand-in for GameEngine/Include/Common/Player.h (layer 7), academy statistics
** tests only (p1w6-energy). AcademyStats.cpp reaches its player for: the
** template's base side, the money, the energy, the objects (iterateObjects),
** and its own AcademyStats (getAcademyStats). The wrapper sets them per call
** (wrap_academy_stats.cpp). The real AcademyStats class is held by value. */
#ifndef SHIM_ACADEMY_PLAYER_H
#define SHIM_ACADEMY_PLAYER_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/AcademyStats.h"
#include "Common/Energy.h"
#include "Common/Money.h"
#include "Common/PlayerTemplate.h"
class Object;
class Player
{
public:
	Player() : m_hasTemplate(false), m_money(NULL), m_energy(NULL), m_nobj(0) {}
	const PlayerTemplate *getPlayerTemplate() const { return m_hasTemplate ? &m_template : NULL; }
	Money *getMoney() const { return m_money; }
	Energy *getEnergy() const { return m_energy; }
	AcademyStats *getAcademyStats() { return &m_academy; }
	void iterateObjects(void (*fn)(Object *, void *), void *userData) const;
	PlayerTemplate m_template;
	Bool m_hasTemplate;
	Money *m_money;
	Energy *m_energy;
	AcademyStats m_academy;
	Object *m_objects[64];
	int m_nobj;
};
#endif
