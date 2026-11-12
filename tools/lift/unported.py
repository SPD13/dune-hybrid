#!/usr/bin/env python3
"""The seg000 routines without a readable port: entry, size (to the next
entry) and the listing's label ('?' for none: usually an entry inside an
instruction or a routine, which only the harness calls).

    python tools/lift/unported.py [LISTING] > ../dune-lift/unported.txt

Reads crates/dune-rec/src/generated.rs (made by the lift from your own
DNCDPRG.EXE) and crates/dune-rec/src/hand/mod.rs.
"""
import bisect
import os
import re
import sys

repo = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
lst = sys.argv[1] if len(sys.argv) > 1 else os.path.join(repo, '..', 'Cryogenic', 'doc', 'DNCDPRG.lst')
gen = open(os.path.join(repo, 'crates/dune-rec/src/generated.rs'), encoding='utf-8').read()
table = re.search(r'pub static ROUTINES[^=]*=\s*&\[(.*?)\];', gen, re.S).group(1)
entries = sorted(int(a, 16) for a in re.findall(r'\(0x([0-9a-f]+),', table))
hand_src = open(os.path.join(repo, 'crates/dune-rec/src/hand/mod.rs'), encoding='utf-8').read()
hand = {int(a, 16) for a in re.findall(r'^\s*\(0x([0-9a-f]+),\s*[a-z_]+::', hand_src, re.M)}
names = {}
for line in open(lst, encoding='latin-1'):
    m = re.match(r'seg000:([0-9a-f]{4})\s+(\w+)(?:\s+proc|:)', line)
    if m:
        names.setdefault(int(m.group(1), 16), m.group(2))
for a in entries:
    if a in hand:
        continue
    i = bisect.bisect_right(entries, a)
    end = entries[i] if i < len(entries) else 0xf4b0
    print(f'{a:04x} {end - a:5d} {names.get(a, "?")}')
