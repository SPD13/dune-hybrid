#!/usr/bin/env python3
"""Turn the flags a routine answers in into named `bool`s at its callers.

    python tools/lift/name_results.py --list [MODULE]   the call sites that read a result
    python tools/lift/name_results.py MODULE...         convert the routines of MODULE

A routine that answers a question (is the point in the rect? is PCM on?)
answers in CF or ZF, and the hand ports that call it read the flag right
after the call:

    c.call(code::CHECK_PCM_ENABLED, 0xabe3)?;
    let enabled = !c.flag(ZF);

For each routine of MODULE named in `RESULTS` this writes, after its port,
a named call returning the answer as a `bool`

    /// [`check_pcm_enabled`] called: whether PCM voices are on.
    pub(super) fn pcm_enabled(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
        Ok(!c.call_zf(0xae2f, ret)?)
    }

and rewrites each call site in every hand port where a read of that flag
follows before anything can change the flags (a flag operation, another
call, a helper handed the context, a loop opened in between):

    let enabled = sound::pcm_enabled(&mut c, 0xabe3)?;

Rust's scoping keeps the `bool` to the code the call dominates; the shadow
check compares every changed port with the original.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
HAND = os.path.join(REPO, 'crates/dune-rec/src/hand')

# Routine address: (flag, true when the flag is set (else when clear),
# name of the named call, what the answer means).
RESULTS = {
    # sound
    0xae2f: ('ZF', False, 'pcm_enabled', 'whether PCM sound is enabled.'),
    0xabcc: ('ZF', False, 'voc_playing', 'whether a VOC sample plays.'),
    0xaba3: ('ZF', False, 'voice_file_open', 'whether the voice file is open.'),
    0xa814: ('CF', True, 'next_lipsync', 'whether there was another lip-sync byte (AL = it).'),
    0xde0c: ('CF', True, 'midi_wait_cut_short', 'whether the player cut the wait for the music short.'),
    0xa9e7: ('ZF', True, 'pcm_done', 'whether either PCM state is 3 (done).'),
    # voice
    0xaec6: ('CF', True, 'sound_is_off', "whether sound is off (the command line's bit 10h, or music disabled)."),
    0xae28: ('ZF', False, 'music_on', 'whether music is enabled (settings bit 100h).'),
    0xa90b: ('CF', False, 'voice_file_found', 'whether the voice file was found (and its first part read).'),
    # cursor
    0xd6fe: ('CF', True, 'inside_rect', 'whether the point (DX, BX) is strictly inside the rect at DI.'),
    0x9285: ('CF', True, 'on_dialogue_place', "whether the cursor (DX, BX) is on a speaker's place of the dialogue (CX = which)."),
    0x92c9: ('CF', True, 'in_dialogue_rect', 'whether (DX, BX) is in one of the two dialogue rects (CX = its byte).'),
    # dialogue
    0x9ab4: ('CF', True, 'number_part_begins', "whether the sentence's number part begins here."),
    0x9f9e: ('CF', False, 'entry_said', 'whether an entry of the dialogue list was said.'),
    # math
    0xe3b7: ('ZF', True, 'rand_masked_zero', 'whether the random word ANDed with BX is 0 (AX = it).'),
    # files and DOS
    0xf1fb: ('CF', False, 'res_or_file_found', 'whether the resource or file was found (BX the handle, CX:BP the size).'),
    0xf314: ('CF', False, 'res_index_found', 'whether the name has a resource index (AX = it).'),
    0xf2ea: ('CF', True, 'read_short', 'whether fewer bytes were read than asked.'),
    0xf2a7: ('CF', False, 'seeked_to_res', 'whether DUNE.DAT holds the resource (seeked to it, its size in BP:CX).'),
    0xf204: ('CF', False, 'file_opened', 'whether the file opened (BX the handle, CX:BP its size).'),
    # input and frames
    0xdd5a: ('ZF', False, 'key_taken', "whether a key was pressed (AL = its scancode, taken)."),
    0xde54: ('ZF', True, 'esc_hit', 'whether the last key was Esc (taken).'),
    0xdd63: ('CF', True, 'user_input', 'whether the player pressed Esc, a key or a button (else the frame went on).'),
    0xcad4: ('CF', False, 'frame_due', 'whether the next frame is due.'),
    0xddf0: ('CF', True, 'voice_wait_cut_short', 'whether a key or click cut the wait for the voice short.'),
    # video
    0xcdbf: ('CF', False, 'video_bytes_read', 'whether a video file was open to read from.'),
    0xcd8f: ('CF', False, 'header_size_read', 'whether the 2 bytes were read (AX = them).'),
    0xc92b: ('CF', False, 'video_opened', 'whether the video and its header were read.'),
    0xcda0: ('CF', False, 'body_read', "whether the body's first frame was read."),
    0xcc2b: ('CF', False, 'ring_has_room', 'whether CX more bytes fit in the ring buffer.'),
    0xc9e8: ('CF', True, 'frame_interrupted', 'whether a key or click came (taken).'),
    0xcc85: ('ZF', False, 'video_done', 'whether no video plays (its state past 1).'),
    0xc9f4: ('ZF', False, 'frame_advanced', 'whether the video advanced a frame (AX = the frame before).'),
    0xcaa0: ('CF', False, 'frame_decoded', "whether the next frame's video chunk was all read (and so decoded)."),
    # map
    0x62d6: ('CF', True, 'outside_view', 'whether (DX, BX) is outside the view.'),
    0x62c9: ('CF', True, 'record_off_view', 'whether the screen place of the record at SI is outside the view.'),
    0x5d36: ('CF', True, 'location_kind_ok', 'whether the location at DI is of a kind below 28h, or from 28h with bit 8 of byte 0Ah.'),
    0x627e: ('CF', True, 'location_shown', 'whether the location at DI is a place to show.'),
    0x5d1d: ('CF', True, 'point_in_view', 'whether (DX, BX) is in the view.'),
    0x5ed0: ('ZF', True, 'marker_found', "whether a marker's word is DI (BP = its entry)."),
    0x82a0: ('ZF', True, 'map_list_with_40', 'whether the current UI elements are one of the two map lists, with bit 40h of [46EBh].'),
    0x409a: ('ZF', True, 'location_found', "whether a location's word 6 is DI (SI = it)."),
    # globe
    0xbc4e: ('CF', True, 'cursor_on_globe', 'whether the cursor (DX, BX) is on the globe (then made relative to it).'),
    0xbd25: ('CF', True, 'point_on_globe', 'whether the screen point (DX, BX) is on the globe.'),
    # screens and panels
    0xd6b7: ('CF', True, 'cursor_on_element', 'whether the cursor is on an element of the current list (DI = it).'),
    0xd0e3: ('CF', False, 'special_char_found', 'whether AL is one of the 9 special characters (AL, AH then its data).'),
    0xd694: ('ZF', True, 'cursor_has_elements', 'whether the cursor image has a UI element table (DI = it).'),
    0xa672: ('CF', True, 'on_slider', 'whether the cursor is on a slider (SI = it).'),
    0xa69f: ('CF', True, 'on_switch', 'whether the cursor is on a switch (SI = it).'),
    0xa83f: ('CF', True, 'voc_opened', "whether the voice's file was opened."),
    # troops
    0x6906: ('CF', True, 'troop_is_yours', 'whether troop AL is yours (byte 3 below 80h; SI = it).'),
    0x6946: ('CF', True, 'your_particle_at', 'whether the particle under (DX, BX) is of a troop of yours (SI = it, DI = the troop).'),
    0x6917: ('ZF', True, 'troop_particle_found', 'whether troop SI has a particle shown (DI = it).'),
    0x686e: ('CF', False, 'troop_on_screen', 'whether troop SI has a place on the screen (DX, BX).'),
    0x6757: ('CF', False, 'troop_particle_spawned', 'whether troop AX was on screen (its particle then made).'),
    0x7ee2: ('CF', True, 'range_found', 'whether DX lies in one of the 7 ranges at SI (DI = which).'),
    # story
    0x127c: ('CF', True, 'four_in_phases_15_to_1f', 'whether AL is 4 and the phase is 15h..1Fh.'),
    0x2aaf: ('CF', True, 'pending_event_found', 'whether kind AL has a pending event (AX, DI = its words).'),
    0x2ad8: ('CF', True, 'event_told', 'whether the next pending event was told.'),
    0x2017: ('ZF', False, 'sietch_to_call', 'whether there is a sietch to call (DI = it).'),
    0xa1e2: ('ZF', True, 'scripted_action_allowed', 'whether [47A5h] is FFh.'),
    0x5124: ('CF', True, 'already_there', 'whether the map position is the place at DI itself (no direction to it).'),
    0xe851: ('CF', True, 'allocator_below_limit', 'whether [39B9h] + 2F13h is below [0CE68h].'),
    # troops, more
    0x1e24: ('CF', True, 'troop_with_400_here', "whether a troop of the location DI's chain has bit 400h of word 12h."),
    0x1e01: ('CF', True, 'character_at_flagged_troop', 'whether the character record SI is at a location with a troop of bit 400h.'),
    0x6d19: ('CF', True, 'troop_merged', 'whether troop SI merged into another of its location.'),
    # map, more
    0x8256: ('CF', False, 'picks_done', 'whether the third location was picked (else one not taken, or more to pick).'),
    # dialogue and scripts, more
    0xb150: ('ZF', False, 'dialogue_found', 'whether a dialogue entry was found (DI = it).'),
    0xa396: ('ZF', False, 'condition_holds', 'whether condition AX of the CONDIT script holds.'),
}

FLAG_READ = re.compile(r'(!?)c\.flag\((?:pc::cpu::)?(CF|ZF)\)')
CALL = re.compile(r'c\.call\(((?:\w+::)*)(\w+), (0x[0-9a-f]+)\)\?;')
# Anything that may change the flags, or hand them to code not followed.
WRITER = re.compile(
    r'\bc\.(?!run\b|reg\b|reg8\b|sreg\b|set_reg\b|set_reg8\b|set_sreg\b|byte\b|word\b|'
    r'set_byte\b|set_word\b|push\b|pop\b|below_sp\b|flag\b|ret\b|\w+_state\b|'
    r'voice_buffer\b|es_byte\b|es_word\b|set_es_byte\b|set_es_word\b|seg_byte\b|seg_word\b|'
    r'set_seg_byte\b|set_seg_word\b)\w+|\(c\b|&mut c\b|\bc\.m\b|\bc,')
LOOP = re.compile(r'^\s*(?:\'\w+: )?(?:loop|while|for)\b')


def read(path):
    raw = open(path, 'rb').read()
    return raw.decode('utf-8').replace('\r\n', '\n'), b'\r\n' in raw


def write(path, text, crlf):
    if crlf:
        text = text.replace('\n', '\r\n')
    open(path, 'wb').write(text.encode('utf-8'))


def consts():
    """Constant name -> address, from entry.rs and each module's own."""
    out = {}
    for f in os.listdir(HAND):
        if f.endswith('.rs'):
            for n, v in re.findall(r'pub const (\w+): u16 = 0x([0-9a-fA-F]+);', read(os.path.join(HAND, f))[0]):
                out.setdefault((f, n), int(v, 16))
                out.setdefault((None, n), int(v, 16))
    return out


