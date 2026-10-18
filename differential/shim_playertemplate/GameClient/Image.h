/* Shim for GameEngine/Include/GameClient/Image.h, PlayerTemplate build only
** (differential tests): the one-lookup stand-in of the INI build
** (differential/shim_ini/GameClient/Image.h). PlayerTemplate.cpp's five image
** getters call TheMappedImageCollection->findImageByName, which
** differential/wrap_ini.cpp defines to abort; the tests never reach them (the
** Rust side has the names, a cut point). */
#include "../../shim_ini/GameClient/Image.h"
