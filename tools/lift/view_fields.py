#!/usr/bin/env python3
"""Record fields read raw (`c.byte(si.wrapping_add(0x15))`) in ports whose
doc names the register a troop (or a location), through the view.

    python tools/lift/view_fields.py troop|location
"""
import re, os, sys
sys.path.insert(0, 'tools/lift')
from name_results import HAND, read, write

VIEWS = {
    'troop': ('Troop', re.compile(r'\btroop (?:at )?(DI|SI|BX|BP)\b'), {
        ('byte', 0): 'id', ('byte', 1): 'next', ('byte', 2): 'slot', ('byte', 3): 'state',
        ('byte', 0x10): 'byte_10', ('byte', 0x12): 'byte_12', ('byte', 0x14): 'day', ('byte', 0x15): 'motivation',
        ('byte', 0x19): 'byte_19', ('byte', 0x1a): 'size',
        ('word', 4): 'location', ('word', 6): 'x', ('word', 8): 'y', ('word', 0x0a): 'changed_at',
        ('word', 0x0e): 'word_0e', ('word', 0x10): 'word_10', ('word', 0x12): 'word_12',
        ('set_byte', 1): 'set_next', ('set_byte', 2): 'set_slot', ('set_byte', 3): 'set_state',
        ('set_byte', 0x10): 'set_byte_10', ('set_byte', 0x12): 'set_byte_12', ('set_byte', 0x14): 'set_day',
        ('set_byte', 0x15): 'set_motivation', ('set_byte', 0x19): 'set_byte_19', ('set_byte', 0x1a): 'set_size',
        ('set_word', 4): 'set_location', ('set_word', 0x0a): 'set_changed_at', ('set_word', 0x0e): 'set_word_0e',
        ('set_word', 0x10): 'set_word_10', ('set_word', 0x12): 'set_word_12',
    }),
    'location': ('Location', re.compile(r'\blocation (?:at |record )?(DI|SI|BX|BP)\b'), {
        ('byte', 8): 'kind', ('byte', 0x0a): 'byte_0a', ('byte', 0x0b): 'byte_0b', ('byte', 0x12): 'spice',
        ('byte', 0x10): 'region', ('byte', 9): 'first_troop', ('byte', 0x19): 'byte_19', ('byte', 0x1a): 'byte_1a',
        ('byte', 0x11): 'harvest_per_spice', ('byte', 0x13): 'harvest_left', ('word', 2): 'x', ('word', 4): 'y',
        ('set_byte', 8): 'set_kind', ('set_byte', 0x0a): 'set_byte_0a', ('set_byte', 0x0b): 'set_byte_0b',
        ('set_byte', 0x12): 'set_spice', ('set_byte', 9): 'set_first_troop', ('set_byte', 0x19): 'set_byte_19',
        ('set_byte', 0x13): 'set_harvest_left',
    }),
}
ty, DOC, FIELDS = VIEWS[sys.argv[1]]
ACC = re.compile(r'c\.(set_byte|set_word|byte|word)\((di|si|bx|bp)\.wrapping_add\((0x[0-9a-f]+|\d+)\)(\)|, )')

total = 0
for f in sorted(os.listdir(HAND)):
    if not f.endswith('.rs') or f == 'game.rs':
        continue
    path = os.path.join(HAND, f)
    text, crlf = read(path)
    lines = text.split('\n')
    i = 0
    changed = False
    while i < len(lines):
        if re.match(r'(?:pub(?:\(super\))? )?fn \w+\(', lines[i]):
            j = i - 1
            while j >= 0 and lines[j].startswith('///'):
                j -= 1
            doc = ' '.join(l[3:] for l in lines[j + 1:i])
            regs = {r.lower() for r in DOC.findall(doc)}
            # The troops' jobs and callbacks: DI the troop's location.
            if ty == 'Location' and re.search(r"\bthe location's\b|DI the location\b", doc):
                regs.add('di')
            ctx = 'c' if 'c: &mut Ctx' in lines[i] else '&mut c'
            k = i
            while k < len(lines) and lines[k] != '}':
                k += 1
            if regs:
                body = '\n'.join(lines[i:k + 1])
                # The register's local must be bound once, from the register.
                ok = {r for r in regs if len(re.findall(rf'\blet (?:mut )?{r}\b', body)) == 1
                      and re.search(rf'\blet {r} = c\.reg\({r.upper()}\);', body)}
                for n in range(i, k + 1):
                    if re.search(r'(?:^|[=(,]\s*|\bmove\s+)\|[^|\n]*\|(?!\|)', lines[n]):
                        continue  # closures take their own context

                    def sub(m):
                        op, reg, off, tail = m.groups()
                        name = FIELDS.get((op, int(off, 0)))
                        if reg not in ok or name is None:
                            return m.group(0)
                        if op.startswith('set_'):
                            return f'{ty}({reg}).{name}({ctx}, '
                        return f'{ty}({reg}).{name}({ctx})'
                    new = ACC.sub(sub, lines[n])
                    if new != lines[n]:
                        total += new.count(f'{ty}(') - lines[n].count(f'{ty}(')
                        lines[n] = new
                        changed = True
            i = k
        i += 1
    if changed:
        text = '\n'.join(lines)
        uses = '\n'.join(l for l in text.split('\n') if l.startswith('use '))
        if not re.search(rf'\b{ty}\b', uses):
            text = re.sub(r'^(use super::)', rf'use super::game::{ty};\n\1', text, count=1, flags=re.M)
        write(path, text, crlf)
print('sites', total)