def ports():
    """Address -> (module, port fn)."""
    t = read(os.path.join(HAND, 'mod.rs'))[0]
    return {int(a, 16): (m, f) for a, m, f in re.findall(r'\(0x([0-9a-f]+), (\w+)::(\w+),', t)}


def sites(text, fname, cs):
    """(call match, address, [(read match start, end, negated, flag)], ctx arg)
    for each call whose result flags are read before anything changes them."""
    out = []
    lines_at = [0]
    for m in CALL.finditer(text):
        name = m.group(2)
        addr = cs.get((fname, name), cs.get((None, name)))
        if addr is None:
            continue
        reads = []
        pos = m.end()
        depth = 0
        while pos < len(text):
            nl = text.find('\n', pos)
            nl = len(text) if nl < 0 else nl
            line = text[pos:nl]
            code = re.sub(r'//.*', '', line)
            if LOOP.match(code) and depth >= 0:
                break
            # The reads on this line come before any writer on it when the
            # line is an `if`/`let` test of them.
            rs = list(FLAG_READ.finditer(code))
            w = WRITER.search(FLAG_READ.sub('', code))
            for r in rs:
                if w and w.start() < r.start():
                    break
                reads.append((pos + r.start(), pos + r.end(), r.group(1) == '!', r.group(2)))
            if w:
                break
            if code.strip().startswith('}') and depth == 0:
                break  # the end of the call's block
            depth += code.count('{') - code.count('}')
            pos = nl + 1
        if reads:
            out.append((m, addr, reads, ctx_arg(text, m.start())))
    return out


