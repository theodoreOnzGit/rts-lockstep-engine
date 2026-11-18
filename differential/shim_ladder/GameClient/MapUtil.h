/* Stand-in for GameClient/MapUtil.h, differential tests only (differential/wrap_draft_ladder.cpp, #215): `MapCache::getMapDir`,
** `findMap` and `MapMetaData::m_numPlayers`, defined in the wrapper. */
#ifndef SHIM_LADDER_MAPUTIL_H
#define SHIM_LADDER_MAPUTIL_H
#include "Common/AsciiString.h"
#include "Common/STLTypedefs.h"
struct MapMetaData { Int m_numPlayers; };
class MapCache
{
public:
	AsciiString getMapDir() const;
	const MapMetaData *findMap(AsciiString mapName);
};
extern MapCache *TheMapCache;
#endif
