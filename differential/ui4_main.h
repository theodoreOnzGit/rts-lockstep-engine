/* The shared operations, dump and main of group ui4's reference programs (#215), included by each differential/wrap_draft_ui4_<menu>.cpp after its own
** stand-ins and cuts. The wrapper defines, before including this: `u4_menus[]` (the menus' system and input callbacks, by number), `u4_menu_setup()` (run
** once, before the case's first operation), `u4_menu_op(wm, cmd, in)` (its own `@ <cmd>` operations; false when `cmd` is none of them) and
** `u4_menu_dump(o)` (its statics, appended to the dump). Input (stdin): one operation per line; output (stdout): per operation `#<n>`, the messages and
** stand-in calls logged, wrap_wm.cpp's dump of the windows, the layouts', the static texts' and the wrapper's own dump (tests/ui4_common/mod.rs writes
** the same on the Rust side). Operations: wrap_wm.cpp's (W, M, K, H, E, F, T, t, I, e, ...) and, `@` first:
**   @ nm <win> <name>            the window's id is the name key of <name> (what the WND reader gives a window)
**   @ lay <n> <win>...           the windows the next winCreateLayout holds
**   @ tx <win> <text>            GadgetTextEntrySetText
**   @ wu <parent> <menu> x y w h status style   a user window with that menu's system and input callbacks (-1: no parent)
**   @ sys <menu> <win> <msg> <kind> <d1> <d2>   the menu's system callback called directly (kind 1: <d1> is a window index, else a value); logs `ret 0|1`
**   @ inp <menu> <win> <msg> <kind> <d1> <d2>   the menu's input callback */
static void u4_op(TestWindowManager *wm, std::istringstream &in)
{
	std::string cmd;
	in >> cmd;
	if (cmd == "nm")
	{
		int a;
		std::string n;
		in >> a >> n;
		u4_win(a)->winSetWindowId(TheNameKeyGenerator->nameToKey(n.c_str()));
	}
	else if (cmd == "tx")
	{
		int a;
		std::string t;
		in >> a >> t;
		GadgetTextEntrySetText(u4_win(a), u4_text(t));
	}
	else if (cmd == "lay")
	{
		int n = u4_li(in);
		u4_layout_members.clear();
		for (int i = 0; i < n; ++i) u4_layout_members.push_back(u4_li(in));
	}
	else if (cmd == "wu")
	{
		int parent, menu, x, y, w, h;
		unsigned status, style;
		in >> parent >> menu >> x >> y >> w >> h >> status >> style;
		WinInstanceData inst;
		inst.init();
		inst.m_style = style;
		GameWindow *p = u4_win(parent);
		inst.m_owner = p;
		GameWindow *u = wm->winCreate(p, status, x, y, w, h, u4_menus[menu].sys, &inst);
		u->winSetInputFunc(u4_menus[menu].inp);
	}
	else if (cmd == "sys" || cmd == "inp")
	{
		int menu, win, kind;
		unsigned msg;
		unsigned long d1, d2;
		in >> menu >> win >> msg >> kind >> d1 >> d2;
		WindowMsgData a = (WindowMsgData)d1;
		if (kind == 1) a = (WindowMsgData)(uintptr_t)u4_win((int)d1);
		WindowMsgHandledType r = cmd == "sys" ? u4_menus[menu].sys(u4_win(win), msg, a, (WindowMsgData)d2) : u4_menus[menu].inp(u4_win(win), msg, a, (WindowMsgData)d2);
		g_log << "ret " << (r == MSG_HANDLED ? 1 : 0) << "\n";
	}
	else
		u4_menu_op(wm, cmd, in);
}

/* the layouts and the static texts, then the wrapper's statics, in the format tests/ui4_common/mod.rs writes */
static void u4_dump(std::ostringstream &o)
{
	for (size_t i = 0; i < u4_layouts.size(); ++i)
	{
		if (!u4_layout_dead[i].empty())
		{
			o << "lay" << i << u4_layout_dead[i] << "\n";
			continue;
		}
		WindowLayout *l = u4_layouts[i];
		o << "lay" << i << " hidden " << (int)l->isHidden() << " wins";
		for (GameWindow *w = l->getFirstWindow(); w; w = w->winGetNextInLayout()) o << " " << widx(w);
		o << "\n";
	}
	for (size_t i = 0; i < g_windows.size(); ++i)
		if (!g_freed[i] && BitTest(g_windows[i]->winGetInstanceData()->getStyle(), GWS_STATIC_TEXT))
		{
			o << "stt" << i;
			dump_text(o, GadgetStaticTextGetText(g_windows[i]));
			o << "\n";
		}
	u4_menu_dump(o);
}

static void u4_run_case(void)
{
	TestWindowManager *wm = new TestWindowManager;
	TheWindowManager = wm;
	wm->init();
	TheDisplay->m_width = 800;
	TheDisplay->m_height = 600;
	u4_menu_setup();
	for (size_t i = 0; i < g_ops.size(); ++i)
	{
		std::istringstream in(g_ops[i]);
		char op = 0;
		in >> op;
		if (op == '@') u4_op(wm, in);
		else run_op(wm, in, op);
		g_dd_from = 1 << 30;
		std::ostringstream o;
		o << "#" << i << "\n" << g_log.str();
		g_log.str("");
		dump(o, wm);
		u4_dump(o);
		fputs(o.str().c_str(), stdout);
		fflush(stdout);
	}
}

static ucontext_t u4_main_ctx, u4_case_ctx;

int main(void)
{
	/* the heap in brk, below 4 GB in a non-PIE program (wrap_wm.cpp's main) */
	mallopt(M_MMAP_THRESHOLD, 1 << 30);
	mallopt(M_MMAP_MAX, 0);
	std::string line;
	while (std::getline(std::cin, line)) if (!line.empty()) g_ops.push_back(line);
	const size_t stack = 8 << 20;
	char *st = (char *)malloc(stack);
	if ((uintptr_t)st + stack > 0xFFFFFFFFu) { fprintf(stderr, "ui4_ref: the heap is above 4 GB\n"); return 3; }
	getcontext(&u4_case_ctx);
	u4_case_ctx.uc_stack.ss_sp = st;
	u4_case_ctx.uc_stack.ss_size = stack;
	u4_case_ctx.uc_link = &u4_main_ctx;
	makecontext(&u4_case_ctx, u4_run_case, 0);
	swapcontext(&u4_main_ctx, &u4_case_ctx);
	return 0;
}
