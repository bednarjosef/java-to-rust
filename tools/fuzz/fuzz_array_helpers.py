"""The helper methods of the `--arrays` programs (see fuzz_arrays.py): static methods that take
arrays and return them, each of a role in ROLES, generated before main. `ArrayGen` mixes in
`ArrayHelperGen`, and its `st_arr_call` calls them.
"""

PRIMS = ['byte', 'short', 'char', 'int', 'long', 'float', 'double', 'boolean']
ELEMS = PRIMS + ['String']
DEFAULTS = {
    'int': '0', 'long': '0L', 'double': '0.0', 'float': '0.0f', 'char': "'a'",
    'boolean': 'false', 'byte': '(byte) 0', 'short': '(short) 0', 'String': '"none"',
}


def jtype(t):
    """The Java type of a tracked array type: the tag dropped."""
    return t.rstrip('?!')


def elem_of(t):
    return jtype(t).split('[')[0]


def dims_of(t):
    return jtype(t).count('[]')


def array_type(elem, dims, loose=False, private=False):
    return elem + '[]' * dims + ('?' if loose else '!' if private else '')


def is_private(t):
    return t.endswith('!')


# Helper methods: (role, dimensions). Roles that also accept arrays tagged `?` are listed in
# LOOSE_ROLES; the rest are only called with plain-tagged arrays.
ROLES = [
    ('sum', 1), ('scale', 1), ('same', 1), ('fresh', 1), ('copy', 1), ('keep', 1), ('pick', 1),
    ('into', 1), ('reassign', 1), ('first', 1), ('rec', 1), ('swap', 1), ('show', 1),
    ('sum', 2), ('scale', 2), ('fresh', 2), ('row', 2), ('setRow', 2), ('swapRows', 2),
    ('addRow', 2), ('same', 2), ('sum', 3), ('fresh', 3),
]
LOOSE_ROLES = {'sum', 'scale', 'into', 'reassign', 'first', 'rec', 'swap'}
SHAPE = {1: '', 2: 'Grid', 3: 'Cube'}


