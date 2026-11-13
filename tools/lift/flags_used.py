#!/usr/bin/env python3
"""Which seg000 routines return flags their callers read.

    python tools/lift/flags_used.py [--all] > ../dune-lift/flags_used.txt

For every direct `call X` in the generated code's instruction comments
(crates/dune-rec/src/generated.rs, made from your own DNCDPRG.EXE), the
code after the call is followed (through unconditional jumps, both ways
of conditional ones, up to 12 instructions) until each flag is either
read (a conditional jump, adc/sbb, rcl/rcr, pushf, lahf, cmc, loop[n]z,
into) or overwritten by an instruction that sets it. A routine whose
returned flags no caller reads needs no flag-exact result in a readable
port: only its registers and memory matter. Prints, per routine: the
flags read after some call (C Z S O P A D), and the number of call
sites; `--all` lists routines with none too.

Indirect calls (through registers and tables) are not followed: a
routine reached only that way is listed as `indirect`.
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


READ = {}  # routine -> flags its callers read (grown to a fixed point)


def reads_after(ret):
    """Flags read after a call returning to `ret` (before being set)."""
    out = set()
    stack = [(ret, frozenset(ALL), 0)]
    seen = set()
    while stack:
        a, live, n = stack.pop()
        if not live or n > 12 or (a, live) in seen or a not in insn:
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
            # Flags still live at a return go to this routine's caller:
            # read if that routine's callers read them.
            out |= live & READ.get(owner(a), set())
            continue
        if t.startswith('call '):
            # Flags into a callee: not followed.
            continue
        if any(t.startswith(s) for s in STOP):
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
    return out


calls = {}
for a, t in insn.items():
    if t.startswith('call ') and 'far' not in t and 'ptr' not in t:
        tg = target(t)
        if tg is not None and a in nxt:
            calls.setdefault(tg, []).append(nxt[a])

# Indirectly called routines (no direct call site): assume all flags read.
for e in entries:
    READ[e] = set() if e in calls else set(ALL)
changed = True
while changed:
    changed = False
    for e in entries:
        if e not in calls:
            continue
        fl = set()
        for r in calls[e]:
            fl |= reads_after(r)
        if fl != READ[e]:
            READ[e] = fl
            changed = True

names = dict((int(e, 16), n) for e, n in re.findall(r'\(0x([0-9a-f]+), "([^"]+)"\)', src))
show_all = '--all' in sys.argv
used = 0
for e in entries:
    sites = calls.get(e, [])
    fl = READ[e]
    flags = ''.join(f for f in 'CZSOPA' if f in fl)
    if flags:
        used += 1
    if flags or show_all:
        print(f'{e:04x} {flags or "-":6} {len(sites) if sites else "indirect":>8}  {names.get(e, "")}')
print(f'# {used} of {len(entries)} routines return flags a caller reads', file=sys.stderr)
