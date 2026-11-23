#!/usr/bin/env python3
"""Turn the registers a routine is handed into arguments at its callers.

    python tools/lift/name_args.py --list MODULE   the routines of MODULE, their docs and the
                                                   registers their callers set right before
    python tools/lift/name_args.py MODULE...       convert the routines of MODULE named in `ARGS`

The counterpart of `name_regs.py` for what goes in. A hand port that
calls a routine sets its input registers first:

    c.set_reg8(r8::AL, troop);
    c.call(entry::TROOP_PTR, 0x1fa8)?;

For each routine of MODULE named in `ARGS` its named call (the one
`name_regs.py` or `name_results.py` wrote, or a new one) takes them as
arguments and sets them itself, right before the call:

    /// [`troop_ptr`] called: troop's record.
    pub(super) fn troop_record(c: &mut Ctx, troop: u8, ret: u16) -> Result<Troop, Bail> {
        c.set_reg8(r8::AL, troop);
        c.call(0x6906, ret)?;
        ...

and each call site takes the value its `set_reg` line (in the run of
`set_reg` lines right before the call) held:

    let troop = troops::troop_record(&mut c, troop, 0x1fa8)?;

A value that is more than a plain name or number is bound where it was
set (`let troop = ...;`), so it is worked out at the same point as
before. A plain `c.call` is named only where it sets all the inputs
this way; others stay as they are. A named call already there (from the
earlier passes) takes them everywhere: a value the site did not set
itself (it came from a stretch of the original run, or from further up)
is read from the register right before it (`let troop = c.reg8(r8::AL);`). Setting a register to the value it
holds is no change, so the machine sees the same as before; a fold is
left out when a `set_reg` line after it reads the register, or when the
site sets the register at another width.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from name_results import HAND, read, write, consts, ports, ctx_arg  # noqa: E402

# Routine address: (name of a new named call (None when one exists),
# [(register, argument name, type)], what it does with them, for the doc
# of a new named call). Types: 'u16' (AX..), 'u8' (AL..), or a newtype over
# u16 (`Troop`).
ARGS = {
    # troops
    0x6603: ('run_for_each_troop', [('BP', 'routine', 'u16')], "BP's routine for each troop."),
    0x661d: ('run_for_your_troops', [('BP', 'routine', 'u16')],
             "BP's routine for each troop of yours (`troop_ptr` CF)."),
    0x6639: ('run_for_location_troops', [('BP', 'routine', 'u16')],
             "BP's routine for each troop at the location DI or travelling to it."),
    0x6757: (None, [('AL', 'troop', 'u8')], None),
    0x6ad4: ('give_job', [('CL', 'job', 'u8')], 'the troop SI takes the job CL.'),
    0x6edd: ('raise_skill', [('BX', 'skill', 'u16'), ('AL', 'points', 'u8')],
             "the troop SI's skill BX up by AL (at most 5Fh)."),
    0x6f48: ('raise_motivation', [('AL', 'points', 'u8')], "the troop SI's motivation up by AL (at most 100)."),
    0x6f56: ('raise_all_motivation', [('AL', 'points', 'u8')],
             "every troop's motivation up by AL (at most 100), but those not yours or started."),
    0x6fb0: ('drop_charisma', [('AL', 'points', 'u8')], 'charisma down by AL (at least 1), motivation with it.'),
    # story, records, map, globe
    0x6f78: ('raise_charisma', [('AL', 'points', 'u8')], 'charisma up by AL (at most C8h), motivation with it.'),
    0x121f: ('raise_phase', [('AL', 'phase', 'u8')], 'the game phase raised to AL (never lowered).'),
    0x331e: ('talk_about_location', [('DI', 'location', 'u16')], 'the location DI becomes the one talked about.'),
    0x29ee: ('add_event', [('AX', 'event', 'u16')], 'the pending event pair AX added, with DI 0.'),
    0x36ee: ('run_for_records_here', [('BP', 'routine', 'u16')], "BP's routine for each record here."),
    0x5e4f: (None, [('AX', 'base', 'u16')], None),
    0x2e98: ('note_place', [('DI', 'place', 'u16')], 'the name of the place DI noted for the phrases.'),
    0x5e6d: (None, [('AL', 'kind_limit', 'u8')], None),
    0x5ed0: (None, [('DI', 'location', 'u16')], None),
    0x81d7: ('add_point', [('DX', 'along', 'u16'), ('BX', 'row', 'u16')],
             'an entry at DI for the map point (DX, BX) and its screen point.'),
    0xb647: (None, [('DX', 'along', 'u16'), ('BX', 'row', 'u16')], None),
    0x0fd9: ('run_clock', [('CX', 'steps', 'u16')], 'the game clock run CX steps at once.'),
    # math
    0xe3b7: (None, [('BX', 'mask', 'u16')], None),
    0xe3df: (None, [('BX', 'max', 'u16')], None),
    # screens, drawing
    0xc108: ('transition_drawn', [('BP', 'draw', 'u16'), ('AL', 'transition', 'u8')],
             'BP drawn into the front buffer, then transition AL to the screen.'),
    0xc0d5: ('transition_front_buffer', [('AL', 'transition', 'u8')],
             'transition AL from the front buffer to the screen.'),
    0xc097: ('draw_on_front_buffer', [('BP', 'draw', 'u16')], 'BP called with the front buffer as the screen.'),
    0x189a: ('redraw_room', [('AL', 'transition', 'u8')], 'the room redrawn, directly or by transition AL.'),
    0xc22f: ('draw_sprite_clobbering', [('AX', 'sprite', 'u16'), ('DX', 'x', 'u16'), ('BX', 'y', 'u16')],
             'sprite AX of the loaded sheet at (DX, BX); BX, DX clobbered.'),
    0xc305: ('draw_sprite_clipped_at', [('AX', 'sprite', 'u16'), ('DX', 'x', 'u16'), ('BX', 'y', 'u16')],
             'sprite AX at (DX, BX), clipped.'),
    0xc2f2: ('draw_resource_sprite_0', [('AL', 'resource', 'u8')], 'resource AL opened, its sprite 0 drawn at (0, 0).'),
    0xc21b: ('draw_icons', [('SI', 'list', 'u16')], 'the sprites listed at SI (sprite, x, y), up to FFFFh.'),
    0xd1f2: ('draw_ui_elements', [('SI', 'list', 'u16'), ('CX', 'count', 'u16')],
             'the icon sprites loaded, then the CX UI elements at SI drawn.'),
    0xc370: ('tile_sprite', [('AX', 'sprite', 'u16'), ('SI', 'rect', 'u16')],
             'sprite AX tiled over the rect at SI.'),
    0xc551: ('outline_rect', [('SI', 'rect', 'u16')], 'the outline of the rect at SI.'),
    0x7b1b: ('fill_and_outline_rect', [('SI', 'rect', 'u16')], 'the panel fill, then the rect SI outlined.'),
    0xc6ad: ('redraw_rect', [('SI', 'rect', 'u16')], 'the rect at SI redrawn on screen.'),
    0x5b99: ('copy_rect_to', [('SI', 'from', 'u16'), ('DI', 'to', 'u16')], 'the rect at SI copied to DI.'),
    0xdb74: ('hide_mouse_over', [('SI', 'rect', 'u16')], 'the cursor hidden if it meets the rect at SI.'),
    0xa502: ('draw_slider', [('SI', 'slider', 'u16')], 'the slider at SI.'),
    # text
    0xcf70: (None, [('SI', 'string', 'u16')], None),
    0xd194: ('draw_string_at', [('AX', 'string', 'u16'), ('DX', 'x', 'u16'), ('BX', 'y', 'u16'),
                                ('CX', 'colour', 'u16')], 'string AX at (DX, BX) in colour CX.'),
    0x8865: ('draw_interpolated_at', [('AX', 'string', 'u16'), ('DX', 'x', 'u16'), ('BX', 'y', 'u16'),
                                      ('CX', 'colour', 'u16')],
             'string AX with its fields filled in, at (DX, BX) in colour CX.'),
    0xd04e: ('set_draw_position', [('DX', 'x', 'u16'), ('BX', 'y', 'u16')], 'text goes on at (DX, BX).'),
    # screens
    0xd95e: ('set_click', [('AX', 'handler', 'u16')], 'the click handler AX.'),
    0xd72b: ('set_buttons_from', [('SI', 'buttons', 'u16')], 'the buttons from SI.'),
    0xd454: (None, [('CL', 'item', 'u8')], None),
    # tasks, waits
    0xda25: ('add_task', [('SI', 'routine', 'u16'), ('BP', 'every', 'u16')],
             'routine SI added to the frame tasks, every BP frames.'),
    0xda5f: ('remove_task', [('SI', 'routine', 'u16')], 'routine SI removed from the frame tasks.'),
    0xe387: ('wait_ticks', [('AX', 'ticks', 'u16')], 'AX ticks of the timer.'),
    0xe3a0: ('wait_ticks_with_tasks', [('AX', 'ticks', 'u16')], 'AX ticks, running the frame tasks after each.'),
    0xddb0: ('wait_or_input', [('AX', 'ticks', 'u16')], 'AX ticks, or until a key or click.'),
    0xe353: ('draw_then_wait', [('BP', 'draw', 'u16'), ('AX', 'ticks', 'u16')],
             'BP called, then AX ticks from before it waited, running the frame tasks.'),
    # files, sound, saves
    0xc13e: ('open_resource', [('AX', 'resource', 'u16')], 'resource AX opened.'),
    0xf0b9: ('open_resource_into', [('SI', 'resource', 'u16')], 'resource SI read into ES:DI.'),
    0xcdbf: (None, [('CX', 'count', 'u16')], None),
    0xab15: ('start_voc', [('AL', 'voc', 'u8')], 'VOC AL started, unless a voice plays or PCM is off.'),
    0xb389: ('save_to_slot', [('CL', 'slot', 'u8')], 'the game saved to slot CL.'),
    # second pass
    0xe57b: ('load_driver', [('AX', 'driver', 'u16'), ('SI', 'vtable', 'u16'), ('CX', 'count', 'u16')],
             "driver AX loaded, the segments of its CX far pointers at SI filled in."),
    0xe2db: ('replace_digits', [('AX', 'number', 'u16'), ('SI', 'string', 'u16')],
             'AX written into the number of string SI.'),
    0xe290: ('draw_number_at_pos', [('AX', 'number', 'u16'), ('DX', 'x', 'u16'), ('BX', 'y', 'u16')],
             'AX drawn as 3 digits at (DX, BX).'),
    0x5323: ('octant_of_place', [('BP', 'place', 'u16')], 'the octant of the place at [BP] (SS) into [BP+2].'),
    0xf11c: ('make_room', [('CX', 'paragraphs', 'u16')], 'CX paragraphs made free above the allocator.'),
    0xc4f0: ('draw_head_and_update_rect', [('SI', 'rect', 'u16')],
             'the head drawn if needed, the screen rect at SI updated.'),
    0xf2d6: ('seek_dune_dat', [('DX', 'high', 'u16'), ('AX', 'low', 'u16')], 'DUNE.DAT sought to DX:AX.'),
    0x9ef1: ('talk_text_voice', [('AL', 'talking', 'u8')], 'the text voice with the talking character AL.'),
    0x96d8: ('speak_as', [('AX', 'character', 'u16')], 'the character AX talks.'),
    0x93df: ('start_speaking', [('AX', 'speaker', 'u16')], 'the speaker AL (CL) starts talking.'),
    0x920f: ('open_head_resource', [('AX', 'head', 'u16')], 'resource AX + 2 opened.'),
    0x8b11: ('lay_out_text', [('SI', 'text', 'u16')], 'the text SI laid out in its box.'),
    0xa49c: ('draw_counter_of', [('SI', 'counter', 'u16')], 'the counter at SI on the back buffer.'),
    0x6f93: ('lower_motivation', [('AL', 'points', 'u8')], 'the troop at SI loses AL motivation.'),
    0x7f27: ('note_spare_equipment', [('DI', 'location', 'u16')], 'the skill needs of the location DI into 46FEh.'),
    0x1e24: (None, [('DI', 'location', 'u16')], None),
    0xd1a6: ('draw_phrases', [('SI', 'list', 'u16')], 'the strings listed at SI (string, x, y, colour).'),
    0x31f6: ('tell_about_troop', [('SI', 'troop', 'u16')], 'the troop SI described in the condition variables.'),
    0x503c: ('enter_location_at', [('DI', 'location', 'u16')], 'entering the location at DI.'),
    0x0820: ('open_sky_palette', [('BL', 'palette', 'u8')], 'palette BL of SKYRS.'),
    0xc4aa: ('copy_rect_to_screen', [('SI', 'rect', 'u16')], 'the rect at SI from the screen buffer to the front.'),
    0xc477: ('copy_rect_to_fb2', [('SI', 'rect', 'u16')], 'the rect at SI from framebuffer 1 to 2.'),
    0x425b: ('discover_at', [('DI', 'location', 'u16')], 'the location DI discovered.'),
    0x19df: ('draw_mark_row', [('CL', 'marks', 'u8')], 'up to 5 marks in a row from (DX, BX), for CL.'),
    0x6447: ('draw_circle_of', [('CX', 'radius', 'u16')], 'the circle of radius CX around the location DI.'),
    0xf2fc: ('copy_file_name', [('DX', 'name', 'u16')], 'the string at DX copied to the filename buffer.'),
    0xdbb2: ('hide_mouse_at', [('DX', 'x', 'u16'), ('BX', 'y', 'u16')],
             'the cursor hidden over (DX, BX), drawn again on return.'),
}

FAMILY = {'AX': ('AX', 'AL', 'AH'), 'BX': ('BX', 'BL', 'BH'), 'CX': ('CX', 'CL', 'CH'), 'DX': ('DX', 'DL', 'DH'),
          'SI': ('SI',), 'DI': ('DI',), 'BP': ('BP',)}
FAMILY_OF = {r: f for f, rs in FAMILY.items() for r in rs}
SET = re.compile(r'^([ \t]*)c\.set_reg(8?)\((?:\w+::)*(\w+), (.+)\);[ \t]*(//.*)?\n')
NAMED = re.compile(r'^pub\(super\) fn (\w+)\(c: &mut Ctx, ([^)]*)\) -> ([^{]+) \{\n((?:.*\n)*?)\}\n', re.M)


def named_calls():
    """Address -> [(module, name)] of the named calls already written."""
    out = {}
    for f in os.listdir(HAND):
        if f.endswith('.rs'):
            for m in NAMED.finditer(read(os.path.join(HAND, f))[0]):
                a = re.search(r'c\.call(?:_cf|_zf)?\(0x([0-9a-f]+), ret\)', m.group(4))
                if a:
                    out.setdefault(int(a.group(1), 16), []).append((f[:-3], m.group(1)))
    return out


def set_block(text, ls):
    """The `set_reg` lines right before the line at `ls`: [(start, end, 8?, reg, value)]."""
    out = []
    while ls > 0:
        ps = text.rfind('\n', 0, ls - 1) + 1
        m = SET.fullmatch(text[ps:ls])
        if not m:
            break
        out.insert(0, (ps, ls, m.group(2) == '8', m.group(3).upper(), m.group(4)))
        ls = ps
    return out


def reads_reg(expr, reg):
    fam = FAMILY_OF.get(reg, reg)
    return any(re.search(rf'\b{r}\b', expr) for r in FAMILY[fam]) and 'c.' in expr


def pure(expr):
    return 'c.' not in expr and '?' not in expr and '(' not in expr.replace('as u', '')


def call_sites(text, fname, cs, named):
    """(start, end, address, kind, groups) of each call to a routine: raw
    `c.call(K, ret)?;` statements and named calls `path::name(c, ret)`."""
    out = []
    for m in re.finditer(r'c\.call\(((?:\w+::)*)(\w+), ([^;\n]+?)\)\?;', text):
        a = cs.get((fname, m.group(2)), cs.get((None, m.group(2))))
        if a is not None:
            out.append((m.start(), m.end(), a, 'raw', m))
    names = {}
    for a, ns in named.items():
        for mod, name in ns:
            names[name] = (a, mod)
    for m in re.finditer(r'(?<![\w.])((?:\w+::)*)(\w+)\(((?:&mut )?c), ', text):
        a, mod = names.get(m.group(2), (None, None))
        if a is None:
            continue
        path = m.group(1)
        if path and not path.rstrip(':').endswith(mod):
            continue
        if not path and fname[:-3] != mod:
            continue
        out.append((m.start(), m.end(), a, 'named', m))
    return sorted(out, key=lambda t: t[0])


def list_module(module):
    cs, ps, named = consts(), ports(), named_calls()
    stats = {}
    for f in sorted(os.listdir(HAND)):
        if not f.endswith('.rs'):
            continue
        text = read(os.path.join(HAND, f))[0]
        for s, e, a, kind, m in call_sites(text, f, cs, named):
            if ps.get(a, (None,))[0] != module:
                continue
            st = stats.setdefault(a, [0, {}])
            st[0] += 1
            ls = text.rfind('\n', 0, s) + 1
            for _, _, w8, reg, _ in set_block(text, ls):
                st[1][reg] = st[1].get(reg, 0) + 1
    text = read(os.path.join(HAND, module + '.rs'))[0]
    for a in sorted(stats):
        port = ps[a][1]
        dm = re.search(rf'((?:^///.*\n)+)pub fn {port}\(', text, re.M)
        doc = ' '.join(l[3:].strip() for l in dm.group(1).splitlines()) if dm else ''
        n, regs = stats[a]
        mark = '*' if a in ARGS else ' '
        nm = ' '.join(n for _, n in named.get(a, []))
        print(f'{mark}{a:04x} {port} [{nm}] calls={n} sets: ' + ' '.join(f'{r}:{k}' for r, k in sorted(regs.items(), key=lambda kv: -kv[1])))
        print(f'      {doc[:300]}')


def wrapper_sets(args):
    out = []
    for reg, name, ty in args:
        v = name if ty in ('u16', 'u8') else f'{name}.0'
        out.append(f'    c.set_reg8(r8::{reg}, {v});\n' if ty == 'u8' else f'    c.set_reg({reg}, {v});\n')
    return ''.join(out)


def params(args):
    return ''.join(f'{name}: {ty}, ' for _, name, ty in args)


def write_wrappers(modules, ps, named):
    for module in modules:
        path = os.path.join(HAND, module + '.rs')
        text, crlf = read(path)
        for a, (new, args, doc) in sorted(ARGS.items()):
            if ps.get(a, (None,))[0] != module:
                continue
            if a in named:
                for _, name in named[a]:
                    m = re.search(rf'^pub\(super\) fn {name}\(c: &mut Ctx, ret: u16\) -> [^{{]+ \{{\n', text, re.M)
                    if not m:
                        continue  # done already
                    head = m.group(0).replace('c: &mut Ctx, ', 'c: &mut Ctx, ' + params(args))
                    text = text[:m.start()] + head + wrapper_sets(args) + text[m.end():]
            else:
                if re.search(rf'\bfn {new}\b', text):
                    sys.exit(f'{module}.rs has a fn {new} already')
                port = ps[a][1]
                m = re.search(rf'^pub fn {port}\(.*?^\}}\n', text, re.M | re.S)
                fn = (f'\n/// [`{port}`] called: {doc}\n'
                      f'pub(super) fn {new}(c: &mut Ctx, {params(args)}ret: u16) -> Result<(), Bail> {{\n'
                      f'{wrapper_sets(args)}    c.call(0x{a:04x}, ret)\n}}\n')
                text = text[:m.end()] + fn + text[m.end():]
                named[a] = [(module, new)]
        write(path, uses_r8(text), crlf)


def uses_r8(text):
    """`r8` imported where the text uses it."""
    if re.search(r'(?<!::)\br8::', text) and not re.search(r'^use super::\{[^}]*\br8\b', text, re.M):
        text = re.sub(r'^(use super::\{[^}]*?)\};', r'\1, r8};', text, count=1, flags=re.M)
    return text


def fn_body(text, at):
    s = max(text.rfind('\nfn ', 0, at), text.rfind('\npub fn ', 0, at), text.rfind('\npub(super) fn ', 0, at))
    e = text.find('\n}\n', at)
    return text[s + 1:(len(text) if e < 0 else e + 2)]


def fresh(name, body, taken):
    for n in (name, name + '_', name + '2'):
        if not re.search(rf'\b{n}\b', body) and n not in taken:
            return n
    return None


def convert_site(text, s, e, a, kind, m, f, ps, named):
    """The text with the site converted (None to leave it)."""
    _, args, _ = ARGS[a]
    ls = text.rfind('\n', 0, s) + 1
    block = set_block(text, ls)
    body = fn_body(text, s)
    edits = []  # (start, end, replacement)
    vals, before = [], []
    taken = set()
    folded = 0
    for reg, name, ty in args:
        want8 = ty == 'u8'
        hits = [b for b in block if FAMILY_OF.get(b[3], b[3]) == FAMILY_OF.get(reg, reg)]
        hit = None
        if len(hits) == 1 and hits[0][3] == reg and hits[0][2] == want8:
            later = [b for b in block if b[0] > hits[0][0]]
            if not any(reads_reg(b[4], reg) for b in later):
                hit = hits[0]
        if hit:
            expr = hit[4]
            if ty not in ('u16', 'u8'):
                expr = expr[:-2] if expr.endswith('.0') else f'{ty}({expr})'
            if pure(expr):
                edits.append((hit[0], hit[1], ''))
                vals.append(expr)
            else:
                n = fresh(name, body, taken)
                if n is None:
                    return None
                taken.add(n)
                ind = SET.fullmatch(text[hit[0]:hit[1]]).group(1)
                edits.append((hit[0], hit[1], f'{ind}let {n} = {expr};\n'))
                vals.append(n)
            folded += 1
        else:
            if kind == 'raw':
                vals.append(None)
                continue
            n = fresh(name, body, taken)
            if n is None:
                return None
            taken.add(n)
            rd = f'c.reg8(r8::{reg})' if ty == 'u8' else f'c.reg({reg})'
            if ty not in ('u16', 'u8'):
                rd = f'{ty}({rd})'
            before.append(f'let {n} = {rd};')
            vals.append(n)
    if kind == 'raw' and None in vals:
        return None  # a plain call is named only where it sets all its inputs
    ind = re.match(r'[ \t]*', text[ls:]).group(0)
    pre = ''.join(f'{b}\n{ind}' for b in before)
    mod, name = named[a][0]
    if kind == 'raw':
        if text[ls:s].strip():
            return None  # a raw call is a statement of its own
        if mod + '.rs' == f:
            # A local of the same name hides the fn.
            path_ = f'self::{name}' if re.search(rf'\blet (?:mut )?{name}\b', body) else name
        elif re.search(rf'^\s*(?:pub )?mod {mod}\b', text, re.M):
            path_ = f'super::{mod}::{name}'
        else:
            path_ = f'{mod}::{name}'
        arg = ctx_arg(text, s)
        new = f'{pre}{path_}({arg}, {", ".join(vals)}, {m.group(3)})?;'
        out = text[:s] + new + text[e:]
    else:
        if before and text[ls:s].strip() and not re.fullmatch(r'[ \t]*(?:let [^=]+ = |if !?|while !?)', text[ls:s]):
            return None
        if before and text[ls:s].strip().startswith('while'):
            return None  # the read would not repeat with the loop
        new = m.group(0) + ', '.join(vals) + ', '
        out = text[:ls] + ind + pre + text[ls + len(ind):s] + new + text[e:]
    for st, en, rep in sorted(edits, reverse=True):
        out = out[:st] + rep + out[en:]
    return out


def convert(modules):
    cs, ps, named = consts(), ports(), named_calls()
    clash = [f'{ps[a][0]}.rs has a fn {new} already' for a, (new, _, _) in ARGS.items()
             if new and a not in named and ps.get(a, (None,))[0] in modules
             and re.search(rf'\bfn {new}\b', read(os.path.join(HAND, ps[a][0] + '.rs'))[0])]
    if clash:
        sys.exit('\n'.join(clash))
    write_wrappers(modules, ps, named)
    todo = {a for a in ARGS if ps.get(a, (None,))[0] in modules}
    total = skipped = 0
    for f in sorted(os.listdir(HAND)):
        if not f.endswith('.rs'):
            continue
        path = os.path.join(HAND, f)
        text, crlf = read(path)
        ss = [t for t in call_sites(text, f, cs, named) if t[2] in todo]
        # Named calls in the wrapper definitions themselves are not sites.
        ss = [t for t in ss if not text[text.rfind('\n', 0, t[0]) + 1:t[0]].startswith('pub(super) fn')]
        if not ss:
            continue
        for s, e, a, kind, m in reversed(ss):
            if kind == 'named':
                # Already converted (has its arguments)?
                after = text[e:text.find(')', e)]
                if after.count(',') >= len(ARGS[a][1]):
                    continue
            t = convert_site(text, s, e, a, kind, m, f, ps, named)
            if t is None:
                if kind == 'named':
                    print(f'cannot convert named site {f}:{text.count(chr(10), 0, s) + 1}', file=sys.stderr)
                skipped += 1
            else:
                text, total = t, total + 1
        mods = {ps[a][0] for a in todo} - {f[:-3]}
        for mod in sorted(mods):
            if re.search(rf'(?<!::)\b{mod}::\w+\((?:&mut )?c,', text) and not re.search(
                    rf'^use super::(?:{mod}\b|\{{[^}}]*\b{mod}\b)', text, re.M):
                text = re.sub(r'^(use super::)', rf'use super::{mod};\n\1', text, count=1, flags=re.M)
        write(path, uses_r8(text), crlf)
    print('sites', total, 'left', skipped)


def main():
    args = sys.argv[1:]
    if args and args[0] == '--list':
        list_module(args[1])
    else:
        convert(set(args))


if __name__ == '__main__':
    main()
