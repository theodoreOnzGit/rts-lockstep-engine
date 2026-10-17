/* Shim for winsock2.h, differential tests only. crc.h includes it for htonl
** (never called there); XferCRC.cpp calls htonl, which on Linux comes from
** <arpa/inet.h> with the same meaning (host to big-endian, 32 bits). */
#include <arpa/inet.h>
