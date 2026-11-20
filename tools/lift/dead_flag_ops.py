#!/usr/bin/env python3
"""Drop the flags of single operations no one can see, in any hand port.

    python tools/lift/dead_flag_ops.py [--dry] crates/dune-rec/src/hand/MODULE.rs...

`drop_flags.py` works on whole ports: it converts a port only when none
of its callers read flags and nothing in it can see them. This one works
on each flag-exact operation alone, so it also reaches the ports whose
callers do read flags (their result is often the flags of the last
compare only). For each statement-level `c.cmp(..)`, `c.test(..)`,
`c.add(..)`, ... it follows the code on from it, along every path, with
the set of flags the operation wrote still pending, until each of them
is written again or the path ends:

- the flags are seen (the operation stays) by a `c.flag(X)` of a pending
  X, `adc` / `sbb` / `rcl` / `rcr` (CF), the flags word, an INT or far
  call (`c.interp`), a call or jump to a routine that reads flags on
  entry (`flags_used.py --in`, `--at` for the middle of one), a stop
  (`c.run` / `c.check` at an address the original reads flags from,
  `--at`), the port's `c.ret()` when its callers read a pending flag
  (`hand/flags_read.rs`), a jump or tail whose routine passes the flags
  through to its RET when so, and anything not followed: a helper handed
  the context, `c.m.cpu`, a loop, a `match`, a block that is an
  expression, `break` / `continue`, a loop the operation is in reaching
  its end;
- they are written again by `cmp`, `test`, `add`, `sub`, `and`, `or`,
  `xor`, `neg` (all six), `inc` / `dec` (all but CF), `set_flag(X, ..)`
  (X), `aam` (SF, ZF, PF), or a call to a routine that passes none of the
  flags it is entered with through to its RET.

An `if` is followed into each branch and past the chain. An operation
whose flags no path shows is removed (`c.cmp`, `c.test`) or becomes its
flag-free form (`pure::add(..)` for `c.add(..)`). The shadow check then
compares every changed port with the original.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
_argv, sys.argv = sys.argv, sys.argv[:1]
import drop_flags as df  # noqa: E402  (its tables: masks, entry reads, passes, read_at)
sys.argv = _argv

CF, PF, AF, ZF, SF, OF = 0x001, 0x004, 0x010, 0x040, 0x080, 0x800
ALL = CF | PF | AF | ZF | SF | OF
FLAG_BITS = {'CF': CF, 'PF': PF, 'AF': AF, 'ZF': ZF, 'SF': SF, 'OF': OF}
WRITES = {'cmp': ALL, 'test': ALL, 'add': ALL, 'sub': ALL, 'and': ALL, 'or': ALL, 'xor': ALL,
          'neg': ALL, 'inc': ALL & ~CF, 'dec': ALL & ~CF}
CANDIDATE = re.compile(r'\bc\.(cmp|test|add|sub|and|or|xor|neg|inc|dec)\(')


class Live(Exception):
    pass


# --- Parsing a body into statements and blocks ---------------------------

def tokens(src):
    """Statements (split at `;` outside brackets) and `{` / `}` at bracket
    depth 0, with their offsets in `src`."""
    out, depth, start, i = [], 0, 0, 0
    n = len(src)
    while i < n:
        ch = src[i]
        if ch == '/' and src.startswith('//', i):
            j = src.find('\n', i)
            j = n if j < 0 else j
            if src[start:i].strip() == '':
                start = j
            i = j
            continue
        if ch == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == '\\' else 1
            i = j + 1
            continue
        if ch == "'" and re.match(r"'(\\.|[^\\'])'", src[i:i + 4]):
            i = src.index("'", i + 1) + 1
            continue
        if ch in '([':
            depth += 1
        elif ch in ')]':
            depth -= 1
        elif depth == 0 and ch in ';{}':
            text = src[start:i].strip()
            if ch == ';':
                out.append(('stmt', text + ';', start, i + 1))
            else:
                if text:
                    out.append(('head', text, start, i))
                out.append((ch, ch, i, i + 1))
            start = i + 1
        i += 1
    rest = src[start:].strip()
    if rest:
        out.append(('stmt', rest, start, n))
    return out


class Block:
    def __init__(self, kind, head, parent):
        self.kind, self.head, self.parent, self.items = kind, head, parent, []


def tree(src):
    """The body as nested blocks: items are ('stmt', text, start, end) or
    Block. A block's kind: 'if', 'else', 'loop', 'match', 'other'."""
    root = Block('fn', '', None)
    cur = root
    head = None
    for t in tokens(src):
        if t[0] == 'head':
            head = t[1]
        elif t[0] == '{':
            h = head or ''
            if re.match(r'if\b', h):
                kind = 'if'
            elif re.match(r'else\b', h):
                kind = 'else'
            elif re.match(r'(loop|while|for)\b', h) or re.match(r"'\w+:\s*(loop|while|for)\b", h):
                kind = 'loop'
            elif re.match(r'match\b', h):
                kind = 'match'
            else:
                kind = 'other'
            b = Block(kind, h, cur)
            cur.items.append(b)
            cur = b
            head = None
        elif t[0] == '}':
            if head:  # a block's tail expression
                cur.items.append(('stmt', head, 0, 0))
                head = None
            cur = cur.parent
            if cur is None:
                raise ValueError('unbalanced')
        else:
            if head:
                t = ('stmt', head + ' ' + t[1], t[2], t[3])
                head = None
            cur.items.append(t)
    return root


