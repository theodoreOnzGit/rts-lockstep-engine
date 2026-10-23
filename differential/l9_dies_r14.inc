/* Differential-test driver for the die batch R14 (p1w13-sonnet; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateObjectDie.cpp, CrushDie.cpp, CreateCrateDie.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the dying object (id 1: the module, maybe a body, an AI, a veterancy level
** and status bits), the damage dealer (id 2), the object the creation list makes (id 3: maybe a body) and one more object (id 4), each maybe with an
** AI; the game logic's list newest first; the object creation lists OCL_A and OCL_B of l9_upgrades_b.inc, which make the object the test names (or
** nothing). Every call is a line of the effect log (ulog). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateObjectDie.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CrushDie.cpp"   /* batch R14b */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateCrateDie.cpp"   /* batch R14c */

/* batch R14b: a drawable's two masks, as the indices of their flags */
void Drawable::clearAndSetModelConditionFlags(const ModelConditionFlags &clr, const ModelConditionFlags &set)
{
	std::string line = "clearsetflags";
	char b[16];
	for (int i = 0; i < MODELCONDITION_COUNT; ++i) if (clr.test(i)) { sprintf(b, " %d", i); line += b; }
	line += " |";
	for (int i = 0; i < MODELCONDITION_COUNT; ++i) if (set.test(i)) { sprintf(b, " %d", i); line += b; }
	ulog("%s", line.c_str());
}
void AIUpdateInterface::transferAttack(ObjectID fromID, ObjectID toID) { ulog("transferattack %d %d %d", (int)m_owner->m_oid, (int)fromID, (int)toID); }
/* batch R14c: CreateCrateDie */
void AIUpdateInterface::notifyCrate(ObjectID id) { ulog("notifycrate %d %d", (int)m_owner->m_oid, (int)id); }
void Drawable::setTerrainDecalSize(Real x, Real y) { ulog("decalsize %08x %08x", realBits(x), realBits(y)); }
CrateSystem *TheCrateSystem = 0;

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

/* batch R14b: the crushed vehicle's body keeps its two flags */
class R14CrushBody : public InactiveBody {
public:
	R14CrushBody(Thing *t, const ModuleData *d) : InactiveBody(t, d) {}
	virtual Bool getFrontCrushed() const { return m_front; }
	virtual Bool getBackCrushed() const { return m_back; }
	virtual void setFrontCrushed(Bool v) { m_front = v; }
	virtual void setBackCrushed(Bool v) { m_back = v; }
	Bool m_front = false, m_back = false;
};

ModuleData *l9r14bRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = CrushDie::friend_newModuleData(&ini);
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

/* batch R14c */
namespace {
const KindOfType g_r14cKinds[4] = { KINDOF_VEHICLE, KINDOF_INFANTRY, KINDOF_AIRCRAFT, KINDOF_STRUCTURE };
void r14cSetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 4; ++i) if (bits >> i & 1) o->m_kindOf.set(g_r14cKinds[i]);
	o->m_tmpl.m_kindOf = o->m_kindOf;
}
int g_r14cRel = 0;
Relationship r14cTeamRel(const Team *a, const Team *b) { return (a->m_dummy == 2 && b->m_dummy == 1) ? (Relationship)g_r14cRel : ALLIES; }
void r14cSetTeam(Object *o, Team *t) { ulog("setteam %d %d", (int)o->m_oid, t ? (int)t->m_dummy : -1); }
bool g_r14cDrawable = false;
std::vector<Drawable *> g_r14cDrawables;
Object *r14cNewObject(const ThingTemplate *tmplate, Team *team)
{
	Object *o = r13bNewObject(tmplate, team);
	if (g_r14cDrawable) {
		Drawable *d = new Drawable;
		d->m_ownerObject = o;
		o->m_drawable = d;
		g_r14cDrawables.push_back(d);
	}
	return o;
}
ModuleData *l9r14cRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = CreateCrateDie::friend_newModuleData(&ini);
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

