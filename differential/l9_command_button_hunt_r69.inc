/* Differential-test driver for CommandButtonHuntUpdate (p1w13-sonnet, batch R69; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CommandButtonHuntUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: six objects (ids 1-6: the hunter and five
** others; a team of three, each with its player, disabled or not), the teams' relationships, the hunter's command set (an override name
** of the test's; the stand-in template has none), the command sets SetA and SetB of four buttons (ControlBar.h's stand-ins: a name, a
** command type, a weapon slot, a special power template; findCommandSet below), the hunter's AI (idle, the last command's source, an
** attack priority info of the test's: AttackPriorityInfo::getPriority below, on its table), its special ability update (a power type,
** active) and maybe a REAL special power module of SP_0..SP_3 (batch R60's way), Object::getSpecialPowerModule (batch R53's, verbatim), the
** REAL special power store, the scan (the objects the test lists; the call a line with the relationship filter's flags), the distance
** (the test's table), the four ActionManager tests (lines, answered by the test's bits), doCommandButtonAtObject and setWeaponLock (lines),
** TheAI's m_attackPriorityDistanceModifier (the test's), the REAL Object::getRelationship, UpdateModule, INI, Xfer and logic random values;
** the game logic (its frame, its wake calls applied, batch R35's way). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CommandButtonHuntUpdate.cpp"

/* the GUICommandType values the Rust translation names (logic::modules::command_button_hunt_update) */
static_assert(GUI_COMMAND_FIRE_WEAPON == 20 && GUI_COMMAND_SPECIAL_POWER == 21 && GUI_COMMAND_SWITCH_WEAPON == 26, "GUICommandType");
static_assert(GUICOMMANDMODE_HIJACK_VEHICLE == 27 && GUICOMMANDMODE_CONVERT_TO_CARBOMB == 28 && GUICOMMANDMODE_SABOTAGE_BUILDING == 29, "GUICommandType");
static_assert(GUI_COMMAND_GUARD == 9, "GUICommandType");

namespace {

ModuleData *l9r69Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = CommandButtonHuntUpdate::friend_newModuleData(&ini);
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

/* a REAL special power module (getSpecialPowerModule finds it by its template); the hunt calls nothing of it */
class R69Power : public SpecialPowerModule {
public:
	R69Power(Thing *t, const ModuleData *d) : SpecialPowerModule(t, d) {}
	virtual ~R69Power() {}
};

std::vector<unsigned char> g_r69Saved;
Object *g_r69Objs[6];
Team g_r69Teams[3];
Player g_r69Players[3];
int g_r69Rel[3][3];
CommandButton g_r69Buttons[4];
CommandSet g_r69Sets[2];
const char *const g_r69SetNames[4] = { "", "SetA", "SetB", "SetC" };
const char *const g_r69ButtonNames[5] = { "", "BtnA", "BtnB", "BtnC", "BtnX" };
/* the command types, by index (the same list in the Rust test) */
const GUICommandType g_r69Types[8] = { GUI_COMMAND_NONE, GUI_COMMAND_FIRE_WEAPON, GUI_COMMAND_SPECIAL_POWER, GUI_COMMAND_SWITCH_WEAPON,
	GUICOMMANDMODE_HIJACK_VEHICLE, GUICOMMANDMODE_CONVERT_TO_CARBOMB, GUICOMMANDMODE_SABOTAGE_BUILDING, GUI_COMMAND_GUARD };
/* the special ability update's power types, by index (0: none) */
const int g_r69AbilityTypes[6] = { -1, SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK, SPECIAL_INFANTRY_CAPTURE_BUILDING, SPECIAL_TIMED_CHARGES,
	SPECIAL_TANKHUNTER_TNT_ATTACK, SPECIAL_HACKER_DISABLE_BUILDING };
unsigned g_r69Allowed = 0;
Real g_r69Dist[6];
const SpecialPowerTemplate *r69Template(unsigned k)
{
	if (k == 0 || k > 4) return 0;
	char name[8];
	sprintf(name, "SP_%u", k - 1);
	return (const SpecialPowerTemplate *)ref_sp_find_raw(name);
}
unsigned r69TemplateId(const SpecialPowerTemplate *t) { return t ? t->getID() : 0; }
Object *r69Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 6) ? g_r69Objs[(int)id - 1] : 0; }
Relationship r69TeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return (Relationship)g_r69Rel[from->m_dummy - 1][to->m_dummy - 1];
}
Real r69Distance(const Object *a, const Object *b, DistanceCalculationType dc)
{
	ulog("distsq %d %d %d", (int)a->m_oid, (int)b->m_oid, (int)dc);
	return g_r69Dist[b->m_oid - 1];
}
void r69ScanHook(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	int n = 0, rel = -1;
	while (filters && filters[n]) {
		if (PartitionFilterRelationship *r = dynamic_cast<PartitionFilterRelationship *>(filters[n])) rel = r->m_flags;
		++n;
	}
	ulog("huntscan %08x %08x %08x %08x %d %d %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), n, rel, g_ahRangeDc);
}
Real r69F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
/* storage for TheActionManager: the hunt calls four of its non-virtual tests, defined below; nothing constructs it */
alignas(ActionManager) unsigned char g_r69ActionManager[sizeof(ActionManager)];
Bool r69Allowed(int kind, const Object *other) { return (g_r69Allowed >> (6 * kind + (int)other->m_oid - 1) & 1) != 0; }

}  // namespace