# --- Following the flags ---------------------------------------------------

class Port:
    def __init__(self, name, entries, consts):
        self.name, self.entries, self.consts = name, entries, consts
        self.mask = 0
        for a in entries:
            self.mask |= df.masks.get(a, ALL)

    def addr(self, t):
        t = t.strip()
        m = re.fullmatch(r'(0x[0-9a-fA-F]+)(?:u16)?', t)
        if m:
            return int(m.group(1), 16)
        m = re.fullmatch(r'(?:code|entry)::(\w+)', t)
        if m and m.group(1) in self.consts:
            return self.consts[m.group(1)]
        if t == 'start' and len(self.entries) == 1:
            return self.entries[0]
        return None


EVENT = re.compile(
    r'c\.(?P<m>ret|jump|call|run|check|stretch|flag|adc|sbb|rcl1|rcr1|interp|set_flag|aam|'
    r'cmp|test|add|sub|and|or|xor|neg|inc|dec)\(|'
    r'(?P<tail>\b\w+)\(c\.m,\s*(?P<tailat>[^)]*)\)|'
    r'(?P<helper>\b\w+)\((?:&mut c\b|c[,)])|'
    r'(?P<cpu>c\.m\.)|'
    r'(?P<flagsword>\bflags\(\)|set_flags)|'
    r'(?P<kw>\b(?:break|continue|Err|Bail)\b|c\.iret\()')


def args_at(text, pos):
    """The argument text of the call whose `(` is at `pos - 1`."""
    depth, i = 1, pos
    while i < len(text) and depth:
        if text[i] in '([':
            depth += 1
        elif text[i] in ')]':
            depth -= 1
        i += 1
    return text[pos:i - 1]


def split_args(a):
    out, depth, cur = [], 0, ''
    for ch in a:
        if ch in '([':
            depth += 1
        elif ch in ')]':
            depth -= 1
        if ch == ',' and depth == 0:
            out.append(cur)
            cur = ''
        else:
            cur += ch
    out.append(cur)
    return [x.strip() for x in out]


# With `--explicit`: the flags the operation's result is seen as (the
# port's return, a `c.flag`), collected instead of stopping there.
SEEN = None


def seen(flags, why):
    if SEEN is None:
        raise Live(why)
    SEEN[0] |= flags


