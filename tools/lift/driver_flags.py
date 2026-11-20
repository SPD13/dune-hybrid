#!/usr/bin/env python3
"""The result flags each driver entry reads on entry.

    python tools/lift/driver_flags.py [--why]

The three drivers (graphics, sound, music) are lifted into
crates/dune-rec/src/generated_{vga,sound,music}.rs, each instruction
listed as `// ADDR: TEXT`. The game calls them through its tables of far
pointers (`call far dword ptr [T + 4k]`; `load_driver_ax_with_vtable_at_si`
fills the segments, the offsets are the driver's jump table, 100h + 3k):
the graphics driver's at 38B5h, the music driver's at 396Dh, the sound
driver's at 3989h.

From each slot's entry, every path is walked (both ways of a conditional
jump, into near calls and on after them) with the flags not yet written;
a flag read while still unwritten is read on entry. Readers: conditional
jumps, `loope` / `loopne`, `adc` / `sbb` / `rcl` / `rcr` / `cmc`,
`pushf`, `lahf`, the decimal adjusts, `into`. Writers: the arithmetic and
logic ones (all six), `inc` / `dec` (all but CF), shifts by 1 or an
immediate (all but AF), rotates (CF, OF), `mul` / `imul` (CF, OF),
`clc` / `stc`, `sahf`, `popf`. A path ends at a return (the flags then
go back to the caller: what it reads after the call is the caller's
analysis) or a `hlt`. An INT goes on (no service reads the result flags).
Anything not followed (an indirect jump or call, a far call, an address
not lifted) counts as reading every flag still unwritten.

Prints, per table address, the slot and the flags read on entry (`-` for
none). Used by `dead_flag_ops.py` and `drop_flags.py` (`DRIVER_READS`).
"""
import os
import re
import sys

REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
SRC = os.path.join(REPO, 'crates', 'dune-rec', 'src')
DRIVERS = (('vga', 0x38b5, 'generated_vga.rs'), ('music', 0x396d, 'generated_music.rs'),
           ('sound', 0x3989, 'generated_sound.rs'))
CF, PF, AF, ZF, SF, OF = 0x001, 0x004, 0x010, 0x040, 0x080, 0x800
ALL = CF | PF | AF | ZF | SF | OF
NAMES = (('C', CF), ('P', PF), ('A', AF), ('Z', ZF), ('S', SF), ('O', OF))
JCC = {'jo': OF, 'jno': OF, 'jb': CF, 'jc': CF, 'jnae': CF, 'jnb': CF, 'jae': CF, 'jnc': CF,
       'je': ZF, 'jz': ZF, 'jne': ZF, 'jnz': ZF, 'jbe': CF | ZF, 'jna': CF | ZF, 'ja': CF | ZF,
       'jnbe': CF | ZF, 'js': SF, 'jns': SF, 'jp': PF, 'jpe': PF, 'jnp': PF, 'jpo': PF,
       'jl': SF | OF, 'jnge': SF | OF, 'jge': SF | OF, 'jnl': SF | OF, 'jle': ZF | SF | OF,
       'jng': ZF | SF | OF, 'jg': ZF | SF | OF, 'jnle': ZF | SF | OF}
READS = {'adc': CF, 'sbb': CF, 'rcl': CF, 'rcr': CF, 'cmc': CF, 'pushf': ALL, 'pushfw': ALL,
         'lahf': CF | PF | AF | ZF | SF, 'daa': CF | AF, 'das': CF | AF, 'aaa': AF, 'aas': AF,
         'into': OF, 'loope': ZF, 'loopz': ZF, 'loopne': ZF, 'loopnz': ZF, 'salc': CF}
WRITES = {'add': ALL, 'sub': ALL, 'cmp': ALL, 'and': ALL, 'or': ALL, 'xor': ALL, 'test': ALL,
          'neg': ALL, 'adc': ALL, 'sbb': ALL, 'inc': ALL & ~CF, 'dec': ALL & ~CF,
          'mul': CF | OF, 'imul': CF | OF, 'clc': CF, 'stc': CF, 'cmc': CF,
          'sahf': CF | PF | AF | ZF | SF, 'popf': ALL, 'popfw': ALL, 'daa': ALL & ~OF, 'das': ALL & ~OF,
          'aaa': CF | AF, 'aas': CF | AF, 'aam': PF | ZF | SF, 'aad': PF | ZF | SF}
SHIFTS = ('shl', 'sal', 'shr', 'sar')
ROTATES = ('rol', 'ror', 'rcl', 'rcr')


def load(path):
    text = open(path, encoding='utf-8').read()
    insn, order = {}, []
    for a, t in re.findall(r'// ([0-9a-f]{4}): (.*)', text):
        a = int(a, 16)
        if a not in insn:
            insn[a] = t.strip()
        order.append(a)
    # The next instruction: the one listed after it (lifted code lists a
    # block's instructions in order), the nearest above when listed apart.
    nxt = {}
    for a, b in zip(order, order[1:]):
        if 0 < b - a <= 6:
            nxt.setdefault(a, b)
    addrs = sorted(insn)
    for a, b in zip(addrs, addrs[1:]):
        nxt.setdefault(a, b)
    slots = {int(k): int(e, 16) for k, e in re.findall(r'/// `\w+_slot_(\d+)` \(driver:([0-9a-f]{4})\)', text)}
    return insn, nxt, slots


