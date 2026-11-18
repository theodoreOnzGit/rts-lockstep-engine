/* Differential-test wrapper for the network user draft (GitHub #215, group netlogic; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/User.cpp and Include/GameNetwork/User.h   (#included whole, unchanged)
** on differential/shim_ladder (networkdefs.h: the base types and the memory pool object; UnicodeString: differential/shim's with
** `set(const UnicodeString &)` added) and the real AsciiString build. `User` is renamed LdUser.
**
** Entry point (extern "C", tests/differential_draft_user.rs): us_run(in, nin, out, cap); the input encoding is at the function. */
#define User LdUser
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include "Common/GameMemory.h"
#include "Common/UnicodeString.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/User.cpp"

namespace {
typedef std::vector<unsigned> Words;
struct Rd { const unsigned *w; int n; int i; unsigned next() { return i < n ? w[i++] : 0; } };
UnicodeString readU(Rd &r)
{
	unsigned n = r.next();
	std::basic_string<WideChar> s;
	for (unsigned i = 0; i < n; ++i) { unsigned c = r.next() & 0xffff; s += (WideChar)(c ? c : 1); }
	UnicodeString u; u.set(s.c_str()); return u;
}
void emitU(Words &o, UnicodeString t) { o.push_back((unsigned)t.getLength()); for (int i = 0; i < t.getLength(); ++i) o.push_back((unsigned)t.str()[i]); }
void dump(Words &o, LdUser *u) { emitU(o, u->GetName()); o.push_back(u->GetPort()); o.push_back(u->GetIPAddr()); }
}

/* in: [nusers, per user: name (count, one unit per word), addr, port (32 bits: the constructor truncates it), nops, per op: code, a, b]; ops:
**   0 a b  *users[a] = users[b]   1 a b  users[a] == users[b]   2 a b  users[a] != users[b]   3 a b  users[a]->setName(users[b]->GetName())   4 a port  SetPort
**   5 a addr  SetIPAddr   6 a  the user default-constructed, then SetPort(a) and SetIPAddr(a * 3), dumped
** out: after each op, the users in short; and the result of a comparison. */
extern "C" int us_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	Rd r = {in, nin, 0};
	std::vector<LdUser *> us;
	unsigned n = r.next();
	for (unsigned i = 0; i < n; ++i) { UnicodeString nm = readU(r); unsigned addr = r.next(), port = r.next(); us.push_back(new LdUser(nm, addr, port)); }
	Words o;
	for (unsigned i = 0; i < n; ++i) dump(o, us[i]);
	unsigned nops = r.next();
	for (unsigned k = 0; k < nops; ++k) {
		unsigned code = r.next(), a = r.next() % n, b = r.next();
		switch (code) {
		case 0: *us[a] = us[b % n]; break;
		case 1: o.push_back(*us[a] == us[b % n]); break;
		case 2: o.push_back(*us[a] != us[b % n]); break;
		case 3: us[a]->setName(us[b % n]->GetName()); break;
		case 4: us[a]->SetPort((UnsignedShort)b); break;
		case 5: us[a]->SetIPAddr(b); break;
		case 6: { LdUser u; u.SetPort((UnsignedShort)a); u.SetIPAddr(a * 3); dump(o, &u); break; }
		default: break;
		}
		for (unsigned i = 0; i < n; ++i) dump(o, us[i]);
	}
	for (unsigned i = 0; i < n; ++i) delete us[i];
	int m = (int)o.size();
	if (m <= cap) memcpy(out, o.data(), m * sizeof(unsigned));
	return m;
}
