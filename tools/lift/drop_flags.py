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

- it reads a flag itself (`c.flag` but right after a call, where it is
  the callee's; the flags word, `adc` / `sbb`, `rcl` / `rcr`);
- it calls or jumps to a routine that reads flags on entry, or to one the
  analysis does not know (`flags_used.py --in`), or through a register;
- it hands its context to a helper that could (the same rules, followed
  through the helpers it hands it on to; the helpers stay as they are);
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


HELPERS = {}
for f in os.listdir(os.path.join(REPO, 'crates/dune-rec/src/hand')):
    if f.endswith('.rs'):
        t = open(os.path.join(REPO, 'crates/dune-rec/src/hand', f), encoding='utf-8').read()
        for m in re.finditer(r'^(?:pub(?:\(super\))? )?fn (\w+)\(c: &mut Ctx[^\n]*\n(.*?)^\}', t, re.M | re.S):
            HELPERS.setdefault(m.group(1), []).append(m.group(0))


def blocker(text, consts, seen):
    """Why the flags a port leaves could be seen in `text` (its body, or a
    helper's it runs on into), or None."""
    if re.search(r'flags\(\)|set_flags|c\.adc\(|c\.sbb\(|c\.rc[lr]', text):
        return 'reads flags'
    # A flag read right after a call sees the callee's flags, not the
    # port's own: only such reads are allowed.
    prev = ''
    for line in text.split('\n'):
        if line.strip() == '':
            continue
        if 'c.flag(' in line and not re.match(r'^\s*c\.call\(.*\)\?;\s*$', prev):
            return 'reads flags'
        prev = line
    if 'c.interp(' in text:
        # An INT pushes the flags and its service sees them; a far call's
        # code isn't followed by the analysis.
        return 'an INT or far call sees the flags'
    for kind, target in re.findall(r'c\.(call|jump)\(([^,)]+)', text):
        t = target.strip()
        if re.fullmatch(r'0x[0-9a-fA-F]+', t):
            addr = int(t, 16)
        elif t.startswith('code::') and t[6:] in consts:
            addr = consts[t[6:]]
        else:
            return f'{kind} through {t}'
        if reads_on_entry(addr):
            return f'{kind} {addr:04x} reads flags'
    for addr in re.findall(r'\(c\.m, 0x([0-9a-f]+)\)', text):
        if reads_on_entry(int(addr, 16)):
            return f'tail {addr} reads flags'
    for h in re.findall(r'(\w+)\((?:&mut c|c[,)])', text.replace('Ctx::new(m)', '')):
        if h in seen:
            continue
        seen.add(h)
        bodies = HELPERS.get(h)
        if not bodies:
            return f'hands c to {h}'
        for b in bodies:
            why = blocker(b, consts, seen)
            if why:
                return f'{h}: {why}'
    return None


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
        elif not re.search(rf'c\.({OPS}|cmp|test)\(', text):
            i = j + 1
            continue
        else:
            why = blocker(text, consts, set())
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
        if 'pure::' in text and not re.search(r'^use super::pure;', text, re.M):
            text = re.sub(r'^(use super::)', r'use super::pure;\n\1', text, count=1, flags=re.M)
        open(path, 'w', encoding='utf-8', newline='\n').write(text)
    return done, left


for p in sys.argv[1:]:
    done, left = convert(p)
    print(f'{os.path.basename(p)}: {len(done)} converted, {len(left)} left')
    for n, why in left:
        print(f'  {n}: {why}')
