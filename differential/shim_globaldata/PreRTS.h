/* Forced first include of GlobalData.cpp and INIGameData.cpp, differential
** tests only (differential/wrap_globaldata.cpp). The real precompiled header
** PreRTS.h brings in windows.h and most of the engine; among its engine
** headers is Common/GlobalData.h, which neither file includes itself
** (GlobalData.cpp relies on PreRTS.h for it). This one brings the same
** GlobalData.h, in the real PreRTS.h's position (before the .cpp's own
** `#define DEFINE_*_NAMES` lines and includes), and what GlobalData.h and
** the INI headers rely on the real PreRTS.h for: BaseType.h's
** `#define PI` (real Lib/BaseType.h line 86), <bitset> (BitFlags.h), the error
** codes of Common/Errors.h, the ScienceType enum (INI.h names it before
** declaring it).
**
** GlobalData.h forward-declares three enumerations (`enum BodyDamageType;`,
** `enum AIDebugOptions;`, `typedef enum _TerrainLOD;`), which MSVC accepts
** and standard C++ (clang without -fms-compatibility) rejects as forward
** references. So their definitions come first, from the headers build.rs
** extracts verbatim (OUT_DIR/globaldata_gen); a declaration of an enum that
** is already defined is accepted. The name lists of those headers are
** requested here (the macros are defined again, identically, by
** GlobalData.cpp). */
#ifndef SHIM_GLOBALDATA_PRERTS_H
#define SHIM_GLOBALDATA_PRERTS_H
#include <stdio.h>
#include <time.h>
#include <string.h>
#include <bitset>
#ifndef PI
#define PI 3.14159265359f
#endif
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/Errors.h"
#include "Common/Science.h"
#define DEFINE_TERRAIN_LOD_NAMES
#define DEFINE_BODYDAMAGETYPE_NAMES
#include "GameClient/TerrainVisual.h"
#include "GameLogic/Module/BodyModule.h"
#include "GameLogic/AI.h"
#include "Common/GlobalData.h"
#endif
