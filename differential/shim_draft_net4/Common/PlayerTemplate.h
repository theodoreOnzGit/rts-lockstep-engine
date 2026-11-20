/* Stand-in for Common/PlayerTemplate.h (#215, group net4): PersistentStorageThread.cpp's `formatPlayerKVPairs` asks the player-template store only
** for `getPlayerTemplateCount()`. The store here is a struct with that one member; the wrapper defines `ThePlayerTemplateStore`. */
#ifndef NET4_PLAYERTEMPLATE_H
#define NET4_PLAYERTEMPLATE_H
struct Net4PlayerTemplateStore
{
	int count;
	int getPlayerTemplateCount() { return count; }
};
extern Net4PlayerTemplateStore *ThePlayerTemplateStore;
#endif
