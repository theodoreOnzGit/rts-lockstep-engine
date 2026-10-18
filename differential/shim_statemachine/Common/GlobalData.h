/* Stand-in for GameEngine/Include/Common/GlobalData.h, differential tests
** only (StateMachine build). StateMachine.cpp reads TheGlobalData only for
** m_stateMachineDebug in STATE_MACHINE_DEBUG code, which a release build
** does not compile: nothing is declared. */
#ifndef SM_SHIM_GLOBALDATA_H
#define SM_SHIM_GLOBALDATA_H
#endif
