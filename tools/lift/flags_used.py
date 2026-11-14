#!/usr/bin/env python3
"""Which seg000 routines return flags their callers read.

    python tools/lift/flags_used.py [--all] > ../dune-lift/flags_used.txt

For every direct `call X` in the generated code's instruction comments
(crates/dune-rec/src/generated.rs, made from your own DNCDPRG.EXE), the
code after the call is followed (through unconditional jumps, both ways
of conditional ones, up to 40 instructions) until each flag is either
read (a conditional jump, adc/sbb, rcl/rcr, pushf, lahf, cmc, loop[n]z,
into) or overwritten by an instruction that sets it. A routine whose
returned flags no caller reads needs no flag-exact result in a readable
port: only its registers and memory matter. Prints, per routine: the
flags read after some call (C Z S O P A D), and the number of call
sites; `--all` lists routines with none too.

Indirect calls and jumps (through registers and tables) take their
targets from the coverage files (`j SITE TARGET` lines, $LIFT_DATA/cov):
an indirect call adds its return site, as a direct one does; a routine
reached by an indirect jump returns to the jumping routine's callers, so
it gets their flags. A routine no call or jump of any run reaches is
listed as `unknown` and assumed to have all its flags read.
"""
import os
import re
import sys

repo = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
src = open(os.path.join(repo, 'crates/dune-rec/src/generated.rs'), encoding='utf-8').read()

insn = {}
for a, text in re.findall(r'// ([0-9a-f]{4}): (.*)', src):
    insn.setdefault(int(a, 16), text.strip())
addrs = sorted(insn)
nxt = {a: addrs[i + 1] for i, a in enumerate(addrs[:-1])}
table = re.search(r'pub static ROUTINES[^=]*=\s*&\[(.*?)\];', src, re.S).group(1)
entries = sorted({int(a, 16) for a in re.findall(r'\(0x([0-9a-f]+),', table)})

ALL = set('CZSOPA')
# Flags each mnemonic reads / writes (8086; undefined results count as written).
JCC = {
    'jo': 'O', 'jno': 'O', 'jb': 'C', 'jae': 'C', 'jnb': 'C', 'jc': 'C', 'jnc': 'C',
    'je': 'Z', 'jz': 'Z', 'jne': 'Z', 'jnz': 'Z', 'jbe': 'CZ', 'ja': 'CZ', 'jna': 'CZ', 'jnbe': 'CZ',
    'js': 'S', 'jns': 'S', 'jp': 'P', 'jnp': 'P', 'jpe': 'P', 'jpo': 'P',
    'jl': 'SO', 'jge': 'SO', 'jnge': 'SO', 'jnl': 'SO', 'jle': 'ZSO', 'jg': 'ZSO', 'jng': 'ZSO', 'jnle': 'ZSO',
}
READS = {'adc': 'C', 'sbb': 'C', 'rcl': 'C', 'rcr': 'C', 'cmc': 'C', 'pushf': 'CZSOPA', 'lahf': 'CZSPA',
         'loopz': 'Z', 'loope': 'Z', 'loopnz': 'Z', 'loopne': 'Z', 'into': 'O', 'daa': 'CA', 'das': 'CA',
         'aaa': 'A', 'aas': 'A', 'salc': 'C'}
WRITES = {
    'add': ALL, 'adc': ALL, 'sub': ALL, 'sbb': ALL, 'cmp': ALL, 'and': ALL, 'or': ALL, 'xor': ALL,
    'test': ALL, 'neg': ALL, 'inc': set('ZSOPA'), 'dec': set('ZSOPA'), 'shl': ALL, 'sal': ALL,
    'shr': ALL, 'sar': ALL, 'mul': ALL, 'imul': ALL, 'div': ALL, 'idiv': ALL, 'clc': {'C'}, 'stc': {'C'},
    'cmc': {'C'}, 'popf': ALL, 'sahf': set('CZSPA'), 'rol': {'C', 'O'}, 'ror': {'C', 'O'},
    'rcl': {'C', 'O'}, 'rcr': {'C', 'O'}, 'scasb': ALL, 'scasw': ALL, 'cmpsb': ALL, 'cmpsw': ALL,
    'daa': ALL, 'das': ALL, 'aaa': ALL, 'aas': ALL, 'aam': ALL, 'aad': ALL,
}
STOP = ('ret', 'retf', 'iret', 'int ', 'hlt', 'call ', 'jmp far', 'jmp word', 'jmp dword', 'jmp bx', 'jmp ax', 'jmp si', 'jmp di', 'jmp bp', 'jmp cx', 'jmp dx')


def target(t):
    m = re.search(r'0x([0-9A-Fa-f]+)\s*$', t)
    return int(m.group(1), 16) if m else None


import bisect


def owner(a):
    """The routine an address belongs to (the nearest entry at or before it)."""
    i = bisect.bisect_right(entries, a) - 1
    return entries[i] if i >= 0 else None


