#!/usr/bin/env python3
"""After `drop_flags.py`: remove the reads only the dropped flag helpers
used.

    python tools/lift/drop_unused_reads.py

Runs `cargo check` on dune-rec and, for each `unused variable` warning in
`hand/`, deletes its `let` line when that is a plain read (a register,
memory through `c.byte` / `c.word` / their segment forms, or a copy), or
renames a loop or tuple binding to `_`. Repeats until no such warning is
left. Reads have no side effects here, so the ports stay exact.
"""
import os
import re
import subprocess

REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
READ = re.compile(r'^\s*let (mut )?(\w+) = (c\.(reg8?|byte|word|byte_in|word_in|byte_at_seg|word_at|sreg|code_word)\([^;]*\)|\w+|\w+ as \w+|\w+ & 0x[0-9a-f]+|\(?[\w ]+ >> \d+\)? as u8);\s*$')

for _ in range(20):
    r = subprocess.run(['cargo', 'check', '--release', '-p', 'dune-rec', '--target-dir', 'target-hand',
                        '--message-format', 'short', '--color', 'never'], cwd=REPO, capture_output=True, text=True,
                       env=dict(os.environ, CARGO_BUILD_JOBS='2'))
    hits = re.findall(r'^(crates[\\/]dune-rec[\\/]src[\\/]hand[\\/]\w+\.rs):(\d+):(\d+): warning: unused variable: `(\w+)`', r.stderr, re.M)
    if not hits:
        break
    by_file = {}
    for path, line, col, name in hits:
        by_file.setdefault(path.replace('\\', '/'), []).append((int(line), int(col), name))
    changed = False
    for path, items in by_file.items():
        full = os.path.join(REPO, path)
        lines = open(full, encoding='utf-8').read().split('\n')
        for line, col, name in sorted(items, reverse=True):
            text = lines[line - 1]
            m = READ.match(text)
            if m and m.group(2) == name:
                del lines[line - 1]
                changed = True
            else:
                # A binding in a pattern: `_`.
                k = col - 1
                if text[k:k + len(name)] == name:
                    lines[line - 1] = text[:k] + '_' + text[k + len(name):]
                    changed = True
        open(full, 'w', encoding='utf-8', newline='\n').write('\n'.join(lines))
    if not changed:
        print('left:', hits)
        break
print(subprocess.run(['cargo', 'check', '--release', '-p', 'dune-rec', '--target-dir', 'target-hand',
                      '--message-format', 'short', '--color', 'never'], cwd=REPO, capture_output=True, text=True,
                     env=dict(os.environ, CARGO_BUILD_JOBS='2')).stderr[-2000:])
