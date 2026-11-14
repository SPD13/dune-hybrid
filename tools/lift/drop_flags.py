#!/usr/bin/env python3
"""Drop the dead flag bookkeeping from hand ports.

    python tools/lift/drop_flags.py crates/dune-rec/src/hand/MODULE.rs...

For each port (`pub fn NAME(m: &mut Machine, start: u16)` with its
`entered(start, 0xADDR)`) whose callers read no flags (mask 0 in
`hand/flags_read.rs`), the flag-only statements (`c.cmp(..);`,
`c.test(..);`, and any flag helper whose result is unused) are removed and
the others become their flag-free forms (`pure::add(..)` for
`c.add(..)`). A port is left as it is when anything in it could still see
those flags:

- it reads a flag itself (`c.flag`, the flags word, `adc` / `sbb`,
  `rcl` / `rcr`);
- it calls or jumps to a routine that reads flags on entry, or to one the
  analysis does not know (`flags_used.py --in`), or through a register;
- it hands its context to a helper (which might read them);
- it has an INT or far call run (`c.interp`): an INT pushes the flags and
  its service sees them;
- a flag helper spans lines (the script works line by line).

The shadow check then compares every converted port with the original.
Prints, per file, the ports converted and those left with the reason.
"""
import os
import re
import subprocess
import sys

REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
OPS = 'add|sub|and|or|xor|inc|dec|neg|shl|shl1|shr1|rol1|ror1'
EMPTY_IF = re.compile(r'\n[ \t]*if [^\n{]*\{\n[ \t]*\}(?=\n(?![ \t]*else))')

masks = {}
for a, m in re.findall(r'\(0x([0-9a-f]+), 0x([0-9a-f]+)\)', open(os.path.join(REPO, 'crates/dune-rec/src/hand/flags_read.rs')).read()):
    masks[int(a, 16)] = int(m, 16)

out = subprocess.run([sys.executable, os.path.join(REPO, 'tools/lift/flags_used.py'), '--in'],
                     capture_output=True, text=True).stdout
entry_reads = {}
for line in out.splitlines():
    p = line.split()
    if len(p) >= 2:
        entry_reads[int(p[0], 16)] = p[1] != '-'


def reads_on_entry(addr):
    return entry_reads.get(addr, True)


def convert(path):
    src = open(path, encoding='utf-8').read()
    consts = {n: int(v, 16) for n, v in re.findall(r'pub const (\w+): u16 = 0x([0-9a-fA-F]+);', src)}
    lines = src.split('\n')
    done, left = [], []
    i = 0
    while i < len(lines):
        m = re.match(r'pub fn (\w+)\(m: &mut Machine, start: u16\)', lines[i])
        if not m:
            i += 1
            continue
        name = m.group(1)
        j = i + 1
        while j < len(lines) and lines[j] != '}':
            j += 1
        body = lines[i:j + 1]
        text = '\n'.join(body)
        e = re.search(r'entered\(start, 0x([0-9a-f]+)\)', text)
        why = None
        if not e:
            why = 'no entry'
        elif masks.get(int(e.group(1), 16), 0x8d5) != 0:
            why = 'callers read flags'
        elif re.search(r'c\.flag\(|flags\(\)|set_flags|c\.adc\(|c\.sbb\(|c\.rc[lr]', text):
            why = 'reads flags'
        elif 'c.interp(' in text:
            # An INT pushes the flags and its service sees them; a far
            # call's code isn't followed by the analysis.
            why = 'an INT or far call sees the flags'
        elif not re.search(rf'c\.({OPS}|cmp|test)\(', text):
            why = None  # nothing to do
            i = j + 1
            continue
        else:
            for kind, target in re.findall(r'c\.(call|jump)\(([^,)]+)', text):
                t = target.strip()
                if re.fullmatch(r'0x[0-9a-fA-F]+', t):
                    addr = int(t, 16)
                elif t.startswith('code::') and t[6:] in consts:
                    addr = consts[t[6:]]
                else:
                    why = f'{kind} through {t}'
                    break
                if reads_on_entry(addr):
                    why = f'{kind} {addr:04x} reads flags'
                    break
            if not why:
                for addr in re.findall(r'\(c\.m, 0x([0-9a-f]+)\)', text):
                    if reads_on_entry(int(addr, 16)):
                        why = f'tail {addr} reads flags'
                        break
            if not why and re.search(r'\b\w+\(&mut c\b|\b\w+\(c[,)]', text.replace('Ctx::new(m)', '')):
                why = 'hands c to a helper'
        if why:
            left.append((name, why))
            i = j + 1
            continue
        new = []
        bad = False
        for line in body:
            if re.match(rf'^\s*c\.({OPS}|cmp|test)\(.*\);\s*(//.*)?$', line):
                continue
            if re.search(rf'c\.(cmp|test)\(', line):
                bad = True
            new.append(re.sub(rf'\bc\.({OPS})\(', r'pure::\1(', line))
        if bad:
            left.append((name, 'a flag helper spans lines'))
            i = j + 1
            continue
        lines[i:j + 1] = new
        done.append(name)
        i = i + len(new)
    if done:
        text = '\n'.join(lines)
        # An `if` whose body held only flag helpers is left empty: gone
        # (its condition only reads).
        text = EMPTY_IF.sub('', text)
        if not re.search(r'^use super::pure;', text, re.M):
            text = re.sub(r'^(use super::)', r'use super::pure;\n\1', text, count=1, flags=re.M)
        open(path, 'w', encoding='utf-8', newline='\n').write(text)
    return done, left


for p in sys.argv[1:]:
    done, left = convert(p)
    print(f'{os.path.basename(p)}: {len(done)} converted, {len(left)} left')
    for n, why in left:
        print(f'  {n}: {why}')
