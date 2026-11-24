#!/usr/bin/env python3
"""Calls and tail jumps of the hand ports against the listing.

    python tools/lift/audit_calls.py [LISTING]

For each `c.call(TARGET, RET)`, the instruction before RET must be a call
to TARGET; for each `c.run(A, K)` followed by `c.jump(TARGET)`, the K-th
instruction from A must be a jmp to TARGET. Where two labels share a name
(`ui_head_animate_down` at 1803h and 181Eh), the label whose CODE XREF
names the instruction decides. Prints each mismatch.
"""
import re, glob, collections, os, sys

REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
LST = sys.argv[1] if len(sys.argv) > 1 else os.environ.get('LST', os.path.join(REPO, '..', 'Cryogenic', 'doc', 'DNCDPRG.lst'))
HAND = os.path.join(REPO, 'crates', 'dune-rec', 'src', 'hand', '*.rs')
L = open(LST, encoding='latin1').read().splitlines()
lab = re.compile(r'^seg000:([0-9a-f]{4})  (\w+):\s*(?:; CODE XREF: (.*))?')
ins = re.compile(r'^seg000:([0-9a-f]{4})\s{10,}(\w+)\s*(.*?)\s*(?:;.*)?$')
names = collections.defaultdict(list); xref = {}
insns = []
for l in L:
    m = lab.match(l)
    if m:
        a = int(m.group(1), 16); names[m.group(2)].append(a)
        xref[a] = set(int(x, 16) for x in re.findall(r'seg000:([0-9a-f]{4})', m.group(3) or ''))
        continue
    m = ins.match(l)
    if m: insns.append((int(m.group(1), 16), m.group(2), m.group(3)))
by_next = {}
for i in range(len(insns) - 1):
    by_next[insns[i + 1][0]] = insns[i]
const = {}
for f in glob.glob(HAND):
    mod = os.path.basename(f)[:-3]
    for m in re.finditer(r'pub const (\w+): u16 = 0x([0-9a-f]+);', open(f, encoding='utf8').read()):
        const[(mod, m.group(1))] = int(m.group(2), 16)
def val(s):
    s = s.strip()
    if s.startswith('0x'): return int(s, 16)
    if '::' in s: return const.get(tuple(s.split('::')[-2:]))
    return const.get(('entry', s))
bad = 0
for f in glob.glob(HAND):
    src = open(f, encoding='utf8').read()
    for m in re.finditer(r'c\.call\(\s*([\w:]+)\s*,\s*(0x[0-9a-f]+)\s*\)', src):
        t, r = val(m.group(1)), int(m.group(2), 16)
        if t is None or r not in by_next: continue
        a, op, arg = by_next[r]
        if op != 'call': continue
        name = arg.split()[-1] if arg else ''
        if re.fullmatch(r'[0-9a-f]+h', name):
            ok = int(name[:-1], 16) == t
        elif name in names:
            cands = names[name]
            if len(cands) > 1:
                cands = [x for x in cands if a in xref.get(x, ())] or cands
            ok = t in cands
        else:
            continue
        if not ok:
            bad += 1
            line = src[:m.start()].count('\n') + 1
            print(f'{os.path.relpath(f, REPO)}:{line}: call {t:04x} ret {r:04x}; listing {a:04x} call {arg} -> {[hex(x) for x in names.get(name, [])]}')
idx = {a: i for i, (a, _, _) in enumerate(insns)}
for f in glob.glob(HAND):
    src = open(f, encoding='utf8').read()
    for m in re.finditer(r'c\.run\(\s*(0x[0-9a-f]+)\s*,\s*(\d+)\s*\)\?;\s*(?://[^\n]*\n\s*)?(?:return\s+)?c\.jump\(\s*([\w:]+)\s*\)', src):
        a, k, t = int(m.group(1), 16), int(m.group(2)), val(m.group(3))
        if t is None or a not in idx: continue
        i = idx[a] + k - 1
        if i >= len(insns): continue
        ja, op, arg = insns[i]
        if op != 'jmp': continue
        name = arg.split()[-1]
        if re.fullmatch(r'[0-9a-f]+h', name): ok = int(name[:-1], 16) == t
        elif name in names:
            cands = names[name]
            if len(cands) > 1: cands = [x for x in cands if ja in xref.get(x, ())] or cands
            ok = t in cands
        else: continue
        if not ok:
            bad += 1
            line = src[:m.start()].count('\n') + 1
            print(f'{os.path.relpath(f, REPO)}:{line}: jump {t:04x}; listing {ja:04x} jmp {arg} -> {[hex(x) for x in names.get(name, [])]}')
print(bad, 'mismatches')