# Targets of indirect calls and jumps (site -> targets) seen in the
# coverage runs ($LIFT_DATA/cov, `j SITE TARGET` lines), seg000 only.
lift_data = os.environ.get('LIFT_DATA', os.path.join(repo, '..', 'dune-lift'))
cov_dir = os.path.join(lift_data, 'cov')
IND = {}
for f in sorted(os.listdir(cov_dir)) if os.path.isdir(cov_dir) else []:
    for line in open(os.path.join(cov_dir, f), encoding='utf-8'):
        p = line.split()
        if len(p) >= 3 and p[0] == 'j' and p[1].startswith('1000:') and p[2].startswith('1000:'):
            IND.setdefault(int(p[1][5:], 16), set()).add(int(p[2][5:], 16))


def far(t):
    """A far call or an INT: into BIOS, DOS or a driver, which read the
    registers, not the flags, and give them back (IRET) or set their own."""
    return t.startswith('int ') or (t.startswith('call ') and ('far' in t or 'dword' in t))


LIMIT = 200  # instructions followed from a return or a routine's entry
READ = {}  # routine -> flags its callers read (grown to a fixed point)
IN = {}    # routine -> flags it reads as they were when it was called
PASS = {}  # routine -> flags that can reach one of its RETs unwritten


def direct_call(t):
    """The target of a direct near call, else None."""
    if t.startswith('call ') and 'far' not in t and 'ptr' not in t:
        return target(t)
    return None


def walk(start, after_return):
    """Flags read before being set, from `start`.

    `after_return`: `start` follows a call; flags live at a RET go to this
    routine's callers (read if they read them). Otherwise `start` is a
    routine's entry and the walk gives what it reads of its caller's flags;
    flags live at its RET pass back to the caller, which follows them on.
    A call reads what the callee reads on entry; on after it go only the
    flags that can reach one of the callee's RETs unwritten. Returns, for
    an entry, also those flags (what reaches a RET unwritten). What can't be followed
    (an INT, an indirect jump or call, past the limit) counts as read."""
    out = set()
    passed = set()
    stack = [(start, frozenset(ALL), 0)]
    seen = set()
    while stack:
        a, live, n = stack.pop()
        if not live or (a, live) in seen:
            continue
        if n > LIMIT or a not in insn:
            out |= live
            passed |= live
            continue
        seen.add((a, live))
        t = insn[a]
        op = t.split()[0]
        if op in ('rep', 'repe', 'repne', 'repz', 'repnz', 'lock') and len(t.split()) > 1:
            op = t.split()[1]
        if op in JCC:
            out |= live & set(JCC[op])
            tg = target(t)
            if tg is not None:
                stack.append((tg, live, n + 1))
            if a in nxt:
                stack.append((nxt[a], live, n + 1))
            continue
        if op in READS:
            out |= live & set(READS[op])
        if op in ('ret', 'retf', 'iret'):
            if after_return:
                out |= live & READ.get(owner(a), set())
            else:
                passed |= live
            continue
        if far(t):
            # Passed through: still live after it.
            if a in nxt:
                stack.append((nxt[a], live, n + 1))
            continue
        if t.startswith('call '):
            d = direct_call(t)
            ds = [d] if d is not None else sorted(IND.get(a, ()))
            if not ds or any(x not in IN for x in ds):
                out |= live
                passed |= live
                continue
            for x in ds:
                out |= live & IN[x]
            live = live & set.union(*(PASS[x] for x in ds))
            if a in nxt:
                stack.append((nxt[a], live, n + 1))
            continue
        if t.startswith('jmp') and target(t) is None and 'far' not in t and 'dword' not in t and a in IND:
            # An indirect jump: on at each target the runs took.
            for x in IND[a]:
                stack.append((x, live, n + 1))
            continue
        if any(t.startswith(s) for s in STOP):
            out |= live
            passed |= live
            continue
        live = live - WRITES.get(op, set())
        if op == 'jmp' or op.startswith('jmp'):
            tg = target(t)
            if tg is not None and 'far' not in t:
                stack.append((tg, live, n + 1))
            continue
        if op in ('loop', 'jcxz'):
            tg = target(t)
            if tg is not None:
                stack.append((tg, live, n + 1))
        if a in nxt:
            stack.append((nxt[a], live, n + 1))
    return out if after_return else (out, passed)


for e in entries:
    IN[e] = set()
    PASS[e] = set()
changed = True
while changed:
    changed = False
    for e in entries:
        fl, ps = walk(e, False)
        if fl != IN[e] or ps != PASS[e]:
            IN[e], PASS[e] = fl, ps
            changed = True


def reads_after(ret):
    """Flags read after a call returning to `ret` (before being set)."""
    return walk(ret, True)


calls = {}
for a, t in insn.items():
    if t.startswith('call ') and 'far' not in t and 'ptr' not in t:
        tg = target(t)
        if tg is not None and a in nxt:
            calls.setdefault(tg, []).append(nxt[a])