def follow_stmt(port, text, pending, skip=0):
    """The events of a statement from offset `skip`: returns the pending
    flags after it, None when the path ends there (dead on it). Raises
    Live when a pending flag is seen."""
    ends = re.match(r'\s*return\b', text) is not None
    if ends and not EVENT.search(text, skip):
        raise Live('return')  # `return Ok(())` and the like: not followed
    for m in EVENT.finditer(text, skip):
        if pending == 0:
            return None
        if m.group('kw'):
            raise Live('break/continue')
        if m.group('cpu'):
            raise Live('c.m')
        if m.group('flagsword'):
            raise Live('flags word')
        if m.group('tail'):
            name = m.group('tail')
            if name in ('Ctx', 'new'):
                continue
            a = port.addr(m.group('tailat'))
            if a is None:
                raise Live('tail ' + name)
            if df.reads_on_entry(a):
                raise Live(f'tail {a:04x}')
            if df.passes.get(a, a not in df.mid) and port.mask & pending:
                seen(port.mask & pending, f'tail {a:04x}')
            return None
        if m.group('helper'):
            if m.group('helper') in ('Ctx', 'new'):
                continue
            raise Live('helper ' + m.group('helper'))
        meth = m.group('m')
        args = split_args(args_at(text, m.end()))
        if meth == 'ret':
            if port.mask & pending:
                seen(port.mask & pending, 'ret')
            return None
        if meth == 'jump':
            a = port.addr(args[0])
            if a is None:
                raise Live('jump')
            if df.reads_on_entry(a):
                raise Live(f'jump {a:04x}')
            if df.passes.get(a, a not in df.mid) and port.mask & pending:
                seen(port.mask & pending, f'jump {a:04x}')
            return None
        if meth == 'call':
            a = port.addr(args[0])
            if a is None or df.reads_on_entry(a):
                raise Live('call')
            if not df.passes.get(a, True):
                return None
            continue
        if meth in ('run', 'check', 'stretch'):
            a = port.addr(args[0])
            if a is None:
                raise Live('run')
            reads = df.reads_on_entry(a) if a in port.entries else df.read_at.get(a, True)
            if reads:
                raise Live(f'stop {a:04x}')
            continue
        if meth == 'flag':
            f = FLAG_BITS.get(args[0].split('::')[-1])
            if f is not None and f & pending:
                seen(f & pending, 'flag')
            continue
        if meth in ('adc', 'sbb', 'rcl1', 'rcr1'):
            if pending & CF:
                raise Live(meth)
            pending &= ~(ALL if meth in ('adc', 'sbb') else CF)
            continue
        if meth == 'interp':
            raise Live('interp')
        if meth == 'set_flag':
            f = FLAG_BITS.get(args[0].split('::')[-1], 0)
            pending &= ~f
            continue
        if meth == 'aam':
            pending &= ~(SF | ZF | PF)
            continue
        pending &= ~WRITES[meth]
    if pending == 0 or ends:
        return None
    return pending


def follow_items(port, items, i, pending):
    """From item `i` of a block's items on: the pending flags falling out
    of its end, or None when every path ended."""
    while i < len(items):
        it = items[i]
        if isinstance(it, Block):
            if it.kind == 'if':
                # The chain: if, else if, ..., else.
                chain = [it]
                j = i + 1
                while j < len(items) and isinstance(items[j], Block) and items[j].kind == 'else':
                    chain.append(items[j])
                    j += 1
                falls = False
                pend = pending
                for b in chain:
                    # The condition is evaluated before the branch (and
                    # before the next `else if`'s).
                    cond = re.sub(r'^(else\s+)?(if\b)?', '', b.head.strip())
                    if cond.strip():
                        pend = follow_stmt(port, cond, pend)
                        if pend is None:
                            break  # the later branches see nothing
                    r = follow_items(port, b.items, 0, pend)
                    if r is not None:
                        falls = True
                else:
                    if chain[-1].head.strip() != 'else':
                        falls = True  # the path that takes no branch
                if not falls:
                    return None
                i = j
                continue
            raise Live(it.kind)
        r = follow_stmt(port, it[1], pending)
        if r is None:
            return None
        pending = r
        i += 1
    return pending


def dead(port, root, block, idx, op_pos, written):
    """Whether the operation in statement `block.items[idx]` (at offset
    `op_pos` in its text) has flags no path sees."""
    try:
        stmt = block.items[idx][1]
        r = follow_stmt(port, stmt, written, op_pos)
        if r is None:
            return True
        b, i, pending = block, idx + 1, r
        while True:
            r = follow_items(port, b.items, i, pending)
            if r is None:
                return True
            if b.kind not in ('if', 'else'):
                raise Live('end of ' + b.kind)
            # Past the if-chain this block is part of.
            parent = b.parent
            k = parent.items.index(b) + 1
            while k < len(parent.items) and isinstance(parent.items[k], Block) and parent.items[k].kind == 'else':
                k += 1
            b, i, pending = parent, k, r
    except Live as e:
        WHY[re.sub(r' [0-9a-f]{4}$| \w+$', '', str(e))] = WHY.get(re.sub(r' [0-9a-f]{4}$| \w+$', '', str(e)), 0) + 1
        return False


WHY = {}  # why operations stay (`--why`)
EXPLICIT = "--explicit" in sys.argv


