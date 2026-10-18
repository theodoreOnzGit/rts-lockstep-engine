/* Shim for GameEngine/Include/Common/Registry.h (included as
** "Common/registry.h"), differential tests only. GlobalData's constructor
** reads the user data directory's leaf name from the registry, but only after
** SHGetSpecialFolderPath finds My Documents, which the stand-in does not.
** The declaration is the real header's (GetStringFromRegistry). */
#ifndef SHIM_GLOBALDATA_REGISTRY_H
#define SHIM_GLOBALDATA_REGISTRY_H
#include "Common/AsciiString.h"
inline Bool GetStringFromRegistry(AsciiString, AsciiString, AsciiString &) { return FALSE; }
#endif
