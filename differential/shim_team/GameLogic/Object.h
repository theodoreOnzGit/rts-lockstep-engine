/* Stand-in for GameLogic/Object.h in the Team build (p1w9-sonnet): Team.h includes it. The real Object.h pulls in the
** whole object layer; Team.cpp needs only the handful of members declared here, and the DLINK that puts an Object in a
** Team's member list. Everything this class does is defined, with the other stand-ins, in team_standins.h (declared
** here because Team.h needs `Object` complete before the real Team classes are defined). The class is renamed by the
** #define lines of wrap_team.cpp, like every stand-in of the build. */
#ifndef TEAM_SHIM_OBJECT_H
#define TEAM_SHIM_OBJECT_H
#include "Common/GameMemory.h"
#include "Common/GameCommon.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/DisabledTypes.h"
#include "Common/KindOf.h"
#include "Common/AsciiString.h"
#include "Common/Snapshot.h"
#include "GameLogic/Damage.h"
#include "Lib/BaseType.h"
#include "team_standins_decl.h"
#endif
