/* Forced include (-include) for the StateMachine build, differential tests
** only (p1w3-statemachine). It does two things before any other line of the
** translation unit (the wrapper and the unchanged StateMachine.cpp):
**
** 1. Includes the standard headers the upstream text needs, and brings
**    std::min and std::max into the global namespace (StateMachine.h's
**    MIN_SLEEP calls `min` unqualified; the real PreRTS.h gets it from the
**    Windows headers).
** 2. Renames, by macro, the engine classes this build stands in for
**    (Object, GameLogic, TheGameLogic, ThingTemplate) to Sm* names, so that
**    no strong symbol of this archive can collide with a stand-in or a
**    translation of the same upstream class in another fixture (later
**    waves translate Object and GameLogic themselves). The upstream text is
**    unchanged; only these names differ. Then `private` and `protected` are
**    made public, so that the wrapper can read the machine's members (as
**    the Science and Upgrade builds do); the standard headers are included
**    first so that the macros do not reach into them. */
#ifndef SHIM_STATEMACHINE_PRELUDE_H
#define SHIM_STATEMACHINE_PRELUDE_H
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <algorithm>
#include <functional>
#include <exception>
#include <new>
using std::min;
using std::max;

#define Object SmObject
#define GameLogic SmGameLogic
#define TheGameLogic SmTheGameLogic
#define ThingTemplate SmThingTemplate

#define private public
#define protected public
#endif
