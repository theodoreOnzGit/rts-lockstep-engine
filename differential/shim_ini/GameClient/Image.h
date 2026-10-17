/* Shim for GameEngine/Include/GameClient/Image.h (upstream commit 0a05454d), INI
** differential tests only. INI.cpp includes it for one lookup used by a
** field parser that needs a game system not yet translated; only that
** lookup is declared. It is defined in differential/wrap_ini.cpp to abort:
** the tests never reach it. */
#ifndef SHIM_IMAGE_H
#define SHIM_IMAGE_H
#include "Common/AsciiString.h"
class Image;
class ImageCollection { public: const Image *findImageByName(const AsciiString &name); };
extern ImageCollection *TheMappedImageCollection;
#endif
