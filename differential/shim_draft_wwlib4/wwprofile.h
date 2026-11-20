/* (#215, group wwlib4) stand-in for WWLib/wwprofile.h: the WWLOG_* timing macros of saveload.cpp compile to nothing (log output only). */
#ifndef WWLIB4_WWPROFILE_H
#define WWLIB4_WWPROFILE_H
#define WWLOG_PREPARE_TIME_AND_MEMORY(x) ((void)0)
#define WWLOG_INTERMEDIATE(x) ((void)0)
#endif
