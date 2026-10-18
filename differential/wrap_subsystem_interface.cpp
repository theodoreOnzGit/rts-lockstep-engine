/* Differential-test wrapper for GameEngine/Include/Common/SubsystemInterface.h
** and Source/Common/System/SubsystemInterface.cpp (upstream commit 0a05454d),
** compiled unchanged (build.rs), with the stand-ins of differential/shim_subsys
** for INI and Xfer. TheSubsystemList is NULL here, so the constructors do not
** register; the list is the one the wrapper creates.
**
** ref_subsys_run runs a script of two-int ops and writes one text line per
** event (the log), returning its length or -1 if it exceeds cap:
**   0 v   initSubsystem(new LogSys(v), no INI paths, name(v)); name(v) is ""
**         for v == 0 and "N<v>" otherwise. Logs "init <v> <name>" from init().
**   1 _   postProcessLoadAll()        logs "post <id>" per subsystem, in order
**   2 _   resetAll()                  logs "reset <id>" per subsystem
**   3 _   shutdownAll()               logs "drop <id>" per subsystem
** When the script ends the list is deleted, which shuts down what remains.
** Each LogSys logs "drop <id>" when deleted. */
#include <stdio.h>
#include <string.h>
#include <string>
#include <vector>

#include "PreRTS.h"
#include "Common/SubsystemInterface.h"

SubsystemInterfaceList *TheSubsystemList = NULL;

namespace {
std::string g_log;

void logf(const char *fmt, int a, const std::string &b = std::string())
{
	char line[256];
	snprintf(line, sizeof(line), fmt, a, b.c_str());
	g_log += line;
	g_log += '\n';
}

struct LogSys : public SubsystemInterface {
	int id;
	explicit LogSys(int i) : id(i) {}
	~LogSys() { logf("drop %d%s", id); }
	void init() { logf("init %d %s", id, getName().str()); }
	void postProcessLoad() { logf("post %d%s", id); }
	void reset() { logf("reset %d%s", id); }
	void update() {}
};
}

extern "C" int ref_subsys_run(const int *ops, int n, char *out, int cap)
{
	g_log.clear();
	SubsystemInterfaceList *list = new SubsystemInterfaceList();
	for (int i = 0; i < n; ++i) {
		int op = ops[2 * i], v = ops[2 * i + 1];
		switch (op) {
		case 0: {
			char name[32];
			if (v == 0) name[0] = '\0';
			else snprintf(name, sizeof(name), "N%d", v);
			list->initSubsystem(new LogSys(v), NULL, NULL, NULL, NULL, AsciiString(name));
			break;
		}
		case 1: list->postProcessLoadAll(); break;
		case 2: list->resetAll(); break;
		case 3: list->shutdownAll(); break;
		}
	}
	delete list;
	if ((int)g_log.size() > cap) return -1;
	memcpy(out, g_log.data(), g_log.size());
	return (int)g_log.size();
}
