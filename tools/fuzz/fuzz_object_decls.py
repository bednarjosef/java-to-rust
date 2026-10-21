"""The declarations of the `--objects` programs: interfaces, class trees, generic classes and
the static helper methods of Prog, written from the world that `ObjectGen.plan_world` decided.

Bodies use Gen's primitive and string statements with the fields, parameters and statics in
scope, and add calls of their own: a method calls only groups of a lower rank, or `super` of its
own group (see fuzz_objects.py), so every call chain ends. Object expressions are off in bodies
(`obj_hooks`), since they could call any method.
"""

GENERIC_CLASSES = {
    'GBox': [
        'static class GBox<T> {',
        '    T val;',
        '    GBox(T val) {',
        '        this.val = val;',
        '    }',
        '    T get() {',
        '        return val;',
        '    }',
        '    void set(T v) {',
        '        val = v;',
        '    }',
        'TEXT    public String toString() {',
        'TEXT        return "GBox(" + val + ")";',
        'TEXT    }',
        '}',
    ],
    'GPair': [
        'static class GPair<A, B> {',
        '    final A first;',
        '    final B second;',
        '    GPair(A first, B second) {',
        '        this.first = first;',
        '        this.second = second;',
        '    }',
        '    A first() {',
        '        return first;',
        '    }',
        '    B second() {',
        '        return second;',
        '    }',
        '    GPair<B, A> swap() {',
        '        return new GPair<>(second, first);',
        '    }',
        'TEXT    public String toString() {',
        'TEXT        return "<" + first + ", " + second + ">";',
        'TEXT    }',
        '}',
    ],
    'GNode': [
        'static class GNode<T> {',
        '    T val;',
        '    GNode<T> next;',
        '    GNode(T val, GNode<T> next) {',
        '        this.val = val;',
        '        this.next = next;',
        '    }',
        '}',
    ],
    'GStack': [
        'static class GStack<T> {',
        '    GNode<T> head;',
        '    int size;',
        '    void push(T v) {',
        '        head = new GNode<>(v, head);',
        '        size++;',
        '    }',
        '    T pop() {',
        '        T v = head.val;',
        '        head = head.next;',
        '        size--;',
        '        return v;',
        '    }',
        '    T peek() {',
        '        return head.val;',
        '    }',
        '    boolean isEmpty() {',
        '        return head == null;',
        '    }',
        '    int size() {',
        '        return size;',
        '    }',
        'TEXT    public String toString() {',
        'TEXT        String s = "[";',
        'TEXT        for (GNode<T> n = head; n != null; n = n.next) {',
        'TEXT            s += n.val + ";";',
        'TEXT        }',
        'TEXT        return s + "]";',
        'TEXT    }',
        '}',
    ],
    'GSorted': [
        'static class GSorted<T extends Comparable<T>> {',
        '    GNode<T> head;',
        '    int size;',
        '    void add(T v) {',
        '        if (head == null || v.compareTo(head.val) <= 0) {',
        '            head = new GNode<>(v, head);',
        '        } else {',
        '            GNode<T> n = head;',
        '            while (n.next != null && n.next.val.compareTo(v) < 0) {',
        '                n = n.next;',
        '            }',
        '            n.next = new GNode<>(v, n.next);',
        '        }',
        '        size++;',
        '    }',
        '    T get(int i) {',
        '        GNode<T> n = head;',
        '        for (int k = 0; k < i; k++) {',
        '            n = n.next;',
        '        }',
        '        return n.val;',
        '    }',
        '    int size() {',
        '        return size;',
        '    }',
        'TEXT    public String toString() {',
        'TEXT        String s = "{";',
        'TEXT        for (GNode<T> n = head; n != null; n = n.next) {',
        'TEXT            s += n.val + " ";',
        'TEXT        }',
        'TEXT        return s + "}";',
        'TEXT    }',
        '}',
    ],
}


