"""String, StringBuilder, String.format and Arrays code for fuzz.py, enabled by `--strings`.

`Gen` mixes in `StringGen`: each `st_*` method is a statement kind and each other method builds
an expression. Indices are guarded so that no generated program throws. Strings are capped,
because a loop can assign the same variable many times and each assignment may double it.

The type lists below repeat those in fuzz.py, which imports this module; keep them in step.
"""

PRIMS = ['byte', 'short', 'char', 'int', 'long', 'float', 'double', 'boolean']
INTEGRAL = ['byte', 'short', 'char', 'int', 'long']
ARRAY_ELEMS = ['int', 'long', 'double', 'float', 'char', 'boolean', 'byte', 'short']

# Java source text of String literals: non-ASCII, escaped, surrogate pairs, separators.
STRINGS = [
    '""', '"abc"', '"Hello, World"', '"a b  c"', '"  pad  "', '"x,y,,z"', '"ab,ab"', '"a\\tb"',
    '"héllo wörld"', '"日本語"', '"\\u00e9t\\u00e9"', '"\\uD83D\\uDE00!"', '"a\\\\b"', '"ß"',
    '"MiXeD 42"', '"-1;2"', '"😀x"',
]
CHARS = ["'a'", "'Z'", "'0'", "' '", "'é'", "'\\u0041'", "'\\n'"]
SEPARATORS = ['","', '" "', '"ab"', '"-"', '";"', '"l"']
REPLACE_TARGETS = ['"a"', '"ab"', '""', '"é"', '"l"', '"  "']
SWITCH_LABELS = ['"abc"', '"x"', '""', '"Hello, World"', '"é"', '"a,b"']
# Format specifiers with the kind of argument each takes: %d and %x need integers, %c a char.
FORMATS = [
    ('%d', 'integral'), ('%x', 'integral'), ('%08d', 'integral'), ('%c', 'char'),
    ('%5.2f', 'float'), ('%s', 'any'), ('%-8s', 'any'),
]
FORMAT_INTEGRAL = ['byte', 'short', 'int', 'long']  # %d of a char throws, so char is out

STRING_STATEMENTS = {
    'str_decl': 3, 'str_assign': 4, 'str_print': 6, 'str_switch': 2, 'sb_decl': 2,
    'sb_op': 4, 'arrays': 3,
}
STRING_NESTING = {'str_switch'}