const CommandSet *ControlBar::findCommandSet( const AsciiString& name )
{
	for (int i = 0; i < 2; ++i) if (g_r69Sets[i].getName() == name) return &g_r69Sets[i];
	return 0;
}
void Object::doCommandButtonAtObject( const CommandButton *commandButton, Object *obj, CommandSourceType cmdSource )
{
	ulog("cmdbutton %d %s %d %d", (int)m_oid, commandButton->getName().str(), (int)obj->m_oid, (int)cmdSource);
}
Int AttackPriorityInfo::getPriority(const ThingTemplate *tThing) const
{
	/* ScriptEngine.cpp 182-195 on the test's table (the template is each object's own; none has an override) */
	Int priority = m_defaultPriority;
	if (tThing==NULL) return priority;
	for (int i = 0; i < 6; ++i) if (m_tmpl[i] == tThing) priority = m_priority[i];
	return priority;
}
Bool ActionManager::canDoSpecialPowerAtObject( const Object *obj, const Object *target, CommandSourceType commandSource, const SpecialPowerTemplate *spTemplate, UnsignedInt commandOptions, Bool checkSourceRequirements)
{
	ulog("can 0 %d %d %d %u", (int)obj->m_oid, (int)target->m_oid, (int)commandSource, r69TemplateId(spTemplate));
	return r69Allowed(0, target);
}
Bool ActionManager::canHijackVehicle( const Object *obj, const Object *ObjectToHijack, CommandSourceType commandSource )
{
	ulog("can 1 %d %d %d 0", (int)obj->m_oid, (int)ObjectToHijack->m_oid, (int)commandSource);
	return r69Allowed(1, ObjectToHijack);
}
Bool ActionManager::canSabotageBuilding( const Object *obj, const Object *objectToSabotage, CommandSourceType commandSource )
{
	ulog("can 2 %d %d %d 0", (int)obj->m_oid, (int)objectToSabotage->m_oid, (int)commandSource);
	return r69Allowed(2, objectToSabotage);
}
Bool ActionManager::canConvertObjectToCarBomb( const Object *obj, const Object *objectToConvert, CommandSourceType commandSource )
{
	ulog("can 3 %d %d %d 0", (int)obj->m_oid, (int)objectToConvert->m_oid, (int)commandSource);
	return r69Allowed(3, objectToConvert);
}
ActionManager *TheActionManager = 0;

