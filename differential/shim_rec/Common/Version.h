/* Stand-in for Common/Version.h, differential tests only (differential/wrap_recorder.cpp): the version string, its build time and number. */
#ifndef SHIM_REC_VERSION_H
#define SHIM_REC_VERSION_H
#include "Common/UnicodeString.h"
class Version
{
public:
	UnicodeString m_version, m_buildTime; UnsignedInt m_number;
	UnicodeString getUnicodeVersion() { return m_version; }
	UnicodeString getUnicodeBuildTime() { return m_buildTime; }
	UnsignedInt getVersionNumber() { return m_number; }
};
extern Version *TheVersion;
#endif