def blank_comments(src):
    """`src` with its `//` comments (outside strings) blanked to spaces,
    keeping every offset."""
    out, i, n = list(src), 0, len(src)
    while i < n:
        if src[i] == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == '\\' else 1
            i = j + 1
        elif src.startswith('//', i):
            j = src.find('\n', i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = ' '
            i = j
        else:
            i += 1
    return ''.join(out)


def statements(block):
    for i, it in enumerate(block.items):
        if isinstance(it, Block):
            yield from statements(it)
        else:
            yield block, i, it


def convert(path, dry):
    raw = open(path, 'rb').read()
    crlf = b'\r\n' in raw
    src = raw.decode('utf-8').replace('\r\n', '\n')
    consts = {n: int(v, 16) for n, v in re.findall(r'pub const (\w+): u16 = 0x([0-9a-fA-F]+);', src)}
    consts.update({n: int(v, 16) for n, v in re.findall(
        r'pub const (\w+): u16 = 0x([0-9a-fA-F]+);',
        open(os.path.join(df.REPO, 'crates/dune-rec/src/hand/entry.rs'), encoding='utf-8').read())})
    edits = []  # (start, end, replacement) in src
    report = []
    for m in re.finditer(r'^pub fn (\w+)\(m: &mut Machine, start: u16\)[^\n]*\{\n(.*?)^\}', src, re.M | re.S):
        name, base = m.group(1), m.start(2)
        body = blank_comments(m.group(2))
        entries = df.port_entries(body)
        if not entries or not CANDIDATE.search(body):
            continue
        port = Port(name, entries, consts)
        try:
            root = tree(body)
        except (ValueError, AttributeError):
            report.append((name, 'unparsed'))
            continue
        n = 0
        for block, idx, it in list(statements(root)):
            text, s0 = it[1], it[2]
            ops = list(CANDIDATE.finditer(text))
            if len(ops) != 1 or it[3] == 0:
                continue
            op = ops[0]
            if not dead(port, root, block, idx, op.end(), WRITES[op.group(1)]):
                if EXPLICIT:
                    global SEEN
                    SEEN = [0]
                    ok = dead(port, root, block, idx, op.end(), WRITES[op.group(1)])
                    flags, SEEN = SEEN[0], None
                    if ok and flags and not flags & ~(CF | ZF):
                        print(f'  {name}: {" ".join(k for k, v in FLAG_BITS.items() if v & flags)}: {text}')
                continue
            # The statement's own text in the body (tokens are stripped).
            at = body.index(text.rstrip(';'), s0)
            # A statement that only sets flags (`c.cmp(..);`, or `c.or(W,
            # ax, ax);` whose result is unused) goes, unless its arguments
            # do something.
            if re.fullmatch(r'c\.\w+\((?:(?!\bc\.|\(c\b|&mut c\b).)*\);', text, re.S):
                # The whole statement goes, with its line when alone on it.
                ls = body.rfind('\n', 0, at) + 1
                le = body.find('\n', at)
                le = len(body) if le < 0 else le
                line = body[ls:le]
                if line.strip() == text:
                    edits.append((base + ls, base + le + 1, ''))
                else:
                    end = at + len(text)
                    while end < len(body) and body[end] == ' ':
                        end += 1
                    edits.append((base + at, base + end, ''))
            elif op.group(1) in ('cmp', 'test'):
                continue
            else:
                p = at + op.start()
                edits.append((base + p, base + p + 2, 'pure::'))
            n += 1
        if n:
            report.append((name, f'{n} dropped'))
    if edits and not dry:
        edits.sort()
        out, last = [], 0
        for s, e, r in edits:
            assert s >= last, (path, s, last)
            out.append(src[last:s])
            out.append(r)
            last = e
        out.append(src[last:])
        text = ''.join(out)
        text = df.EMPTY_IF.sub('', text)
        if 'pure::' in text and not re.search(r'^use super::pure;', text, re.M) \
                and not re.search(r'^use super::\{[^}]*\bpure\b', text, re.M):
            text = re.sub(r'^(use super::)', r'use super::pure;\n\1', text, count=1, flags=re.M)
        if crlf:
            text = text.replace('\n', '\r\n')
        open(path, 'wb').write(text.encode('utf-8'))
    return len(edits), report


def main():
    args = sys.argv[1:]
    dry = '--dry' in args
    total = 0
    for p in [a for a in args if not a.startswith("--")]:
        n, report = convert(p, dry)
        total += n
        print(f'{os.path.basename(p)}: {n} operations')
        for name, what in report:
            print(f'  {name}: {what}')
    print('total', total)
    if '--why' in args:
        for k, v in sorted(WHY.items(), key=lambda kv: -kv[1]):
            print(f'{v:6} {k}')


if __name__ == '__main__':
    main()
