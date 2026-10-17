/* Shim for GameEngine/Include/Common/Registry.h (included as
** "Common/registry.h"), differential tests only. Win32BIGFileSystem::init
** reads the install path from the Windows registry; init is not exercised.
** The declaration is copied from the real header (line 39). */
#ifndef SHIM_REGISTRY_H
#define SHIM_REGISTRY_H
#include "Common/AsciiString.h"
Bool GetStringFromGeneralsRegistry(AsciiString path, AsciiString key, AsciiString& val);
#endif
