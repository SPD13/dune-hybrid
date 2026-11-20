#!/usr/bin/env python3
"""Hoist the stretch both arms of an `if` start with (Stage A step 3).

    python tools/lift/hoist_runs.py [--dry] [FILE.rs ...]

    if COND {                         c.run(A, if COND { n } else { m })?;
        c.run(A, n)?;                 if COND {
        ...                   ->          ...
    } else {                          } else {
        c.run(A, m)?;                     ...
        ...                           }
    }

Both paths run the original's code from A, n or m instructions of it
depending on the branch the original takes further on: one stretch, its
count the path's. Nothing comes between the condition and the stretch in
either form, so what the port computes is unchanged; the `if` is the
same. An arm left empty goes (`if !(COND)` for an empty first arm), and
an `if` with both arms empty goes altogether.

A condition that reads the machine (`c.flag(CF)`) is read once, before
the stretch, into a `let` (`cf`, `zf`): with interrupts taken in place
the stretch can change the state the `if` would otherwise read after it.
Other conditions reading the machine are left alone, and so are `if let`,
`else if` chains and conditions spanning lines.

Run it until it changes nothing (a hoisted stretch can make the arm's
next stretch the first statement again), then `merge_runs.py` (the
hoisted stretch often follows another). Prints the hoists per file.
"""
import os
import re
import sys

HAND = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'crates', 'dune-rec', 'src', 'hand')

RUN = re.compile(r'^(\s*)c\.run\((.+?), (.+)\)\?;\s*(?://\s*(.*))?$')
IF = re.compile(r'^(\s*)if (.+) \{\s*$')
FLAG = re.compile(r'^(!?)c\.flag\(((?:pc::cpu::)?(CF|ZF))\)$')
FN = re.compile(r'^(pub(\(\w+\))? )?fn (\w+)')


def split_run_args(line):
    """(indent, address, count, comment) of a `c.run(A, n)?;` line, the
    address and count split at the top-level comma."""
    m = re.match(r'^(\s*)c\.run\((.*)\)\?;\s*(?://\s*(.*))?$', line)
    if not m:
        return None
    args, depth = m.group(2), 0
    for i, ch in enumerate(args):
        if ch in '([{':
            depth += 1
        elif ch in ')]}':
            depth -= 1
        elif ch == ',' and depth == 0:
            return m.group(1), args[:i].strip(), args[i + 1:].strip(), m.group(3)
    return None


def block_end(lines, i, ind):
    """The line closing the block opened at line i (indent `ind`)."""
    for j in range(i + 1, len(lines)):
        if lines[j].startswith(ind + '}'):
            return j
        if lines[j].strip() and not lines[j].startswith(ind + ' '):
            return None
    return None


def fn_text(lines, i):
    """The text of the top-level fn around line i."""
    s = i
    while s > 0 and not FN.match(lines[s]):
        s -= 1
    e = i
    while e < len(lines) and not lines[e].startswith('}'):
        e += 1
    return '\n'.join(lines[s:e + 1])


NEG = {'==': '!=', '!=': '==', '<': '>=', '>=': '<', '>': '<=', '<=': '>'}


def negate(cond):
    """`!COND`, written plainly for a name or a single comparison."""
    if re.fullmatch(r'!\w+', cond):
        return cond[1:]
    if re.fullmatch(r'\w+', cond):
        return '!' + cond
    if '&&' not in cond and '||' not in cond and '!' not in cond.replace('!=', ''):
        ops = re.findall(r'\s(==|!=|<=|>=|<|>)\s', cond)
        if len(ops) == 1 and '<<' not in cond and '>>' not in cond:
            return re.sub(r'\s(==|!=|<=|>=|<|>)\s', lambda m: f' {NEG[m.group(1)]} ', cond, count=1)
    return f'!({cond})'


def count(cond, n, m):
    if n == m:
        return n
    return f'if {cond} {{ {n} }} else {{ {m} }}'


def hoist_once(lines):
    """One hoist (the first found); the new lines and a description, or None."""
    for i, line in enumerate(lines):
        mi = IF.match(line)
        if not mi or line.lstrip().startswith('}'):
            continue
        ind, cond = mi.group(1), mi.group(2)
        if cond.startswith('let ') or cond.count('{') != cond.count('}'):
            continue
        a = split_run_args(lines[i + 1])
        if not a or a[0] != ind + '    ':
            continue
        j = block_end(lines, i, ind)
        if j is None or lines[j].rstrip() != ind + '} else {':
            continue
        b = split_run_args(lines[j + 1])
        if not b or b[0] != ind + '    ' or b[1] != a[1]:
            continue
        k = block_end(lines, j, ind)
        if k is None or lines[k].rstrip() != ind + '}':
            continue
        pre = []
        if re.search(r'\bc\b', cond):
            f = FLAG.match(cond)
            if not f:
                continue
            name = f.group(3).lower()
            if re.search(rf'\b{name}\b', fn_text(lines, i)):
                continue
            pre = [f'{ind}let {name} = c.flag({f.group(2)});']
            cond = f.group(1) + name
        note = a[3] or b[3]
        run = f'{ind}c.run({a[1]}, {count(cond, a[2], b[2])})?;' + (f' // {note}' if note else '')
        then, other = lines[i + 2:j], lines[j + 2:k]
        if then and other:
            body = [f'{ind}if {cond} {{'] + then + [f'{ind}}} else {{'] + other + [f'{ind}}}']
        elif then:
            body = [f'{ind}if {cond} {{'] + then + [f'{ind}}}']
        elif other:
            neg = negate(cond)
            body = [f'{ind}if {neg} {{'] + other + [f'{ind}}}']
        else:
            body = []
        return lines[:i] + pre + [run] + body + lines[k + 1:], f'line {i + 1}: {a[1]}'
    return None


def main():
    args = sys.argv[1:]
    dry = '--dry' in args
    files = [a for a in args if a != '--dry'] or sorted(
        os.path.join(HAND, f) for f in os.listdir(HAND) if f.endswith('.rs'))
    total = 0
    for path in files:
        raw = open(path, 'rb').read().decode('utf-8')
        crlf = '\r\n' in raw
        lines = raw.replace('\r\n', '\n').split('\n')
        done = []
        while True:
            r = hoist_once(lines)
            if not r:
                break
            lines, what = r
            done.append(what)
        if done:
            print(f'{os.path.basename(path)}: {len(done)}')
            total += len(done)
            if not dry:
                text = '\n'.join(lines)
                open(path, 'wb').write((text.replace('\n', '\r\n') if crlf else text).encode('utf-8'))
    print('total:', total)


if __name__ == '__main__':
    main()
