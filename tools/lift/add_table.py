"""add_table.py SPEC.json: a table of one record added to the record tables
(`hand/record_table.rs`, after the last one), its field on `Native`, its
two `Ctx` methods, its name in `suite.sh`'s gate, and the ports' plain
accesses to it routed through the view.

The typed view (`hand/<field>.rs`, implementing `record_table::Single`)
is written by hand first. SPEC (JSON):
  variant  the `Which` variant (CamelCase)
  name     its name in the output ("map view")
  base     its offset in the data segment ("0x46e3")
  size     its size ("0x0c")
  field    the module and `Native` field (snake_case)
  type     the view's type
  doc      the variant's doc line
  ctxdoc   what the `Ctx` methods' docs call it
  fields   {address or globals name: [field expression, "word"|"byte"]}

Only `c.word(X)`, `c.byte(X)`, `c.set_word(X, v);` and `c.set_byte(X, v);`
are routed; accesses of a different width, through pointers, or to
halves of 32-bit fields are left for hand edits (grep for them after).
Each file's own line endings are kept."""
import json
import os
import re
import sys

spec = json.load(open(sys.argv[1]))
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
os.chdir(os.path.join(REPO, 'crates', 'dune-rec', 'src', 'hand'))


def load(p):
    b = open(p, 'rb').read().decode('utf-8')
    crlf = '\r\n' in b
    return b.replace('\r\n', '\n'), crlf


def save(p, s, crlf):
    open(p, 'wb').write((s.replace('\n', '\r\n') if crlf else s).encode('utf-8'))


def rep(p, pairs):
    s, crlf = load(p)
    for a, b in pairs:
        assert s.count(a) == 1, (p, a)
        s = s.replace(a, b)
    save(p, s, crlf)


V, fld, ty = spec['variant'], spec['field'], spec['type']

# record_table.rs: the variant after the last one (P), in each match.
s, crlf = load('record_table.rs')
P = re.search(r'    (\w+),\n}\n\nimpl Which', s).group(1)
Pf = re.search(r'        Which::%s => &mut n\.(\w+),' % P, s).group(1)
nall = int(re.search(r'ALL: \[Which; (\d+)\]', s).group(1))
# The name, base, size and count arms, in that order.
arms = [m for m in re.finditer(r'            Which::%s => ([^\n]*),\n' % P, s) if '&mut' not in m.group(1)]
assert len(arms) == 4, len(arms)
off = 0
for m, v in zip(arms, ['"%s"' % spec['name'], spec['base'], spec['size'], '1']):
    ins = '            Which::%s => %s,\n' % (V, v)
    s = s[:m.end() + off] + ins + s[m.end() + off:]
    off += len(ins)
save('record_table.rs', s, crlf)
rep('record_table.rs', [
    ("    %s,\n}\n\nimpl Which" % P, "    %s,\n    /// %s\n    %s,\n}\n\nimpl Which" % (P, spec['doc'], V)),
    ("ALL: [Which; %d]" % nall, "ALL: [Which; %d]" % (nall + 1)),
    ("        Which::%s,\n    ];" % P, "        Which::%s,\n        Which::%s,\n    ];" % (P, V)),
    ("        Which::%s => &mut n.%s," % (P, Pf), "        Which::%s => &mut n.%s,\n        Which::%s => &mut n.%s," % (P, Pf, V, fld)),
])

rep('native.rs', [("    pub %s: super::record_table::RecordTable,\n" % Pf,
                   "    pub %s: super::record_table::RecordTable,\n    pub %s: super::record_table::RecordTable,\n" % (Pf, fld))])

s, crlf = load('mod.rs')
m = re.search(r'pub mod %s;\n' % Pf, s)
assert m
s = s[:m.end()] + 'pub mod %s;\n' % fld + s[m.end():]
# After the last table's `update_` method (`update_single` or `update_nth`).
m = list(re.finditer(r'        record_table::update_(?:single|nth)\(self[^\n]*\n    \}\n', s))[-1]
add = """    /// %s (Rust state, see [`%s`]), in step with the memory.
    fn %s(&mut self) -> %s::%s {
        record_table::single(self)
    }
    /// %s changed by `f` (written through).
    fn update_%s(&mut self, f: impl FnOnce(&mut %s::%s)) {
        record_table::update_single(self, f)
    }
""" % (spec['ctxdoc'], fld, fld, fld, ty, spec['ctxdoc'], fld, fld, ty)
s = s[:m.end()] + add + s[m.end():]
save('mod.rs', s, crlf)


def setter(fl, value):
    # The closure's parameter must not shadow a name in the value.
    t = 'v' if re.search(r'\bt\b', value) else 't'
    return 'c.update_%s(|%s| %s.%s = %s);' % (fld, t, t, fl, value)


n = 0
skip = ('flags_read.rs', 'globals.rs', 'record_table.rs', fld + '.rs')
for f in [x for x in os.listdir('.') if x.endswith('.rs') and x not in skip]:
    s, crlf = load(f)
    n0 = s
    for k, (fl, kind) in spec['fields'].items():
        ek = r'(?:g::|super::globals::)?' + re.escape(k)
        s, a1 = re.subn(r'\bc\.' + kind + r'\(' + ek + r'\)', 'c.%s().%s' % (fld, fl), s)
        s, a2 = re.subn(r'\bc\.set_' + kind + r'\(' + ek + r', ([^;\n]*)\);', lambda m: setter(fl, m.group(1)), s)
        n += a1 + a2
    if s != n0:
        save(f, s, crlf)

p = os.path.join(REPO, 'tools', 'lift', 'suite.sh')
b = open(p, 'rb').read().decode()
m = re.search(r'(for t in troops locations [^;]*); do', b)
assert m
b = b[:m.end(1)] + ' "%s"' % spec['name'] + b[m.end(1):]
open(p, 'wb').write(b.encode())
print('routed', n)
