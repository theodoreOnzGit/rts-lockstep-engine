/* (#215, group wwlib4) stand-in for WWLib/systimer.h: TIMEGETTIME() reads a clock the wrapper scripts (Test_Time). */
#ifndef WWLIB4_SYSTIMER_H
#define WWLIB4_SYSTIMER_H
unsigned int Test_Time(void);
#define TIMEGETTIME() Test_Time()
#endif