def ctx_arg(text, at):
    """`c` inside a helper handed the context, `&mut c` in a port."""
    fn = max(text.rfind('\nfn ', 0, at), text.rfind('\npub fn ', 0, at), text.rfind('\npub(super) fn ', 0, at))
    head = text[fn:text.find('{', fn)]
    if 'c: &mut Ctx' in head:
        return 'c'
    # Inside a closure handed the context?
    for cm in re.finditer(r'\|c(?:: &mut Ctx)?\|[^{\n]*\{', text[fn:at]):
        s = fn + cm.end()
        depth, i = 1, s
        while i < at and depth:
            depth += {'{': 1, '}': -1}.get(text[i], 0)
            i += 1
        if depth:
            return 'c'
    return '&mut c'


def list_sites(module):
    cs, ps = consts(), ports()
    for f in sorted(os.listdir(HAND)):
        if not f.endswith('.rs'):
            continue
        text = read(os.path.join(HAND, f))[0]
        for m, addr, reads, arg in sites(text, f, cs):
            mod = ps.get(addr, ('?', '?'))
            if module and mod[0] != module:
                continue
            line = text.count('\n', 0, m.start()) + 1
            flags = ' '.join(('!' if n else '') + fl for _, _, n, fl in reads)
            print(f'{addr:04x} {mod[0]}::{mod[1]:40} {f}:{line}  {flags}')


