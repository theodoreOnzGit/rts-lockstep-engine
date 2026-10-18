/* Stand-in for GameEngine/Include/Common/ObjectStatusTypes.h, score keeper tests
** only (p1w6-energy): the one enumerator ScoreKeeper.cpp names, with the real
** header's BitFlags left out (see Common/KindOf.h of this directory). */
#ifndef SHIM_SCORE_OBJECTSTATUSTYPES_H
#define SHIM_SCORE_OBJECTSTATUSTYPES_H
enum ObjectStatusTypes
{
	OBJECT_STATUS_NONE = 0,
	OBJECT_STATUS_UNDER_CONSTRUCTION = 3,
};
#endif
