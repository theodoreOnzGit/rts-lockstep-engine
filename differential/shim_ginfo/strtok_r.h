/* Stand-in for strtok_r.h, differential tests only (differential/wrap_game_info.cpp): Westwood's strtok_r (WWLib/strtok_r.cpp, compiled by the wrapper under another name: glibc declares its own). */
#ifndef SHIM_GINFO_STRTOK_R_H
#define SHIM_GINFO_STRTOK_R_H
char *wwStrtokR(char *strptr, const char *delimiters, char **lasts);
#endif
