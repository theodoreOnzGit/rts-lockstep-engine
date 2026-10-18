/* Stand-in for GameEngine/Include/GameClient/Image.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate): the lookup resolveNames
** makes when there is a mapped image collection (there is none in the
** tests). (Renamed Tt* by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_IMAGE_H
#define SHIM_TT_IMAGE_H
#include "Common/AsciiString.h"
class Image;
class ImageCollection
{
public:
	const Image *findImageByName(const AsciiString &name);
};
extern ImageCollection *TheMappedImageCollection;
#endif
