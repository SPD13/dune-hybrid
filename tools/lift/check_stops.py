#!/usr/bin/env python3
"""Where a port can stop with flags the original reads.

    python tools/lift/check_stops.py [crates/dune-rec/src/hand/MODULE.rs ...]

A port stops (bails) at the start of any `c.run(ADDR, ..)` when an
interrupt is due there, and the original goes on from ADDR with the
port's flags. Lists every such ADDR that is not the return address of a
call or far call just made (where the flags are the callee's), and from
which the original reads flags before setting them (`flags_used.py
--at`): the port's flags there must be exact (keep the flag helper of the
original's last flag-setting instruction before it). A routine's own
entry (flags as its caller left them) shows up too and is fine.
"""
import glob
import os
import re
import subprocess
import sys

REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
files = sys.argv[1:] or sorted(glob.glob(os.path.join(REPO, 'crates/dune-rec/src/hand/*.rs')))
FN = re.compile(r'^(?:pub(?:\(super\))? )?fn (\w+)\((?:m: &mut Machine, start: u16|c: &mut Ctx)[^\n]*\n(.*?)^\}', re.M | re.S)
items = []
for f in files:
    for m in FN.finditer(open(f, encoding='utf-8').read()):
        body = m.group(2)
        rets = {int(a, 16) for a in re.findall(r'c\.(?:call|interp)\([^,]+, 0x([0-9a-f]+)\)', body)}
        entry = re.search(r'entered\(start, 0x([0-9a-f]+)\)', body)
        if entry:
            rets.add(int(entry.group(1), 16))
        for a in re.findall(r'c\.run\(0x([0-9a-f]+),', body):
            if int(a, 16) not in rets:
                items.append((int(a, 16), os.path.basename(f), m.group(1)))
addrs = sorted({a for a, _, _ in items})
out = subprocess.run([sys.executable, os.path.join(REPO, 'tools/lift/flags_used.py'), '--at']
                     + [f'{a:04x}' for a in addrs], capture_output=True, text=True).stdout
reads = dict(line.split() for line in out.splitlines())
for a, f, fn in sorted(set(items)):
    fl = reads.get(f'{a:04x}', '?')
    if fl not in ('-', '?'):
        print(f'{a:04x} {fl:6} {f} {fn}')
