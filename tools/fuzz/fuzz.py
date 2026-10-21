#!/usr/bin/env python3
"""Generates a random Java program in the subset the translator supports, from a seed.

The same seed always gives the same program. Every program terminates: loops count up to a
small bound, and recursion counts down from a depth of at most 4. A program throws only on
purpose, through the static field `zero0`, which is never assigned. Each statement is one
line and a block opens on a line ending in `{`, so the reducer can delete whole statements.

    python3 fuzz.py SEED [SIZE] [--strings] [--arrays]     # prints the program for SEED
"""

import random
import sys

from fuzz_arrays import ARRAY_NESTING, ARRAY_STATEMENTS, ArrayGen
from fuzz_strings import STRING_NESTING, STRING_STATEMENTS, StringGen

NUMERIC = ['byte', 'short', 'char', 'int', 'long', 'float', 'double']
INTEGRAL = ['byte', 'short', 'char', 'int', 'long']
SMALL = ['byte', 'short', 'char']
PRIMS = NUMERIC + ['boolean']
ARRAY_ELEMS = ['int', 'long', 'double', 'float', 'char', 'boolean', 'byte', 'short']
SCALAR_STATICS = [
    ('int', 'sInt'), ('long', 'sLong'), ('float', 'sFloat'), ('double', 'sDouble'),
    ('char', 'sChar'), ('boolean', 'sBoolean'), ('byte', 'sByte'), ('short', 'sShort'),
]

# Assignment contexts: the types whose values may be assigned to each type (widening only).
WIDENS = {
    'byte': ['short', 'int', 'long', 'float', 'double'],
    'short': ['int', 'long', 'float', 'double'],
    'char': ['int', 'long', 'float', 'double'],
    'int': ['long', 'float', 'double'],
    'long': ['float', 'double'],
    'float': ['double'],
}
SOURCES = {t: [s for s in WIDENS if t in WIDENS[s]] + [t] for t in PRIMS}

# Operand types of a binary operation whose binary numeric promotion is the key.
OPERANDS = {
    'int': ['byte', 'short', 'char', 'int'],
    'long': ['byte', 'short', 'char', 'int', 'long'],
    'float': ['byte', 'short', 'char', 'int', 'long', 'float'],
    'double': NUMERIC,
}

LITERALS = {
    'byte': ['(byte)1', '(byte)-3', '(byte)127', '(byte)-128'],
    'short': ['(short)1', '(short)-300', '(short)32767', '(short)-32768'],
    'char': ["'a'", "'Z'", "'0'", "' '", "'~'", "'\\t'", "'\\\\'", "'\\u0041'"],
    'int': ['0', '1', '2', '3', '7', '31', '32', '33', '255', '65535', '1000000',
            '2147483647', '(-2147483647 - 1)', '-1', '-5', '123456789'],
    'long': ['0L', '1L', '4294967296L', '9223372036854775807L', '(-9223372036854775807L - 1L)',
             '-1L', '1234567890123L', '63L', '64L'],
    'float': ['0.0f', '1.5f', '-0.1f', '3.4e38f', '1.0e-45f', '-0.0f', '16777217f'],
    'double': ['0.0', '-0.0', '1.5', '-2.25', '0.1', '1e10', '1e300', '4.9e-324',
               '(0.0 / 0.0)', '(1.0 / 0.0)', '100.0'],
    'boolean': ['true', 'false'],
}

# Int constants that each small type can hold, for conditionals of that type.
SMALL_CONSTANTS = {
    'byte': ['0', '7', '-3', '127', '-128'],
    'short': ['0', '300', '-300', '32767', '-32768'],
    'char': ['0', '65', '97', '65535'],
}

# Statement kinds with their weights. The last three nest blocks and need a depth to spare.
STATEMENTS = {
    'decl': 6, 'assign': 8, 'incr': 2, 'store': 4, 'array': 3, 'print': 8, 'acc': 2,
    'if': 5, 'loop': 4, 'switch': 2, 'jump': 3, 'vcall': 2, 'ret': 2,
}
NESTING = {'if', 'loop', 'switch'}


def generate(seed, size=30, strings=False, arrays=False):
    """Returns the source of `public class Prog` for `seed`, with `size` statements in main.

    With `strings`, the programs also use String, StringBuilder, String.format and Arrays
    (see fuzz_strings.py); with `arrays`, arrays of every depth and their aliases, moves, rows
    and loops (see fuzz_arrays.py). Without either, the programs are the same as before those
    options existed.
    """
    return Gen(seed, size, strings, arrays).program()


