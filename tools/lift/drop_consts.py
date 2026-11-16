"""drop_consts.py NAME...: remove `pub const NAME` lines from hand/globals.rs
(a doc comment right above goes too unless the next line is another const
it documents), and turn [`g::NAME`] doc links elsewhere into plain text."""
import os, re, sys
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
os.chdir(os.path.join(REPO, 'crates', 'dune-rec', 'src', 'hand'))
names = sys.argv[1:]


def load(p):
    b = open(p, 'rb').read().decode('utf-8')
    crlf = '\r\n' in b
    return b.replace('\r\n', '\n'), crlf


def save(p, s, crlf):
    open(p, 'wb').write((s.replace('\n', '\r\n') if crlf else s).encode('utf-8'))


s, crlf = load('globals.rs')
lines = s.split('\n')
out = []
i = 0
while i < len(lines):
    m = re.match(r'pub const (\w+): u16 = ', lines[i])
    if m and m.group(1) in names:
        # Its doc block (just above) is kept when the next line is another
        # const that is kept (the doc covers a group).
        j = len(out)
        while j > 0 and out[j - 1].startswith('///'):
            j -= 1
        nxt = lines[i + 1] if i + 1 < len(lines) else ''
        nm = re.match(r'pub const (\w+): u16 = ', nxt)
        if j < len(out) and not (nm and nm.group(1) not in names):
            del out[j:]
        i += 1
        continue
    out.append(lines[i])
    i += 1
save('globals.rs', '\n'.join(out), crlf)

for f in os.listdir('.'):
    if not f.endswith('.rs') or f == 'globals.rs':
        continue
    s, crlf = load(f)
    n0 = s
    for n in names:
        s = re.sub(r' \(\[`g::%s`\]\)' % n, '', s)
        s = re.sub(r'\[`g::%s`\]' % n, n, s)
    if s != n0:
        save(f, s, crlf)
