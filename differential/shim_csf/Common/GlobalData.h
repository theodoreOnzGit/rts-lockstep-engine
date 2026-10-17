/* Shim for GameEngine/Include/Common/GlobalData.h, differential tests only.
** GameText.cpp reads TheGlobalData only in _DEBUG/_INTERNAL builds (the
** jabber/munkee test modes); the release build compiled here needs nothing
** from the header beyond the class name, which archive_prelude.h declares. */
#ifndef SHIM_GLOBALDATA_H
#define SHIM_GLOBALDATA_H
#endif
