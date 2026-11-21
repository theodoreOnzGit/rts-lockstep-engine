/* differential/shim_ui4 (#215, group ui4): stand-in for GameNetwork/DisconnectManager.h, which GameClient/DisconnectMenu.h includes for the pointer type
** `DisconnectManager *` and `MAX_SLOTS` (GameInfo.h: an enum, 8). The menu only stores the pointer. */
#ifndef SHIM_UI4_DISCONNECTMANAGER_H
#define SHIM_UI4_DISCONNECTMANAGER_H
enum { MAX_SLOTS = 8 };
class DisconnectManager {};
#endif
