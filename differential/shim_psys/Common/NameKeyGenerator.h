/* Stand-in for GameEngine/Include/Common/NameKeyGenerator.h, ParticleSys build only (differential tests, #46):
** FXListStore keys its lists by NAMEKEY(name). NameKeyType and NAMEKEY are upstream's (lines 49-54 and
** 137-138); the generator is a stand-in (renamed by shim_psys/PreRTS.h): keys 1, 2, ... in order of first use of
** each exact name, as upstream's (which is case-sensitive; its keys are numbers the tests never compare). */
#ifndef __NAMEKEYGENERATOR_H_
#define __NAMEKEYGENERATOR_H_
#include <map>
#include <string>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"

enum NameKeyType
{
	NAMEKEY_INVALID					= 0,
	NAMEKEY_MAX							= 1<<23,					// max ordinal value of a NameKey (some code relies on these fitting into 24 bits safely)
	FORCE_NAMEKEYTYPE_LONG	= 0x7fffffff	// a trick to ensure the NameKeyType is a 32-bit int
};

class NameKeyGenerator
{
public:
	NameKeyType nameToKey(const char *name)
	{
		std::map<std::string, NameKeyType>::iterator it = m_keys.find(name);
		if (it != m_keys.end()) return it->second;
		NameKeyType k = (NameKeyType)(m_keys.size() + 1);
		m_keys[name] = k;
		m_names[k] = name;
		return k;
	}
	NameKeyType nameToKey(const AsciiString &name) { return nameToKey(name.str()); }
	AsciiString keyToName(NameKeyType k) { return AsciiString(m_names[k].c_str()); }
	std::map<std::string, NameKeyType> m_keys;
	std::map<NameKeyType, std::string> m_names;
};
extern NameKeyGenerator *TheNameKeyGenerator;
/* lines 137-138, verbatim */
inline NameKeyType NAMEKEY(const AsciiString& name) { return TheNameKeyGenerator->nameToKey(name); }
inline NameKeyType NAMEKEY(const char* name) { return TheNameKeyGenerator->nameToKey(name); }
#endif
