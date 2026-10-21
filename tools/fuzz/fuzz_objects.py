"""Class trees, interfaces, `Object` values and generic containers for fuzz.py, enabled by
`--objects`.

`Gen` mixes in `ObjectGen`. Before anything is emitted, `plan_world` decides the program's
types: one or two interfaces with abstract, default and static methods and a constant; one or two
class trees of one to three levels, with abstract and concrete classes, fields with initializers,
static counters, constructors calling `super(...)` and `this(...)`, overridden and overloaded
methods, `super.m()` calls, `toString`, `equals` and `hashCode` overrides, and a class that
implements `Comparable`; and generic classes (a box, a pair, a linked stack, a sorted list) and
generic methods. Each `st_obj_*` method is a statement kind of `main`, and the other methods build
expressions.

Every program terminates and is deterministic:
- Each instance method group has a rank, and a method body calls only groups of a lower rank (on
  `this` or on another object) or `super` of its own group, so no call chain can repeat. No
  method creates an object, so constructors cannot recurse either.
- No identity hash is printed: an object is printed only when every class it can be has a
  program `toString` (`printable`), and `hashCode` only when every such class overrides it.
- No value that may be null is dereferenced, except on purpose (see `object_throw`). A variable
  that may hold null has its type written with a leading `?` in the scopes (`?C1`), an array whose
  elements may be null `?C1[]`; both are only read through a null test.

Variables of object types live in Gen's scopes beside the primitive ones, with types the
primitive code never asks for, so it never sees them.
"""

from fuzz_object_decls import ObjectDeclGen
from fuzz_object_stmts import ObjectStmtGen

BOXES = {'Integer': 'int', 'Long': 'long', 'Double': 'double', 'Character': 'char',
         'Boolean': 'boolean'}
# `?String` is a String field without an initializer, which is null until it is assigned.
FIELD_TYPES = ['int', 'int', 'int', 'long', 'double', 'boolean', 'String', 'String', '?String']
CTOR_PARAM_TYPES = ['int', 'int', 'long', 'String']
WORDS = ['"ab"', '"x"', '"hello"', '"Zed"', '""', '"q r"', '"日本"']
GENERICS = ['GBox', 'GPair', 'GStack', 'GSorted']

OBJECT_STATEMENTS = {
    'obj_decl': 7, 'obj_assign': 3, 'obj_field': 4, 'obj_call': 4, 'obj_print': 7,
    'obj_test': 4, 'obj_array': 2, 'obj_store': 2, 'obj_loop': 2, 'obj_null': 2,
    'obj_container': 4, 'obj_static': 1, 'obj_try': 3,
}
OBJECT_NESTING = {'obj_test', 'obj_loop', 'obj_null', 'obj_try'}


class Decl:
    """A class or interface of the generated program."""

    def __init__(self, name, kind):
        self.name = name
        self.kind = kind  # 'class' or 'interface'
        self.abstract = False
        self.sup = None  # the superclass, a Decl
        self.ifaces = []  # interfaces implemented (or extended, for an interface)
        self.subs = []
        self.fields = []  # own instance fields of primitive or String type: (name, type)
        self.links = []  # own object fields, which may be null: (name, type)
        self.statics = []  # own static int fields
        self.constants = []  # an interface's int constants: (name, value)
        self.static_methods = []  # own static int methods of one int parameter
        self.ctors = []  # parameter type lists, distinct lengths
        self.impls = {}  # group name -> 'abstract' or 'concrete', declared here
        self.text = self.equals = self.hash = False  # declares toString / equals / hashCode
        self.comparable = False  # declares `implements Comparable<Self>`