def convert(modules):
    cs, ps = consts(), ports()
    todo = {a: r for a, r in RESULTS.items() if ps.get(a, (None,))[0] in modules}
    # The named calls, after their ports.
    for module in modules:
        path = os.path.join(HAND, module + '.rs')
        text, crlf = read(path)
        for a, (flag, when_set, name, doc) in sorted(todo.items()):
            mod, port = ps[a]
            if mod != module or re.search(rf'\bfn {name}\(', text):
                continue
            m = re.search(rf'^pub fn {port}\(.*?^\}}\n', text, re.M | re.S)
            # The port's own answer: `c.set_flag(F, x); c.ret()` as `c.ret_f(x)`.
            body = re.sub(
                rf'c\.set_flag\((?:pc::cpu::)?{flag}, ([^;\n]*)\);\n\s*(return )?c\.ret\(\)',
                lambda r: f'{r.group(2) or ""}c.ret_{flag.lower()}({r.group(1)})', m.group(0))
            call = f'c.call_{flag.lower()}(0x{a:04x}, ret)'
            body_ = call if when_set else f'Ok(!{call}?)'
            fn = (f'\n/// [`{port}`] called: {doc}\n'
                  f'pub(super) fn {name}(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {{\n'
                  f'    {body_}\n}}\n')
            text = text[:m.start()] + body + fn + text[m.end():]
        write(path, text, crlf)
    # The call sites.
    total = 0
    for f in sorted(os.listdir(HAND)):
        if not f.endswith('.rs'):
            continue
        path = os.path.join(HAND, f)
        text, crlf = read(path)
        edits = []
        for m, addr, reads, arg in sites(text, f, cs):
            if addr not in todo:
                continue
            flag, when_set, name, _ = todo[addr]
            if any(fl != flag for _, _, _, fl in reads):
                continue
            mod = ps[addr][0]
            if mod + '.rs' == f:
                path_ = name
            elif re.search(rf'^\s*(?:pub )?mod {mod}\b', text, re.M):
                path_ = f'super::{mod}::{name}'  # a local module of that name
            else:
                path_ = f'{mod}::{name}'
            call = f'{path_}({arg}, {m.group(3)})?'
            total += 1
            # One read, on the next line, as all of `if <flag> {` or of
            # `let x = <flag>;`: the call goes there.
            if len(reads) == 1:
                s, e, neg, _ = reads[0]
                ls = text.rfind('\n', 0, s) + 1
                le = text.find('\n', e)
                between = text[m.end():ls]
                rest = text[e:le]
                if between.strip() == '' and (
                        re.fullmatch(r'\s*(?:\} else )?if ', text[ls:s]) and rest == ' {'
                        or re.fullmatch(r'\s*let \w+ = ', text[ls:s]) and rest == ';'):
                    edits.append((text.rfind('\n', 0, m.start()) + 1, ls, ''))
                    edits.append((s, e, f'!{call}' if neg == when_set else call))
                    continue
            var = name
            edits.append((m.start(), m.end(), f'let {var} = {call};'))
            for s, e, neg, _ in reads:
                edits.append((s, e, f'!{var}' if neg == when_set else var))
        if not edits:
            continue
        edits.sort()
        out, last = [], 0
        for s, e, r in edits:
            out.append(text[last:s])
            out.append(r)
            last = e
        out.append(text[last:])
        text = rename_flag_locals(''.join(out), {r[2] for r in todo.values()})
        mods ={ps[a][0] for a in todo} - {f[:-3]}
        for mod in sorted(mods):
            if re.search(rf'(?<!::)\b{mod}::\w+\((?:&mut )?c,', text) and not re.search(
                    rf'^use super::(?:{mod}\b|\{{[^}}]*\b{mod}\b)', text, re.M):
                text = re.sub(r'^(use super::)', rf'use super::{mod};\n\1', text, count=1, flags=re.M)
        write(path, text, crlf)
    print('sites', total)


