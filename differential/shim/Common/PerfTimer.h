/* Shim for GameEngine/Include/Common/PerfTimer.h, differential tests only:
** profiling macros, compiled out (as in a build without PERF_TIMERS). */
#ifndef SHIM_PERFTIMER_H
#define SHIM_PERFTIMER_H
#define USE_PERF_TIMER(x)
#define DECLARE_PERF_TIMER(x)
#endif
