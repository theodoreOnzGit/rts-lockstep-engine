/* Stand-in for GameNetwork/LANAPICallbacks.h, differential tests only (differential/wrap_recorder.cpp): TheLAN (NULL: no network in the tests). */
#ifndef SHIM_REC_LANAPICALLBACKS_H
#define SHIM_REC_LANAPICALLBACKS_H
class LANAPI { public: GameInfo *GetMyGame(); };
extern LANAPI *TheLAN;
#endif
