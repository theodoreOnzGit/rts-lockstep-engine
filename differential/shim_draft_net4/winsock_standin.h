/* Stand-in for the winsock calls DownloadManager.cpp and IPEnumeration.cpp make (#215, group net4): the types the upstream code names (WORD, WSADATA,
** HOSTENT, MAKEWORD, LOBYTE, HIBYTE) and WSAStartup/WSACleanup/gethostname/gethostbyname answering from variables the wrapper sets. Never upstream code. */
#ifndef NET4_WINSOCK_STANDIN_H
#define NET4_WINSOCK_STANDIN_H
#include <string.h>
#include <arpa/inet.h>
typedef unsigned short WORD;
typedef unsigned char BYTE;
#define MAKEWORD(a, b) ((WORD)(((BYTE)(a)) | (((WORD)((BYTE)(b))) << 8)))
#define LOBYTE(w) ((BYTE)(w))
#define HIBYTE(w) ((BYTE)(((WORD)(w) >> 8) & 0xFF))
struct WSADATA { WORD wVersion; };
struct HOSTENT { char *h_name; char **h_aliases; short h_addrtype; short h_length; char **h_addr_list; };
static int g_wsaErr = 0;
static WORD g_wsaVer = 0x0202;
static int g_hostnameFail = 0;
static char g_hostname[64] = "host";
static int g_lookupFail = 0;
static char *g_addrList[16];
static HOSTENT g_hostEnt;
static inline int WSAStartup(WORD, WSADATA *d) { d->wVersion = g_wsaVer; return g_wsaErr; }
static inline int WSACleanup() { return 0; }
static inline int WSAGetLastError() { return 0; }
static inline int net4_gethostname(char *buf, int len) { if (g_hostnameFail) return -1; strncpy(buf, g_hostname, len); return 0; }
static inline HOSTENT *net4_gethostbyname(const char *) { return g_lookupFail ? 0 : &g_hostEnt; }
#define gethostname net4_gethostname
#define gethostbyname net4_gethostbyname
#endif
