/* Stand-in for GameLogic/Object.h in the BuildAssistant build (p1w10-sonnet): BuildAssistant.h includes it. The real Object.h pulls in the
** whole object layer; BuildAssistant.cpp needs the handful of members declared in ba_standins_decl.h. Every class there is renamed by the
** #define lines of differential/wrap_build_assistant.cpp. */
#ifndef BA_SHIM_OBJECT_H
#define BA_SHIM_OBJECT_H
#include "ba_standins_decl.h"
#endif
