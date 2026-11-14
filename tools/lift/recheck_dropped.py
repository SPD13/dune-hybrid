#!/usr/bin/env python3
"""After a change to the flag analysis: put back the ports whose dropped
flags could now be seen.

    python tools/lift/recheck_dropped.py BASE [FILE=BASE2 ...]

BASE is a commit with the ports as they were before `drop_flags.py` ran
(per file another one may be given, e.g. `troops.rs=0b4394e` where hand
rewrites came later). Every port that differs from its BASE form is
checked again with `drop_flags.py`'s rules on that form, with the current
masks (`hand/flags_read.rs`) and entry reads (`flags_used.py --in`); one
it would no longer convert gets its BASE form back, but only when it is
the tools' output (BASE with lines dropped and `pure::`) and not listed in
`hand_ports.txt` (pruned by hand); hand rewrites are only reported
(CHECK): their exits and calls need a look.

Masks only shrink safely: run this whenever `flags_used.py` changes.
"""
import glob
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.join(HERE, '..', '..')
sys.path.insert(0, HERE)
args = sys.argv[1:]
sys.argv = [sys.argv[0]]
import drop_flags as d  # noqa: E402  (after argv: it converts the files it is given)

if not args:
    sys.exit(__doc__)
base = args[0]
per_file = dict(a.split('=', 1) for a in args[1:])
hand = set(per_file)
PORT = re.compile(r'^pub fn (\w+)\(m: &mut Machine, start: u16\)[^\n]*\n.*?^\}', re.M | re.S)

def tool_output(base, body):
    """Whether `body` is what the tools make of `base`: some of its lines
    dropped, `c.op(` become `pure::op(`, unused bindings `_` (a line
    subsequence). Anything else is a hand rewrite."""
    want = base.split('\n')
    k = 0
    for ln in body.split('\n'):
        ln = re.sub(r'\bpure::(\w+)\(', r'c.\1(', ln)
        # A binding the tools renamed `_` matches any name.
        pat = re.compile(re.sub(r'(?<![\w\\])_(?!\w)', r'\\w+', re.escape(ln)) + '$')
        while k < len(want) and not (want[k] == ln or pat.match(want[k])):
            k += 1
        if k == len(want):
            return False
        k += 1
    return True


by_hand = set()
for line in open(os.path.join(HERE, 'hand_ports.txt'), encoding='utf-8'):
    p = line.split()
    if len(p) == 2 and not line.startswith('#'):
        by_hand.add((p[0], p[1]))

restored = 0
for path in sorted(glob.glob(os.path.join(REPO, 'crates/dune-rec/src/hand/*.rs'))):
    name = os.path.basename(path)
    rel = 'crates/dune-rec/src/hand/' + name
    old = subprocess.run(['git', 'show', f'{per_file.get(name, base)}:{rel}'], cwd=REPO,
                         capture_output=True, text=True, encoding='utf-8').stdout
    if not old:
        continue
    consts = {k: int(v, 16) for k, v in re.findall(r'pub const (\w+): u16 = 0x([0-9a-fA-F]+);', old)}
    before = {m.group(1): m.group(0) for m in PORT.finditer(old)}
    cur = open(path, encoding='utf-8').read()
    changed = False
    for m in list(PORT.finditer(cur)):
        fn, body = m.group(1), m.group(0)
        if fn not in before or before[fn] == body:
            continue
        starts = d.port_entries(before[fn])
        mask = max((d.masks.get(a, 0x8d5) for a in starts), default=0x8d5)
        if mask and not d.only_tails(before[fn]):
            why = 'callers read flags'
        else:
            why = d.blocker(before[fn], consts, set())
        if not why:
            continue
        if (name, fn) in by_hand or not tool_output(before[fn], body):
            # A hand rewrite: its exits and calls need a look, not the
            # old form back.
            print(f'CHECK {name} {fn}: {why}')
            continue
        print(f'restore {name} {fn}: {why}')
        cur = cur.replace(body, before[fn], 1)
        changed = True
        restored += 1
    if changed:
        if 'pure::' not in cur:
            cur = re.sub(r'^use super::pure;\n', '', cur, flags=re.M)
        open(path, 'w', encoding='utf-8', newline='\n').write(cur)
print(f'{restored} restored')