class StringGen:
    # Setup, called by Gen.program.

    def string_header(self):
        self.out.append('import java.util.Arrays;')
        self.out.append('')

    def string_statics(self):
        self.emit(f'static String sStr = {self.str_literal()};')
        self.statics.append(('sStr', 'String', True))

    # String expressions. Each returns Java text of type String, or of the type named.

    def str_literal(self):
        return self.pick(STRINGS)

    def str_leaf(self):
        names = self.visible('String')
        if names and self.chance(0.6):
            return self.pick(names)
        return self.str_literal()

    def str_expr(self, d):
        if d <= 0 or self.chance(0.2):
            return self.str_leaf()
        kind = self.pick(['concat', 'concat', 'valueof', 'substring', 'replace', 'upper', 'trim',
                          'split', 'format', 'sb', 'arrays'])
        text = None
        if kind == 'concat':
            text = f'({self.str_expr(d - 1)} + ({self.concat_part(d - 1)}))'
        elif kind == 'valueof':
            text = f'String.valueOf({self.expr(self.pick(PRIMS), d - 1)})'
        elif kind == 'substring':
            text = self.substring_call(d)
        elif kind == 'replace':
            text = self.replace_call(d)
        elif kind == 'upper':
            text = f'{self.str_expr(d - 1)}.toUpperCase()'
        elif kind == 'trim':
            text = f'{self.str_expr(d - 1)}.trim()'
        elif kind == 'split':
            text = self.split_element(d)
        elif kind == 'format':
            text = self.format_call(d)
        elif kind == 'sb':
            names = self.visible('StringBuilder')
            text = f'{self.pick(names)}.toString()' if names else None
        elif kind == 'arrays':
            text = self.arrays_text()
        return text or self.str_leaf()

    def concat_part(self, d):
        """The right operand of a concatenation: a string or a value of any primitive type."""
        if self.chance(0.4):
            return self.str_expr(d)
        return self.expr(self.pick(PRIMS), d)

    def substring_call(self, d):
        """Indices are reduced modulo length + 1, so each lies in [0, length]."""
        s = self.str_leaf()
        begin = f'Math.floorMod({self.expr("int", d - 1)}, {s}.length() + 1)'
        if self.chance(0.6):
            return f'{s}.substring({begin})'
        end = f'Math.floorMod({self.expr("int", d - 1)}, {s}.length() + 1)'
        return f'{s}.substring(0, {end})'

    def replace_call(self, d):
        s = self.str_expr(d - 1)
        if self.chance(0.3):
            return f'{s}.replace({self.pick(CHARS)}, {self.pick(CHARS)})'
        return f'{s}.replace({self.pick(REPLACE_TARGETS)}, {self.str_literal()})'

    def split_element(self, d):
        """An element of `s.split(sep)`, or "" when the split has no parts."""
        parts = f'{self.str_leaf()}.split({self.pick(SEPARATORS)})'
        index = self.expr('int', d - 1)
        return f'({parts}.length == 0 ? "" : {parts}[Math.floorMod({index}, {parts}.length)])'

    def format_call(self, d):
        specs = self.r.sample(FORMATS, self.r.randint(1, 3))
        args = [self.format_arg(kind, d) for _, kind in specs]
        template = '|'.join(spec for spec, _ in specs)
        return f'String.format("<{template}>", {", ".join(args)})'

    def format_arg(self, kind, d):
        if kind == 'integral':
            return self.expr(self.pick(FORMAT_INTEGRAL), d - 1)
        if kind == 'char':
            return self.expr('char', d - 1)
        if kind == 'float':
            return self.expr(self.pick(['float', 'double']), d - 1)
        if self.chance(0.6):
            return self.str_expr(d - 1)
        return self.expr(self.pick(PRIMS), d - 1)

    def arrays_text(self):
        names = self.visible(self.pick(ARRAY_ELEMS) + '[]')
        return f'Arrays.toString({self.pick(names)})' if names else None

    # Expressions of other types, called from Gen's expression kinds.

    def string_num(self, t, d):
        """A numeric value of type `t` read from a string: a length, an index, a comparison."""
        if t == 'char' and self.chance(0.5):
            return self.char_at(d)
        text = self.str_int(d)
        return text if t == 'int' else f'(({t})({text}))'

    def char_at(self, d):
        s = self.str_leaf()
        index = self.expr('int', d - 1)
        return f"({s}.length() > 0 ? {s}.charAt(Math.floorMod({index}, {s}.length())) : 'z')"

    def str_int(self, d):
        s = self.str_expr(d - 1)
        kind = self.pick(['length', 'indexOf', 'compareTo', 'count'])
        if kind == 'length':
            return f'{s}.length()'
        if kind == 'indexOf':
            return f'{s}.indexOf({self.pick(STRINGS + CHARS)})'
        if kind == 'compareTo':
            return f'{s}.compareTo({self.str_expr(d - 1)})'
        return f'{s}.split({self.pick(SEPARATORS)}).length'

    def string_bool(self, d):
        kind = self.pick(['equals', 'startsWith', 'contains', 'compare', 'arrays'])
        if kind == 'arrays':
            names = self.visible(self.pick(ARRAY_ELEMS) + '[]')
            if names:
                return f'Arrays.equals({self.pick(names)}, {self.pick(names)})'
            kind = 'equals'
        s = self.str_expr(d - 1)
        if kind == 'compare':
            op = self.pick(['<', '>', '<=', '>=', '==', '!='])
            return f'({s}.compareTo({self.str_expr(d - 1)}) {op} 0)'
        return f'{s}.{kind}({self.str_expr(d - 1)})'

    # Statements.

    def st_str_decl(self, depth):
        name = self.name('s')
        self.emit(f'String {name} = {self.str_expr(3)};')
        self.frames[-1].append((name, 'String', True))

    def st_str_assign(self, depth):
        names = self.visible('String', True)
        if not names:
            return
        name = self.pick(names)
        if self.chance(0.3):
            self.emit(f'{name} += {self.concat_part(2)};')
        else:
            self.emit(f'{name} = {self.str_expr(3)};')
        self.guarded(f'{name}.length() > 80', f'{name} = {name}.substring(0, 40);')

    def st_str_print(self, depth):
        self.emit(f'System.out.println({self.str_expr(3)});')

    def st_str_switch(self, depth):
        self.emit(f'switch ({self.str_expr(2)}) {{')
        labels = self.r.sample(SWITCH_LABELS, self.r.randint(1, 3))
        if self.chance(0.4):
            labels.append(None)  # default
        self.indent += 1
        for label in labels:
            self.emit('default: {' if label is None else f'case {label}: {{')
            self.block(depth, ('switch', None))
            self.emit('}')
        self.indent -= 1
        self.emit('}')

    def st_sb_decl(self, depth):
        name = self.name('sb')
        init = self.str_expr(2) if self.chance(0.4) else ''
        self.emit(f'StringBuilder {name} = new StringBuilder({init});')
        self.frames[-1].append((name, 'StringBuilder', True))

    def st_sb_op(self, depth):
        names = self.visible('StringBuilder')
        if not names:
            return self.st_sb_decl(depth)
        sb = self.pick(names)
        kind = self.pick(['append', 'append', 'insert', 'reverse', 'set', 'delete'])
        if kind == 'append':
            self.emit(f'{sb}.append({self.concat_part(2)});')
            self.guarded(f'{sb}.length() > 200', f'{sb}.setLength(100);')
        elif kind == 'insert':
            offset = f'Math.floorMod({self.expr("int", 2)}, {sb}.length() + 1)'
            self.emit(f'{sb}.insert({offset}, {self.concat_part(2)});')
        elif kind == 'reverse':
            self.emit(f'{sb}.reverse();')
        elif kind == 'set':
            index = f'Math.floorMod({self.expr("int", 2)}, {sb}.length())'
            self.guarded(f'{sb}.length() > 0', f'{sb}.setCharAt({index}, {self.expr("char", 2)});')
        else:
            index = f'Math.floorMod({self.expr("int", 2)}, {sb}.length())'
            self.guarded(f'{sb}.length() > 0', f'{sb}.deleteCharAt({index});')

    def st_arrays(self, depth):
        t = self.pick(ARRAY_ELEMS)
        names = self.visible(t + '[]')
        if not names:
            return
        a = self.pick(names)
        if t != 'boolean' and self.chance(0.5):  # Arrays.sort has no boolean[] overload
            self.emit(f'Arrays.sort({a});')
        else:
            self.emit(f'Arrays.fill({a}, {self.source(t, 2)});')
