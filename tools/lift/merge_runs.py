#!/usr/bin/env python3
"""Merge a hand port's back-to-back stretches into one (Stage A step 3).

    python tools/lift/merge_runs.py [--dry] [FILE.rs ...]

`c.run(A, n)?;` followed, at the same nesting, only by plain statements
(register and memory writes, flag helpers, `let`s) and then `c.run(B, m)?;`
is the one straight path of n + m instructions from A: the second `run`
goes and the first counts both (`c.run(A, n + m)?;`). The code between
keeps its order, so what the port computes is unchanged; only the bail
points get fewer. A bail at the merged `run` comes before anything is
written (the rule for every `run`); with `REC_INPLACE` an interrupt inside
it is placed by stepping the original from the stretch's start
(`Machine::state_at`), so the registers and flags the port sets between
no longer have to be exact at a boundary there.

Left alone:
- anything between that can stop, hand off or run code: `?`, `return`,
  blocks (`{` `}`), `c.call`, `c.jump`, `c.ret`, `c.interp`, `c.check`,
  `c.count`, `c.rep*`, `c.sti`, port I/O, the context handed to a helper;
- ports that read or write memory the interrupt handlers share (the
  clock's countdown, the ticks, the video frame's bytes, the sound states,
  CS:[0E8D4h]): there each such access must stay right after a one-
  instruction stretch (see `hand/ticks.rs`).

Prints, per file, the merges made (with --dry: that would be made).
"""
import os
import re
import sys

HAND = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'crates', 'dune-rec', 'src', 'hand')

RUN = re.compile(r'^(\s*)c\.run\(([^,()]+(?:\([^()]*\))?), ([^;]+?)\)\?;\s*(?://\s*(.*))?$')
BAD = re.compile(r'\?|\breturn\b|[{}]|c\.(call|jump|ret|interp|check|count|rep|run|sti|cli|out|inp|port|boundary)\b'
                 r'|\(&mut c\b|\bc\.m\b|\(c[,)]|\bc\)|\bc,')
SHARED = re.compile(r'\b(PCM_STATE|PCM_STATE_2|MIDI_FUNC5_CX|TIMER|CLOCK_COUNTDOWN|HNM_FLAG_MSB)\b'
                    r'|0x(46db|ce7a|dbb5|dbd0|3817|381f|e8d4)\b|\bclock\(|update_clock|sound_state|video_state'
                    r'|update_video')
IDENT = re.compile(r'\b[A-Za-z_]\w*\b')
# What a statement binds or changes: `let (mut) PATTERN =`, `NAME (op)= `.
BINDS = (re.compile(r'\blet\s+(?:mut\s+)?(.+?)(?::[^=]*)?=(?!=)'),
         re.compile(r'^\s*([A-Za-z_]\w*)\s*(?:[-+*/|&^]|<<|>>)?=(?!=)'))
# Ports the plan doc lists as reading or writing handler-shared memory in
# one-instruction stretches ("Interrupts in place").
SHARED_PORTS = {'pcm_test_audio_done', 'rand', 'wait_a_bit', 'lip_sync_frame_task', 'process_frame_tasks',
                'get_mouse_pos_etc', 'hnm_do_frame', 'main_pass', 'get_mouse_cursor_image_addr',
                'measure_pit_speed'}
FN = re.compile(r'^(pub(\(\w+\))? )?fn (\w+)')


def functions(lines):
    """(start, end) line ranges of the top-level fns (to the closing `}` at
    column 0)."""
    out, start = [], None
    for i, l in enumerate(lines):
        if start is None and FN.match(l):
            start = i
        elif start is not None and l.startswith('}'):
            out.append((start, i))
            start = None
    return out


def add(n, m):
    if n.isdigit() and m.isdigit():
        return str(int(n) + int(m))
    return f'{n} + {m}'


def merge_fn(lines, a, b):
    """Merge in lines[a..=b]; returns the number of merges."""
    if any(SHARED.search(l) for l in lines[a:b + 1]):
        return 0
    m = FN.match(lines[a])
    if m and m.group(3) in SHARED_PORTS:
        return 0
    merges = 0
    i = a
    while i <= b:
        m = RUN.match(lines[i])
        if not m:
            i += 1
            continue
        indent, at, n, note = m.group(1), m.group(2), m.group(3), m.group(4)
        j = i + 1
        bound = set()  # names bound or assigned since the first `run`
        while j <= b:
            l = lines[j]
            s = l.strip()
            m2 = RUN.match(l)
            if m2:
                # The second count is worked out where the first `run` is:
                # it may not use what the code between binds or changes.
                # A one-instruction stretch is kept as it is: that is how a
                # port reads or writes memory a handler shares (the keys'
                # bytes in `get_mouse_pos_etc`, ...), right after its `run`.
                if (m2.group(1) != indent or bound & set(IDENT.findall(m2.group(3)))
                        or n.strip() == '1' or m2.group(3).strip() == '1'):
                    break
                n = add(n, m2.group(3))
                notes = [x for x in (note, m2.group(4)) if x]
                if len(notes) == 2 and notes[1] in notes[0].split(' | '):
                    notes = notes[:1]
                note = ' | '.join(notes) if notes else None
                lines[i] = f'{indent}c.run({at}, {n})?;' + (f' // {note}' if note else '')
                del lines[j]
                b -= 1
                merges += 1
                continue
            if s == '' or s.startswith('//'):
                j += 1
                continue
            # A line at a deeper indent continues a statement (fine); one
            # at a shallower indent ends the block.
            if BAD.search(l) or len(l) - len(l.lstrip()) < len(indent):
                break
            for pat in BINDS:
                for g in pat.findall(l):
                    bound |= set(IDENT.findall(g))
            j += 1
        i += 1
    return merges


def main():
    args = sys.argv[1:]
    dry = '--dry' in args
    files = [x for x in args if x != '--dry'] or sorted(
        os.path.join(HAND, f) for f in os.listdir(HAND) if f.endswith('.rs'))
    total = 0
    for p in files:
        raw = open(p, 'rb').read().decode('utf-8')
        crlf = '\r\n' in raw
        lines = raw.replace('\r\n', '\n').split('\n')
        n = 0
        for a, b in reversed(functions(lines)):
            n += merge_fn(lines, a, b)
        if n:
            print(f'{os.path.basename(p)}: {n}')
            total += n
            if not dry:
                s = '\n'.join(lines)
                if crlf:
                    s = s.replace('\n', '\r\n')
                open(p, 'wb').write(s.encode('utf-8'))
    print(f'total: {total}')


if __name__ == '__main__':
    main()