# Indirect calls and jumps seen in the coverage runs.
jumped = {}  # target -> routines that jump there indirectly
for site, tgs in IND.items():
    t = insn.get(site, '')
    for tg in tgs:
        if t.startswith('call ') and not far(t) and site in nxt:
            if nxt[site] not in calls.setdefault(tg, []):
                calls[tg].append(nxt[site])
        elif t.startswith('jmp') and 'far' not in t and 'dword' not in t and owner(site) is not None:
            jumped.setdefault(tg, set()).add(owner(site))

# Direct jumps into another routine's entry (tail calls), and code that
# runs on into the next routine: its RET returns to the jumper's callers.
entry_set = set(entries)
for a, t in insn.items():
    op = t.split()[0]
    if (op in JCC or op == 'jmp') and 'far' not in t and 'ptr' not in t:
        tg = target(t)
        if tg in entry_set and owner(a) not in (None, tg):
            jumped.setdefault(tg, set()).add(owner(a))
        elif tg is not None and owner(tg) not in (None, owner(a)) and owner(a) is not None:
            # Into the middle of another routine: its RETs then return to
            # the jumping routine's callers too.
            jumped.setdefault(owner(tg), set()).add(owner(a))
for e in entries:
    i = bisect.bisect_left(addrs, e)
    if i > 0:
        prev = addrs[i - 1]
        t = insn[prev]
        if nxt.get(prev) == e and not any(t.startswith(x) for x in ('ret', 'retf', 'iret', 'jmp', 'hlt')) and owner(prev) not in (None, e):
            jumped.setdefault(e, set()).add(owner(prev))

# Routines nothing reaches in any run: assume all flags read.
for e in entries:
    READ[e] = set() if e in calls or e in jumped else set(ALL)
changed = True
while changed:
    changed = False
    for e in entries:
        if e not in calls and e not in jumped:
            continue
        fl = set()
        for r in calls.get(e, []):
            fl |= reads_after(r)
        for j in jumped.get(e, ()):
            fl |= READ[j]
        if fl != READ[e]:
            READ[e] = fl
            changed = True

names = dict((int(e, 16), n) for e, n in re.findall(r'\(0x([0-9a-f]+), "([^"]+)"\)', src))
show_all = '--all' in sys.argv
if '--at' in sys.argv:
    # The flags read from each address given (hex) before being set,
    # following on into the routine's callers at its RET: where a port can
    # stop inside a routine (`c.run(at, ..)`), the original goes on from
    # there with the port's flags.
    for x in sys.argv[sys.argv.index('--at') + 1:]:
        a = int(x, 16)
        fl = ''.join(f for f in 'CZSOPA' if f in walk(a, True)) if a in insn else '?'
        print(f'{a:04x} {fl or "-"}')
    sys.exit(0)
if '--in' in sys.argv:
    # What each routine reads of the flags it is called or jumped to with:
    # a port must set these before `c.call`/`c.jump` to it.
    # Then the flags that can reach one of its RETs as it was entered
    # with them: a port that jumps to it (or returns right after calling
    # it) must have those exact too, where its own callers read them.
    for e in entries:
        fl = ''.join(f for f in 'CZSOPA' if f in IN.get(e, ALL))
        ps = ''.join(f for f in 'CZSOPA' if f in PASS.get(e, ALL))
        print(f'{e:04x} {fl or "-"} {ps or "-"}')
    sys.exit(0)
used = 0
for e in entries:
    sites = calls.get(e, [])
    fl = READ[e]
    flags = ''.join(f for f in 'CZSOPA' if f in fl)
    if flags:
        used += 1
    how = len(sites) if sites else 'jumped' if e in jumped else 'unknown'
    if flags or show_all:
        print(f'{e:04x} {flags or "-":6} {how:>8}  {names.get(e, "")}')
print(f'# {used} of {len(entries)} routines return flags a caller reads', file=sys.stderr)

if '--rust' in sys.argv:
    # The table the shadow check uses (crates/dune-rec/src/hand/flags_read.rs):
    # per routine whose callers read fewer than all six flags, the mask of
    # those they read. Addresses and flag masks only: no game data.
    bit = {'C': 0x001, 'P': 0x004, 'A': 0x010, 'Z': 0x040, 'S': 0x080, 'O': 0x800}
    out = ['//! Generated by `tools/lift/flags_used.py --rust`: for each seg000 routine',
           '//! whose callers read fewer than all of CF, PF, AF, ZF, SF and OF after',
           '//! it returns, the mask of the ones they read. The shadow check ignores',
           '//! the others in the result of a port, so readable ports need not keep them.',
           '',
           'pub static FLAGS_READ: &[(u16, u16)] = &[']
    for e in entries:
        fl = READ[e]
        if fl != ALL:
            m = sum(bit[f] for f in fl)
            out.append(f'    (0x{e:04x}, 0x{m:03x}),')
    out.append('];')
    path = os.path.join(repo, 'crates/dune-rec/src/hand/flags_read.rs')
    open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out) + '\n')