class ObjectDeclGen:
    def object_decls(self):
        """Plans the world and emits its types, helpers and object statics into Prog."""
        self.plan_world()
        saved = (self.kinds, self.methods, self.obj_hooks)
        self.kinds = {k: w for k, w in self.kinds.items() if not k.startswith('obj_')}
        self.methods, self.obj_hooks = [], False
        for d in self.decls.values():
            if d.kind == 'interface':
                self.emit_interface(d)
        for c in self.classes_in_order():
            self.emit_class(c)
        for g in ('GBox', 'GPair', 'GNode', 'GStack', 'GSorted'):
            if g in self.generics:
                for line in GENERIC_CLASSES[g]:
                    if line.startswith('TEXT'):
                        if not self.generic_text:
                            continue
                        line = line[4:]
                    self.emit(line)
        self.emit_helpers()
        self.exception_decls()
        self.kinds, self.methods, self.obj_hooks = saved
        if self.chance(0.5):
            t = self.pick([d.name for d in self.decls.values()])
            self.emit(f'static {t} sObj = {self.new_object(t, 1)};')
            self.statics.append(('sObj', t, True))

    def body_scope(self, owner, params, instance=True):
        """The variables a body of `owner` sees besides Prog's statics."""
        scope = []
        if owner.kind == 'class':
            if instance:
                scope += [(f, t, True) for f, t in self.visible_fields(owner)]
                scope += [(f, '?' + t, True) for f, t in self.visible_links(owner)]
            for a in self.ancestors(owner):
                scope += [(s, 'int', True) for s in a.statics]
        for i in (self.all_ifaces(owner) if owner.kind == 'class' else [owner]):
            scope += [(k, 'int', False) for k, _ in i.constants]
        return scope + [(n, t, True) for n, t in params]

    def enter_body(self, scope, ret):
        saved = (self.frames, self.loops, self.method, self.ret, self.self_ok, self.indent)
        self.frames, self.loops = [scope], []
        self.method = {'name': '', 'ret': ret, 'params': [], 'recursive': False} \
            if ret not in (None, 'String') else None
        self.ret = ret if ret != 'String' else None
        self.self_ok = False
        return saved

    def leave_body(self, saved):
        self.frames, self.loops, self.method, self.ret, self.self_ok, self.indent = saved

    def emit_interface(self, d):
        self.emit(f'interface {d.name} {{')
        self.indent += 1
        for k, v in d.constants:
            self.emit(f'int {k} = {v};')
        for key, impl in d.impls.items():
            self.emit_method(d, key, impl == 'abstract')
        for m in d.static_methods:
            self.emit_static_method(d, m)
        self.indent -= 1
        self.emit('}')

    def emit_class(self, c):
        head = 'static ' + ('abstract ' if c.abstract else '') + f'class {c.name}'
        if c.sup:
            head += f' extends {c.sup.name}'
        impls = [i.name for i in c.ifaces] + ([f'Comparable<{c.name}>'] if c.comparable else [])
        if impls:
            head += ' implements ' + ', '.join(impls)
        self.emit(head + ' {')
        self.indent += 1
        saved = self.enter_body([], None)
        for f, t in c.fields:
            if t == '?String':
                self.emit(f'String {f};')
            elif t == 'String' or self.chance(0.7):
                self.emit(f'{t} {f} = {self.value_of(t, 1)};')
            else:
                self.emit(f'{t} {f};')
        self.leave_body(saved)
        for f, t in c.links:
            self.emit(f'{t} {f};')
        for s in c.statics:
            self.emit(f'static int {s} = {self.pick(["0", "1", "sInt", "(sInt++)"])};')
        for i, sig in enumerate(c.ctors):
            self.emit_ctor(c, i, sig)
        for key, impl in c.impls.items():
            self.emit_method(c, key, impl == 'abstract')
        if c.sup is None:
            self.emit_touch(c)
        if c.text:
            self.emit_text(c)
        if c.equals:
            self.emit_equals(c)
        if c.hash:
            self.emit_hash(c)
        if c.comparable:
            self.emit_compare(c)
        for m in c.static_methods:
            self.emit_static_method(c, m)
        self.indent -= 1
        self.emit('}')

    def emit_ctor(self, c, index, sig):
        params = [(f'p{i}', t) for i, t in enumerate(sig)]
        self.emit(f'{c.name}({", ".join(f"{t} {n}" for n, t in params)}) {{')
        saved = self.enter_body(self.body_scope(c, params, instance=False), None)
        self.indent += 1
        if index > 0 and self.chance(0.7):
            self.emit(f'this({self.ctor_args(c.ctors[0], 1)});')
        elif c.sup:
            ssig = self.pick(c.sup.ctors)
            if ssig or self.chance(0.5):
                self.emit(f'super({self.ctor_args(ssig, 1)});')
            elif [] not in c.sup.ctors:
                self.emit(f'super({self.ctor_args(c.sup.ctors[0], 1)});')
        self.frames = [self.body_scope(c, params)]
        for n, t in params:
            fields = [f for f, ft in c.fields if ft == t]
            if fields and self.chance(0.7):
                self.emit(f'this.{self.pick(fields)} = {n};')
        for f, t in c.fields:
            if t == 'int' and self.chance(0.3):
                self.emit(f'{f} += {self.expr("int", 1)};')
        for s in c.statics:
            self.emit(f'{s}++;')
        if self.chance(0.3):
            fields = [f for f, _ in self.visible_fields(c)]
            shown = ' + " " + '.join(fields[:2]) if fields else '""'
            self.emit(f'System.out.println("new {c.name} " + {shown});')
        if self.chance(0.04):
            calls = [k for k in self.visible_group_names(c) if self.groups[k]['ret'] == 'int'
                     and 'peer' not in self.groups[k]['params']]
            if calls:
                self.emit(f'acc += {self.inner_call(self.pick(calls), 1)};')
        if self.chance(0.3):
            self.stmt(1)
        self.indent -= 1
        self.emit('}')
        self.leave_body(saved)

    def inner_call(self, key, d):
        """A call of group `key` inside a body, with int arguments."""
        g = self.groups[key]
        args = []
        for p in g['params']:
            if p in ('long', 'double'):
                args.append(f'(({p}) {self.expr("int", d)})')
            elif p == 'peer':
                args.append('this')
            else:
                args.append(self.expr('int', d))
        return f'{g["name"]}({", ".join(args)})'

    def emit_method(self, owner, key, abstract):
        g = self.groups[key]
        root = self.root(g['owner']).name if g['owner'].kind == 'class' else None
        params = [(f'a{i}', root if p == 'peer' else p) for i, p in enumerate(g['params'])]
        sig = ', '.join(f'{t} {n}' for n, t in params)
        public = 'public ' if g['owner'].kind == 'interface' and owner.kind == 'class' else ''
        text = f'{public}{g["ret"]} {g["name"]}({sig})'
        if abstract:
            self.emit(f'{"" if owner.kind == "interface" else "abstract "}{text};')
            return
        self.emit(f'{"default " if owner.kind == "interface" else ""}{text} {{')
        saved = self.enter_body(self.body_scope(owner, params), g['ret'])
        self.indent += 1
        for _ in range(self.r.randint(0, 2)):
            self.stmt(1)
        parts = []
        sup = self.super_call(owner, key)
        if sup:
            r = self.name('r')
            self.emit(f'{g["ret"]} {r} = {sup};')
            parts.append(r)
        lower = [k for k in self.visible_group_names(owner)
                 if self.groups[k]['rank'] < g['rank'] and self.groups[k]['ret'] == 'int']
        for k in self.r.sample(lower, min(len(lower), self.r.randint(0, 2))):
            if 'peer' in self.groups[k]['params']:
                continue
            parts.append(self.inner_call(k, 1))
        peers = [n for n, t in params if t == root]
        if peers:
            self.peer_parts(owner, peers[0], g['rank'], parts)
        if owner.kind == 'class':
            for f, t in self.visible_links(owner):
                if self.chance(0.5):
                    self.link_part(f, t, g['rank'], parts)
        if g['ret'] == 'String':
            shown = ' + '.join(f'({p})' for p in parts) if parts else '""'
            self.emit(f'return "{owner.name}." + {self.expr("int", 2)} + ":" + {shown};')
        else:
            self.emit(f'return {" + ".join([self.expr("int", 2)] + parts)};')
        self.indent -= 1
        self.emit('}')
        self.leave_body(saved)

    def super_call(self, owner, key):
        """`super.m(..)` or `I.super.m(..)` when `owner` overrides an implementation."""
        if owner.kind != 'class' or self.chance(0.4):
            return None
        if owner.sup and self.implementation(owner.sup, key) == 'concrete':
            return 'super.' + self.inner_call(key, 1)
        for i in owner.ifaces:
            others = [j for j in owner.ifaces if j is not i and key in j.impls]
            if i.impls.get(key) == 'concrete' and not others and (
                    owner.sup is None or self.implementation(owner.sup, key) is None):
                return f'{i.name}.super.' + self.inner_call(key, 1)
        return None

    def peer_parts(self, owner, peer, rank, parts):
        """Reads of another object of the tree, passed as a parameter."""
        root = self.root(owner)
        fields = [f for f, t in self.visible_fields(root) if t == 'int']
        if fields:
            parts.append(f'{peer}.{self.pick(fields)}')
        calls = [k for k in self.visible_group_names(root) if self.groups[k]['rank'] < rank
                 and self.groups[k]['ret'] == 'int' and 'peer' not in self.groups[k]['params']]
        if calls:
            parts.append(f'{peer}.{self.inner_call(self.pick(calls), 1)}')

    def link_part(self, link, t, rank, parts):
        d = self.decls[t]
        fields = [f for f, ft in self.visible_fields(d) if ft == 'int']
        calls = [k for k in self.visible_group_names(d) if self.groups[k]['rank'] < rank
                 and self.groups[k]['ret'] == 'int' and 'peer' not in self.groups[k]['params']]
        reads = [f'{link}.{f}' for f in fields] + [f'{link}.{self.inner_call(k, 1)}'
                                                   for k in calls]
        if reads:
            parts.append(f'({link} != null ? {self.pick(reads)} : -1)')

    def emit_touch(self, c):
        """`touch` returns `this`, for aliases made by a call."""
        self.emit(f'{c.name} touch(int k) {{')
        self.indent += 1
        fields = [f for f, t in c.fields if t in ('int', 'long', 'double')]
        if fields:
            self.emit(f'{self.pick(fields)} += k;')
        self.emit('return this;')
        self.indent -= 1
        self.emit('}')

    def emit_text(self, c):
        self.emit('public String toString() {')
        self.indent += 1
        fields = [f for f, _ in self.visible_fields(c)[:3]]
        text = ' + ", " + '.join(fields) if fields else '""'
        tail = ''
        if c.sup and self.has_text(c.sup) and self.chance(0.5):
            tail = ' + "/" + super.toString()'
        self.emit(f'return "{c.name}(" + {text} + ")"{tail};')
        self.indent -= 1
        self.emit('}')

    def emit_equals(self, c):
        self.emit('public boolean equals(Object o) {')
        self.indent += 1
        self.emit('if (this == o) {')
        self.emit('    return true;')
        self.emit('}')
        other = self.name('that')
        if self.chance(0.5):
            self.emit(f'if (!(o instanceof {c.name} {other})) {{')
            self.emit('    return false;')
            self.emit('}')
        else:
            self.emit(f'if (!(o instanceof {c.name})) {{')
            self.emit('    return false;')
            self.emit('}')
            self.emit(f'{c.name} {other} = ({c.name}) o;')
        tests = []
        for f, t in self.visible_fields(c):
            if t == 'String':
                tests.append(f'{f}.equals({other}.{f})')
            elif t not in ('double', '?String'):
                tests.append(f'{f} == {other}.{f}')
        self.emit(f'return {" && ".join(tests) if tests else "true"};')
        self.indent -= 1
        self.emit('}')

    def emit_hash(self, c):
        self.emit('public int hashCode() {')
        self.indent += 1
        terms = ['7']
        for f, t in self.visible_fields(c):
            if t == 'int':
                terms.append(f)
            elif t == 'long':
                terms.append(f'(int) {f}')
            elif t == 'String':
                terms.append(f'{f}.hashCode()')
            elif t == 'boolean':
                terms.append(f'({f} ? 1 : 0)')
        self.emit(f'return {" * 31 + ".join(terms)};')
        self.indent -= 1
        self.emit('}')

    def emit_compare(self, c):
        self.emit(f'public int compareTo({c.name} o) {{')
        self.indent += 1
        keys = [f for f, t in self.visible_fields(c) if t in ('int', 'long', 'double')]
        if keys:
            k = self.pick(keys)
            self.emit(f'return {k} < o.{k} ? -1 : ({k} == o.{k} ? 0 : 1);')
        else:
            self.emit('return 0;')
        self.indent -= 1
        self.emit('}')

    def emit_static_method(self, owner, name):
        self.emit(f'static int {name}(int x) {{')
        scope = self.body_scope(owner, [('x', 'int')], instance=False)
        saved = self.enter_body(scope, 'int')
        self.indent += 1
        if self.chance(0.4):
            self.stmt(1)
        self.emit(f'return {self.expr("int", 2)};')
        self.indent -= 1
        self.emit('}')
        self.leave_body(saved)

    def emit_helpers(self):
        """Static methods of Prog: overloads, generic methods, a factory, tests by pattern."""
        if self.chain:
            for c in self.chain:
                self.emit(f'static String which({c.name} x) {{')
                self.emit(f'    return "which {c.name}";')
                self.emit('}')
            self.emit('static String which(Object x) {')
            self.emit('    return "which Object";')
            self.emit('}')
            for t in ('int', 'long', 'String'):
                self.emit(f'static String which({t} x) {{')
                self.emit(f'    return "which {t} " + x;')
                self.emit('}')
        if 'pick' in self.obj_helpers:
            self.emit('static <T> T pick(boolean c, T a, T b) {')
            self.emit('    return c ? a : b;')
            self.emit('}')
        if 'boxOf' in self.obj_helpers:
            self.emit('static <T> GBox<T> boxOf(T v) {')
            self.emit('    return new GBox<>(v);')
            self.emit('}')
        if 'maxOf' in self.obj_helpers:
            self.emit('static <T extends Comparable<T>> T maxOf(T a, T b) {')
            self.emit('    return a.compareTo(b) >= 0 ? a : b;')
            self.emit('}')
        if 'describe' in self.obj_helpers:
            self.emit_describe()
        for c in self.classes():
            if f'make{c.name}' in self.obj_helpers:
                self.emit_factory(c)

    def emit_describe(self):
        self.emit('static String describe(Object o) {')
        self.indent += 1
        self.emit('if (o instanceof String s) {')
        self.emit('    return "str " + s.length();')
        self.emit('}')
        self.emit('if (o instanceof Integer i) {')
        self.emit('    return "int " + (i + 1);')
        self.emit('}')
        for c in self.r.sample(self.classes(), min(2, len(self.classes()))):
            v = self.name('c')
            fields = self.visible_fields(c)
            shown = f'{v}.{fields[0][0]}' if fields else '""'
            if self.printable(c.name) and self.chance(0.5):
                shown = v
            self.emit(f'if (o instanceof {c.name} {v}) {{')
            self.emit(f'    return "{c.name} " + {shown};')
            self.emit('}')
        self.emit('return o == null ? "null" : "other";')
        self.indent -= 1
        self.emit('}')

    def emit_factory(self, root):
        self.emit(f'static {root.name} make{root.name}(int k) {{')
        self.indent += 1
        saved = self.enter_body([('k', 'int', False)], None)
        options = self.subclasses_of(root.name)
        for c in self.r.sample(options, min(2, len(options)))[:-1]:
            self.emit(f'if (k % 2 == 0) {{')
            self.emit(f'    return {self.new_object(c.name, 1)};')
            self.emit('}')
        self.emit(f'return {self.new_object(self.pick(options).name, 1)};')
        self.leave_body(saved)
        self.indent -= 1
        self.emit('}')
