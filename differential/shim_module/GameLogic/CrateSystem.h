/* Stand-in for GameEngine/Include/GameLogic/CrateSystem.h, Module build only (differential tests, p1w13-sonnet, batch R14c: CreateCrateDie). The
** template's data members keep upstream's names and types (CrateSystem.h lines 45-95); the system is a list the driver fills
** (differential/l9_dies_r14.inc), and findCrateTemplate is its lookup. The terrain build compiles the real header and CrateSystem.cpp; the module
** prelude renames these classes so that the two do not meet. */
#ifndef SHIM_MODULE_CRATE_SYSTEM_H
#define SHIM_MODULE_CRATE_SYSTEM_H
#include <list>
#include <vector>
#include "Common/AsciiString.h"
#include "Common/KindOf.h"
#include "Common/Science.h"
#include "GameLogic/ExperienceTracker.h"

struct crateCreationEntry
{
	AsciiString crateName;
	Real crateChance;
};

typedef std::list< crateCreationEntry >											crateCreationEntryList;
typedef std::list< crateCreationEntry >::iterator						crateCreationEntryIterator;
typedef std::list< crateCreationEntry >::const_iterator			crateCreationEntryConstIterator;

class CrateTemplate
{
public:
	AsciiString m_name;
	Real m_creationChance = 0;
	VeterancyLevel m_veterancyLevel = LEVEL_INVALID;
	KindOfMaskType m_killedByTypeKindof;
	ScienceType m_killerScience = SCIENCE_INVALID;
	crateCreationEntryList m_possibleCrates;
	Bool m_isOwnedByMaker = FALSE;
};

class CrateSystem
{
public:
	const CrateTemplate *findCrateTemplate(AsciiString name) const
	{
		for (size_t i = 0; i < m_templates.size(); ++i) if (m_templates[i]->m_name.compare(name) == 0) return m_templates[i];
		return 0;
	}
	std::vector<CrateTemplate *> m_templates;
};

extern CrateSystem *TheCrateSystem;
#endif