class Gen(StringGen, ArrayGen):
    def __init__(self, seed, size, strings=False, arrays=False):
        self.r = random.Random(seed)
        self.size = size
        self.strings = strings
        self.arrays = arrays
        self.kinds = {**STATEMENTS, **STRING_STATEMENTS} if strings else STATEMENTS
        if arrays:
            self.kinds = {**self.kinds, **ARRAY_STATEMENTS}
        self.nesting = NESTING | STRING_NESTING | ARRAY_NESTING
        self.pinned = set()  # arrays that a loop walks: no statement may assign them
        self.array_methods = []
        self.count = 0
        self.out = []
        self.indent = 0
        self.statics = []  # (name, type, writable)
        self.methods = []  # dicts: name, ret (None for void), params, recursive
        self.frames = [[]]  # local scopes, innermost last: (name, type, writable)
        self.loops = []  # enclosing loops and switches: ('loop', label) or ('switch', None)
        self.method = None  # the method being generated, None in main
        self.ret = None
        self.self_ok = False  # the method being generated may call itself
        self.self_sites = 0
        self.throw_at = self.r.randrange(size) if self.r.random() < 0.15 else -1
        if arrays and self.throw_at < 0 and self.r.random() < 0.2:
            self.throw_at = self.r.randrange(size)

    # Helpers.

    def emit(self, text):
        self.out.append('    ' * self.indent + text)

    def name(self, prefix):
        self.count += 1
        return f'{prefix}{self.count}'

    def pick(self, items):
        return self.r.choice(items)

    def chance(self, p):
        return self.r.random() < p

    def all_vars(self):
        return [v for f in self.frames for v in f] + self.statics

    def visible(self, typ, writable=False):
        return [n for n, t, w in self.all_vars()
                if t == typ and (w or not writable) and not (writable and n in self.pinned)]

    def literal(self, t):
        return self.pick(LITERALS[t])

    def guarded(self, cond, text):
        self.emit(f'if ({cond}) {{')
        self.indent += 1
        self.emit(text)
        self.indent -= 1
        self.emit('}')

    def callables(self, ret):
        options = [m for m in self.methods if m['ret'] == ret]
        if self.self_ok and self.method['ret'] == ret and self.self_sites < 2 and not self.loops:
            options.append(self.method)
        return options

    def args(self, m, d, selfcall):
        if selfcall:
            self.self_sites += 1
        out = []
        for i, (_, t, _) in enumerate(m['params']):
            if i == 0 and m['recursive']:
                if selfcall:
                    out.append('(d - 1)')
                else:
                    out.append(f'Math.floorMod({self.expr("int", d)}, 5)')
            elif t.endswith('[]'):
                names = self.visible(t)
                out.append(self.pick(names) if names else f'new {t} {{{self.literal(t[:-2])}}}')
            else:
                out.append(self.source(t, d))
        return ', '.join(out)

    # Expressions. Each returns Java text of exactly the requested type, or None when the
    # kind is not possible here; the caller then falls back to a leaf.

    def expr(self, t, d):
        if t == 'boolean':
            return self.bool_expr(d)
        if t in SMALL:
            return self.small_expr(t, d)
        return self.num_expr(t, d)

    def source(self, t, d):
        """An expression that may be assigned to `t`: of that type or one widening to it."""
        return self.expr(self.pick(SOURCES[t]), d)

    def num_leaf(self, t, d=0):
        if self.strings and d > 0 and t in NUMERIC and self.chance(0.15):
            return self.string_num(t, d)
        if self.arrays and d > 0 and self.chance(0.15):
            text = self.array_num(t, d)
            if text:
                return text
        names = self.visible(t)
        if names and self.chance(0.6):
            return self.pick(names)
        return self.literal(t)

    def num_expr(self, t, d):
        if d <= 0 or self.chance(0.2):
            return self.num_leaf(t, d)
        kind = self.pick(['arith', 'arith', 'bits', 'unary', 'cast', 'ternary', 'math', 'side',
                          'element', 'call', 'length'])
        if kind == 'bits' and t in ('float', 'double'):
            kind = 'arith'
        if kind == 'length' and t != 'int':
            kind = 'arith'
        text = None
        if kind == 'arith':
            text = self.arith(t, d)
        elif kind == 'bits':
            text = self.bits(t, d)
        elif kind == 'unary':
            ops = ['-', '+'] + (['~'] if t in INTEGRAL else [])
            text = f'({self.pick(ops)}({self.expr(t, d - 1)}))'
        elif kind == 'cast':
            text = f'(({t})({self.expr(self.pick(NUMERIC), d - 1)}))'
        elif kind == 'ternary':
            # Mixed branches have the promoted type, or the small type of the other branch where
            # one is an int constant that it holds; either way a value of type `t`.
            a, b = self.operands(t) if self.chance(0.4) else (t, t)
            text = f'({self.bool_expr(d - 1)} ? {self.expr(a, d - 1)} : {self.expr(b, d - 1)})'
        elif kind == 'math':
            if self.chance(0.3):
                text = f'Math.abs({self.expr(t, d - 1)})'
            else:
                fn = self.pick(['min', 'max'])
                text = f'Math.{fn}({self.expr(t, d - 1)}, {self.expr(t, d - 1)})'
        elif kind == 'side':
            text = self.side(t, d)
        elif kind == 'element':
            text = self.element(t, d)
        elif kind == 'call':
            text = self.call(t, d)
        elif kind == 'length':
            text = self.length()
        return text or self.num_leaf(t, d)

    def small_expr(self, t, d):
        if d <= 0 or self.chance(0.25):
            return self.num_leaf(t, d)
        kind = self.pick(['cast', 'cast', 'ternary', 'side', 'element', 'call'])
        text = None
        if kind == 'cast':
            text = f'(({t})({self.expr(self.pick(NUMERIC), d - 1)}))'
        elif kind == 'ternary':
            a, b = self.expr(t, d - 1), self.expr(t, d - 1)
            if self.chance(0.4):
                # An int constant that `t` holds keeps the conditional of type `t`.
                b = self.pick(SMALL_CONSTANTS[t])
                a, b = (b, a) if self.chance(0.5) else (a, b)
            text = f'({self.bool_expr(d - 1)} ? {a} : {b})'
        elif kind == 'side':
            text = self.side(t, d)
        elif kind == 'element':
            text = self.element(t, d)
        elif kind == 'call':
            text = self.call(t, d)
        return text or self.num_leaf(t, d)

    def bool_expr(self, d):
        if d <= 0 or self.chance(0.2):
            names = self.visible('boolean')
            if names and self.chance(0.6):
                return self.pick(names)
            return self.literal('boolean')
        if self.strings and self.chance(0.15):
            return self.string_bool(d)
        kind = self.pick(['compare', 'compare', 'logic', 'not', 'ternary', 'side', 'element',
                          'call'])
        text = None
        if kind == 'compare':
            op = self.pick(['<', '>', '<=', '>=', '==', '!='])
            lhs = self.expr(self.pick(NUMERIC), d - 1)
            rhs = self.expr(self.pick(NUMERIC), d - 1)
            text = f'({lhs} {op} {rhs})'
        elif kind == 'logic':
            op = self.pick(['&&', '||', '&', '|', '^', '==', '!='])
            text = f'({self.bool_expr(d - 1)} {op} {self.bool_expr(d - 1)})'
        elif kind == 'not':
            text = f'(!({self.bool_expr(d - 1)}))'
        elif kind == 'ternary':
            text = (f'({self.bool_expr(d - 1)} ? {self.bool_expr(d - 1)} : '
                    f'{self.bool_expr(d - 1)})')
        elif kind == 'side':
            text = self.side('boolean', d)
        elif kind == 'element':
            text = self.element('boolean', d)
        elif kind == 'call':
            text = self.call('boolean', d)
        return text or self.literal('boolean')

    def arith(self, t, d):
        a, b = self.operands(t)
        op = self.pick(['+', '-', '*', '/', '%'])
        rhs = self.expr(b, d - 1)
        if op in ('/', '%') and b in INTEGRAL:
            rhs = f'({rhs} | 1)'
        return f'({self.expr(a, d - 1)} {op} {rhs})'

    def bits(self, t, d):
        if self.chance(0.5):
            left = t if t == 'long' else self.pick(['byte', 'short', 'char', 'int'])
            op = self.pick(['<<', '>>', '>>>'])
            count = self.expr(self.pick(INTEGRAL), d - 1)
            return f'({self.expr(left, d - 1)} {op} {count})'
        a, b = self.operands(t)
        op = self.pick(['&', '|', '^'])
        return f'({self.expr(a, d - 1)} {op} {self.expr(b, d - 1)})'

    def operands(self, t):
        """Two operand types whose binary numeric promotion is `t`."""
        a = self.pick(OPERANDS[t])
        b = self.pick(OPERANDS[t])
        if t not in (a, b) and t != 'int':
            a = t
        return a, b

    def side(self, t, d):
        """An assignment or increment of a writable variable or an array element, as an
        expression."""
        arrays = self.visible(t + '[]')
        if arrays and self.chance(0.3):
            a = self.pick(arrays)
            n = f'{a}[Math.floorMod({self.expr("int", d - 1)}, {a}.length)]'
        else:
            names = self.visible(t, True)
            if not names:
                return None
            n = self.pick(names)
        if t != 'boolean' and self.chance(0.4):
            op = self.pick(['++', '--'])
            return f'({op}{n})' if self.chance(0.5) else f'({n}{op})'
        if self.chance(0.2):
            return f'({n} = {self.source(t, d - 1)})'
        return f'({self.compound(n, t, d - 1)})'

    def compound(self, target, t, d):
        """A compound assignment to `target` (a variable or an element), without the `;`."""
        if t == 'boolean':
            op = self.pick(['&=', '|=', '^='])
            return f'{target} {op} {self.expr("boolean", d - 1)}'
        if t in ('float', 'double'):
            op = self.pick(['+=', '-=', '*=', '/=', '%='])
            rt = self.pick(NUMERIC)
        else:
            op = self.pick(['+=', '-=', '*=', '/=', '%=', '&=', '|=', '^=', '<<=', '>>=', '>>>='])
            rt = self.pick(INTEGRAL) if op in ('&=', '|=', '^=', '<<=', '>>=', '>>>=') \
                else self.pick(NUMERIC)
        rhs = self.expr(rt, d - 1)
        if op in ('/=', '%=') and t in INTEGRAL and rt in INTEGRAL:
            rhs = f'({rhs} | 1)'
        return f'{target} {op} {rhs}'

    def element(self, t, d):
        names = self.visible(t + '[]')
        if not names:
            return None
        a = self.pick(names)
        return f'{a}[Math.floorMod({self.expr("int", d - 1)}, {a}.length)]'

    def length(self):
        names = self.visible(self.pick(ARRAY_ELEMS) + '[]')
        return f'{self.pick(names)}.length' if names else None

    def call(self, t, d):
        options = self.callables(t)
        if not options:
            return None
        m = self.pick(options)
        return f'{m["name"]}({self.args(m, d - 1, m is self.method)})'

    # Statements. Each emits lines at the current indent.

    def stmt(self, depth):
        kinds = [k for k in self.kinds if depth > 0 or k not in self.nesting]
        kind = self.r.choices(kinds, [self.kinds[k] for k in kinds])[0]
        getattr(self, 'st_' + kind)(depth)

    def block(self, depth, ctx=None, extra=(), head=None):
        """Emits a block body one level deeper, in a new scope. `head` is emitted first."""
        self.frames.append(list(extra))
        if ctx:
            self.loops.append(ctx)
        self.indent += 1
        if head:
            self.emit(head)
        for _ in range(self.r.randint(1, 3)):
            self.stmt(depth - 1)
        self.indent -= 1
        if ctx:
            self.loops.pop()
        self.frames.pop()

    def st_decl(self, depth):
        t = self.pick(PRIMS)
        v = self.name('v')
        self.emit(f'{t} {v} = {self.source(t, 3)};')
        self.frames[-1].append((v, t, True))

    def st_array(self, depth):
        t = self.pick(ARRAY_ELEMS)
        at = t + '[]'
        names = self.visible(at, True)
        if self.chance(0.5) and names:
            name = self.pick(names)
            self.emit(f'{name} = {self.array_init(t)};')
        else:
            name = self.name('a')
            self.emit(f'{at} {name} = {self.array_init(t)};')
            self.frames[-1].append((name, at, True))

    def array_init(self, t):
        """A new array of `t`, a fresh literal, or an existing array of that type (an alias)."""
        r = self.r.random()
        names = self.visible(t + '[]')
        if r < 0.3 and names:
            return self.pick(names)
        if r < 0.6:
            return f'new {t}[{self.r.randint(1, 6)}]'
        elems = [self.source(t, 2) for _ in range(self.r.randint(1, 4))]
        return f'new {t}[] {{{", ".join(elems)}}}'

    def st_assign(self, depth):
        t = self.pick(PRIMS)
        names = self.visible(t, True)
        if not names:
            return
        name = self.pick(names)
        if self.chance(0.4):
            self.emit(f'{self.compound(name, t, 3)};')
        else:
            self.emit(f'{name} = {self.source(t, 3)};')

    def st_incr(self, depth):
        names = [n for n, t, w in self.all_vars() if w and t in NUMERIC]
        if not names:
            return
        n = self.pick(names)
        op = self.pick(['++', '--'])
        self.emit(f'{n}{op};' if self.chance(0.5) else f'{op}{n};')

    def st_store(self, depth):
        t = self.pick(ARRAY_ELEMS)
        names = self.visible(t + '[]')
        if not names:
            return
        a = self.pick(names)
        target = f'{a}[Math.floorMod({self.expr("int", 2)}, {a}.length)]'
        if t != 'boolean' and self.chance(0.2):
            self.emit(f'{target}++;')
        elif self.chance(0.4):
            self.emit(f'{self.compound(target, t, 3)};')
        else:
            self.emit(f'{target} = {self.source(t, 3)};')

    def st_print(self, depth):
        if self.chance(0.3):
            self.emit(f'System.out.println({self.expr(self.pick(PRIMS), 3)});')
            return
        parts = [f'"p{self.count}:"']
        parts += [self.expr(self.pick(PRIMS), 3) for _ in range(self.r.randint(1, 3))]
        self.emit('System.out.println(' + ' + " " + '.join(parts) + ');')

    def st_acc(self, depth):
        self.emit(f'acc += {self.expr(self.pick(PRIMS), 3)};')

    def st_if(self, depth):
        self.emit(f'if ({self.bool_expr(3)}) {{')
        self.block(depth)
        if self.chance(0.4):
            self.emit('} else {')
            self.block(depth)
        self.emit('}')

    def st_loop(self, depth):
        count = self.r.randint(1, 5)
        label = self.name('L') if self.chance(0.25) else None
        prefix = f'{label}: ' if label else ''
        ctx = ('loop', label)
        kind = self.pick(['for', 'while', 'do'])
        if kind == 'for':
            i = self.name('i')
            self.emit(f'{prefix}for (int {i} = 0; {i} < {count}; {i}++) {{')
            self.block(depth, ctx, extra=[(i, 'int', False)])
            self.emit('}')
            return
        # The counter moves first, so `continue` cannot skip the increment and loop forever.
        c = self.name('c')
        self.emit(f'int {c} = 0;')
        if kind == 'while':
            self.emit(f'{prefix}while ({c} < {count}) {{')
            self.block(depth, ctx, head=f'{c}++;')
            self.emit('}')
        else:
            self.emit(f'{prefix}do {{')
            self.block(depth, ctx, head=f'{c}++;')
            self.emit(f'}} while ({c} < {count});')

    def st_switch(self, depth):
        self.emit(f'switch (Math.floorMod({self.expr("int", 3)}, 4)) {{')
        labels = [k for k in range(4) if self.chance(0.6)] or [0]
        if self.chance(0.4):
            labels.append('default')
        self.indent += 1
        for k in labels:
            self.emit('default: {' if k == 'default' else f'case {k}: {{')
            self.block(depth, ('switch', None))
            self.emit('}')
        self.indent -= 1
        self.emit('}')

    def st_jump(self, depth):
        options = []
        if any(k == 'loop' for k, _ in self.loops):
            options.append('continue;')
        if self.loops:
            options.append('break;')
        for k, label in self.loops:
            if label:
                options += [f'break {label};', f'continue {label};']
        if options:
            self.guarded(self.bool_expr(2), self.pick(options))

    def st_vcall(self, depth):
        options = self.callables(None)
        if not options:
            return
        m = self.pick(options)
        self.emit(f'{m["name"]}({self.args(m, 2, m is self.method)});')

    def st_ret(self, depth):
        if self.method is None:
            return
        text = 'return;' if self.ret is None else f'return {self.source(self.ret, 3)};'
        self.guarded(self.bool_expr(2), text)

    # Methods, the class and main.

    def method_decl(self, index):
        ret = None if self.chance(0.2) else self.pick(PRIMS)
        recursive = self.chance(0.35)
        params = [('d', 'int', False)] if recursive else []
        for i in range(self.r.randint(0, 3)):
            t = self.pick(PRIMS) if self.chance(0.7) else self.pick(ARRAY_ELEMS) + '[]'
            params.append((f'p{i}', t, True))
        m = {'name': f'm{index}', 'ret': ret, 'params': params, 'recursive': recursive}
        sig = ', '.join(f'{t} {n}' for n, t, _ in params)
        self.emit(f'static {ret or "void"} {m["name"]}({sig}) {{')
        saved = (self.frames, self.loops, self.method, self.ret, self.self_ok, self.self_sites)
        self.frames, self.loops = [list(params)], []
        self.method, self.ret = m, ret
        self.self_ok, self.self_sites = False, 0
        self.indent = 2
        if recursive:
            # The base case cannot call the method itself, or it would never stop.
            self.guarded('d <= 0', 'return;' if ret is None else f'return {self.source(ret, 2)};')
            self.self_ok = True
        for _ in range(self.r.randint(2, 5)):
            self.stmt(2)
        if ret is not None:
            self.emit(f'return {self.source(ret, 3)};')
        self.indent = 1
        self.emit('}')
        self.frames, self.loops, self.method, self.ret, self.self_ok, self.self_sites = saved
        self.methods.append(m)

    def dump_array(self, n):
        s, i = self.name('s'), self.name('i')
        self.emit(f'String {s} = "";')
        self.emit(f'for (int {i} = 0; {i} < {n}.length; {i}++) {{')
        self.indent += 1
        self.emit(f'{s} += {n}[{i}] + ",";')
        self.indent -= 1
        self.emit('}')
        self.emit(f'System.out.println("{n}=" + {s});')

    def main(self):
        self.emit('public static void main(String[] args) {')
        self.indent = 2
        self.frames, self.loops, self.method, self.ret, self.self_ok = [[]], [], None, None, False
        for i in range(self.size):
            if i == self.throw_at and self.arrays:
                self.trap()
            elif i == self.throw_at:
                # Exit code 1 with the output so far; the zero is never assigned.
                if self.chance(0.5):
                    self.emit('System.out.println(1 / zero0);')
                else:
                    self.emit('System.out.println(aInt[aInt.length + zero0]);')
            self.stmt(3)
        for n, t, _ in self.frames[0] + self.statics:
            if self.arrays and '[]' in t:
                self.dump_any(n, t)
            elif t.endswith('[]'):
                self.dump_array(n)
            else:
                self.emit(f'System.out.println("{n}=" + {n});')
        self.emit('System.out.println("acc=" + acc);')
        self.indent = 1
        self.emit('}')

    def program(self):
        if self.strings:
            self.string_header()
        elif self.arrays:
            self.out += ['import java.util.Arrays;', '']
        self.out.append('public class Prog {')
        self.indent = 1
        self.emit('static int zero0 = 0;')
        self.emit('static String acc = "";')
        for t, name in SCALAR_STATICS:
            self.emit(f'static {t} {name} = {self.literal(t)};')
            self.statics.append((name, t, True))
        if self.strings:
            self.string_statics()
        # Half the array programs have no array statics: a static array is a way to reach any
        # shared array, which keeps the translator from lending shared arrays to calls.
        array_statics = not self.arrays or self.chance(0.5)
        if self.arrays and array_statics:
            self.array_statics()
        for t in ARRAY_ELEMS if array_statics else []:
            name = 'a' + t.capitalize()
            if self.chance(0.5):
                init = f'new {t}[{self.r.randint(1, 6)}]'
            else:
                init = '{' + ', '.join(self.literal(t) for _ in range(self.r.randint(1, 4))) + '}'
            self.emit(f'static {t}[] {name} = {init};')
            self.statics.append((name, t + '[]', True))
        if self.arrays:
            self.helpers()
        for i in range(self.r.randint(0, 4)):
            self.method_decl(i)
        self.main()
        self.out.append('}')
        return '\n'.join(self.out) + '\n'


if __name__ == '__main__':
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    seed = int(args[0])
    size = int(args[1]) if len(args) > 1 else 30
    sys.stdout.write(generate(seed, size, strings='--strings' in sys.argv,
                              arrays='--arrays' in sys.argv))
