/* Stand-in for GameClient/MapUtil.h, differential tests only (differential/wrap_game_info.cpp): MapMetaData's two fields GameInfo.cpp reads, MapCache as upstream's std::map by lower-case name with findMap (MapUtil.cpp lines 1084-1091, verbatim) and getMapExtension ("map"). */
#ifndef SHIM_GINFO_MAPUTIL_H
#define SHIM_GINFO_MAPUTIL_H
#include <map>
#include "Common/AsciiString.h"
class MapMetaData { public: Int m_numPlayers; UnsignedInt m_CRC; };
class MapCache : public std::map<AsciiString, MapMetaData>
{
public:
	const MapMetaData *findMap(AsciiString mapName)
	{
		mapName.toLower();
		MapCache::iterator it = find(mapName);
		if (it == end())
			return NULL;
		return &(it->second);
	}
	AsciiString getMapExtension() const { return AsciiString("map"); }
};
extern MapCache *TheMapCache;
#endif
