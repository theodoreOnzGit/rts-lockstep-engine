/* Stand-in for GameNetwork/LANAPI.h, differential tests only (differential/wrap_game_info.cpp): m_lanMaxOptionsLength and the constants it is made of, copied from LANAPI.h lines 41-51 and NetworkDefs.h line 57. */
#ifndef SHIM_GINFO_LANAPI_H
#define SHIM_GINFO_LANAPI_H
static const Int GI_MAX_PACKET_SIZE = 476;
static const Int g_lanPlayerNameLength = 12; // reduced length because of game option length
static const Int g_lanLoginNameLength = 1;
static const Int g_lanHostNameLength = 1;
static const Int g_lanGameNameLength = 16; // reduced length because of game option length
static const Int m_lanMaxOptionsLength = GI_MAX_PACKET_SIZE - ( 8 + (g_lanGameNameLength+1)*2 + 4 + (g_lanPlayerNameLength+1)*2
																														+ (g_lanLoginNameLength+1) + (g_lanHostNameLength+1) );
class LANAPI;
extern LANAPI *TheLAN;
#endif
