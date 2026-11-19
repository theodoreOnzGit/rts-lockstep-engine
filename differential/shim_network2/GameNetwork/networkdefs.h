/* Stand-in, differential tests only (#215, group network2): upstream's User.h includes "GameNetwork/networkdefs.h"; the file on
** disk is NetworkDefs.h. Forwards to the unchanged upstream header. */
#include "GameNetwork/NetworkDefs.h"
