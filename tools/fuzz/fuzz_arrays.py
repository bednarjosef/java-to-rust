"""Array code for fuzz.py, enabled by `--arrays`: aliasing, moves, rows, loops and bounds.

`Gen` mixes in `ArrayGen`: each `st_arr_*` method is a statement kind, and the helper methods
(`helpers`) are generated before main. The code is heavy in what decides whether an array can
be a plain `Vec`: second names (`b = a`, `x = c ? a : b`, a row in a local, a parameter, a
static), moves (`a = b; b = c;`, swaps through a temporary), rows replaced and shared, for-each
loops that write the array they walk, and counting loops whose bounds come from `length`,
variables and guards.

Array variables are tracked with their type and a tag. A plain type (`int[]`, `int[][]`) is an
array that is not null, not empty, whose rows are again such arrays and whose strings are not
null, so any index reduced with `Math.floorMod` is in bounds. A type ending in `?` (`int[][]?`)
is not null but may be empty, and its rows may be null or empty and its strings null: every
access to it is guarded. fuzz.py's own code sees only the plain one-dimensional arrays of
primitives, which it already handles. A program throws only at its chosen statement
(`throw_at`), where `trap` writes one of the ways array code fails in Java: past the end, a
negative index or size, a bound taken before the array was replaced, a null row.

The type lists below repeat those in fuzz.py, which imports this module; keep them in step.
"""

from fuzz_array_helpers import (DEFAULTS, ELEMS, LOOSE_ROLES, PRIMS, ArrayHelperGen, array_type,
                                dims_of, elem_of, is_private, jtype)

NUMERIC = ['byte', 'short', 'char', 'int', 'long', 'float', 'double']
INTEGRAL = ['byte', 'short', 'char', 'int', 'long']
STRING_VALUES = ['"a"', '"bc"', '""', '"x y"', '"é"', '"Q"', '"zz"']

ARRAY_STATEMENTS = {
    'arr_decl': 8, 'arr_write': 8, 'arr_print': 6, 'arr_alias': 4, 'arr_move': 5, 'arr_row': 5,
    'arr_each': 3, 'arr_count': 6, 'arr_call': 7, 'arr_compare': 2, 'arr_util': 3,
    'arr_order': 1,
}
ARRAY_NESTING = {'arr_each', 'arr_count'}