/* batch R14b: read `filename` (block `D`) as the data of a CrushDie and make the module on the vehicle (id 1). `cfg` (12 words): [0 flags (bit 0 the
** vehicle has a body, 1 a drawable, 2 its front is crushed, 3 its back is), 1-2 its position, 3 its orientation, 4 its geometry's major radius, 5-6 the
** crusher's (id 2) position (float bits), 7 the logic random seed]. The data dump goes first (the die mux, the three sounds and the three percentages).
** Then `n` ops of 6 words [op, a, b, c, d, e]: 0 onDie (death type a, veterancy b, status words c and d, damage source e; the damage type is crush,
** but for e's bit 8: the type is e >> 9)  1 flags = a  3 the crusher's position = (a, b)  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op
** appends: the CATCH_ALL status, the effect log (count, then each), the xfer status, bytes and CRC, then the body's two flags. */
int ref_l9r14b_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r14Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r14bRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	const CrushDieModuleData *cd = (const CrushDieModuleData *)d;
	putDieMux(cd->m_dieMuxData);
	for (int k = 0; k < 3; ++k) putStr(cd->m_crushSounds[k].getEventName().str(), cd->m_crushSounds[k].getEventName().getLength());
	for (int k = 0; k < 3; ++k) putI32(cd->m_crushSoundPercent[k]);
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	TheAudio = &g_ddAudio;
	InitGameLogicRandom(cfg[7]);
	unsigned flags = cfg[0];
	Team team;
	team.m_dummy = 1;
	ModuleData plain;
	for (int k = 0; k < 2; ++k) {
		g_r14Objs[k] = newObject();
		g_r14Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r14Objs[k]->m_team = &team;
		g_r14Objs[k]->m_next = k ? g_r14Objs[k - 1] : 0;
	}
	g_r14Objs[2] = g_r14Objs[3] = 0;
	Object *obj = g_r14Objs[0], *crusher = g_r14Objs[1];
	R14CrushBody body(obj, &plain);
	body.m_front = (flags & 4) != 0;
	body.m_back = (flags & 8) != 0;
	Drawable draw;
	draw.m_ownerObject = obj;
	obj->m_pos.x = ctF(cfg[1]);
	obj->m_pos.y = ctF(cfg[2]);
	obj->m_orientation = ctF(cfg[3]);
	obj->m_geometry = GeometryInfo(GEOMETRY_BOX, false, 10.0f, ctF(cfg[4]), ctF(cfg[4]));
	crusher->m_pos.x = ctF(cfg[5]);
	crusher->m_pos.y = ctF(cfg[6]);
	g_modFindHook = r14Find;
	g_modDestroyHook = cfDestroyHook;
	DieModule *m = new CrushDie(obj, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 1) flags = o[1];
		if (o[0] == 3) { crusher->m_pos.x = ctF(o[1]); crusher->m_pos.y = ctF(o[2]); }
		obj->m_body = (flags & 1) ? &body : 0;
		obj->m_drawable = (flags & 2) ? &draw : 0;
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
				info.in.m_sourceID = (ObjectID)(o[5] & 255);
				info.in.m_damageType = (o[5] & 256) ? (DamageType)(o[5] >> 9) : DAMAGE_CRUSH;
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
		putU32(body.m_front ? 1 : 0);
		putU32(body.m_back ? 1 : 0);
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	delete m;
	delete d;
	for (int k = 0; k < 2; ++k) { g_r14Objs[k]->m_body = 0; g_r14Objs[k]->m_drawable = 0; delete g_r14Objs[k]; g_r14Objs[k] = 0; }
	TheGameLogic = 0;
	TheAudio = 0;
	return 0;
}