extern "C" {

/* Read `filename` (block `D`) as the data of a CommandButtonHuntUpdate and make the module on object 1 of six at frame cfg[0]. `cfg` (32
** words; all but 13 also settable by op 2): 1 the logic random seed; 2-7 objects 1-6: bits 0-1 their team (0 none, else 1-3), bit 2
** disabled, and for object 1 bits 4-5 its command set name (of g_r69SetNames); 8 the teams' relationships (2 bits each, row-major, team a to
** team b: 0 enemies, 1 neutral, 2 allies); 9 the scan (nibbles of object numbers 1-6, 0 ends); 10 the AI: bit 0 there is one, bit 1 idle,
** bits 2-3 the last command's source, bit 4 an attack priority info, bits 8-15 its default priority (signed); 11 the info's priorities of
** objects 1-6 (nibbles; 15: not listed); 12 the special ability update: bits 0-2 its power type (of g_r69AbilityTypes), bit 3 active; read
** once, 13 the special power module (0 none, else SP_(k-1)); 14 the ActionManager answers (bit 6 * kind + object - 1; kinds 0 the special
** power, 1 hijack, 2 sabotage, 3 car bomb); 15 the distance modifier (float bits); 16-21 the squared distances to objects 1-6 (float bits);
** 23-24, 25-26 SetA's and SetB's 18 slots (3 bits each, 10 in the first word: 0 empty, else button 1-4); 27-30 buttons 1-4: bits 0-1 their
** name (of g_r69ButtonNames), 2-4 their type (of g_r69Types), 5-6 their weapon slot, 7-9 their special power (0 none, else SP_(k-1)).
** The data dump goes first (the scan rate and range), then the constructor's next call frame. Then `n` ops of 5 words [op, a, b, c, d]:
** 0 update()  1 the frame = a  2 cfg[a] = b  3 setCommandButton(g_r69ButtonNames[a])  4 xfer save  5 xfer load of the last save  6 xfer CRC.
** Each op appends: the CATCH_ALL status, the result, the effect log, the xfer status, bytes and CRC, the state (the button's name, the
** button: 0 NULL, else 1-4) and the module's next call frame. */
static int l9r69RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r69Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r69Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const CommandButtonHuntUpdateModuleData *hd = (const CommandButtonHuntUpdateModuleData *)d;
		putU32(hd->m_scanFrames);
		putU32(realBits(hd->m_scanRange));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	gl.m_awakenHook = r35Awaken;   /* (batch R35's: the wake calls applied as GameLogic::friend_awakenUpdateModule applies them) */
	InitGameLogicRandom(cfg[1]);
	ThePartitionManager = &g_ahPm;
	ControlBar bar;
	TheControlBar = &bar;
	TheActionManager = (ActionManager *)g_r69ActionManager;
	AI theAi;
	TheAI = &theAi;
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 6; ++k) {
		g_r69Objs[k] = newObject();
		g_r69Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r69Objs[k]->m_behaviors = none;
	}
	for (int t = 0; t < 3; ++t) {
		g_r69Players[t].m_idx = t;
		g_r69Teams[t].m_dummy = t + 1;
		g_r69Teams[t].m_controller = &g_r69Players[t];
	}
	Object *me = g_r69Objs[0];
	AIUpdateInterface ai;
	ai.m_owner = me;
	AttackPriorityInfo info;
	SpecialAbilityUpdate sau;
	me->m_sau = &sau;
	auto scene = [&]() {
		for (int k = 0; k < 6; ++k) {
			Object *o = g_r69Objs[k];
			unsigned f = cfg[2 + k];
			unsigned t = f & 3;
			o->m_team = t ? &g_r69Teams[t - 1] : 0;
			o->m_player = t ? &g_r69Players[t - 1] : 0;
			o->m_disabled = (f >> 2 & 1) != 0;
			g_r69Dist[k] = r69F(cfg[16 + k]);
		}
		me->m_commandSetStringOverride = AsciiString(g_r69SetNames[cfg[2] >> 4 & 3]);
		for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r69Rel[a][b] = (int)(cfg[8] >> (2 * (3 * a + b)) & 3);
		g_ahRange.clear();
		for (int k = 0; k < 6; ++k) { unsigned id = cfg[9] >> (4 * k) & 15; if (id == 0) break; if (id <= 6) g_ahRange.push_back(g_r69Objs[id - 1]); }
		unsigned a = cfg[10];
		me->m_ai = (a & 1) ? &ai : 0;
		ai.m_idle = (a >> 1 & 1) != 0;
		ai.m_lastCmdSource = (Int)(a >> 2 & 3);
		ai.m_attackInfo = (a >> 4 & 1) ? &info : 0;
		info.m_defaultPriority = (Int)(signed char)(a >> 8 & 0xff);
		for (int k = 0; k < 6; ++k) {
			unsigned p = cfg[11] >> (4 * k) & 15;
			info.m_tmpl[k] = (p == 15) ? 0 : g_r69Objs[k]->getTemplate();
			info.m_priority[k] = (Int)p;
		}
		unsigned s = cfg[12] & 7;
		me->m_sauType = (s >= 1 && s <= 5) ? g_r69AbilityTypes[s] : -1;
		sau.m_active = (cfg[12] >> 3 & 1) != 0;
		g_r69Allowed = cfg[14];
		theAi.m_aiData.m_attackPriorityDistanceModifier = r69F(cfg[15]);
		for (int i = 0; i < 2; ++i) {
			g_r69Sets[i].m_name = AsciiString(g_r69SetNames[i + 1]);
			for (int j = 0; j < MAX_COMMANDS_PER_SET; ++j) {
				unsigned w = cfg[23 + 2 * i + (j >= 10 ? 1 : 0)];
				unsigned b = w >> (3 * (j % 10)) & 7;
				g_r69Sets[i].m_command[j] = (b >= 1 && b <= 4) ? &g_r69Buttons[b - 1] : 0;
			}
		}
		for (int i = 0; i < 4; ++i) {
			unsigned f = cfg[27 + i];
			g_r69Buttons[i].m_name = AsciiString(g_r69ButtonNames[f & 3]);
			g_r69Buttons[i].m_command = g_r69Types[f >> 2 & 7];
			g_r69Buttons[i].m_weaponSlot = (WeaponSlotType)(f >> 5 & 3);
			g_r69Buttons[i].m_specialPower = r69Template(f >> 7 & 7);
		}
	};
	scene();
	g_ahRangeHook = r69ScanHook;
	g_teamRelHook = r69TeamRel;
	g_modFindHook = r69Find;
	g_distDcHook = r69Distance;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	CommandButtonHuntUpdate *m = 0;
	try { m = newInstance(CommandButtonHuntUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	SpecialPowerModuleData *pd = 0;
	R69Power *power = 0;
	if (r69Template(cfg[13])) {
		pd = new SpecialPowerModuleData;
		pd->m_specialPowerTemplate = r69Template(cfg[13]);
		power = new R69Power(me, pd);
	}
	BehaviorModule *behaviors[3] = { m, power, 0 };
	me->m_behaviors = behaviors;
	putU32(m->friend_getNextCallFrame());
	auto buttonCode = [&]() -> unsigned {
		for (int i = 0; i < 4; ++i) if (m->m_commandButton == &g_r69Buttons[i]) return (unsigned)(i + 1);
		return 0;
	};
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 30 && o[1] != 13) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: m->setCommandButton(AsciiString(g_r69ButtonNames[o[1] % 5])); break;
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r69Saved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_r69Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 6: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putStr(m->m_commandButtonName.str(), m->m_commandButtonName.getLength());
		putU32(buttonCode());
		putU32(m->friend_getNextCallFrame());
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_teamRelHook = 0;
	g_modFindHook = 0;
	g_distDcHook = 0;
	me->m_behaviors = 0;
	me->m_sau = 0;
	me->m_ai = 0;
	delete m;
	delete power;
	delete pd;
	delete d;
	for (int k = 0; k < 6; ++k) {
		Object *x = g_r69Objs[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0;
		x->m_commandSetStringOverride = AsciiString::TheEmptyString;
		delete x;
		g_r69Objs[k] = 0;
	}
	TheAI = 0;
	TheActionManager = 0;
	TheControlBar = 0;
	ThePartitionManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r69_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r69RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
