/* Stand-in for GameClient/MapUtil.h, differential tests only (differential/wrap_draft_gsconfig.cpp, #215): the two members of
** `MapCache` that GSConfig.cpp's `<QMMaps>` section calls, `getMapDir()` and `findMap(name)`. The wrapper defines them: the
** directory is "Maps", and a map is found when the sum of its name's bytes is not a multiple of 3. */
#ifndef SHIM_GSCONFIG_MAPUTIL_H
#define SHIM_GSCONFIG_MAPUTIL_H
#include "Common/AsciiString.h"
#include "Common/STLTypedefs.h"
struct MapMetaData { int dummy; };
class MapCache
{
public:
	AsciiString getMapDir() const;
	const MapMetaData *findMap(AsciiString mapName);
};
extern MapCache *TheMapCache;
#endif