/* batch R14c: read `filename` (block `D`) as the data of a CreateCrateDie and make the module on the dying object (id 1, team 1); the killer is id 2
** (team 2). The crate system holds `ncr` templates of 14 words each [0 the name (C<a>), 1 the creation chance (float bits), 2 the veterancy level
** (4: LEVEL_INVALID), 3 the killer's kinds (bits of g_r14cKinds), 4 the killer's science (index into SCIENCE_NAMES_CF, 5: SCIENCE_INVALID), 5 owned
** by the maker, 6 the number of crates (0-3), then each crate's name (0 U1, 1 U2, 2 Missing) and chance (float bits)]. `cfg` (12 words): [0 the
** dying object's player (0xFFFFFFFF none), 1 the killer's, 2 the killer's kinds, 3 flags (bit 0 the killer has an AI, 1 the crates the factory
** makes have a drawable, 2-3 the killer team's relationship to the dying object's (0 enemies, 1 neutral, 2 allies), 4-6 players 0 to 2 are
** computers, 8 findPositionAround finds a position), 4-6 players 0 to 2's sciences (bits of SCIENCE_NAMES_CF), 7 the logic random seed, 8-10 the
** dying object's position (float bits), 11 its layer]. The data dump goes first (the die mux, the crate names). Then `n` ops of 6 words [op, a, b,
** c, d, e]: 0 onDie (death type a, veterancy b, status words c and d, damage source e)  1 flags = a  2 the killer's player = a - 1 (0 none)
** 3 the killer's kinds = a  4 findPositionAround: has = a & 1, x = b, y = c  5 the dying object's layer = a  7 xfer save  8 xfer load of the last
** save  9 xfer CRC. Each op appends: the CATCH_ALL status, the effect log (count, then each), the xfer status, bytes and CRC, then the objects made so
** far (count, then each one's id, team and position). */
int ref_l9r14c_run(const char *filename, const char *xfer_path, const unsigned *cfg, int ncr, const unsigned *crates, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r14Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r14cRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	const CreateCrateDieModuleData *cd = (const CreateCrateDieModuleData *)d;
	putDieMux(cd->m_dieMuxData);
	putU32((unsigned)cd->m_crateNameList.size());
	for (AsciiStringListConstIterator it = cd->m_crateNameList.begin(); it != cd->m_crateNameList.end(); ++it) putStr(it->str(), it->getLength());
	unsigned scienceKeys[5];
	for (int i = 0; i < 5; ++i) scienceKeys[i] = (unsigned)NAMEKEY(SCIENCE_NAMES_CF[i]);
	CrateSystem crateSystem;
	std::vector<CrateTemplate> templates((size_t)ncr);
	static const char *const kCrateNames[3] = { "U1", "U2", "Missing" };
	for (int k = 0; k < ncr; ++k) {
		const unsigned *c = crates + 14 * k;
		CrateTemplate &t = templates[(size_t)k];
		char nm[16];
		sprintf(nm, "C%u", c[0]);
		t.m_name = nm;
		t.m_creationChance = ctF(c[1]);
		t.m_veterancyLevel = c[2] == 4 ? LEVEL_INVALID : (VeterancyLevel)c[2];
		for (int i = 0; i < 4; ++i) if (c[3] >> i & 1) t.m_killedByTypeKindof.set(g_r14cKinds[i]);
		t.m_killerScience = c[4] == 5 ? SCIENCE_INVALID : (ScienceType)scienceKeys[c[4]];
		t.m_isOwnedByMaker = c[5] != 0;
		for (unsigned e = 0; e < c[6]; ++e) {
			crateCreationEntry entry;
			entry.crateName = kCrateNames[c[7 + 2 * e]];
			entry.crateChance = ctF(c[8 + 2 * e]);
			t.m_possibleCrates.push_back(entry);
		}
		crateSystem.m_templates.push_back(&t);
	}
	TheCrateSystem = &crateSystem;
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[7]);
	unsigned flags = cfg[3];
	Team defaults[3];
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		defaults[i].m_dummy = 11 + i;
		players[i].m_defaultTeam = &defaults[i];
		for (int s = 0; s < 5; ++s) if (cfg[4 + i] >> s & 1) players[i].m_sciences.push_back((ScienceType)scienceKeys[s]);
	}
	g_r13bTemplates[0].m_name = "U1";
	g_r13bTemplates[1].m_name = "U2";
	g_findTemplateHook = r13bFindTemplate;
	g_newObjectHook = r14cNewObject;
	g_r13bMade.clear();
	g_r13bNextId = 3;
	g_teamRelHook = r14cTeamRel;
	g_setTeamHook = r14cSetTeam;
	Team teams[2];
	AIUpdateInterface killerAi;
	for (int k = 0; k < 2; ++k) {
		g_r14Objs[k] = newObject();
		g_r14Objs[k]->m_oid = (ObjectID)(k + 1);
		teams[k].m_dummy = k + 1;
		g_r14Objs[k]->m_team = &teams[k];
		g_r14Objs[k]->m_next = k ? g_r14Objs[k - 1] : 0;
	}
	g_r14Objs[2] = g_r14Objs[3] = 0;
	Object *obj = g_r14Objs[0], *killer = g_r14Objs[1];
	killerAi.m_owner = killer;
	obj->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	killer->m_player = cfg[1] == 0xFFFFFFFFu ? 0 : &players[cfg[1]];
	r14cSetKinds(killer, cfg[2]);
	obj->m_pos.x = ctF(cfg[8]);
	obj->m_pos.y = ctF(cfg[9]);
	obj->m_pos.z = ctF(cfg[10]);
	obj->m_layer = (PathfindLayerEnum)cfg[11];
	g_ctFindPos.x = 7.5f;
	g_ctFindPos.y = -3.25f;
	g_ctFindPos.z = 0;
	g_modFindHook = r14Find;
	g_modDestroyHook = cfDestroyHook;
	DieModule *m = new CreateCrateDie(obj, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 1) flags = o[1];
		if (o[0] == 2) killer->m_player = o[1] ? &players[o[1] - 1] : 0;
		if (o[0] == 3) r14cSetKinds(killer, o[1]);
		if (o[0] == 4) { flags = (flags & ~256u) | ((o[1] & 1) << 8); g_ctFindPos.x = ctF(o[2]); g_ctFindPos.y = ctF(o[3]); }
		if (o[0] == 5) obj->m_layer = (PathfindLayerEnum)o[1];
		killer->m_ai = (flags & 1) ? &killerAi : 0;
		g_r14cDrawable = (flags & 2) != 0;
		g_r14cRel = (int)(flags >> 2 & 3);
		for (int p = 0; p < 3; ++p) players[p].m_type = (flags >> (4 + p) & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN;   /* the stand-in has no constructor */
		g_ctFindPosHas = (flags & 256) != 0;
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
		putU32((unsigned)g_r13bMade.size());
		for (size_t j = 0; j < g_r13bMade.size(); ++j) {
			const Object *x = g_r13bMade[j];
			putU32((unsigned)x->m_oid);
			putI32(x->m_team ? (int)x->m_team->m_dummy : -1);
			putU32(realBits(x->m_pos.x));
			putU32(realBits(x->m_pos.y));
			putU32(realBits(x->m_pos.z));
		}
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_teamRelHook = 0;
	g_setTeamHook = 0;
	g_ctFindPosHas = false;
	TheCrateSystem = 0;
	delete m;
	delete d;
	for (size_t j = 0; j < g_r13bMade.size(); ++j) { g_r13bMade[j]->m_xp = 0; g_r13bMade[j]->m_drawable = 0; delete g_r13bMade[j]; }
	g_r13bMade.clear();
	for (size_t j = 0; j < g_r13bTrackers.size(); ++j) delete g_r13bTrackers[j];
	g_r13bTrackers.clear();
	for (size_t j = 0; j < g_r14cDrawables.size(); ++j) delete g_r14cDrawables[j];
	g_r14cDrawables.clear();
	for (int k = 0; k < 2; ++k) { g_r14Objs[k]->m_ai = 0; delete g_r14Objs[k]; g_r14Objs[k] = 0; }
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