def target(t):
    m = re.search(r'\b0x([0-9A-Fa-f]+)\s*$', t)
    return int(m.group(1), 16) if m and '[' not in t else None


def readable_flags(insn, nxt):
    """The flags some instruction of the driver reads: those of its
    readers, and all of them when a `pushf` isn't followed, on every path,
    by a `popf` before a return or a jump not followed (its word could be
    read as data)."""
    r = 0
    for a, t in insn.items():
        op = t.split()[0].lower()
        if op in ('pushf', 'pushfw'):
            if not paired(insn, nxt, a):
                r = ALL
            continue
        r |= JCC.get(op, 0) | READS.get(op, 0)
    return r


def paired(insn, nxt, a):
    """Whether every path from the `pushf` at `a` reaches a `popf` before a
    return, a jump not followed or the end (calls stepped over)."""
    seen, todo = set(), [nxt.get(a)]
    while todo:
        x = todo.pop()
        if x is None:
            return False
        if x in seen:
            continue
        seen.add(x)
        t = insn.get(x)
        if t is None:
            return False
        op = t.split()[0].lower()
        if op in ('popf', 'popfw'):
            continue
        if op in ('ret', 'retn', 'retf', 'iret', 'hlt', 'pushf', 'pushfw'):
            return False
        d = target(t)
        if op == 'jmp':
            if d is None or 'far' in t:
                return False
            todo.append(d)
            continue
        if op in JCC or op.startswith('loop') or op == 'jcxz':
            if d is not None:
                todo.append(d)
        if op == 'call' and (d is None or 'far' in t):
            return False
        todo.append(nxt.get(x))
    return True


readable = ALL


def reads_on_entry(insn, nxt, entry, why):
    """The flags some path from `entry` reads before writing them."""
    read, seen = 0, set()
    todo = [(entry, ALL, ())]
    while todo:
        a, unwritten, saved = todo.pop()
        while True:
            if (a, unwritten, saved) in seen or not (unwritten or any(saved)):
                break
            seen.add((a, unwritten, saved))
            t = insn.get(a)
            if t is None:
                read |= unwritten
                why.append(f'{a:04x}: not lifted')
                break
            op = t.split()[0].lower()
            if op in ('rep', 'repe', 'repz', 'repne', 'repnz', 'lock') and len(t.split()) > 1:
                op = t.split()[1].lower()
            if op in ('pushf', 'pushfw'):
                # Kept aside for the popf that takes them back (they reach
                # the caller again, or a reader after it).
                saved = saved + (unwritten,)
                a = nxt.get(a)
                if a is None:
                    break
                continue
            if op in ('popf', 'popfw') and saved:
                unwritten, saved = saved[-1], saved[:-1]
                a = nxt.get(a)
                if a is None:
                    break
                continue
            r = (JCC.get(op, 0) | READS.get(op, 0))
            if op in ROTATES and op in ('rcl', 'rcr'):
                r |= CF
            if r & unwritten:
                read |= r & unwritten
                why.append(f'{a:04x}: {t}')
            w = WRITES.get(op, 0)
            if op in SHIFTS or op in ROTATES:
                count = t.split(',')[-1].strip().lower()
                if count == 'cl':
                    w = 0  # a count of 0 writes none
                elif op in SHIFTS:
                    w = ALL & ~AF
                else:
                    w = CF | OF
            unwritten &= ~w
            if op in ('ret', 'retn', 'retf', 'iret', 'hlt'):
                if saved:
                    # A pushf's word not taken back: treated as read.
                    read |= saved[-1]
                    why.append(f'{a:04x}: {t} with a pushf pending')
                break
            if op == 'jmp':
                d = target(t)
                if d is None or 'far' in t:
                    lost = (unwritten | (saved[-1] if saved else 0)) & readable
                    read |= lost
                    if lost:
                        why.append(f'{a:04x}: {t} (not followed)')
                    break
                a = d
                continue
            if op == 'call':
                d = target(t)
                if d is None or 'far' in t:
                    lost = (unwritten | (saved[-1] if saved else 0)) & readable
                    read |= lost
                    if lost:
                        why.append(f'{a:04x}: {t} (not followed)')
                    break
                # The callee: the flags unwritten so far, not the words a
                # pushf of the caller's left (its RET ends the walk there;
                # the caller's goes on after the call).
                todo.append((d, unwritten, ()))
            if op in JCC or op in ('loop', 'loope', 'loopz', 'loopne', 'loopnz', 'jcxz'):
                d = target(t)
                if d is not None:
                    todo.append((d, unwritten, saved))
            if a not in nxt:
                read |= unwritten
                why.append(f'{a:04x}: end of the code')
                break
            a = nxt[a]
    return read


def table():
    """{table address: (driver, slot, flags read on entry, why)}."""
    out = {}
    global readable
    for name, base, f in DRIVERS:
        insn, nxt, slots = load(os.path.join(SRC, f))
        readable = readable_flags(insn, nxt)
        for k, e in sorted(slots.items()):
            why = []
            out[base + 4 * k] = (name, k, reads_on_entry(insn, nxt, e, why), why)
    return out


def flags(m):
    return ''.join(n for n, v in NAMES if v & m) or '-'


if __name__ == '__main__':
    for a, (name, k, r, why) in sorted(table().items()):
        print(f'{a:04x} {name}_slot_{k:02d} {flags(r)}')
        if '--why' in sys.argv and r:
            for w in why[:5]:
                print('   ', w)