class ArrayHelperGen:
    def helpers(self):
        """A few helper methods with array parameters or results, each of a role in ROLES."""
        self.array_methods = []
        for index in range(self.r.randint(3, 8)):
            role, dims = self.pick(ROLES)
            if role == 'keep' and not self.visible(array_type(self.elem_for_keep(), 1)):
                continue
            elem = self.elem_for_keep() if role == 'keep' else self.pick(ELEMS)
            if role == 'keep' and not self.visible(array_type(elem, 1), True):
                continue
            self.helper(role, dims, elem, index)

    def elem_for_keep(self):
        names = [t for _, t, _ in self.statics if t.endswith('[]') and dims_of(t) == 1]
        return elem_of(self.pick(names)) if names else 'int'


    def helper(self, role, dims, elem, index):
        t = array_type(elem, dims)
        jt = jtype(t)
        name = f'{role}{elem.capitalize()}{SHAPE[dims]}{index}'
        m = {'name': name, 'role': role, 'dims': dims, 'elem': elem}
        saved = (self.frames, self.loops, self.method, self.ret, self.self_ok, self.indent)
        self.frames, self.loops, self.method, self.ret, self.self_ok = [[]], [], None, None, False
        self.indent = 1
        lines = getattr(self, 'helper_' + role)(m, elem, dims, jt)
        for line in lines:
            self.emit(line)
        self.frames, self.loops, self.method, self.ret, self.self_ok, self.indent = saved
        self.array_methods.append(m)

    def loop_lines(self, var, body, form=None):
        """A loop over the indexes of `var` whose body is `body` (lines using `i`)."""
        form = form or self.pick(['up', 'up', 'down', 'each'])
        if form == 'down':
            return ([f'for (int i = {var}.length - 1; i >= 0; i--) {{']
                    + ['    ' + b for b in body] + ['}'])
        return [f'for (int i = 0; i < {var}.length; i++) {{'] + ['    ' + b for b in body] + ['}']

    def helper_sum(self, m, elem, dims, jt):
        lines = [f'static int {m["name"]}({jt} p) {{', '    int s = 0;']
        if dims == 1:
            if self.chance(0.4):
                body = ['    for (' + elem + ' x : p) {', f'        s = s * 3 + {self.as_int("x", elem, True)};', '    }']
            else:
                body = ['    ' + b for b in self.loop_lines('p', [f's = s * 3 + {self.as_int("p[i]", elem, True)};'])]
        elif dims == 2:
            if self.chance(0.5):
                body = ['    for (' + elem + '[] row : p) {', '        if (row == null) {',
                        '            continue;', '        }',
                        '        for (int j = 0; j < row.length; j++) {',
                        f'            s += {self.as_int("row[j]", elem, True)} * (j + 1);',
                        '        }', '    }']
            else:
                body = ['    for (int i = 0; i < p.length; i++) {',
                        '        for (int j = 0; p[i] != null && j < p[i].length; j++) {',
                        f'            s += {self.as_int("p[i][j]", elem, True)};',
                        '        }', '    }']
        else:
            body = ['    for (' + elem + '[][] plane : p) {',
                    '        for (int i = 0; plane != null && i < plane.length; i++) {',
                    '            for (int j = 0; plane[i] != null && j < plane[i].length; j++) {',
                    f'                s += {self.as_int("plane[i][j]", elem, True)};',
                    '            }', '        }', '    }']
        return lines + body + ['    return s;', '}']

    def helper_scale(self, m, elem, dims, jt):
        self.frames = [[('k', 'int', False)]]
        lines = [f'static void {m["name"]}({jt} p, int k) {{']
        if dims == 1:
            r = self.r.random()
            if r < 0.5:
                lines += ['    ' + b for b in self.loop_lines('p', [f'p[i] = {self.combine(elem, "p[i]", "k")};'], 'up')]
            elif r < 0.75:
                lines += ['    if (p.length > 0) {',
                          f'        p[Math.floorMod(k, p.length)] = {self.combine(elem, "p[0]", "k")};',
                          '    }']
            else:
                # Writes to the array a for-each is walking: later iterations see them.
                lines += [f'    for ({elem} x : p) {{',
                          f'        p[Math.floorMod(k, p.length)] = {self.combine(elem, "x", "k")};',
                          '        k++;', '    }']
        else:
            inner = 'p[i][j]' if dims == 2 else 'p[i][j][0]'
            guard = 'p[i] != null' if dims == 2 else 'p[i] != null && p[i][j] != null && p[i][j].length > 0'
            limit = 'p[i] != null && j < p[i].length'
            lines += ['    for (int i = 0; i < p.length; i++) {',
                      f'        for (int j = 0; {limit}; j++) {{',
                      f'            if ({guard}) {{',
                      f'                {inner} = {self.combine(elem, inner, "k")};',
                      '            }', '        }', '    }']
        return lines + ['}']

    def helper_same(self, m, elem, dims, jt):
        target = 'p[0]' if dims == 1 else 'p[0][0]'
        return [f'static {jt} {m["name"]}({jt} p) {{',
                f'    {target} = {self.elem_value(elem, 1)};', '    return p;', '}']

    def helper_fresh(self, m, elem, dims, jt):
        self.frames = [[('n', 'int', False)]]
        sizes = '[1 + Math.floorMod(n, 4)]' + ''.join(f'[{self.r.randint(1, 3)}]' for _ in range(dims - 1))
        lines = [f'static {jt} {m["name"]}(int n) {{']
        if dims == 1 and self.chance(0.3):
            lines += ['    if (n > 2) {', f'        return new {jt} {self.literal_grid(elem, 1)};', '    }',
                      f'    {jt} r = new {elem}{sizes};']
        else:
            lines += [f'    {jt} r = new {elem}{sizes};']
        target = 'r[i]' if dims == 1 else 'r[i][0]' if dims == 2 else 'r[i][0][0]'
        value = self.combine(elem, DEFAULTS[elem], 'i') if elem != 'String' else '"f" + i'
        lines += ['    for (int i = 0; i < r.length; i++) {', f'        {target} = {value};', '    }']
        if elem == 'String' and dims == 1:
            pass
        elif elem == 'String':
            # The other elements of a new String grid are null: fill them, rows are plain.
            fill = 'r[i][j] = "g";' if dims == 2 else 'r[i][j][0] = "g";'
            lines = lines[:-3] + ['    for (int i = 0; i < r.length; i++) {',
                                  '        for (int j = 0; j < r[i].length; j++) {',
                                  f'            {fill}', '        }', '    }']
            if dims == 3:
                lines = lines[:-5] + ['    for (int i = 0; i < r.length; i++) {',
                                      '        for (int j = 0; j < r[i].length; j++) {',
                                      '            java.util.Arrays.fill(r[i][j], "c");',
                                      '        }', '    }']
        if elem == 'String' and dims == 1:
            lines = lines[:-3] + ['    for (int i = 0; i < r.length; i++) {',
                                  '        r[i] = "f" + i;', '    }']
        return lines + ['    return r;', '}']

    def helper_copy(self, m, elem, dims, jt):
        r = self.r.random()
        if r < 0.4:
            body = [f'    {jt} r = new {elem}[p.length];', '    for (int i = 0; i < p.length; i++) {',
                    '        r[i] = p[i];', '    }', '    return r;']
        elif r < 0.7 or elem not in PRIMS:
            body = ['    return p.clone();']
        else:
            body = ['    return Arrays.copyOf(p, p.length);']
        return [f'static {jt} {m["name"]}({jt} p) {{'] + body + ['}']

    def helper_keep(self, m, elem, dims, jt):
        statics = [n for n, t, _ in self.statics if t == array_type(elem, 1)]
        m['static'] = self.pick(statics)
        return [f'static void {m["name"]}({jt} p) {{', f'    {m["static"]} = p;', '}']

    def helper_pick(self, m, elem, dims, jt):
        if self.chance(0.5):
            body = ['    return c ? p : q;']
        else:
            body = ['    if (c) {', '        return p;', '    }', '    return q;']
        return [f'static {jt} {m["name"]}({jt} p, {jt} q, boolean c) {{'] + body + ['}']

    def helper_into(self, m, elem, dims, jt):
        return [f'static void {m["name"]}({jt} src, {jt} dst) {{',
                '    for (int i = 0; i < src.length && i < dst.length; i++) {',
                '        dst[i] = src[dst.length - 1 - i];', '    }', '}']

    def helper_reassign(self, m, elem, dims, jt):
        return [f'static int {m["name"]}({jt} p) {{', '    int before = p.length;',
                f'    p = new {elem}[before + 1];', f'    p[before] = {self.elem_value(elem, 1)};',
                '    return before * 10 + p.length;', '}']

    def helper_first(self, m, elem, dims, jt):
        return [f'static {elem} {m["name"]}({jt} p) {{',
                f'    return p.length > 0 ? p[0] : {DEFAULTS[elem]};', '}']

    def helper_rec(self, m, elem, dims, jt):
        return [f'static int {m["name"]}({jt} p, int i) {{', '    if (i >= p.length) {',
                '        return 0;', '    }',
                f'    return {self.as_int("p[i]", elem, True)} + 2 * {m["name"]}(p, i + 1);', '}']

    def helper_swap(self, m, elem, dims, jt):
        return [f'static void {m["name"]}({jt} p, int i, int j) {{', '    if (p.length == 0) {',
                '        return;', '    }', '    int a = Math.floorMod(i, p.length);',
                '    int b = Math.floorMod(j, p.length);', f'    {elem} t = p[a];',
                '    p[a] = p[b];', '    p[b] = t;', '}']

    def helper_show(self, m, elem, dims, jt):
        return [f'static void {m["name"]}({jt} p, int n) {{', '    for (int j = 0; j < n; j++) {',
                f'        System.out.println("show" + j + "=" + p[j]);', '    }', '}']

    def helper_row(self, m, elem, dims, jt):
        return [f'static {elem}[] {m["name"]}({jt} p, int i) {{',
                '    return p[Math.floorMod(i, p.length)];', '}']

    def helper_setRow(self, m, elem, dims, jt):
        return [f'static void {m["name"]}({jt} p, int i, {elem}[] r) {{',
                '    p[Math.floorMod(i, p.length)] = r;', '}']

    def helper_swapRows(self, m, elem, dims, jt):
        return [f'static void {m["name"]}({jt} p, int i, int j) {{',
                '    int a = Math.floorMod(i, p.length);', '    int b = Math.floorMod(j, p.length);',
                f'    {elem}[] t = p[a];', '    p[a] = p[b];', '    p[b] = t;', '}']

    def helper_addRow(self, m, elem, dims, jt):
        return [f'static void {m["name"]}({jt} p, {elem}[] r) {{',
                '    for (int i = 0; i < p.length; i++) {',
                '        for (int j = 0; j < p[i].length && j < r.length; j++) {',
                f'            p[i][j] = {self.combine(elem, "p[i][j]", "(int) j") if elem != "String" else "p[i][j] + r[j].length()"};',
                '        }', '    }', '}'] if elem not in ('boolean',) else [
                f'static void {m["name"]}({jt} p, {elem}[] r) {{',
                '    for (int i = 0; i < p.length; i++) {',
                '        for (int j = 0; j < p[i].length && j < r.length; j++) {',
                '            p[i][j] = p[i][j] ^ r[j];', '        }', '    }', '}']
