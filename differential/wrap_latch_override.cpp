/* Differential-test wrapper for GameEngine/Include/Common/LatchRestore.h,
** Overridable.h and Override.h (upstream commit 0a05454d). The headers are
** included unchanged; the only shim is differential/shim/Common/GameMemory.h,
** which supplies MemoryPoolObject (deleteInstance is `delete this`, as the
** real one) and the pool glue macros as plain virtual destructors.
**
** ref_latch_restore: a script of latch operations on one variable, of type
** int (pair == 0) or a two-int struct (pair == 1). Each op is two ints:
**   0 v   open a LatchRestore<T> on the variable with value v (pushed)
**   1 v   assign v to the variable (while latched or not)
**   2 _   log the variable (int: 1 value; pair: a then b)
**   3 _   close the innermost latch (delete, which restores)
** Open latches are closed at the end, innermost first, and the variable is
** logged once more. Returns the log length, or -1 if it exceeds cap.
**
** ref_overridable_chains: a forest of override chains of Tpl (a derived
** class with an id and a value). spec holds 3 ints per node: value,
** next (index of the next override, -1 for none; always greater than the
** node's own index), flags (bit 0: markAsOverride; bit 1: cut the link to
** the next override with setNextOverride(NULL), which makes the displaced
** node a root of its own). Chains are walked from each root in index order.
** For every node it logs id, value, getNextOverride id, getFinalOverride id,
** OVERRIDE<Tpl>'s operator->, operator*, the conversion to const Tpl* and
** getNonOverloadedPointer (ids, -1 for NULL). Then deleteOverrides() is
** called on the root and the id of its result is logged (-1 for NULL).
** After every root is processed, one liveness flag per node is logged
** (1 if the node was not deleted), and then the surviving results are
** deleted. Returns the log length, or -1 if it exceeds cap. */
#include <stddef.h>
#include <string.h>
#include <vector>

#include "Lib/BaseType.h"
#include "Common/GameMemory.h"
#include "Common/LatchRestore.h"
#include "Common/Overridable.h"
#include "Common/Override.h"

namespace {

struct Pair {
	int a;
	int b;
};

template <class T> T mk(int v);
template <> int mk<int>(int v) { return v; }
template <> Pair mk<Pair>(int v) { Pair p = {v, -v}; return p; }

template <class T> void emit(const T &v, std::vector<int> &o);
template <> void emit<int>(const int &v, std::vector<int> &o) { o.push_back(v); }
template <> void emit<Pair>(const Pair &v, std::vector<int> &o) { o.push_back(v.a); o.push_back(v.b); }

template <class T> int latch_run(const int *ops, int n, int *out, int cap)
{
	T var = mk<T>(0);
	std::vector<LatchRestore<T> *> stack;
	std::vector<int> log;
	for (int i = 0; i < n; ++i) {
		int op = ops[2 * i], v = ops[2 * i + 1];
		switch (op) {
		case 0: stack.push_back(new LatchRestore<T>(var, mk<T>(v))); break;
		case 1: var = mk<T>(v); break;
		case 2: emit(var, log); break;
		case 3: delete stack.back(); stack.pop_back(); break;
		}
	}
	while (!stack.empty()) {
		delete stack.back();
		stack.pop_back();
	}
	emit(var, log);
	if ((int)log.size() > cap) return -1;
	for (size_t i = 0; i < log.size(); ++i) out[i] = log[i];
	return (int)log.size();
}

int *g_alive = NULL;

struct Tpl : public Overridable {
	int id;
	int value;
	Tpl(int i, int v) : id(i), value(v) {}
	~Tpl() { if (g_alive) g_alive[id] = 0; }
};

int id_of(const Tpl *t) { return t ? t->id : -1; }

} // namespace

extern "C" int ref_latch_restore(int pair, const int *ops, int n, int *out, int cap)
{
	return pair ? latch_run<Pair>(ops, n, out, cap) : latch_run<int>(ops, n, out, cap);
}

extern "C" int ref_overridable_chains(const int *spec, int n, int *out, int cap)
{
	std::vector<Tpl *> nodes(n);
	std::vector<int> alive(n, 1);
	std::vector<char> has_pred(n, 0);
	g_alive = alive.data();
	for (int i = 0; i < n; ++i) nodes[i] = new Tpl(i, spec[3 * i]);
	for (int i = 0; i < n; ++i) {
		int next = spec[3 * i + 1];
		int flags = spec[3 * i + 2];
		if (next >= 0) {
			nodes[i]->setNextOverride(nodes[next]);
			if (flags & 2) nodes[i]->setNextOverride(NULL); /* cut: the displaced node is a root */
			else has_pred[next] = 1;
		}
		if (flags & 1) nodes[i]->markAsOverride();
	}

	std::vector<int> log;
	std::vector<Overridable *> kept;
	for (int r = 0; r < n; ++r) {
		if (has_pred[r]) continue;
		Tpl *root = nodes[r];
		for (const Overridable *cur = root; cur; cur = cur->getNextOverride()) {
			const Tpl *t = static_cast<const Tpl *>(cur);
			OVERRIDE<Tpl> ov(t);
			const Tpl *final_t = static_cast<const Tpl *>(cur->getFinalOverride());
			log.push_back(t->id);
			log.push_back(t->value);
			log.push_back(id_of(static_cast<const Tpl *>(cur->getNextOverride())));
			log.push_back(final_t->id);
			log.push_back(id_of(ov.operator->()));
			log.push_back(id_of(*ov));
			log.push_back(id_of(static_cast<const Tpl *>(ov)));
			log.push_back(id_of(static_cast<const Tpl *>(ov.getNonOverloadedPointer())));
		}
		Overridable *res = root->deleteOverrides();
		log.push_back(res ? static_cast<Tpl *>(res)->id : -1);
		kept.push_back(res);
	}
	for (int i = 0; i < n; ++i) log.push_back(alive[i]);

	for (size_t i = 0; i < kept.size(); ++i) delete kept[i];
	g_alive = NULL;
	if ((int)log.size() > cap) return -1;
	for (size_t i = 0; i < log.size(); ++i) out[i] = log[i];
	return (int)log.size();
}
