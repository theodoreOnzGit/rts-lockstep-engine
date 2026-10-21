/* The crash trap of the batch M drivers (p1w12-sonnet). A planted bug in the Rust side can steer a case past an input that is undefined upstream (the
** Rust records it and the case is never sent), and the C++ then dereferences it. Rather than let that kill the process, the C++ run is made a
** mismatch: status 99, which the test reports. (Not a way to hide a crash in the C++: the unplanted run has no such case, and the test fails on 99.)
** A driver defines `static int xInner(...)` with the signature below and `extern "C" int ref_x_run(...)` as a call of l9TrappedRun. */
#include <setjmp.h>
#include <signal.h>
namespace {
sigjmp_buf g_l9tJmp;
volatile sig_atomic_t g_l9tArmed = 0;
void l9tCrashHandler(int) { if (g_l9tArmed) siglongjmp(g_l9tJmp, 1); }
typedef int (*L9RunFn)(const char *, const char *, const unsigned *, int, const unsigned *, unsigned char *, int, int *, char *, int, unsigned *);
int l9TrappedRun(L9RunFn f, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len,
	char *msg, int msgCap, unsigned *code)
{
	struct sigaction sa, oldSegv, oldBus;
	memset(&sa, 0, sizeof sa);
	sa.sa_handler = l9tCrashHandler;
	sigemptyset(&sa.sa_mask);
	sigaction(SIGSEGV, &sa, &oldSegv);
	sigaction(SIGBUS, &sa, &oldBus);
	g_l9tArmed = 1;
	int rc;
	if (sigsetjmp(g_l9tJmp, 1) == 0) {
		rc = f(filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
	} else {
		rc = 99;
		*len = 0;
		TheGameLogic = 0; ThePlayerList = 0; TheGameText = 0; TheInGameUI = 0; TheAnim2DCollection = 0; TheParticleSystemManager = 0; ThePartitionManager = 0;
		g_spLocal = 0;
	}
	g_l9tArmed = 0;
	sigaction(SIGSEGV, &oldSegv, 0);
	sigaction(SIGBUS, &oldBus, 0);
	return rc;
}
}  // namespace
