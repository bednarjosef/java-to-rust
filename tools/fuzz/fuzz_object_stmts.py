"""The statements of `main` (and of Prog's static methods) in the `--objects` programs, over
the world and with the expressions of fuzz_objects.py, and the deliberate failures and final
dump that Gen.main asks for.
"""

from fuzz_exceptions import ExceptionGen


class ObjectStmtGen(ExceptionGen):

    # Statements of main (and of the static methods of Prog). Each emits lines at the indent.

    def st_obj_decl(self, depth):
        t = self.var_types()
        name = self.name('o')
        if self.chance(0.15):
            self.emit(f'{t} {name} = {self.ref_expr(t, 2, nullable=True)};')
            self.frames[-1].append((name, '?' + t, True))
        else:
            self.emit(f'{t} {name} = {self.ref_expr(t, 2)};')
            self.frames[-1].append((name, t, True))

    def st_obj_assign(self, depth):
        options = [(n, t) for n, t, w in self.all_vars() if w and n not in self.pinned
                   and (t.lstrip('?') in self.decls or t.lstrip('?') == 'Object'
                        or '<' in t) and not t.endswith('[]')]
        if not options:
            return self.st_obj_decl(depth)
        n, t = self.pick(options)
        self.emit(f'{n} = {self.ref_expr(t.lstrip("?"), 2, nullable=t.startswith("?"))};')

    def field_targets(self):
        """Objects whose fields a statement may write: (text, class)."""
        out = [(n, self.decls[vt]) for n, vt in self.object_vars() if vt in self.decls
               and self.decls[vt].kind == 'class']
        for n, at, _ in self.all_vars():
            if at.endswith('[]') and not at.startswith('?') and at[:-2] in self.decls \
                    and self.decls[at[:-2]].kind == 'class':
                out.append((f'{n}[Math.floorMod({self.expr("int", 1)}, {n}.length)]',
                            self.decls[at[:-2]]))
        for n, vt in self.object_vars():
            if vt.startswith('GBox<') and vt[5:-1] in self.decls \
                    and self.decls[vt[5:-1]].kind == 'class':
                out.append((f'{n}.get()', self.decls[vt[5:-1]]))
        return out

    def st_obj_field(self, depth):
        targets = [(t, c) for t, c in self.field_targets()
                   if self.visible_fields(c) or self.visible_links(c)]
        if not targets:
            return self.st_obj_decl(depth)
        obj, c = self.pick(targets)
        fields = self.visible_fields(c)
        links = self.visible_links(c)
        if links and (not fields or self.chance(0.3)):
            f, ft = self.pick(links)
            self.emit(f'{obj}.{f} = {self.ref_expr(ft, 2, nullable=True)};')
            return
        f, ft = self.pick(fields)
        target = f'{obj}.{f}'
        if ft in ('String', '?String'):
            op = '+=' if self.chance(0.3) else '='
            self.emit(f'{target} {op} {self.word(2)};')
            self.guarded(f'{target}.length() > 40', f'{target} = "";')
        elif ft == 'boolean':
            self.emit(f'{target} = {self.bool_expr(2)};')
        elif self.chance(0.2):
            self.emit(f'{target}++;')
        elif self.chance(0.5):
            self.emit(f'{self.compound(target, ft, 2)};')
        else:
            self.emit(f'{target} = {self.source(ft, 2)};')

    def st_obj_call(self, depth):
        options = [(n, vt, k) for n, vt in self.object_vars() if vt in self.decls
                   for k in self.visible_group_names(self.decls[vt])]
        if not options:
            return self.st_obj_decl(depth)
        n, vt, k = self.pick(options)
        if self.chance(0.2) and self.decls[vt].kind == 'class':
            self.emit(f'{n}.touch({self.expr("int", 2)});')
            return
        self.emit(f'{n}.{self.call_text(k, 2)};')

    def st_obj_print(self, depth):
        parts = [f'"o{self.count}:"'] + [self.object_part(3)
                                          for _ in range(self.r.randint(1, 3))]
        self.emit('System.out.println(' + ' + " " + '.join(parts) + ');')

    def st_obj_test(self, depth):
        objs = self.object_vars(nonnull=False)
        kind = self.pick(['pattern', 'pattern', 'cast', 'negated', 'chain'])
        if kind == 'chain':
            options = [n for n, vt in objs if vt.lstrip('?') == 'Object']
            if options:
                self.instanceof_chain(self.pick(options), depth)
                return
            kind = 'pattern'
        options = [(n, c) for n, vt in objs for c in self.classes()
                   if '<' not in vt and vt.lstrip('?') != c.name
                   and self.castable(vt.lstrip('?'), c.name)]
        if not options:
            return self.st_obj_decl(depth)
        n, c = self.pick(options)
        p = self.name('q')
        bound = [(p, c.name, False)]
        if kind == 'pattern':
            self.emit(f'if ({n} instanceof {c.name} {p}) {{')
            self.block(depth, extra=bound)
            if self.chance(0.4):
                self.emit('} else {')
                self.block(depth)
        elif kind == 'cast':
            self.emit(f'if ({n} instanceof {c.name}) {{')
            self.block(depth, extra=bound, head=f'{c.name} {p} = ({c.name}) {n};')
        else:
            self.emit(f'if (!({n} instanceof {c.name} {p})) {{')
            self.block(depth)
            self.emit('} else {')
            self.block(depth, extra=bound)
        self.emit('}')

    def instanceof_chain(self, n, depth):
        tests = ['String', 'Integer'] + [c.name for c in self.classes()]
        self.r.shuffle(tests)
        first = True
        for t in tests[:self.r.randint(1, 3)]:
            p = self.name('q')
            self.emit(f'{"if" if first else "} else if"} ({n} instanceof {t} {p}) {{')
            self.block(depth, extra=[(p, t, False)])
            first = False
        if self.chance(0.5):
            self.emit('} else {')
            self.block(depth)
        self.emit('}')

    def st_obj_null(self, depth):
        options = [(n, t) for n, t, w in self.all_vars() if t.startswith('?')
                   and not t.endswith('[]')]
        if not options:
            return self.st_obj_decl(depth)
        n, t = self.pick(options)
        narrowed = [(n, t[1:], False)]
        self.pinned.append(n)
        if self.chance(0.5):
            self.emit(f'if ({n} != null) {{')
            self.block(depth, extra=narrowed)
            if self.chance(0.5):
                self.emit('} else {')
                self.block(depth)
        else:
            self.emit(f'if ({n} == null) {{')
            self.block(depth)
            self.emit('} else {')
            self.block(depth, extra=narrowed)
        self.emit('}')
        self.pinned.pop()

    def st_obj_array(self, depth):
        t = self.pick(self.element_types())
        name = self.name('arr')
        if self.chance(0.6):
            elems = [self.ref_expr(t, 2, poly=False) for _ in range(self.r.randint(1, 3))]
            self.emit(f'{t}[] {name} = new {t}[] {{{", ".join(elems)}}};')
            self.frames[-1].append((name, t + '[]', True))
        else:
            self.emit(f'{t}[] {name} = new {t}[{self.r.randint(1, 4)}];')
            self.frames[-1].append((name, f'?{t}[]', True))
            if self.chance(0.6):
                self.emit(f'{name}[0] = {self.ref_expr(t, 2)};')

    def object_arrays(self):
        return [(n, at) for n, at, _ in self.all_vars() if at.endswith('[]')
                and at.lstrip('?')[:-2] in self.element_types()]

    def st_obj_store(self, depth):
        arrays = self.object_arrays()
        if not arrays:
            return self.st_obj_array(depth)
        n, at = self.pick(arrays)
        index = f'Math.floorMod({self.expr("int", 2)}, {n}.length)'
        value = self.ref_expr(at.lstrip('?')[:-2], 2, nullable=at.startswith('?'))
        self.emit(f'{n}[{index}] = {value};')

    def st_obj_loop(self, depth):
        arrays = self.object_arrays()
        stacks = [(n, vt) for n, vt in self.object_vars() if vt.startswith('GStack<')]
        ctx = ('loop', None)
        e = self.name('e')
        if stacks and (not arrays or self.chance(0.3)):
            n, vt = self.pick(stacks)
            i = self.name('i')
            elem = vt[7:-1]
            self.emit(f'for (int {i} = 0; {i} < 3 && !{n}.isEmpty(); {i}++) {{')
            self.block(depth, ctx, extra=[(i, 'int', False), (e, elem, False)],
                       head=f'{elem} {e} = {n}.pop();')
            self.emit('}')
            return
        if not arrays:
            return self.st_obj_array(depth)
        n, at = self.pick(arrays)
        elem = at.lstrip('?')[:-2]
        etype = ('?' if at.startswith('?') else '') + elem
        if self.chance(0.5):
            self.emit(f'for ({elem} {e} : {n}) {{')
            self.block(depth, ctx, extra=[(e, etype, False)])
        else:
            i = self.name('i')
            self.emit(f'for (int {i} = 0; {i} < {n}.length; {i}++) {{')
            self.block(depth, ctx, extra=[(i, 'int', False), (e, etype, False)],
                       head=f'{elem} {e} = {n}[{i}];')
        self.emit('}')

    def st_obj_container(self, depth):
        options = [(n, vt) for n, vt in self.object_vars() if '<' in vt]
        if not options:
            t = self.generic_type()
            name = self.name('o')
            self.emit(f'{t} {name} = {self.ref_expr(t, 2)};')
            self.frames[-1].append((name, t, True))
            return
        n, vt = self.pick(options)
        head, args = self.split_generic(vt)
        if head == 'GBox':
            self.emit(f'{n}.set({self.ref_expr(args[0], 2, poly=False)});')
        elif head == 'GStack':
            if self.chance(0.7):
                self.emit(f'{n}.push({self.ref_expr(args[0], 2, poly=False)});')
            else:
                self.guarded(f'!{n}.isEmpty()', f'{n}.pop();')
        elif head == 'GSorted':
            self.emit(f'{n}.add({self.ref_expr(args[0], 2, poly=False)});')
        else:
            self.emit(f'System.out.println("pair " + {self.object_part(2)});')

    def st_obj_static(self, depth):
        options = [(x, s) for x in self.classes() for s in x.statics]
        if not options:
            return self.st_obj_print(depth)
        x, s = self.pick(options)
        holder = self.pick([x] + self.descendants(x))
        if self.chance(0.5):
            self.emit(f'{holder.name}.{s} += {self.expr("int", 2)};')
        else:
            self.emit(f'System.out.println("static " + {holder.name}.{s});')

    # Deliberate failures and the final dump, called from Gen.main.

    def object_throw(self):
        """A failing cast or a null dereference, where Java exits with code 1. False when the
        program's types allow neither."""
        pairs = [(a, b) for a in self.concretes() for b in self.classes()
                 if not self.is_sub(a.name, b.name)]
        with_fields = [c for c in self.classes() if self.visible_fields(c)]
        if self.chance(0.3) and self.throw_uncaught():
            return True
        bad = self.name('bad')
        if pairs and (not with_fields or self.chance(0.5)):
            a, b = self.pick(pairs)
            self.emit(f'Object {bad} = {self.new_object(a.name, 1)};')
            self.emit(f'System.out.println((({b.name}) {bad}) == null);')
            return True
        if with_fields:
            c = self.pick(with_fields)
            self.emit(f'{c.name} {bad} = null;')
            self.emit(f'System.out.println({bad}.{self.pick(self.visible_fields(c))[0]});')
            return True
        return False

    def dump_object(self, n, t):
        """Prints variable `n` at the end of main if it is of an object type; False if not."""
        bare = t.lstrip('?')
        elem = bare[:-2] if bare.endswith('[]') else None
        if elem is not None and elem not in self.element_types():
            return False
        if elem is None and not (bare in self.decls or bare == 'Object' or '<' in bare):
            return False
        if elem is not None:
            if self.printable(elem):
                s, i = self.name('s'), self.name('i')
                self.emit(f'String {s} = "";')
                self.emit(f'for (int {i} = 0; {i} < {n}.length; {i}++) {{')
                self.indent += 1
                self.emit(f'{s} += {n}[{i}] + ",";')
                self.indent -= 1
                self.emit('}')
                self.emit(f'System.out.println("{n}=" + {s});')
            else:
                self.emit(f'System.out.println("{n}.length=" + {n}.length);')
            return True
        if self.printable(bare):
            self.emit(f'System.out.println("{n}=" + {n});')
        elif bare in self.decls and self.visible_fields(self.decls[bare]):
            f = self.visible_fields(self.decls[bare])[0][0]
            read = f'{n}.{f}' if not t.startswith('?') else \
                f'({n} == null ? "null" : "" + {n}.{f})'
            self.emit(f'System.out.println("{n}.{f}=" + {read});')
        return True
