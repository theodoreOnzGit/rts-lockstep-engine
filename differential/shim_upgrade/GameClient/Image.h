/* Stand-in for GameEngine/Include/GameClient/Image.h, Upgrade/SpecialPower
** build only (differential tests, p1w2-upgrade): the same declarations as
** differential/shim_ini/GameClient/Image.h, which INI.cpp is built against
** (the definitions are in wrap_ini.cpp and abort; UpgradeCenter::reset, the
** only caller here, finds TheMappedImageCollection null and does not call). */
#ifndef SHIM_UPGRADE_IMAGE_H
#define SHIM_UPGRADE_IMAGE_H
#include "Common/AsciiString.h"
class Image;
class ImageCollection { public: const Image *findImageByName(const AsciiString &name); };
extern ImageCollection *TheMappedImageCollection;
#endif
