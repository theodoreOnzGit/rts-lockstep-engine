/* (#215, group wwlib3) stand-in for WWLib/mempool.h as multilist.h uses it: AutoPoolClass is allocation machinery (a pool of
** fixed-size nodes); nothing the lists do depends on it, so it is an empty base and DEFINE_AUTO_POOL declares nothing. */
#ifndef WWLIB3_MEMPOOL_H
#define WWLIB3_MEMPOOL_H
template<class T, int BLOCK_SIZE = 64> class AutoPoolClass {};
#define DEFINE_AUTO_POOL(T, BLOCK_SIZE)
#endif
