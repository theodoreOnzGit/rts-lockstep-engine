/* Differential-test driver for the lead's layer-9 example (upstream commit 0a05454d), p1l9-example:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/CommandSetUpgrade.cpp   (#included, unchanged)
** with CommandSetUpgrade.h unchanged, in wrap_module.cpp's unit (it is #included at that file's end), so that
** it shares that unit's stand-ins (Object, Thing, Player, ControlBar under differential/shim_module), its
** effect log (g_events) and its dump helpers, and reaches the REAL UpgradeCenter of wrap_upgrade.cpp.
**
** This is the pattern for every layer-9 batch: one .inc file like this one, #included in its marked block
** at the end of wrap_module.cpp; the batch's stand-in additions go in its marked blocks of the
** shim_module headers. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/CommandSetUpgrade.cpp"

ControlBar *TheControlBar = 0;
void ControlBar::markUIDirty() { event(3, 0); }

namespace {
/* read the block `D` of `filename` as CommandSetUpgradeModuleData (readData's wrapping of a throw) */
ModuleData *l9exRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = CommandSetUpgrade::friend_newModuleData(&ini);
		} catch (...) {
			char buff[1024];
			sprintf(buff, "Error parsing INI file '%s' (Line: '%s')\n", ini.m_filename.str(), currentLine.str());
			throw INIException(buff);
		}
	} catch (...) {
		ini.unPrepFile();
		throw;
	}
	ini.unPrepFile();
	return data;
}
}  // namespace

extern "C" {

/* Read `filename` (block `D`) and make a CommandSetUpgrade on an Object whose controlling player is a Player
** with completed upgrades `player_mask` (no player if `has_player` is 0) and whose own completed upgrades are
** `obj_mask` (4 words each). The data dump (the mux, then CommandSet, CommandSetAlt, TriggerAlt) goes first.
** Then `n` ops of 5 words [op, m0..m3]:
**   0 wouldUpgrade(m)  1 attemptUpgrade(m)  2 resetUpgrade(m)  3 forceRefreshUpgrade()  4 isAlreadyUpgraded()
**   5 the object's completed upgrades = m  6 the player's completed upgrades = m (nothing without a player)
** Each op appends: the CATCH_ALL status, the Bool result, isAlreadyUpgraded(), the effects in order (count,
** then each as a string: "fx<id>", "remove <name>", "dirty"), and the object's command-set override. */
int ref_l9ex_cmdset_run(const char *filename, int has_player, const unsigned *player_mask, const unsigned *obj_mask, int n,
	const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	CommandSetUpgradeModuleData *d = 0;
	try { d = (CommandSetUpgradeModuleData *)l9exRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	putMux(d->m_upgradeMuxData);
	putAscii(d->m_newCommandSet);
	putAscii(d->m_newCommandSetAlt);
	putAscii(d->m_triggerAlt);
	Player *player = has_player ? new Player : 0;
	if (player) player->m_upgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, player_mask);
	Object *obj = newObject();
	obj->m_player = player;
	obj->m_objectUpgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, obj_mask);
	CommandSetUpgrade *m = new CommandSetUpgrade(obj, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		g_events.clear();
		int st = 0, res = 0;
		unsigned c2 = 0;
		char m2[16];
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		try {
			switch (o[0]) {
			case 0: res = m->wouldUpgrade(mask) ? 1 : 0; break;
			case 1: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 2: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 3: m->forceRefreshUpgrade(); break;
			case 4: res = m->isAlreadyUpgraded() ? 1 : 0; break;
			case 5: obj->m_objectUpgradesCompleted = mask; break;
			case 6: if (player) player->m_upgradesCompleted = mask; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		g_dump.push_back((unsigned char)res);
		g_dump.push_back(m->isAlreadyUpgraded() ? 1 : 0);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			char b[32];
			if (e.kind == 1) { sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
			else if (e.kind == 3) log.push_back("dirty");
			else log.push_back("other");
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putAscii(obj->m_commandSetStringOverride);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	delete player;
	return 0;
}

}  /* extern "C" */
