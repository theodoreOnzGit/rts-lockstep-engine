/* Group wol8, first part: the types and statics the GameSpyInfo class needs (see wrap_draft_wol8_rc_post.h for the whole description). */
static Int listboxLobbyPlayersID = 0;
static Int w8_game_list_id = 0;
static Int GetGameListBoxID(void) { return w8_game_list_id; }
static GPProfile w8_local_profile = 0;
class GameSpyStagingRoom
{
public:
	AsciiString m_ip;
	UnsignedShort m_port;
	AsciiString getLadderIP(void) const { return m_ip; }
	UnsignedShort getLadderPort(void) const { return m_port; }
};
typedef std::map<Int, GameSpyStagingRoom *> StagingRoomMap;
static StagingRoomMap w8_rooms;
struct LadderInfo { int unused; };
static std::set<std::pair<std::string, int> > w8_ladders;
static LadderInfo w8_ladder_dummy;
class LadderList
{
public:
	const LadderInfo *findLadder(const AsciiString &ip, UnsignedShort port) { return w8_ladders.find(std::make_pair(std::string(ip.str()), (int)port)) != w8_ladders.end() ? &w8_ladder_dummy : NULL; }
};
static LadderList w8_ladder_list;
static LadderList *TheLadderList = &w8_ladder_list;
#include "peer_rc.inc"
static std::string w8_hex(const char *s)
{
	std::string o;
	char b[4];
	if (!*s) return "-";
	for (; *s; ++s) { snprintf(b, sizeof b, "%02x", (unsigned char)*s); o += b; }
	return o;
}