class ObjectGen(ObjectDeclGen, ObjectStmtGen):
    # The world: decided before anything is emitted.

    def plan_world(self):
        self.decls = {}  # name -> Decl
        self.groups = {}  # name -> dict: name, params, ret, rank, owner, java
        self.obj_hooks = True  # object expressions may appear (off in class bodies)
        self.pinned = []  # null-tested variables that must not be assigned in this block
        self.generics = set(self.r.sample(GENERICS, self.r.randint(1, 3)))
        if 'GSorted' in self.generics or 'GStack' in self.generics:
            self.generics.add('GNode')
        self.generic_text = self.chance(0.5)  # the generic classes have a toString
        ifaces = [self.plan_interface() for _ in range(self.r.randint(0, 2))]
        trees = [self.plan_tree(ifaces) for _ in range(self.r.randint(1, 2))]
        for i in ifaces:
            if not any(self.is_sub(c.name, i.name) for c in self.classes()):
                self.pick([c for c in self.classes()]).ifaces.append(i)
        if self.chance(0.6):
            self.pick(trees).comparable = True
        self.comparable = next((c for c in self.classes() if c.comparable), None)
        for c in self.classes_in_order():
            self.plan_members(c)
        self.chain = self.overload_chain(trees) if self.chance(0.6) else []
        self.helpers = {h for h, p in (('pick', 0.5), ('describe', 0.5), ('maxOf', 0.6))
                        if self.chance(p)}
        if 'GBox' in self.generics and self.chance(0.5):
            self.helpers.add('boxOf')
        self.helpers |= {f'make{t.name}' for t in trees if self.chance(0.4)}

    def plan_interface(self):
        d = Decl(self.name('I'), 'interface')
        self.decls[d.name] = d
        for _ in range(self.r.randint(1, 2)):
            self.new_group(d, 'abstract')
        if self.chance(0.7):
            self.new_group(d, 'concrete')  # a default method
        if self.chance(0.6):
            d.constants.append((self.name('K'), self.r.randint(-3, 9)))
        if self.chance(0.6):
            d.static_methods.append(self.name('ism'))
        return d

    def plan_tree(self, ifaces):
        root = self.plan_class(None, ifaces)
        frontier = [root]
        for _ in range(self.r.randint(0, 3)):
            parent = self.pick(frontier)
            if len(self.ancestors(parent)) >= 3:
                continue
            child = self.plan_class(parent, ifaces)
            frontier.append(child)
        for c in [root] + self.descendants(root):
            if not c.subs:
                c.abstract = False  # every leaf is concrete
        return root

    def plan_class(self, sup, ifaces):
        d = Decl(self.name('C'), 'class')
        self.decls[d.name] = d
        d.sup = sup
        if sup:
            sup.subs.append(d)
        d.abstract = self.chance(0.4)
        if ifaces and self.chance(0.4):
            d.ifaces = self.r.sample(ifaces, self.r.randint(1, len(ifaces)))
        for _ in range(self.r.randint(0 if sup else 1, 2)):
            d.fields.append((self.name('f'), self.pick(FIELD_TYPES)))
        if sup is None and self.chance(0.4):
            d.links.append((self.name('link'), d.name))
        if self.chance(0.4):
            d.statics.append(self.name('cnt'))
        if self.chance(0.3):
            d.static_methods.append(self.name('sm'))
        n = self.r.randint(0, 2)
        d.ctors.append([self.pick(CTOR_PARAM_TYPES) for _ in range(n)])
        if self.chance(0.4):
            d.ctors.append([self.pick(CTOR_PARAM_TYPES) for _ in range(n + 1)])
        if sup is None:
            for _ in range(self.r.randint(1, 3)):
                self.new_group(d, 'abstract' if self.chance(0.3) else 'concrete')
            if self.chance(0.5):
                self.new_group(d, 'concrete', name='ov', params=['int'])
        else:
            if self.chance(0.3):
                self.new_group(d, 'abstract' if self.chance(0.3) else 'concrete')
            if self.chance(0.3) and 'ov(int)' in self.visible_group_names(d):
                self.new_group(d, 'concrete', name='ov', params=[self.pick(['long', 'double'])])
        return d

    def new_group(self, owner, impl, name=None, params=None, ret=None):
        """A new method group declared in `owner`, with a random rank."""
        if params is None:
            params = self.pick([['int'], ['int'], ['int', 'int'], [], ['peer']])
            if owner.kind == 'interface' and 'peer' in params:
                params = ['int']
        ret = ret or ('String' if self.chance(0.15) else 'int')
        base = name or self.name('g')
        java = base
        key = f'{base}({",".join(params)})' if name else base
        self.groups[key] = {'key': key, 'name': java, 'params': params, 'ret': ret,
                            'rank': self.r.random(), 'owner': owner}
        owner.impls[key] = impl
        return key

    def plan_members(self, c):
        """Which inherited groups `c` implements or overrides, and its Object methods."""
        if c.kind != 'class':
            return
        for key, impl in c.impls.items():
            if impl == 'abstract' and not c.abstract:
                c.impls[key] = 'concrete'
        for key in self.visible_group_names(c):
            if key in c.impls:
                continue
            state = self.implementation(c, key)
            if state is None or state == 'abstract':
                if not c.abstract or self.chance(0.5):
                    c.impls[key] = 'concrete'
            elif self.chance(0.4):
                c.impls[key] = 'concrete'
        c.text = self.chance(0.75 if c.sup is None else 0.4)
        c.equals = self.chance(0.3)
        c.hash = c.equals or self.chance(0.2)

    def implementation(self, c, key):
        """'concrete' when `c` or a superclass implements group `key`, 'abstract' when only
        declared there, None when it comes from an interface without a default."""
        for a in self.ancestors(c):
            if key in a.impls:
                return a.impls[key]
        for i in self.all_ifaces(c):
            if key in i.impls:
                return 'default' if i.impls[key] == 'concrete' else None
        return None

    def overload_chain(self, trees):
        """The classes of one superclass chain, for overloads on the parameter's class."""
        c = self.pick(trees)
        chain = [c]
        while c.subs and self.chance(0.7):
            c = self.pick(c.subs)
            chain.append(c)
        return chain

    # Queries over the world.

    def classes(self):
        return [d for d in self.decls.values() if d.kind == 'class']

    def classes_in_order(self):
        """Superclasses before their subclasses."""
        return sorted(self.classes(), key=lambda d: len(self.ancestors(d)))

    def concretes(self):
        return [d for d in self.classes() if not d.abstract]

    def ancestors(self, c):
        """`c` and its superclasses, nearest first."""
        out = []
        while c is not None:
            out.append(c)
            c = c.sup
        return out

    def descendants(self, c):
        out = []
        for s in c.subs:
            out += [s] + self.descendants(s)
        return out

    def all_ifaces(self, c):
        out = []
        for a in self.ancestors(c):
            for i in a.ifaces:
                if i not in out:
                    out.append(i)
        return out

    def is_sub(self, a, b):
        """Whether a value of type `a` may be assigned to `b` (both without the `?`)."""
        if a == b or b == 'Object':
            return True
        d = self.decls.get(a)
        if d is None:
            return False
        if d.kind == 'interface':
            return False
        return any(x.name == b for x in self.ancestors(d) + self.all_ifaces(d))

    def subclasses_of(self, t):
        """The concrete classes whose objects a value of type `t` may be."""
        return [c for c in self.concretes() if self.is_sub(c.name, t)]

    def visible_group_names(self, d):
        """The method groups callable on a value of type `d`."""
        out = []
        owners = self.ancestors(d) + self.all_ifaces(d) if d.kind == 'class' else [d]
        for o in owners:
            out += [k for k in o.impls if k not in out]
        return out

    def visible_fields(self, d):
        out = []
        for a in reversed(self.ancestors(d)):
            out += a.fields
        return out

    def visible_links(self, d):
        out = []
        for a in reversed(self.ancestors(d)):
            out += a.links
        return out

    def has_text(self, c):
        return any(a.text for a in self.ancestors(c))

    def has_hash(self, c):
        return any(a.hash for a in self.ancestors(c))

    def printable(self, t):
        """Whether printing a value of type `t` shows no identity hash."""
        t = t.lstrip('?')
        if t.endswith('[]'):
            return False
        if t == 'Object':
            return all(self.has_text(c) for c in self.concretes())
        if t in self.decls:
            return all(self.has_text(c) for c in self.subclasses_of(t))
        if '<' in t:
            return self.generic_text and all(map(self.printable, self.split_generic(t)[1]))
        return True  # strings and boxes

    def hashable(self, t):
        t = t.lstrip('?')
        if t in self.decls:
            return all(self.has_hash(c) for c in self.subclasses_of(t))
        return t in BOXES or t == 'String'

    def split_generic(self, t):
        head, rest = t.split('<', 1)
        args, depth, cur = [], 0, ''
        for ch in rest[:-1]:
            if ch == ',' and depth == 0:
                args.append(cur.strip())
                cur = ''
                continue
            depth += ch == '<'
            depth -= ch == '>'
            cur += ch
        args.append(cur.strip())
        return head, args

    def ref_types(self):
        """Types that variables, fields and type arguments of the program may have."""
        out = [d.name for d in self.decls.values()]
        return out + ['Object', 'String', 'Integer']

    def element_types(self):
        return [d.name for d in self.decls.values()] + ['Object']

    def type_arg(self, depth=0):
        r = self.r.random()
        if r < 0.35:
            return self.pick([d.name for d in self.decls.values()])
        if r < 0.55:
            return 'String'
        if r < 0.75:
            return 'Integer'
        if r < 0.85:
            return 'Object'
        if depth == 0 and 'GBox' in self.generics:
            return f'GBox<{self.type_arg(1)}>'
        return 'Integer'

    def generic_type(self):
        heads = [g for g in self.generics if g != 'GNode']
        head = self.pick(heads)
        if head == 'GPair':
            return f'GPair<{self.type_arg()}, {self.type_arg()}>'
        if head == 'GSorted':
            return f'GSorted<{self.comparable_type()}>'
        return f'{head}<{self.type_arg()}>'

    def comparable_type(self):
        options = ['String', 'Integer'] + ([self.comparable.name] if self.comparable else [])
        return self.pick(options)

    def var_types(self):
        """A type for a new variable."""
        r = self.r.random()
        if r < 0.55:
            return self.pick([d.name for d in self.decls.values()])
        if r < 0.7:
            return 'Object'
        return self.generic_type()

    def vars_of(self, t, nullable=False, writable=False):
        """Variables whose values may be assigned to `t`: (name, declared type)."""
        out = []
        for n, vt, w in self.all_vars():
            if writable and not w:
                continue
            bare = vt.lstrip('?')
            if vt.startswith('?') and not nullable or vt.endswith('[]'):
                continue
            if not (bare in self.decls or bare in ('Object', 'String') or '<' in bare):
                continue
            if self.is_sub(bare, t):
                out.append((n, vt))
        return out

    def object_vars(self, nonnull=True):
        """Object variables (no arrays) with their types, those that may be null left out."""
        return [(n, t) for n, t, _ in self.all_vars()
                if (t.lstrip('?') in self.decls or t == 'Object' or '<' in t)
                and not t.endswith('[]') and not (nonnull and t.startswith('?'))]

    # Expressions of reference types. Each returns Java text of a type assignable to `t`.

    def ref_expr(self, t, d, nullable=False, poly=True):
        """`poly`: the expression may be a diamond or a generic call, whose type Java infers
        from the context; only in assignments and returns."""
        if nullable and self.chance(0.15):
            return 'null'
        if t == 'Object':
            return self.object_value(d)
        if t == 'String':
            return self.word(d)
        if t in BOXES:
            return self.box_value(t, d)
        if '<' in t:
            return self.generic_value(t, d, poly)
        names = self.vars_of(t, nullable)
        options = ['new'] * 3
        if names:
            options += ['var'] * 4
        if d > 0:
            options += ['ternary', 'cast', 'element', 'container', 'pick', 'make', 'touch']
            if self.comparable and self.is_sub(self.comparable.name, t) \
                    and 'maxOf' in self.helpers:
                options.append('max')
        kind = self.pick(options)
        text = None
        if kind == 'var':
            return self.pick(names)[0]
        if kind == 'ternary':
            text = f'({self.bool_expr(d - 1)} ? {self.ref_expr(t, d - 1, nullable, False)} : ' \
                   f'{self.ref_expr(t, d - 1, nullable, False)})'
        elif kind == 'cast':
            text = self.guarded_cast(t, d)
        elif kind == 'element':
            arrays = [(n, at) for n, at, _ in self.all_vars()
                      if at.endswith('[]') and not at.startswith('?')
                      and self.is_sub(at[:-2], t)]
            if arrays:
                a = self.pick(arrays)[0]
                text = f'{a}[Math.floorMod({self.expr("int", d - 1)}, {a}.length)]'
        elif kind == 'container':
            text = self.container_read(t, d)
        elif kind == 'pick' and 'pick' in self.helpers:
            text = f'pick({self.bool_expr(d - 1)}, {self.ref_expr(t, d - 1)}, ' \
                   f'{self.ref_expr(t, d - 1)})'
            if not poly:
                text = f'Prog.<{t}>{text}'
        elif kind == 'make':
            roots = [c for c in self.classes() if c.sup is None and self.is_sub(c.name, t)
                     and f'make{c.name}' in self.helpers]
            if roots:
                text = f'make{roots[0].name}({self.expr("int", d - 1)})'
        elif kind == 'touch':
            options = [(n, vt) for n, vt in self.vars_of(t)
                       if vt in self.decls and self.decls[vt].kind == 'class'
                       and self.is_sub(self.root(self.decls[vt]).name, t)]
            if options:
                n, vt = self.pick(options)
                text = f'{n}.touch({self.expr("int", d - 1)})'
        elif kind == 'max':
            k = self.comparable.name
            a, b = self.ref_expr(k, d - 1), self.ref_expr(k, d - 1)
            text = f'maxOf(({k}) {a}, ({k}) {b})'
        return text or self.new_object(t, d)

    def root(self, c):
        return self.ancestors(c)[-1]

    def new_object(self, t, d):
        options = self.subclasses_of(t)
        if not options:
            # An interface no class implements cannot happen: plan_world assigns each one.
            raise ValueError(f'no class for {t}')
        c = self.pick(options)
        sig = self.pick(c.ctors)
        return f'new {c.name}({self.ctor_args(sig, d - 1)})'

    def ctor_args(self, sig, d):
        return ', '.join(self.value_of(p, max(d, 0)) for p in sig)

    def value_of(self, p, d):
        if p == 'String':
            return self.word(d)
        return self.source(p, d)

    def guarded_cast(self, t, d):
        """A downcast that cannot fail: `(x instanceof C ? (C) x : other)`."""
        targets = [c for c in self.classes() if self.is_sub(c.name, t)]
        if not targets:
            return None
        c = self.pick(targets)
        sources = [(n, vt) for n, vt in self.object_vars()
                   if vt != c.name and self.castable(vt, c.name)]
        if not sources:
            return None
        n, _ = self.pick(sources)
        return f'({n} instanceof {c.name} ? ({c.name}) {n} : {self.new_object(c.name, d - 1)})'

    def castable(self, src, dst):
        """Whether `(dst) src` compiles: `dst` below `src`, or either an interface."""
        if '<' in src or '<' in dst:
            return False
        if src == 'Object' or self.is_sub(dst, src):
            return True
        s, t = self.decls.get(src), self.decls.get(dst)
        return s is not None and t is not None and 'interface' in (s.kind, t.kind)

    def word(self, d):
        if self.strings and d > 0 and self.chance(0.5):
            return self.str_expr(min(d, 2))
        names = [n for n, t, _ in self.all_vars() if t == 'String']
        if names and self.chance(0.4):
            return self.pick(names)
        return self.pick(WORDS)

    def box_value(self, t, d):
        if t == 'Integer':
            return self.int_value(d)
        return self.literal(BOXES[t])

    def int_value(self, d):
        """An expression of type int exactly, for boxing: a conditional of a small type and an
        int constant would box as a Character or a Short."""
        text = self.expr('int', d)
        return f'((int) ({text}))' if '?' in text else text

    def object_value(self, d):
        r = self.r.random()
        if r < 0.45:
            return self.ref_expr(self.pick([x.name for x in self.decls.values()]), d)
        if r < 0.6:
            return self.word(d)
        if r < 0.75:
            return self.expr('int', d)
        plain = [n for n, vt in self.object_vars() if '<' not in vt]
        if r < 0.85 and plain:
            return self.pick(plain)
        return self.literal(self.pick(['double', 'char', 'boolean', 'long']))

    def generic_value(self, t, d, poly):
        head, args = self.split_generic(t)
        names = [n for n, vt, _ in self.all_vars() if vt == t]
        if names and self.chance(0.5):
            return self.pick(names)
        diamond = '<>' if poly else f'<{", ".join(args)}>'
        if head == 'GBox':
            if poly and 'boxOf' in self.helpers and self.chance(0.3):
                return f'boxOf({self.ref_expr(args[0], d - 1)})'
            return f'new GBox{diamond}({self.ref_expr(args[0], d - 1, poly=poly)})'
        if head == 'GPair':
            swapped = f'GPair<{args[1]}, {args[0]}>'
            options = [n for n, vt, _ in self.all_vars() if vt == swapped]
            if options and self.chance(0.4):
                return f'{self.pick(options)}.swap()'
            return f'new GPair{diamond}({self.ref_expr(args[0], d - 1, poly=poly)}, ' \
                   f'{self.ref_expr(args[1], d - 1, poly=poly)})'
        return f'new {head}{diamond}()'

    def container_read(self, t, d):
        """A value of type `t` read back out of a generic container."""
        options = []
        for n, vt, _ in self.all_vars():
            if '<' not in vt or vt.startswith('?'):
                continue
            head, args = self.split_generic(vt)
            if head == 'GBox' and self.is_sub(args[0], t):
                options.append(f'{n}.get()')
            elif head == 'GPair':
                if self.is_sub(args[0], t):
                    options.append(f'{n}.first()')
                if self.is_sub(args[1], t):
                    options.append(f'{n}.second()')
            elif head == 'GStack' and self.is_sub(args[0], t):
                options.append(f'({n}.isEmpty() ? {self.ref_expr(t, 0, poly=False)} : '
                               f'{n}.{self.pick(["peek", "pop"])}())')
            elif head == 'GSorted' and self.is_sub(args[0], t):
                options.append(f'({n}.size() == 0 ? {self.ref_expr(t, 0, poly=False)} : '
                               f'{n}.get(Math.floorMod({self.expr("int", d - 1)}, {n}.size())))')
        return self.pick(options) if options else None

    # Expressions of primitive types and strings that read objects.

    def object_num(self, t, d):
        """An int read from an object: a field, a call, a count, a comparison."""
        if t != 'int' or not self.obj_hooks:
            return None
        kind = self.pick(['field', 'field', 'call', 'call', 'static', 'size', 'compare', 'hash',
                          'nullable', 'which', 'unbox'])
        objs = self.object_vars()
        if kind == 'field':
            options = [(n, f) for n, vt in objs if vt in self.decls
                       for f, ft in self.visible_fields(self.decls[vt]) if ft == 'int']
            if options:
                n, f = self.pick(options)
                return f'{n}.{f}'
        elif kind == 'call':
            options = [(n, k) for n, vt in objs if vt in self.decls
                       for k in self.visible_group_names(self.decls[vt])
                       if self.groups[k]['ret'] == 'int']
            if options:
                n, k = self.pick(options)
                return f'{n}.{self.call_text(k, d - 1)}'
        elif kind == 'static':
            return self.static_read(d)
        elif kind == 'size':
            options = [n for n, vt in objs if vt.startswith(('GStack', 'GSorted'))]
            options += [f'{n}.length' for n, vt, _ in self.all_vars()
                        if vt.endswith('[]') and vt.lstrip('?')[:-2] in self.element_types()]
            if options:
                n = self.pick(options)
                return n if n.endswith('.length') else f'{n}.size()'
        elif kind == 'compare':
            return self.compare_text(d)
        elif kind == 'hash':
            options = [n for n, vt in objs if self.hashable(vt)]
            if options:
                return f'{self.pick(options)}.hashCode()'
        elif kind == 'nullable':
            options = [(n, vt[1:]) for n, vt, _ in self.all_vars()
                       if vt.startswith('?') and vt[1:] in self.decls]
            fields = [(n, f) for n, vt in options for f, ft in self.visible_fields(self.decls[vt])
                      if ft == 'int']
            if fields:
                n, f = self.pick(fields)
                return f'({n} == null ? -1 : {n}.{f})'
        elif kind == 'which' and self.chain:
            return f'which({self.which_arg(d - 1)}).length()'
        elif kind == 'unbox':
            options = [f'{n}.get()' for n, vt in objs if vt == 'GBox<Integer>']
            options += [f'{n}.first()' for n, vt in objs
                        if vt.startswith('GPair<Integer,')]
            if options:
                return self.pick(options)
        return None

    def call_text(self, key, d, receiver_type=None):
        g = self.groups[key]
        args = []
        for p in g['params']:
            if p == 'peer':
                args.append(self.ref_expr(self.root(g['owner']).name, d, poly=False))
            elif p in ('long', 'double'):
                args.append(f'(({p}) {self.expr("int", d)})')
            else:
                args.append(self.expr('int', d))
        return f'{g["name"]}({", ".join(args)})'

    def static_read(self, d):
        options = []
        for x in self.decls.values():
            options += [f'{x.name}.{s}' for s in x.statics]
            options += [f'{x.name}.{k}' for k, _ in x.constants]
            options += [f'{x.name}.{m}({self.expr("int", d - 1)})' for m in x.static_methods]
            options += [f'{s.name}.{st}' for a in self.ancestors(x)[1:] for st in a.statics
                        for s in [x]] if x.kind == 'class' else []
        return self.pick(options) if options else None

    def compare_text(self, d):
        kinds = ['string', 'integer'] + (['class'] * 2 if self.comparable else [])
        kind = self.pick(kinds)
        if kind == 'class':
            k = self.comparable.name
            return f'{self.ref_expr(k, d - 1, poly=False)}.compareTo(' \
                   f'{self.ref_expr(k, d - 1, poly=False)})'
        if kind == 'string':
            return f'{self.word(d - 1)}.compareTo({self.word(d - 1)})'
        if 'maxOf' in self.helpers:
            return f'maxOf({self.int_value(d - 1)}, {self.int_value(d - 1)})'
        return None

    def object_bool(self, d):
        if not self.obj_hooks:
            return None
        kind = self.pick(['instanceof', 'instanceof', 'pattern', 'same', 'equals', 'null',
                          'empty'])
        objs = self.object_vars(nonnull=False)
        if kind in ('instanceof', 'pattern'):
            options = [(n, vt, c.name) for n, vt in objs for c in self.classes()
                       if vt.lstrip('?') != c.name and self.castable(vt.lstrip('?'), c.name)]
            options += [(n, vt, b) for n, vt in objs if vt == 'Object' for b in ('String',
                                                                                  'Integer')]
            if not options:
                return None
            n, vt, c = self.pick(options)
            if kind == 'instanceof':
                return f'({n} instanceof {c})'
            p = self.name('q')
            if c == 'String':
                return f'({n} instanceof String {p} && {p}.length() > 1)'
            if c == 'Integer':
                return f'({n} instanceof Integer {p} && {p} > {self.expr("int", d - 1)})'
            fields = [f for f, ft in self.visible_fields(self.decls[c]) if ft == 'int']
            test = f'{p}.{self.pick(fields)} > {self.expr("int", d - 1)}' if fields \
                else f'{p} != null'
            return f'({n} instanceof {c} {p} && {test})'
        if kind == 'same':
            options = [(a, b) for a, at in objs for b, bt in objs
                       if at.lstrip('?') in self.decls and bt.lstrip('?') in self.decls
                       and (self.castable(at.lstrip('?'), bt.lstrip('?'))
                            or self.castable(bt.lstrip('?'), at.lstrip('?')))]
            if options:
                a, b = self.pick(options)
                return f'({a} {self.pick(["==", "!="])} {b})'
        elif kind == 'equals':
            nonnull = self.object_vars()
            if nonnull:
                n, _ = self.pick(nonnull)
                return f'{n}.equals({self.object_value(d - 1)})'
        elif kind == 'null':
            options = [n for n, vt in objs if vt.startswith('?')]
            if options:
                return f'({self.pick(options)} {self.pick(["==", "!="])} null)'
        elif kind == 'empty':
            options = [n for n, vt in objs if vt.startswith('GStack')]
            if options:
                return f'{self.pick(options)}.isEmpty()'
        return None

    def object_text(self, d):
        """A String-typed expression that shows an object."""
        objs = self.object_vars(nonnull=False)
        printable = [n for n, vt in objs if self.printable(vt)]
        r = self.r.random()
        if printable and r < 0.4:
            n = self.pick(printable)
            return n if self.chance(0.6) else f'String.valueOf({n})'
        if r < 0.55 and self.chain:
            return f'which({self.which_arg(d)})'
        if r < 0.7:
            nonnull = [(n, vt) for n, vt in self.object_vars() if vt in self.decls]
            options = [(n, k) for n, vt in nonnull for k in self.visible_group_names(
                self.decls[vt]) if self.groups[k]['ret'] == 'String']
            if options:
                n, k = self.pick(options)
                return f'{n}.{self.call_text(k, d - 1)}'
        if r < 0.85 and 'describe' in self.helpers:
            return f'describe({self.object_value(d - 1)})'
        nonnull = [n for n, vt in self.object_vars() if self.printable(vt)]
        if nonnull:
            return f'{self.pick(nonnull)}.toString()'
        return f'{self.pick(WORDS)}'

    def which_arg(self, d):
        r = self.r.random()
        if r < 0.5:
            c = self.pick(self.chain)
            return self.ref_expr(c.name, d, poly=False)
        if r < 0.7:
            return self.ref_expr(self.pick(self.element_types()), d, poly=False)
        if r < 0.85:
            return self.expr(self.pick(['int', 'long', 'short']), d)
        return self.word(d)

    def field_read(self):
        """A field of any type read through an object variable, or None."""
        options = [(n, f) for n, vt in self.object_vars() if vt in self.decls
                   for f, _ in self.visible_fields(self.decls[vt])]
        if not options:
            return None
        n, f = self.pick(options)
        return f'{n}.{f}'

    def object_part(self, d):
        r = self.r.random()
        text = None
        if r < 0.35:
            text = self.object_text(d)
        elif r < 0.6:
            text = self.object_num('int', d)
        elif r < 0.8:
            text = self.object_bool(d)
        else:
            text = self.field_read()
        return text or self.object_text(d)
