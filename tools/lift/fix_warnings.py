#!/usr/bin/env python3
"""Remove what a hand-port rewrite left unused: the imports and the
`code::` constants rustc warns about in `crates/dune-rec/src/hand`.

    python tools/lift/fix_warnings.py

Builds dune-rec (`cargo check`), applies the compiler's own suggestions for
unused imports, and deletes the lines of constants never used. Keeps each
file's line endings. Run until it reports nothing.
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))


def main():
    out = subprocess.run(['cargo', 'check', '-p', 'dune-rec', '--message-format=json'],
                         cwd=REPO, capture_output=True, text=True, encoding='utf-8').stdout
    edits = {}  # file -> [(byte start, byte end, replacement)]
    for line in out.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get('reason') != 'compiler-message':
            continue
        d = msg['message']
        if d['level'] == 'error':
            print('fix_warnings: the build fails; fix that first')
            return 1
        if d['level'] != 'warning':
            continue
        text = d['message']
        if text.startswith('unused import'):
            for ch in d['children']:
                for sp in ch['spans']:
                    if sp['suggested_replacement'] is not None and 'hand' in sp['file_name']:
                        edits.setdefault(sp['file_name'], []).append(
                            (sp['byte_start'], sp['byte_end'], sp['suggested_replacement']))
        elif text.startswith('constant') and text.endswith('is never used'):
            sp = d['spans'][0]
            if 'hand' in sp['file_name']:
                edits.setdefault(sp['file_name'], []).append((sp['byte_start'], sp['byte_end'], None))
    n = 0
    for f, es in edits.items():
        path = os.path.join(REPO, f)
        raw = open(path, 'rb').read()
        nl = b'\r\n' if b'\r\n' in raw else b'\n'
        for s, e, r in sorted(set(es), key=lambda x: -x[0]):
            if r is None:
                # The whole line of the constant.
                ls = raw.rfind(b'\n', 0, s) + 1
                le = raw.find(b'\n', e) + 1
                raw = raw[:ls] + raw[le:]
            else:
                raw = raw[:s] + r.replace('\n', nl.decode()).encode() + raw[e:]
            n += 1
        open(path, 'wb').write(raw)
    print('fixed', n)


if __name__ == '__main__':
    sys.exit(main())
