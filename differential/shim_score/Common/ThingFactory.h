/* Stand-in for GameEngine/Include/Common/ThingFactory.h (layer 5), score keeper
** tests only: findTemplate(name), which ScoreKeeper::xfer uses on load. The
** wrapper answers for the names it knows (see wrap_score_keeper.cpp). */
#ifndef SHIM_SCORE_THINGFACTORY_H
#define SHIM_SCORE_THINGFACTORY_H
#include "Common/ThingTemplate.h"
class ThingFactory
{
public:
	const ThingTemplate *findTemplate(AsciiString name) const;
};
extern ThingFactory *TheThingFactory;
#endif
