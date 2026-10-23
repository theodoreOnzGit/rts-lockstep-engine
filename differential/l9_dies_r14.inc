/* Differential-test driver for the die batch R14 (p1w13-sonnet; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateObjectDie.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the dying object (id 1: the module, maybe a body, an AI, a veterancy level
** and status bits), the damage dealer (id 2), the object the creation list makes (id 3: maybe a body) and one more object (id 4), each maybe with an
** AI; the game logic's list newest first; the object creation lists OCL_A and OCL_B of l9_upgrades_b.inc, which make the object the test names (or
** nothing). Every call is a line of the effect log (ulog). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateObjectDie.cpp"

void AIUpdateInterface::transferAttack(ObjectID fromID, ObjectID toID) { ulog("transferattack %d %d %d", (int)m_owner->m_oid, (int)fromID, (int)toID); }

namespace {

/* a body: the answers the test gives; a damage is a line, in the probe's two formats */
class R14Body : public InactiveBody {
public:
	R14Body(Thing *t, const ModuleData *d) : InactiveBody(t, d) {}
	virtual Real getMaxHealth() const { return m_max; }
	virtual Real getPreviousHealth() const { return m_prev; }
	virtual Real getCurrentSubdualDamageAmount() const { return m_subdual; }
	virtual const DamageInfo *getLastDamageInfo() const { return &m_last; }
	virtual void attemptDamage(DamageInfo *info)
	{
		const int oid = (int)AsObject(const_cast<Thing *>(static_cast<const Thing *>(getObject())))->m_oid;
		if (info->in.m_damageType == DAMAGE_SUBDUAL_UNRESISTABLE)
			ulog("attemptdamage %d %u %d %d", (int)info->in.m_damageType, realBits(info->in.m_amount), (int)info->in.m_sourceID, (int)info->in.m_deathType);
		else
			ulog("damage %d %08x %d %d %d", oid, realBits(info->in.m_amount), (int)info->in.m_damageType, (int)info->in.m_deathType, (int)info->in.m_sourceID);
	}
	Real m_max = 0, m_prev = 0, m_subdual = 0;
	DamageInfo m_last;
};

Object *g_r14Objs[4];
unsigned g_r14OclResult[2];
Object *r14Find(ObjectID id) { return (id >= 1 && id <= 4) ? g_r14Objs[id - 1] : 0; }
Object *r14OclResult(int list) { return g_r14OclResult[list] ? r14Find((ObjectID)g_r14OclResult[list]) : 0; }

ModuleData *l9r14Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = CreateObjectDie::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r14Saved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a CreateObjectDie (`kind` 0) and make the module on object 1. `cfg` (12 words): [0 flags (bits 0-3 objects 1
** to 4 have an AI, 4 object 1 has a body, 5 object 3 has one), 1 object 1's maximum health, 2 its previous health, 3 its subdual damage (float bits), 4 the
** source of its last damage, 5 what OCL_A makes (0 nothing, else an object id), 6 what OCL_B makes]. The data dump goes first (the die mux, the list,
** the flag). Then `n` ops of 6 words [op, a, b, c, d, e]: 0 onDie (death type a, veterancy b, status words c and d, damage source e)  1 flags = a
** 2 the list results = a, b  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op appends: the CATCH_ALL status, the effect log (count, then
** each), the xfer status, bytes and CRC. */
int ref_l9r14_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r14Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r14Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	const CreateObjectDieModuleData *cd = (const CreateObjectDieModuleData *)d;
	putDieMux(cd->m_dieMuxData);
	putI32(cd->m_ocl == 0 ? -1 : (cd->m_ocl == &g_upOcls[0] ? 0 : 1));
	putU32(cd->m_transferPreviousHealth ? 1 : 0);
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	unsigned flags = cfg[0];
	Team team;
	team.m_dummy = 1;
	AIUpdateInterface ais[4];
	ModuleData plain;
	for (int k = 0; k < 4; ++k) {
		g_r14Objs[k] = newObject();
		g_r14Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r14Objs[k]->m_team = &team;
		g_r14Objs[k]->m_next = k ? g_r14Objs[k - 1] : 0;
		ais[k].m_owner = g_r14Objs[k];
	}
	g_ddFirst = g_r14Objs[3];
	R14Body body1(g_r14Objs[0], &plain), body3(g_r14Objs[2], &plain);
	Object *obj = g_r14Objs[0];
	body1.m_max = ctF(cfg[1]);
	body1.m_prev = ctF(cfg[2]);
	body1.m_subdual = ctF(cfg[3]);
	body1.m_last.in.m_sourceID = (ObjectID)cfg[4];
	g_r14OclResult[0] = cfg[5];
	g_r14OclResult[1] = cfg[6];
	g_modFindHook = r14Find;
	g_oclResultHook = r14OclResult;
	g_modDestroyHook = cfDestroyHook;
	DieModule *m = new CreateObjectDie(obj, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 1) flags = o[1];
		if (o[0] == 2) { g_r14OclResult[0] = o[1]; g_r14OclResult[1] = o[2]; }
		for (int k = 0; k < 4; ++k) g_r14Objs[k]->m_ai = (flags >> k & 1) ? &ais[k] : 0;
		g_r14Objs[0]->m_body = (flags & 16) ? &body1 : 0;
		g_r14Objs[2]->m_body = (flags & 32) ? &body3 : 0;
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		try {
			switch (o[0]) {
			case 0: {
				DamageInfo info;
				info.in.m_deathType = (DeathType)o[1];
				info.in.m_sourceID = (ObjectID)o[5];
				obj->m_level = (VeterancyLevel)o[2];
				obj->m_status = maskFromU32<ObjectStatusMaskType>(OBJECT_STATUS_COUNT, o + 3);
				dynamic_cast<DieModuleInterface *>(m)->onDie(&info);
				break;
			}
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r14Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r14Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_oclResultHook = 0;
	g_modDestroyHook = 0;
	g_ddFirst = 0;
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) { g_r14Objs[k]->m_ai = 0; g_r14Objs[k]->m_body = 0; delete g_r14Objs[k]; g_r14Objs[k] = 0; }
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