class ArrayGen(ArrayHelperGen):
    # Setup, called by Gen.program.

    def array_statics(self):
        """Array statics beyond fuzz.py's own `aInt`..: a string array and some grids. About
        half the programs have none of them, so arrays lent to calls from shared cells are
        tested without a static that could reach the cell (fuzz.py's statics are kept off for
        those programs too: see `Gen.program`)."""
        self.emit(f'static String[] aString = {{{", ".join(self.r.sample(STRING_VALUES, 3))}}};')
        self.statics.append(('aString', 'String[]', True))
        for elem in self.r.sample(ELEMS, 3):
            name = 'g' + elem.capitalize()
            self.emit(f'static {elem}[][] {name} = {self.literal_grid(elem, 2)};')
            self.statics.append((name, array_type(elem, 2), True))

    # Small pieces of Java text.

    def pure_int(self):
        """An int expression without side effects, which may be read more than once."""
        names = self.visible('int')
        if names and self.chance(0.6):
            return self.pick(names)
        return self.pick(['0', '1', '2', '3', '-1', '5', '7', '-4', '11'])

    def index(self, pre):
        """An index in bounds of the plain-tagged array `pre` (Java text without side effects)."""
        r = self.r.random()
        if r < 0.15:
            return '0'
        if r < 0.25:
            return f'{pre}.length - 1'
        return f'Math.floorMod({self.pure_int()}, {pre}.length)'

    def path(self, name, levels):
        """An element `levels` deep into the plain-tagged array `name`."""
        text = name
        for _ in range(levels):
            text = f'{text}[{self.index(text)}]'
        return text

    def guard_path(self, name, levels):
        """(condition, element) for an element `levels` deep into an array tagged `?`: the
        condition checks every row for null and every index against its row's length."""
        conds, text = [], name
        for level in range(levels):
            k = self.pick(['0', '1', '2', f'Math.floorMod({self.pure_int()}, 4)'])
            conds.append(f'{k} < {text}.length')
            text = f'{text}[{k}]'
            if level < levels - 1:
                conds.append(f'{text} != null')
        return ' && '.join(conds), text

    def as_int(self, text, elem, loose):
        """`text`, an element of type `elem`, as an int (text is read twice for strings)."""
        if elem == 'boolean':
            return f'(({text}) ? 1 : 0)'
        if elem == 'String':
            return f'(({text}) == null ? -1 : ({text}).length())' if loose else f'({text}).length()'
        return f'((int) ({text}))'

    def elem_value(self, elem, d, loose=False):
        """A value that may be stored in an array of `elem`; null only into arrays tagged `?`."""
        if elem != 'String':
            return self.source(elem, d)
        r = self.r.random()
        if loose and r < 0.15:
            return 'null'
        if r < 0.5:
            return self.pick(STRING_VALUES)
        if r < 0.75:
            return f'"s" + {self.expr("int", max(d - 1, 0))}'
        src = self.arrays_of('String', None, loose=False)
        if src:
            name, t = self.pick(src)
            return self.path(name, dims_of(t))
        return self.pick(STRING_VALUES)

    def combine(self, elem, old, k):
        """A new element of type `elem` computed from the old one and the int `k`."""
        if elem == 'boolean':
            return f'({old} ^ ({k} > 1))'
        if elem == 'String':
            return f'({old} + {k})'
        op = self.pick(['+', '-', '*'] + (['^'] if elem in INTEGRAL else []))
        if elem == 'int':
            return f'({old} {op} {k})'
        return f'(({elem}) ({old} {op} {k}))'

    def literal_row(self, elem, n=None):
        n = n or self.r.randint(1, 4)
        return '{' + ', '.join(self.elem_value(elem, 1) for _ in range(n)) + '}'

    def literal_grid(self, elem, dims):
        if dims == 1:
            return self.literal_row(elem)
        rows = [self.literal_grid(elem, dims - 1) for _ in range(self.r.randint(1, 3))]
        return '{' + ', '.join(rows) + '}'

    def arrays_of(self, elem, dims, loose=None, writable=False, private=None):
        """Visible array variables, as (name, tracked type): of element `elem` (any when None),
        `dims` dimensions (any when None), tagged `?` or not (either when None), tagged `!` or
        not (either when None)."""
        out = []
        for n, t, w in self.all_vars():
            if '[]' not in t or (writable and (not w or n in self.walked)):
                continue
            if elem is not None and elem_of(t) != elem:
                continue
            if dims is not None and dims_of(t) != dims:
                continue
            if loose is not None and t.endswith('?') != loose:
                continue
            if private is not None and is_private(t) != private:
                continue
            out.append((n, t))
        return out

    def some_array(self, dims=None, elem=None, loose=None, writable=False, private=None):
        found = self.arrays_of(elem, dims, loose, writable, private)
        return self.pick(found) if found else None

    def shared_array(self, dims=None, elem=None, loose=None, writable=False):
        """An array that may be given a second name: not private."""
        return self.some_array(dims, elem, loose, writable, private=False)

    def declare(self, t, init):
        name = self.name('a' if dims_of(t) == 1 else 'g')
        self.emit(f'{jtype(t)} {name} = {init};')
        self.frames[-1].append((name, t, True))
        return name

    def new_array(self, elem, dims, d=2):
        """(text, loose) of a new array of `elem`: `new` with sizes, a literal, or jagged rows
        made by a loop (in which case the text is None and the lines are left to the caller)."""
        r = self.r.random()
        if r < 0.4:
            sizes = ''.join(f'[{self.r.randint(1, 4)}]' for _ in range(dims))
            # A `new String[n]` holds nulls, so it is tagged `?`.
            return f'new {elem}{sizes}', elem == 'String'
        if r < 0.5:
            given = self.r.randint(1, dims)
            sizes = ''.join(f'[{self.r.randint(0, 3)}]' for _ in range(given))
            return f'new {elem}{sizes}{"[]" * (dims - given)}', True
        if r < 0.85 or dims == 1:
            return f'new {elem}{"[]" * dims} {self.literal_grid(elem, dims)}', False
        return None, False

    def fresh(self, elem, dims):
        """Declares a new array of `elem` and returns (name, tracked type). Some plain ones are
        private (`!`): no statement gives them a second name, so they can stay plain."""
        text, loose = self.new_array(elem, dims)
        t = array_type(elem, dims, loose, private=not loose and self.chance(0.5))
        if text is not None:
            return self.declare(t, text), t
        # Jagged rows of growing length, made by a loop.
        n = self.r.randint(1, 4)
        name = self.declare(t, f'new {elem}[{n}]{"[]" * (dims - 1)}')
        i = self.name('i')
        self.emit(f'for (int {i} = 0; {i} < {name}.length; {i}++) {{')
        self.indent += 1
        inner = ''.join(f'[{self.r.randint(1, 3)}]' for _ in range(dims - 2))
        self.emit(f'{name}[{i}] = new {elem}[{i} + 1]{inner};')
        self.indent -= 1
        self.emit('}')
        return name, t

    def plain_or_fresh(self, dims, elem=None, writable=False, private=None):
        """A plain-tagged array variable, declared here when none is visible."""
        found = self.some_array(dims, elem, loose=False, writable=writable, private=private)
        if found:
            return found
        elem = elem or self.pick(ELEMS)
        text = f'new {elem}{"[]" * dims} {self.literal_grid(elem, dims)}'
        t = array_type(elem, dims)
        return self.declare(t, text), t

    # Expressions of fuzz.py's types, called from Gen.num_leaf.

    def array_num(self, t, d):
        """An element of type `t` read from a plain array of any depth, or an increment of one."""
        found = self.some_array(None, t, loose=False)
        if not found:
            return None
        name, at = found
        target = self.path(name, dims_of(at))
        if t != 'boolean' and self.chance(0.15):
            return f'({target}++)' if self.chance(0.5) else f'(--{target})'
        return target

    # Statements.

    def st_arr_decl(self, depth):
        dims = self.r.choices([1, 2, 3], [5, 4, 1])[0]
        elem = self.pick(ELEMS)
        r = self.r.random()
        same = self.some_array(dims, elem, private=False)
        if r < 0.5 or not same:
            self.fresh(elem, dims)
            return
        name, t = same
        loose = t.endswith('?')
        if r < 0.65:
            self.declare(t, name)  # a second name for the same array
        elif r < 0.75:
            self.declare(t, f'{name}.clone()')
        elif r < 0.85:
            other = self.some_array(dims, elem, loose=loose, private=False) or same
            self.declare(t, f'{self.bool_expr(2)} ? {name} : {other[0]}')
        elif not loose and dims > 1:
            self.declare(array_type(elem, dims - 1), self.path(name, 1))  # a row of it
        elif elem in PRIMS and dims == 1:
            if self.chance(0.5):
                self.declare(array_type(elem, 1), f'Arrays.copyOf({name}, {self.r.randint(1, 5)})')
            else:
                k = self.r.randint(1, 5)
                self.declare(array_type(elem, 1), f'Arrays.copyOfRange({name}, 0, {k})')
        else:
            self.declare(t, name)

    def st_arr_write(self, depth):
        found = self.some_array()
        if not found:
            return self.st_arr_decl(depth)
        name, t = found
        elem, dims, loose = elem_of(t), dims_of(t), t.endswith('?')
        if loose:
            cond, target = self.guard_path(name, dims)
            self.guarded(cond, f'{target} = {self.elem_value(elem, 2, loose)};')
            return
        target = self.path(name, dims)
        r = self.r.random()
        if elem not in ('boolean', 'String') and r < 0.2:
            self.emit(f'{target}{self.pick(["++", "--"])};')
        elif elem != 'String' and r < 0.5:
            self.emit(f'{self.compound(target, elem, 2)};')
        elif r < 0.6:
            # A chain: two elements, maybe of two names for one array, take the same value.
            other = self.some_array(dims, elem, loose=False) or found
            value = self.elem_value(elem, 2)
            self.emit(f'{target} = {self.path(other[0], dims)} = {value};')
        else:
            self.emit(f'{target} = {self.elem_value(elem, 3)};')

    def st_arr_print(self, depth):
        parts = [f'"q{self.count}:"']
        for _ in range(self.r.randint(1, 3)):
            found = self.some_array()
            if not found:
                break
            name, t = found
            if t.endswith('?'):
                cond, target = self.guard_path(name, dims_of(t))
                parts.append(f'({cond} ? "" + {target} : "-")')
            elif self.chance(0.2):
                parts.append(f'{self.path(name, self.r.randint(0, dims_of(t) - 1))}.length')
            else:
                parts.append(self.path(name, dims_of(t)))
        self.counter_bump()
        self.emit('System.out.println(' + ' + " " + '.join(parts) + ');')

    def counter_bump(self):
        self.count += 1

    def st_arr_alias(self, depth):
        """A second name for an array or a row, a write through it, a read through the first."""
        dims = self.r.choice([1, 1, 2, 2, 3])
        name, t = self.plain_or_fresh(dims, private=False)
        elem = elem_of(t)
        if dims > 1 and self.chance(0.5):
            alias_t = array_type(elem, dims - 1)
            alias = self.declare(alias_t, self.path(name, 1))
        else:
            alias_t = t
            alias = self.declare(t, name)
        levels = dims_of(alias_t)
        self.emit(f'{self.path(alias, levels)} = {self.elem_value(elem, 2)};')
        reads = [self.path(name, dims), self.path(alias, levels)]
        self.emit(f'System.out.println("alias{self.count}:" + {reads[0]} + " " + {reads[1]});')

    def st_arr_move(self, depth):
        """Reassignments: a move, a swap through a temporary, a rotation, a conditional."""
        found = self.some_array(writable=True)
        if not found:
            return self.st_arr_decl(depth)
        name, t = found
        elem, dims, loose, private = elem_of(t), dims_of(t), t.endswith('?'), is_private(t)
        # Only arrays of the same tag: a private array moves only to and from private ones.
        others = [o for o in self.arrays_of(elem, dims, writable=True) if o[0] != name and o[1] == t]
        r = self.r.random()
        if private and r >= 0.5:
            r = 0.9  # no conditional and no clone, which would give it a second name
        if others and r < 0.3:
            other = self.pick(others)[0]
            self.emit(f'{name} = {other};')
            if private or self.chance(0.6):
                # The rotation `a = b; b = c;`: `b` takes something new after giving its array.
                text, fresh_loose = self.new_array(elem, dims)
                if private and (text is None or fresh_loose):
                    text, fresh_loose = f'new {elem}{"[]" * dims} {self.literal_grid(elem, dims)}', False
                if text is not None and (fresh_loose <= loose):
                    self.emit(f'{other} = {text};')
        elif others and r < 0.5:
            other = self.pick(others)[0]
            tmp = self.name('t')
            self.emit(f'{jtype(t)} {tmp} = {name};')
            self.emit(f'{name} = {other};')
            self.emit(f'{other} = {tmp};')
            if not private:  # read later, the temporary would be a second name
                self.frames[-1].append((tmp, t, True))
        elif others and r < 0.65:
            other = self.pick(others)[0]
            self.emit(f'{name} = {self.bool_expr(2)} ? {other} : {name};')
        elif r < 0.75:
            self.emit(f'{name} = {name}.clone();')
        else:
            text, fresh_loose = self.new_array(elem, dims)
            if text is not None and fresh_loose <= loose:
                self.emit(f'{name} = {text};')

    def st_arr_row(self, depth):
        """Rows: taken into a local, replaced by new rows, by other rows, by a local array."""
        found = self.some_array(None, None, loose=None)
        if not found or dims_of(found[1]) < 2:
            found = self.plain_or_fresh(self.pick([2, 2, 3]))
        name, t = found
        elem, dims, loose = elem_of(t), dims_of(t), t.endswith('?')
        row_t = array_type(elem, dims - 1)
        if loose:
            k = self.pick(['0', '1', f'Math.floorMod({self.pure_int()}, 3)'])
            r = self.r.random()
            if r < 0.3:
                value = 'null'
            elif r < 0.6:
                value = f'new {elem}[{self.r.randint(0, 3)}]{"[]" * (dims - 2)}'
            else:
                value = f'new {elem}{"[]" * (dims - 1)} {self.literal_grid(elem, dims - 1)}'
            self.guarded(f'{k} < {name}.length', f'{name}[{k}] = {value};')
            return
        if name in self.walked:
            # A loop runs over these rows with a bound taken before it: keep their lengths.
            if not is_private(t):
                row = self.declare(row_t, self.path(name, 1))
                self.emit(f'{self.path(row, dims - 1)} = {self.elem_value(elem, 2)};')
            return
        k = self.index(name)
        r = self.r.random()
        if is_private(t):
            r = 0.3  # a private grid's rows are only replaced by new rows
        if r < 0.25:
            row = self.declare(row_t, f'{name}[{k}]')
            self.emit(f'{self.path(row, dims - 1)} = {self.elem_value(elem, 2)};')
            self.emit(f'System.out.println("row{self.count}:" + {self.path(name, dims)});')
        elif r < 0.4:
            sizes = ''.join(f'[{self.r.randint(1, 3)}]' for _ in range(dims - 1))
            self.emit(f'{name}[{k}] = new {elem}{sizes};')
        elif r < 0.55:
            self.emit(f'{name}[{k}] = {name}[{self.index(name)}];')
        elif r < 0.7:
            local = self.some_array(dims - 1, elem, loose=False)
            if local:
                self.emit(f'{name}[{k}] = {local[0]};')
            else:
                self.emit(f'{name}[{k}] = new {elem}{"[]" * (dims - 1)} '
                          f'{self.literal_grid(elem, dims - 1)};')
        elif r < 0.8:
            self.emit(f'{name}[{k}] = {name}[{k}].clone();')
        else:
            a, b = self.name('k'), self.name('k')
            tmp = self.name('t')
            self.emit(f'int {a} = {k};')
            self.emit(f'int {b} = {self.index(name)};')
            self.emit(f'{jtype(row_t)} {tmp} = {name}[{a}];')
            self.emit(f'{name}[{a}] = {name}[{b}];')
            self.emit(f'{name}[{b}] = {tmp};')
            self.frames[-1].append((tmp, row_t, True))

    def st_arr_each(self, depth):
        """A for-each over an array or the rows of one, with writes to the array in the body."""
        found = self.some_array()
        if not found:
            return self.st_arr_decl(depth)
        name, t = found
        elem, dims, loose = elem_of(t), dims_of(t), t.endswith('?')
        var = self.name('x' if dims == 1 else 'r')
        var_t = array_type(elem, dims - 1, loose, is_private(t)) if dims > 1 else elem
        self.emit(f'for ({jtype(var_t)} {var} : {name}) {{')
        self.indent += 1
        self.frames.append([])
        self.loops.append(('loop', None))
        if dims == 1:
            if not loose:
                if self.chance(0.5):
                    self.emit(f'{self.path(name, 1)} = {var};')
                self.frames[-1].append((var, elem, self.chance(0.5)))
            self.emit(f'acc += {self.as_int(var, elem, loose)} + ",";')
        elif loose:
            self.emit(f'if ({var} == null) {{')
            self.emit('    continue;')
            self.emit('}')
            self.emit(f'acc += {var}.length + ";";')
        else:
            if self.chance(0.6):
                self.emit(f'{self.path(var, dims - 1)} = {self.elem_value(elem, 2)};')
            if self.chance(0.3):
                self.emit(f'{self.path(name, dims)} = {self.elem_value(elem, 2)};')
            inner = self.name('y')
            if dims == 2:
                self.emit(f'for ({elem} {inner} : {var}) {{')
                self.emit(f'    acc += {self.as_int(inner, elem, False)};')
            else:
                self.emit(f'for ({elem}[] {inner} : {var}) {{')
                self.emit(f'    acc += {inner}.length;')
            self.emit('}')
            self.frames[-1].append((var, var_t, self.chance(0.3)))
        for _ in range(self.r.randint(0, 2)):
            self.stmt(depth - 1)
        self.loops.pop()
        self.frames.pop()
        self.indent -= 1
        self.emit('}')

    def st_arr_count(self, depth):
        """A counting loop over an array: bounds from `length`, from variables, from guards,
        downward, by twos, from 1; over the rows of a grid; with or without other statements.
        The arrays it walks are held fixed while the body's other statements are generated, so that
        nothing shrinks them under the index."""
        found = self.some_array(loose=False) if self.chance(0.85) else self.some_array()
        if not found:
            return self.st_arr_decl(depth)
        name, t = found
        elem, dims, loose = elem_of(t), dims_of(t), t.endswith('?')
        if loose:
            form = self.pick(['length', 'guard'])
        else:
            form = self.pick(['length', 'length', 'var', 'guard', 'down', 'step', 'le', 'from1',
                              'while'] + (['grid', 'grid'] if dims >= 2 else []))
        if form == 'grid':
            return self.grid_loop(name, t, depth)
        i = self.name('i')
        other = self.some_array(1, elem, loose=False)
        walked = {name}
        # The array whose elements the loop walks: a row of a grid at an index fixed first.
        walk = name
        if dims > 1 and not loose:
            k = self.name('k')
            self.emit(f'int {k} = {self.index(name)};')
            walk = f'{name}[{k}]' if dims == 2 else f'{name}[{k}][0]'
        head, bounded = [], False
        if form == 'var':
            n = self.name('n')
            bound = f'{walk}.length'
            if other and self.chance(0.6):
                bound = f'Math.min({walk}.length, {other[0]}.length)'
                walked.add(other[0])
                bounded = True
            self.emit(f'int {n} = {bound};')
            header = f'for (int {i} = 0; {i} < {n}; {i}++) {{'
        elif form == 'guard':
            header = f'for (int {i} = 0; {i} < {self.r.randint(1, 6)}; {i}++) {{'
        elif form == 'down':
            header = f'for (int {i} = {walk}.length - 1; {i} >= 0; {i}--) {{'
        elif form == 'step':
            header = f'for (int {i} = 0; {i} < {walk}.length; {i} += {self.r.randint(2, 3)}) {{'
        elif form == 'le':
            header = f'for (int {i} = 0; {i} <= {walk}.length - 1; {i}++) {{'
        elif form == 'from1':
            header = f'for (int {i} = 1; {i} < {walk}.length; {i}++) {{'
        elif form == 'while':
            self.emit(f'int {i} = -1;')
            header = f'while ({i} < {walk}.length - 1) {{'
            head.append(f'{i}++;')
        else:
            header = f'for (int {i} = 0; {i} < {walk}.length; {i}++) {{'
        target = f'{walk}[{i}]'
        if loose and dims > 1:
            cond, inner = self.guard_path(target, dims - 1)
            body = [f'if ({target} != null && {cond}) {{',
                    f'    acc += {self.as_int(inner, elem, True)};', '}']
        else:
            body = self.count_body(walk, target, i, elem, other, form, bounded, loose)
        if form == 'guard':
            body = [f'if ({i} < {walk}.length) {{'] + ['    ' + b for b in body] + ['}']
        self.emit(header)
        self.indent += 1
        for line in head + body:
            self.emit(line)
        # Other statements in the body, or none: an innermost loop that only reaches arrays at
        # its index may run over slices.
        if self.chance(0.5):
            self.frames.append([(i, 'int', False)])
            self.loops.append(('loop', None))
            saved = set(self.walked)
            self.walked |= walked
            for _ in range(self.r.randint(1, 2)):
                self.stmt(depth - 1)
            self.walked = saved
            self.loops.pop()
            self.frames.pop()
        self.indent -= 1
        self.emit('}')

    def count_body(self, walk, target, i, elem, other, form, bounded, loose):
        """The statements of a counting loop that reach the array at its index. `bounded`: the
        loop's bound is at most the length of `other` too."""
        lines = []
        r = self.r.random()
        if loose:
            lines.append(f'acc += {self.as_int(target, elem, True)};')
        elif r < 0.25:
            lines.append(f'acc += {self.as_int(target, elem, False)};')  # reads only
        elif r < 0.45 and other:
            src = f'{other[0]}[{i}]' if bounded else f'{other[0]}[Math.floorMod({i}, {other[0]}.length)]'
            lines.append(f'{target} = {self.combine(elem, src, i)};')
        elif r < 0.6 and form == 'from1':
            lines.append(f'{target} = {self.combine(elem, f"{walk}[{i} - 1]", i)};')
        elif r < 0.7:
            lines.append(f'{target} = {walk}[{walk}.length - 1 - {i}];')
        elif r < 0.8 and elem in NUMERIC:
            lines.append(f'{target} += {self.expr(elem, 1)};')
        elif r < 0.9:
            lines.append(f'if ({self.as_int(target, elem, False)} > {self.r.randint(0, 3)}) {{')
            lines.append('    ' + self.pick(['break;', 'continue;', f'{target} = {DEFAULTS[elem]};']))
            lines.append('}')
            lines.append(f'acc += {self.as_int(target, elem, False)};')
        else:
            lines.append(f'{target} = {self.elem_value(elem, 2)};')
            lines.append(f'System.out.println("e" + {i} + "=" + {target});')
        return lines

    def grid_loop(self, name, t, depth):
        """Nested counting loops over a grid: rows by index, then the row's elements; or a
        product-like loop that reads two grids."""
        elem, dims = elem_of(t), dims_of(t)
        i, j = self.name('i'), self.name('j')
        self.emit(f'for (int {i} = 0; {i} < {name}.length; {i}++) {{')
        self.indent += 1
        row = f'{name}[{i}]' if dims == 2 else f'{name}[{i}][0]'
        other = self.some_array(2, elem, loose=False) if dims == 2 else None
        if other and other[0] != name and self.chance(0.5):
            # c[i][j] = c[i][j] op b[k][j]: b's row is guarded, as its length is unknown.
            k = self.name('k')
            o = other[0]
            self.emit(f'int {k} = Math.floorMod({i}, {o}.length);')
            n = self.name('n')
            self.emit(f'int {n} = Math.min({row}.length, {o}[{k}].length);')
            self.emit(f'for (int {j} = 0; {j} < {n}; {j}++) {{')
            self.emit(f'    {row}[{j}] = {self.combine(elem, f"{o}[{k}][{j}]", j)};')
            self.emit('}')
        else:
            self.emit(f'for (int {j} = 0; {j} < {row}.length; {j}++) {{')
            if self.chance(0.5):
                self.emit(f'    {row}[{j}] = {self.combine(elem, f"{row}[{j}]", i)};')
            self.emit(f'    acc += {self.as_int(f"{row}[{j}]", elem, False)};')
            self.emit('}')
        self.indent -= 1
        self.emit('}')

    def st_arr_call(self, depth):
        if not self.array_methods:
            return
        m = self.pick(self.array_methods)
        role, dims, elem, name = m['role'], m['dims'], m['elem'], m['name']
        t = array_type(elem, dims)
        loose_ok = role in LOOSE_ROLES
        # These roles give the argument a second name: the returned array, a static, a row.
        private = False if role in ('same', 'pick', 'keep', 'row', 'setRow') else None
        arg = self.some_array(dims, elem, loose=None if loose_ok else False, private=private)
        if arg is None and role not in ('fresh',):
            arg = self.plain_or_fresh(dims, elem, private=private)
        k = self.pure_int()
        if role == 'sum':
            # Sometimes a row of a deeper array: a row is an array expression too.
            text = arg[0]
            deeper = self.some_array(dims + 1, elem, loose=False)
            if deeper and self.chance(0.3):
                text = f'{deeper[0]}[{self.index(deeper[0])}]'
            self.emit(f'System.out.println("sum{self.count}:" + {name}({text}));')
            self.counter_bump()
        elif role == 'scale':
            self.emit(f'{name}({arg[0]}, {k});')
        elif role in ('same', 'copy'):
            if self.chance(0.5):
                self.declare(arg[1], f'{name}({arg[0]})')
            else:
                writable = self.some_array if role == 'copy' else self.shared_array
                target = writable(dims, elem, loose=False, writable=True)
                if target:
                    self.emit(f'{target[0]} = {name}({arg[0]});')
                else:
                    self.emit(f'{name}({arg[0]});')
        elif role == 'fresh':
            target = self.some_array(dims, elem, loose=False, writable=True)
            if target and self.chance(0.5):
                self.emit(f'{target[0]} = {name}({k});')
            else:
                self.declare(array_type(elem, dims, private=self.chance(0.5)), f'{name}({k})')
        elif role == 'keep':
            self.emit(f'{name}({arg[0]});')
            static = m['static']
            self.emit(f'{static}[0] = {self.elem_value(elem, 1)};')
            self.emit(f'System.out.println("kept{self.count}:" + {arg[0]}[0] + " " + {static}[0]);')
        elif role == 'pick':
            other = self.shared_array(1, elem, loose=False) or arg
            chosen = self.declare(t, f'{name}({arg[0]}, {other[0]}, {self.bool_expr(1)})')
            self.emit(f'{chosen}[0] = {self.elem_value(elem, 1)};')
            self.emit(f'System.out.println("pick{self.count}:" + {arg[0]}[0] + " " + {other[0]}[0]);')
        elif role == 'into':
            other = self.some_array(1, elem, loose=arg[1].endswith('?') or None) or arg
            pair = [arg[0], other[0]] if self.chance(0.7) else [arg[0], arg[0]]
            self.emit(f'{name}({pair[0]}, {pair[1]});')
        elif role in ('reassign', 'rec'):
            extra = ', 0' if role == 'rec' else ''
            self.emit(f'System.out.println("{role}{self.count}:" + {name}({arg[0]}{extra}));')
            self.counter_bump()
        elif role == 'first':
            self.emit(f'System.out.println("first{self.count}:" + {name}({arg[0]}));')
            self.counter_bump()
        elif role == 'swap':
            self.emit(f'{name}({arg[0]}, {k}, {self.pure_int()});')
        elif role == 'show':
            self.emit(f'{name}({arg[0]}, {arg[0]}.length);')
        elif role == 'row':
            if self.chance(0.5):
                row = self.declare(array_type(elem, 1), f'{name}({arg[0]}, {k})')
                self.emit(f'{row}[0] = {self.elem_value(elem, 1)};')
            else:
                self.emit(f'{name}({arg[0]}, {k})[0] = {self.elem_value(elem, 1)};')
        elif role in ('setRow', 'addRow'):
            if arg[0] in self.walked and role == 'setRow':
                return
            r = self.r.random()
            if r < 0.4:
                row = f'{arg[0]}[{self.index(arg[0])}]'
            else:
                row = self.plain_or_fresh(1, elem, private=False if role == 'setRow' else None)[0]
            if role == 'setRow':
                self.emit(f'{name}({arg[0]}, {k}, {row});')
            else:
                self.emit(f'{name}({arg[0]}, {row});')
        elif role == 'swapRows':
            if arg[0] not in self.walked:
                self.emit(f'{name}({arg[0]}, {k}, {self.pure_int()});')

    def st_arr_compare(self, depth):
        found = self.shared_array()
        if not found:
            return
        name, t = found
        others = self.arrays_of(elem_of(t), dims_of(t), private=False)
        parts = [f'({name} == {self.pick(others)[0]})' for _ in range(self.r.randint(1, 3))]
        if dims_of(t) > 1 and not t.endswith('?'):
            parts.append(f'({name}[{self.index(name)}] == {name}[{self.index(name)}])')
        if dims_of(t) == 1 and elem_of(t) in ELEMS and self.chance(0.4):
            parts.append(f'Arrays.equals({name}, {self.pick(others)[0]})')
        self.emit('System.out.println("eq:" + ' + ' + " " + '.join(parts) + ');')

    def st_arr_util(self, depth):
        """java.util.Arrays and System.arraycopy, with ranges that stay in bounds."""
        found = self.shared_array(1)
        if not found:
            return self.st_arr_decl(depth)
        name, t = found
        elem, loose = elem_of(t), t.endswith('?')
        kind = self.pick(['sort', 'fill', 'fillRange', 'sortRange', 'toString', 'hash', 'search',
                          'arraycopy', 'deep', 'chars'])
        if kind == 'sort' and elem != 'boolean' and not loose:
            self.emit(f'Arrays.sort({name});')
        elif kind == 'fill':
            self.emit(f'Arrays.fill({name}, {self.elem_value(elem, 2, loose)});')
        elif kind == 'fillRange':
            lo = f'Math.min({self.r.randint(0, 2)}, {name}.length)'
            self.emit(f'Arrays.fill({name}, {lo}, {name}.length, {self.elem_value(elem, 2, loose)});')
        elif kind == 'sortRange' and elem != 'boolean' and not loose:
            self.emit(f'Arrays.sort({name}, 0, Math.min({self.r.randint(1, 4)}, {name}.length));')
        elif kind == 'hash':
            self.emit(f'System.out.println("hash:" + Arrays.hashCode({name}));')
        elif kind == 'search' and elem != 'boolean' and not loose:
            self.emit(f'System.out.println("bs:" + Arrays.binarySearch({name}, '
                      f'{self.elem_value(elem, 1)}));')
        elif kind == 'arraycopy':
            other = self.some_array(1, elem, loose=None) or found
            n = self.name('n')
            self.emit(f'int {n} = Math.min({name}.length, {other[0]}.length - 1);')
            self.guarded(f'{n} > 0', f'System.arraycopy({name}, 0, {other[0]}, 1, {n});')
        elif kind == 'deep':
            grid = self.shared_array(2)
            if grid:
                self.emit(f'System.out.println(Arrays.deepToString({grid[0]}));')
        elif kind == 'chars' and elem == 'char':
            self.emit(f'System.out.println(new String({name}) + String.valueOf({name}).length());')
        else:
            self.emit(f'System.out.println(Arrays.toString({name}));')

    def st_arr_order(self, depth):
        """Java's order of evaluation around arrays: the array and index of a store are fixed
        before its value, even when the value replaces the array."""
        found = self.shared_array(1, None, loose=False, writable=True)
        if not found:
            return
        name, t = found
        elem = elem_of(t)
        others = [o for o in self.arrays_of(elem, 1, False, private=False) if o[0] != name]
        if not others:
            return
        other = self.pick(others)[0]
        if elem == 'int' and self.chance(0.5):
            self.emit(f'{name}[0] = ({name} = {other}).length;')
        else:
            self.emit(f'{name}[{name}.length - 1] = ({name} = {other})[0];')
        self.emit(f'System.out.println("order:" + {name}[0] + " " + {other}[0]);')

    # The deliberate failures and the final dump.

    def trap(self):
        """One statement that throws in Java, as array code does: past the end, a negative
        index, a bound taken before the array was replaced, a null row, a negative size."""
        kind = self.pick(['past_end', 'below_zero', 'stale_bound', 'declared_size', 'guard_then_new',
                          'row_replaced', 'null_row', 'negative_size', 'jagged', 'show',
                          'arraycopy', 'fill_range', 'copy_range', 'each_null', 'null_string'])
        name, t = self.plain_or_fresh(1)
        elem = elem_of(t)
        if kind == 'past_end':
            self.emit(f'for (int i = 0; i <= {name}.length; i++) {{')
            self.emit(f'    System.out.println("t" + i + ":" + {name}[i]);')
            self.emit('}')
        elif kind == 'below_zero':
            self.emit(f'for (int i = {name}.length - 1; i >= -1; i--) {{')
            self.emit(f'    System.out.println("t" + i + ":" + {name}[i]);')
            self.emit('}')
        elif kind == 'stale_bound':
            n = self.name('n')
            self.emit(f'int {n} = {name}.length + 1;')
            fresh = self.declare(t, name)
            self.emit(f'{fresh} = new {elem}[1];')
            self.emit(f'for (int i = 0; i < {n}; i++) {{')
            self.emit(f'    {fresh}[i] = {self.elem_value(elem, 1)};')
            self.emit(f'    System.out.println("t" + i + ":" + {fresh}[i] + " " + {name}[0]);')
            self.emit('}')
        elif kind == 'declared_size':
            fresh = self.declare(array_type(elem, 1), f'new {elem}[5]')
            self.emit(f'{fresh}[4] = {self.elem_value(elem, 1)};')
            self.emit(f'{fresh} = new {elem}[2];')
            self.emit(f'System.out.println("t:" + {fresh}[4]);')
        elif kind == 'guard_then_new':
            self.emit(f'if (1 < {name}.length + 1) {{')
            self.emit(f'    {name} = new {elem}[1];')
            self.emit(f'    {name}[1] = {self.elem_value(elem, 1)};')
            self.emit('}')
        elif kind == 'row_replaced':
            grid, gt = self.plain_or_fresh(2)
            ge = elem_of(gt)
            n = self.name('n')
            self.emit(f'int {n} = {grid}[0].length + 1;')
            self.emit(f'{grid}[0] = new {ge}[1];')
            self.emit(f'for (int j = 0; j < {n}; j++) {{')
            self.emit(f'    System.out.println("t" + j + ":" + {grid}[0][j]);')
            self.emit('}')
        elif kind == 'null_row':
            g = self.name('g')
            self.emit(f'{elem}[][] {g} = new {elem}[2][];')
            self.emit(f'{g}[1] = new {elem}[2];')
            self.emit(f'System.out.println("t:" + {g}[1][0]);')
            self.emit(f'System.out.println("t:" + {g}[0][0]);')
        elif kind == 'negative_size':
            shape = self.pick([f'[zero0 - 2]', f'[2][zero0 - 1]', f'[zero0 - 1][2]', f'[3][zero0 - 1][]'])
            dims = shape.count('[')
            self.emit(f'{elem}{"[]" * dims} {self.name("g")} = new {elem}{shape};')
        elif kind == 'jagged':
            g = self.name('g')
            self.emit(f'{elem}[][] {g} = {{{self.literal_row(elem, 3)[0:]}, {self.literal_row(elem, 1)}}};')
            self.emit(f'for (int i = 0; i < {g}.length; i++) {{')
            self.emit(f'    for (int j = 0; j < {g}[0].length; j++) {{')
            self.emit(f'        System.out.println("t" + i + j + ":" + {g}[i][j]);')
            self.emit('    }')
            self.emit('}')
        elif kind == 'show':
            shows = [m for m in self.array_methods if m['role'] == 'show']
            if shows:
                m = self.pick(shows)
                arg = self.plain_or_fresh(1, m['elem'])[0]
                self.emit(f'{m["name"]}({arg}, {arg}.length + 1);')
            else:
                self.emit(f'System.out.println({name}[{name}.length]);')
        elif kind == 'arraycopy':
            self.emit(f'System.arraycopy({name}, 0, {name}, 1, {name}.length);')
        elif kind == 'fill_range':
            self.emit(f'Arrays.fill({name}, 0, {name}.length + 1, {self.elem_value(elem, 1)});')
        elif kind == 'copy_range' and elem in PRIMS:
            self.emit(f'System.out.println(Arrays.copyOfRange({name}, 2, 1).length);')
        elif kind == 'each_null':
            g = self.name('g')
            self.emit(f'{elem}[][] {g} = new {elem}[1][];')
            self.emit(f'for ({elem} x : {g}[0]) {{')
            self.emit('    System.out.println("t:" + x);')
            self.emit('}')
        else:
            s = self.name('s')
            self.emit(f'String[] {s} = new String[2];')
            self.emit(f'{s}[0] = "ok";')
            self.emit(f'System.out.println({s}[0].length() + {s}[1].length());')

    def dump_any(self, n, t):
        """Prints every element of any array, rows that are null as `null`."""
        dims = dims_of(t)
        if dims == 1:
            return self.dump_array(n)
        s = self.name('s')
        self.emit(f'String {s} = "";')
        if self.chance(0.2) and dims == 2 and not is_private(t):
            self.emit(f'{s} = Arrays.deepToString({n});')
        elif self.chance(0.5):
            self.dump_rows_each(n, elem_of(t), dims, s)
        else:
            self.dump_rows_index(n, dims, s)
        self.emit(f'System.out.println("{n}=" + {s});')

    def dump_rows_index(self, text, dims, s):
        i = self.name('i')
        self.emit(f'for (int {i} = 0; {i} < {text}.length; {i}++) {{')
        self.indent += 1
        if dims == 1:
            self.emit(f'{s} += {text}[{i}] + ",";')
        else:
            self.emit(f'if ({text}[{i}] == null) {{')
            self.emit(f'    {s} += "null";')
            self.emit('} else {')
            self.indent += 1
            self.dump_rows_index(f'{text}[{i}]', dims - 1, s)
            self.indent -= 1
            self.emit('}')
            self.emit(f'{s} += ";";')
        self.indent -= 1
        self.emit('}')

    def dump_rows_each(self, text, elem, dims, s):
        v = self.name('r')
        self.emit(f'for ({elem}{"[]" * (dims - 1)} {v} : {text}) {{')
        self.indent += 1
        if dims == 1:
            self.emit(f'{s} += {v} + ",";')
        else:
            self.emit(f'if ({v} == null) {{')
            self.emit(f'    {s} += "null";')
            self.emit('    continue;')
            self.emit('}')
            self.dump_rows_each(v, elem, dims - 1, s)
            self.emit(f'{s} += ";";')
        self.indent -= 1
        self.emit('}')