FLAG_LOCAL = re.compile(r'^([ \t]*)let (cf|zf|z) = (!?)((?:\w+::)*(\w+))(\((?:&mut )?c, 0x[0-9a-f]+\)\?;)$', re.M)


def rename_flag_locals(text, names):
    """`let cf = name(..)?;` and the uses of `cf` after it, in its block,
    as `name` (or `!name`)."""
    while True:
        for m in FLAG_LOCAL.finditer(text):
            if m.group(5) in names:
                break
        else:
            return text
        var, neg, name = m.group(2), m.group(3) == '!', m.group(5)
        # The rest of the block: up to the brace that closes it.
        depth, i = 0, m.end()
        while i < len(text):
            ch = text[i]
            if ch == '{':
                depth += 1
            elif ch == '}':
                if depth == 0:
                    break
                depth -= 1
            i += 1
        rest = text[m.end():i]
        rebind = re.search(rf'\blet (?:mut )?{var}\b', rest)
        end = rebind.start() if rebind else len(rest)

        def use(u):
            bang = u.group(1) == '!'
            return name if bang == neg else f'!{name}'
        body = re.sub(rf'(!?)(?<![.\w]){var}\b(?!\s*:)', use, rest[:end])
        head = f'{m.group(1)}let {name} = {m.group(4)}{m.group(6)}'
        text = text[:m.start()] + head + body + rest[end:] + text[i:]


def main():
    args = sys.argv[1:]
    if args and args[0] == '--list':
        list_sites(args[1] if len(args) > 1 else None)
    else:
        convert(set(args))


if __name__ == '__main__':
    main()
