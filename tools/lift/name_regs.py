#!/usr/bin/env python3
"""Turn the registers a routine answers in into named values at its callers.

    python tools/lift/name_regs.py --list [MODULE]   the call sites that read a result
    python tools/lift/name_regs.py MODULE...         convert the routines of MODULE

The register counterpart of `name_results.py`. A routine that hands
back a value (the troop SI points to, the current UI list in BP) leaves
it in a register, and the hand ports that call it read it after the call:

    c.call(entry::TROOP_PTR, 0x1fa8)?;
    let si = c.reg(SI);

For each routine of MODULE named in `RESULTS` this writes, after its port,
a named call returning the registers as values

    /// [`troop_ptr`] called: troop AL's record.
    pub(super) fn troop(c: &mut Ctx, ret: u16) -> Result<Troop, Bail> {
        c.call(0x..., ret)?;
        Ok(Troop(c.reg(SI)))
    }

and rewrites each call site in every hand port where a read of one of
them follows before anything can change it: another call, a helper handed
the context, a write of the register, a loop opened in between, or a
stretch of the original run in place (`c.run`) whose instructions (from
the generated code's listing comments, every path of its length) name the
register:

    let troop = troops::troop(&mut c, 0x1fa8)?;

A read bound to a plain local (`let si = c.reg(SI);`) takes the result's
name for the rest of its block, unless that name is used in the function
already (the site keeps the local then). The registers stay set on the
machine, so the code after (generated, interpreted) sees them as before.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from name_results import HAND, REPO, read, write, consts, ports, ctx_arg, LOOP  # noqa: E402

# Routine address: (name of the named call, [(register, local name, type)],
# what the answer means). Types: 'u16', 'u8' (the low byte), or a newtype
# over u16 (`Troop`).
RESULTS = {
    # cursor, screens, input
    0xd41b: ('current_ui_list', [('BP', 'ui_list', 'u16')], 'the current list of UI elements (BP).'),
    0xd454: ('item_position', [('AX', 'item_x', 'u16'), ('BX', 'item_y', 'u16')],
             'where item CL of the current list goes (AX, BX).'),
    0xe243: ('accelerated_pixels', [('AX', 'pixels', 'u16')],
             'the whole pixels of one axis of an accelerated step (AX, signed).'),
    0xdce0: ('joystick', [('DX', 'joy_x', 'u16'), ('BX', 'joy_y', 'u16'), ('AX', 'buttons', 'u8')],
             'the joystick: its X and Y counts (DX, BX) and its buttons (AL).'),
    # time, math
    0x1ac5: ('game_day', [('AX', 'day', 'u16')], 'the day (game time / 16, from 0; AX).'),
    0xe3cc: ('random_word', [('AX', 'random', 'u16')], 'the next random word (AX).'),
    0xe3b7: ('random_masked', [('AX', 'random', 'u16')], 'a random word ANDed with BX (AX).'),
    0xe3df: ('random_upto', [('AX', 'random', 'u16')], 'a random number 0..=BX (AX).'),
    # troops
    0x6906: ('troop_record', [('SI', 'troop', 'Troop')], 'troop AL\'s record (SI).'),
    0x68eb: ('shown_troop_record', [('SI', 'troop', 'Troop')], 'the record of the troop shown (SI).'),
    0x6efd: ('effective_motivation', [('AX', 'motivation', 'u8')],
             'the troop SI\'s effective motivation (AL).'),
    0x7c63: ('distance_to_map_position', [('AX', 'distance', 'u16')],
             'the troop SI\'s distance to the map position (AX).'),
    0x693b: ('activity', [('AX', 'activity', 'u16')], 'the troop SI\'s activity: its job / 4 (AX).'),
    # records, map, globe
    0x5e4f: ('sal_index', [('AX', 'sal', 'u16'), ('CX', 'kind', 'u8')],
             'AX plus the kind thresholds the location SI reaches (AX), and its kind (CL).'),
    0x5e6d: ('nearest_shown', [('DI', 'nearest', 'u16'), ('AX', 'distance', 'u16')],
             'the nearest shown location (DI, 0 for none) and its distance (AX, FFFFh for none).'),
    0x3efe: ('list_entry_ptr', [('SI', 'entry', 'u16')], 'entry DL of the list DH (SI).'),
    0x6252: ('map_kind_of', [('AX', 'map_kind', 'u16')], 'how the location at DI shows on the map (AX).'),
    0x407e: ('map_position', [('DX', 'map_x', 'u16'), ('BX', 'map_y', 'u16')], 'the map position (DX, BX).'),
    0xb532: ('globe_map_byte', [('AX', 'cell', 'u8')], 'the map byte at angle BX, distance DX (AL).'),
    # intro, video
    0x093f: ('next_script_word', [('AX', 'word', 'u16')], 'the intro script\'s next word (AX).'),
    0xc1f4: ('subresource', [('SI', 'sub', 'u16')], 'the offset of sub-resource AX of the sprite sheet (SI; ES its segment).'),
    # second pass
    0x342d: ('troop_yield_value', [('AX', 'yield_', 'u16')], 'the troop SI\'s yield (AX).'),
    0x6770: ('sprite_list', [('BP', 'sprites', 'u16')], 'the sprite list for the troop SI (BP).'),
    0x7c8f: ('record_distance', [('AX', 'distance', 'u16')],
             'the distance of the record SI (place in words 2 and 4) to the map position (AX).'),
    0x395c: ('sky_palette', [('BX', 'palette', 'u8')], 'the sky palette for the game time (BL).'),
    0x1ae0: ('time_of_day_slot', [('AX', 'slot', 'u16')], 'the time-of-day slot, 0..15 (AX).'),
    0x8ed3: ('word_width_px', [('CX', 'width', 'u16')], 'the width of the word at SI (CX).'),
    0xcf70: ('string_ptr', [('SI', 'string', 'u16')], 'string SI, counted from 1 (SI; ES its segment).'),
    0x6231: ('location_type_string', [('AX', 'type_string', 'u16')],
             'the string index for the kind of the location at DI (AX).'),
    0xd05f: ('draw_position', [('DX', 'pen_x', 'u16'), ('BX', 'pen_y', 'u16')], 'where text goes on (DX, BX).'),
    0x5098: ('troop_sides', [('CX', 'harkonnen', 'u16'), ('DX', 'unstarted', 'u16')],
             'the location DI\'s Harkonnen troops (CX) and unstarted byte-3-6 troops (DX).'),
    0x68d2: ('flagged_troops_refreshed', [('CX', 'refreshed', 'u16')], 'how many flagged troops were refreshed (CX).'),
    0x851f: ('linked_slot', [('CX', 'slot', 'u8')], 'the slot the troop SI was linked into (CL).'),
    0x1cda: ('field_spice_counted', [('DX', 'counted', 'u16')], 'how many locations the estimate counted (DX).'),
}

CALL = re.compile(r'c\.call\(((?:\w+::)*)(\w+), ([^;\n]+?)\)\?;')
READ = re.compile(r'c\.reg\((?:pc::cpu::)?(AX|BX|CX|DX|SI|DI|BP)\)|c\.reg8\((?:\w+::)*(AL|AH|BL|BH|CL|CH|DL|DH)\)')
# Anything that may change a register (besides `set_reg` of it, and
# `c.run`, checked against the listing).
WRITER = re.compile(
    r'\bc\.(?!run\b|reg\b|reg8\b|sreg\b|set_reg\b|set_reg8\b|set_sreg\b|byte\b|word\b|'
    r'set_byte\b|set_word\b|push\b|pop\b|below_sp\b|flag\b|set_flag\b|ret\b|\w+_state\b|'
    r'voice_buffer\b|es_byte\b|es_word\b|set_es_byte\b|set_es_word\b|seg_byte\b|seg_word\b|'
    r'set_seg_byte\b|set_seg_word\b|check\b|count\b|travel\b|player\b|progress\b|conversation\b|'
    r'update_\w+\b)\w+|\(c\b|&mut c\b|\bc\.m\b|\bc,')
FAMILY = {'AX': ('ax', 'al', 'ah'), 'BX': ('bx', 'bl', 'bh'), 'CX': ('cx', 'cl', 'ch'),
          'DX': ('dx', 'dl', 'dh'), 'SI': ('si',), 'DI': ('di',), 'BP': ('bp',)}
# Mnemonics that write registers they don't name.
IMPLICIT = {
    'AX': ('mul', 'imul', 'div', 'idiv', 'cbw', 'cwd', 'lods', 'in ', 'xlat', 'aam', 'aad', 'daa',
           'das', 'aaa', 'aas', 'lahf', 'salc', 'int'),
    'DX': ('mul', 'imul', 'div', 'idiv', 'cwd', 'int'),
    'CX': ('rep', 'loop', 'jcxz', 'int'),
    'BX': ('int',), 'BP': ('int',),
    'SI': ('lods', 'movs', 'cmps', 'outs', 'int', 'rep'),
    'DI': ('stos', 'movs', 'scas', 'cmps', 'ins', 'int', 'rep'),
}
READ_ONLY = {'cmp', 'test', 'push', 'out'}
DEST_FIRST = {'mov', 'add', 'sub', 'and', 'or', 'xor', 'adc', 'sbb', 'inc', 'dec', 'neg', 'not', 'shl', 'shr',
              'sal', 'sar', 'rol', 'ror', 'rcl', 'rcr', 'pop', 'lea', 'les', 'lds', 'in'}
REG8 = {'AL': 'AX', 'AH': 'AX', 'BL': 'BX', 'BH': 'BX', 'CL': 'CX', 'CH': 'CX', 'DL': 'DX', 'DH': 'DX'}


def listing():
    src = open(os.path.join(REPO, 'crates/dune-rec/src/generated.rs'), encoding='utf-8').read()
    insn = {}
    for a, t in re.findall(r'// ([0-9a-f]{4}): (.*)', src):
        insn.setdefault(int(a, 16), t.strip().lower())
    addrs = sorted(insn)
    nxt = dict(zip(addrs, addrs[1:]))
    return insn, nxt


INSN, NXT = listing()


def run_writes(at, n, reg):
    """Whether n instructions run from `at` (any path) may write reg."""
    names = FAMILY[reg]
    todo, seen = [(at, n)], set()
    while todo:
        a, k = todo.pop()
        if k <= 0 or (a, k) in seen:
            continue
        seen.add((a, k))
        t = INSN.get(a)
        if t is None:
            return True
        mn = t.split()[0]
        if any(t.startswith(i) for i in IMPLICIT.get(reg, ())):
            return True
        named = any(re.search(rf'\b{r}\b', t) for r in names)
        if named:
            ops = t[len(mn):].strip().split(',')
            if mn in READ_ONLY or mn.startswith('j'):
                pass  # reads its operands alone
            elif mn in DEST_FIRST:
                # Writes its first operand alone (a register, not memory).
                if any(re.fullmatch(rf'(?:byte |word )?{r}', ops[0].strip()) for r in names):
                    return True
            else:
                return True  # xchg, string ops with prefixes, anything else
        if mn in ('call', 'ret', 'retf', 'iret', 'hlt') or mn == 'jmp' and not re.fullmatch(r'jmp (short )?0x[0-9a-f]+', t):
            return True
        if mn == 'jmp':
            todo.append((int(t.split()[-1], 16), k - 1))
            continue
        if mn.startswith('j') or mn.startswith('loop'):
            m = re.search(r'0x([0-9a-f]+)$', t)
            if not m:
                return True
            todo.append((int(m.group(1), 16), k - 1))
        if a not in NXT:
            return True
        todo.append((NXT[a], k - 1))
    return False


def run_args(code):
    m = re.search(r'\bc\.run\(([^;]*)\)\?', code)
    if not m:
        return None
    args = m.group(1)
    depth = 0
    for i, ch in enumerate(args):
        depth += ch in '([{'
        depth -= ch in ')]}'
        if ch == ',' and depth == 0:
            a, n = args[:i].strip(), args[i + 1:].strip()
            try:
                return int(a, 0), int(n, 0)
            except ValueError:
                return 'unknown'
    return 'unknown'


def sites(text, fname, cs, outs_of):
    """(call match, address, [(read start, end, register, 8-bit name)], ctx arg)
    for each call whose result registers are read before anything changes them."""
    out = []
    for m in CALL.finditer(text):
        addr = cs.get((fname, m.group(2)), cs.get((None, m.group(2))))
        if addr is None:
            continue
        outs = outs_of(addr)
        if not outs:
            continue
        live = set(outs)
        reads = []
        pos, depth = m.end(), 0
        while pos < len(text) and live:
            nl = text.find('\n', pos)
            nl = len(text) if nl < 0 else nl
            code = re.sub(r'//.*', '', text[pos:nl])
            if LOOP.match(code) and depth >= 0:
                break
            if depth < 0:
                break
            ra = run_args(code)
            run_at = code.find('c.run(') if ra else -1
            w = WRITER.search(READ.sub('', code))
            for r in READ.finditer(code):
                reg = r.group(1) or REG8[r.group(2)]
                if reg not in live:
                    continue
                if w and w.start() < r.start() or 0 <= run_at < r.start():
                    break
                reads.append((pos + r.start(), pos + r.end(), reg, r.group(2)))
            if w:
                break
            for reg in list(live):
                if re.search(rf'c\.set_reg\((?:pc::cpu::)?{reg}\b', code) or any(
                        re.search(rf'c\.set_reg8\((?:\w+::)*{h}\b', code) for h, f in REG8.items() if f == reg):
                    live.discard(reg)
                elif ra and (ra == 'unknown' or run_writes(ra[0], ra[1], reg)):
                    live.discard(reg)
            if code.strip().startswith('}') and depth == 0:
                break
            depth += code.count('{') - code.count('}')
            pos = nl + 1
        if reads:
            out.append((m, addr, reads, ctx_arg(text, m.start())))
    return out


def list_sites(module):
    cs, ps = consts(), ports()
    outs = lambda a: list(FAMILY) if module != '--named' else [r for r, _, _ in RESULTS.get(a, ('', []))[1]]
    for f in sorted(os.listdir(HAND)):
        if not f.endswith('.rs'):
            continue
        text = read(os.path.join(HAND, f))[0]
        for m, addr, reads, arg in sites(text, f, cs, outs):
            mod = ps.get(addr, ('?', '?'))
            if module and module != '--named' and mod[0] != module:
                continue
            line = text.count('\n', 0, m.start()) + 1
            regs = ' '.join(r8 or r for _, _, r, r8 in reads)
            print(f'{addr:04x} {mod[0]}::{mod[1]:40} {f}:{line}  {regs}')


def fn_span(text, at):
    """Start and end of the function enclosing `at`."""
    s = max(text.rfind('\nfn ', 0, at), text.rfind('\npub fn ', 0, at), text.rfind('\npub(super) fn ', 0, at))
    e = text.find('\n}\n', at)
    return s + 1, (len(text) if e < 0 else e + 2)


def block_end(text, i):
    depth = 0
    while i < len(text):
        ch = text[i]
        if ch == '{':
            depth += 1
        elif ch == '}':
            if depth == 0:
                return i
            depth -= 1
        i += 1
    return i


def value(reg8, reg, var, ty):
    """The read `c.reg(REG)` / `c.reg8(r8::XL)` in terms of the result."""
    raw = var if ty in ('u16', 'u8') else f'{var}.0'
    if ty == 'u8':
        return var if reg8 and reg8.endswith('L') else None
    if reg8 is None:
        return raw
    return f'({raw} as u8)' if reg8.endswith('L') else f'(({raw} >> 8) as u8)'


def convert(modules):
    cs, ps = consts(), ports()
    todo = {a: r for a, r in RESULTS.items() if ps.get(a, (None,))[0] in modules}
    for module in modules:
        path = os.path.join(HAND, module + '.rs')
        text, crlf = read(path)
        for a, (name, outs, doc) in sorted(todo.items()):
            mod, port = ps[a]
            if mod != module or re.search(rf'\bfn {name}\(', text):
                continue
            m = re.search(rf'^pub fn {port}\(.*?^\}}\n', text, re.M | re.S)
            vals = []
            for reg, _, ty in outs:
                v = f'c.reg({reg})' if ty != 'u8' else f'c.reg({reg}) as u8'
                vals.append(v if ty in ('u16', 'u8') else f'{ty}({v})')
            tys = [ty for _, _, ty in outs]
            rty = tys[0] if len(tys) == 1 else '(' + ', '.join(tys) + ')'
            rv = vals[0] if len(vals) == 1 else '(' + ', '.join(vals) + ')'
            fn = (f'\n/// [`{port}`] called: {doc}\n'
                  f'pub(super) fn {name}(c: &mut Ctx, ret: u16) -> Result<{rty}, Bail> {{\n'
                  f'    c.call(0x{a:04x}, ret)?;\n    Ok({rv})\n}}\n')
            text = text[:m.end()] + fn + text[m.end():]
        write(path, text, crlf)
    total = skipped = 0
    for f in sorted(os.listdir(HAND)):
        if not f.endswith('.rs'):
            continue
        path = os.path.join(HAND, f)
        text, crlf = read(path)
        orig = text
        ss = sites(text, f, cs, lambda a: [r for r, _, _ in todo[a][1]] if a in todo else [])
        if not ss:
            continue
        for m, addr, reads, arg in reversed(ss):
            t = convert_site(text, orig, m, addr, reads, arg, todo, ps, f)
            if t is None:
                skipped += 1
            else:
                text, total = t, total + 1
        text = re.sub(r'(= |\(|, )\((\w+ as u8|\(\w+ >> 8\) as u8)\)(?=[;,)])', r'\1\2', text)
        mods = {ps[a][0] for a in todo} - {f[:-3]}
        for mod in sorted(mods):
            if re.search(rf'(?<!::)\b{mod}::\w+\((?:&mut )?c,', text) and not re.search(
                    rf'^use super::(?:{mod}\b|\{{[^}}]*\b{mod}\b)', text, re.M):
                text = re.sub(r'^(use super::)', rf'use super::{mod};\n\1', text, count=1, flags=re.M)
        write(path, text, crlf)
    print('sites', total, 'skipped', skipped)


REGLIKE = re.compile(r'(?:[abcd][xlh]|si|di|bp)\d*|r|v|w|a|b')


def sub_code(pattern, repl, text):
    """re.sub on the code of each line, not its comment."""
    out = []
    for line in text.split('\n'):
        i = line.find('//')
        code, comment = (line, '') if i < 0 else (line[:i], line[i:])
        out.append(re.sub(pattern, repl, code) + comment)
    return '\n'.join(out)


def rename_safe(rest, var):
    return not (re.search(rf'\blet (?:mut )?\(?[^=;]*\b{var}\b', rest) or re.search(rf'\b{var}\s*[-+*/|&^]?=[^=]', rest)
                or re.search(rf'\|[^|]*\b{var}\b[^|]*\|', rest) or re.search(rf'\{{\s*{var}\b|\b{var}\s*\}}|"[^"\n]*\{{{var}', rest))


def convert_site(text, orig, m, addr, reads, arg, todo, ps, f):
    """The text with the site converted, or None to leave it."""
    name, outs, _ = todo[addr]
    fs, fe = fn_span(orig, m.start())
    body = orig[fs:fe]
    vars_, free = {}, {}
    for reg, var, ty in outs:
        free[reg] = not re.search(rf'\b{var}\b', body)
        if not free[reg]:
            var = f'{var}_'
            if re.search(rf'\b{var}\b', body):
                return None
        vars_[reg] = (var, ty)
    bind = {reg: vars_[reg][0] for reg, _, _ in outs}
    per_reg = {}
    for s, e, reg, r8 in reads:
        per_reg.setdefault(reg, []).append((s, e, r8))
    handled = set()
    # A tuple `let` of reads alone: its names bind the result.
    if len(per_reg) > 1 and all(len(rs) == 1 for rs in per_reg.values()):
        s0 = min(rs[0][0] for rs in per_reg.values())
        ls = text.rfind('\n', 0, s0) + 1
        le = text.find('\n', s0) + 1
        lm = re.fullmatch(r'([ \t]*)let \(([\w, ]+)\) = \((.+)\);\n', text[ls:le])
        if lm:
            names = [x.strip() for x in lm.group(2).split(',')]
            elems = [x.strip() for x in lm.group(3).split(',')]
            regs_of = {text[rs[0][0]:rs[0][1]]: reg for reg, rs in per_reg.items()}
            between = text[m.end():ls]
            if len(names) == len(elems) and all(x in regs_of for x in elems) and len(elems) == len(per_reg) \
                    and all(vars_[regs_of[x]][1] == 'u16' and per_reg[regs_of[x]][0][2] is None for x in elems) \
                    and not any(re.search(rf'\b{n}\b', between) for n in names):
                for n, x in zip(names, elems):
                    bind[regs_of[x]] = n
                    handled.add(regs_of[x])
                text = text[:ls] + text[le:]
    # Each read, from the last back (an edit leaves the text before it as
    # it was): a read that is all of a `let` takes the local away, the
    # result in its place (a register's name renamed to the result's, any
    # other name binding the result); others become the result.
    order = sorted(((s, e, reg, r8) for reg, rs in per_reg.items() if reg not in handled for s, e, r8 in rs),
                   reverse=True)
    for s, e, reg, r8 in order:
        var, ty = vars_[reg]
        val = value(r8, reg, var, ty)
        if val is None:
            return None
        if len(per_reg[reg]) == 1:
            ls = text.rfind('\n', 0, s) + 1
            le = text.find('\n', e) + 1
            lm = re.fullmatch(r'([ \t]*)let (mut )?(\w+) = (.+);\n', text[ls:le])
            read_ = text[s:e]
            v_val = None
            if lm and lm.group(4) == read_:
                v_val = val
            elif lm and ty not in ('u16', 'u8') and lm.group(4) == f'{ty}({read_})' and r8 is None:
                v_val = var
            if v_val is not None:
                v = lm.group(3)
                end = block_end(text, le)
                rest = text[le:end]
                between = text[m.end():ls]
                if REGLIKE.fullmatch(v) and free[reg] and not lm.group(2) and rename_safe(rest, v):
                    rest = sub_code(rf'(?<![.\w]){v}\b(?!\s*:[^:])', v_val, rest)
                    text = text[:ls] + rest + text[end:]
                    continue
                if v_val == var and not re.search(rf'\b{v}\b', between):
                    bind[reg] = (lm.group(2) or '') + v
                    text = text[:ls] + text[le:]
                    continue
        text = text[:s] + val + text[e:]
    used = set(per_reg)
    pat = [bind[reg] if reg in used else '_' for reg, _, _ in outs]
    mod = ps[addr][0]
    if mod + '.rs' == f:
        path_ = name
    elif re.search(rf'^\s*(?:pub )?mod {mod}\b', text, re.M):
        path_ = f'super::{mod}::{name}'
    else:
        path_ = f'{mod}::{name}'
    lhs = pat[0] if len(pat) == 1 else '(' + ', '.join(pat) + ')'
    return text[:m.start()] + f'let {lhs} = {path_}({arg}, {m.group(3)})?;' + text[m.end():]


def main():
    args = sys.argv[1:]
    if args and args[0] == '--list':
        list_sites(args[1] if len(args) > 1 else None)
    else:
        convert(set(args))


if __name__ == '__main__':
    main()
