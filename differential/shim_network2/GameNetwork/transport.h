/* Stand-in, differential tests only (#215, group network2): upstream's Connection.h includes "GameNetwork/transport.h" and the file
** on disk is Transport.h (Windows file systems are case-insensitive). Forwards to the unchanged upstream header. */
#include "GameNetwork/Transport.h"
