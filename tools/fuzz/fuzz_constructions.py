"""Class trees whose superclass constructors need the object being built, for `--objects`.

A constructor of a class with subclasses that uses `this` as a value (stores it in a static or
in another object's field, passes it, compares it) or calls a method that a subclass overrides
runs, in Java, on the whole object while it is being built: the override sees the subclass's
fields still zero, null or false, and the object is reachable from elsewhere before its own
constructor has run. The translator builds such trees zeroed and runs their constructors as
methods on the whole object (`analysis/whole_objects.rs`); these trees exercise that form.

A tree is a chain of one to three classes with subclasses (the root may be generic, with a type
parameter `T` for an item each object carries), with one to three classes without subclasses
hanging off them. Each class with subclasses has constructors that may store `this`, link it
to the previous object, call the overridable `weight()` and `show()`, pass `this` to a static
method, compare it, and throw; some keep the plain form, so a chain mixes both. The classes
without subclasses have fields with initializers that their overrides read, and a second
constructor that delegates with `this(..)`.

The trees are separate from the world of fuzz_objects.py: no world statement sees their types.
`st_obj_build` is the statement of `main` that builds their objects and prints what they hold.
Every call chain ends (an override reads fields and calls only `super` and lower methods), and
every object prints through the root's `toString`.
"""

WORDS = ['"ab"', '"x"', '"hello"', '""', '"q r"']
# The values a constructor's int parameter gets: a negative one may make a constructor throw.
INTS = ['0', '1', '2', '3', '5', '-1', '-4', 'sInt']


class Part:
    """A class of a construction tree."""

    def __init__(self, name, sup):
        self.name = name
        self.sup = sup  # a Part, or None for the root
        self.subs = []
        self.abstract = False
        self.ctors = []  # parameter lists: ('prev' | 'p' | 's' | 'item')
        self.throws = False  # a constructor of this class may throw `Fail`
        self.whole = False  # a constructor uses `this` as a value or calls an override
        self.arg = None  # for a class without subclasses of a generic tree: its type argument
        self.weight = True  # implements `weight()`


class ConstructionGen:
    def construction_decls(self):
        """Plans and emits the construction trees of the program, when it has any."""
        self.parts = []
        if not self.chance(0.6):
            return
        for _ in range(self.r.randint(1, 2)):
            self.parts.append(self.plan_construction())
        for root in self.parts:
            self.emit_construction(root)
        self.kinds = {**self.kinds, 'obj_build': 6}

    # Planning.

    def plan_construction(self):
        root = Part(self.name('Part'), None)
        root.generic = self.chance(0.3)
        root.statics = not root.generic  # `static Part last`, which a generic class cannot have
        root.abstract = self.chance(0.4)
        inner = [root]
        for _ in range(self.r.randint(0, 2)):
            mid = Part(self.name('Mid'), inner[-1])
            mid.abstract = self.chance(0.3)
            inner[-1].subs.append(mid)
            inner.append(mid)
        leaves = [self.plan_leaf(inner[-1], root)]
        for _ in range(self.r.randint(0, 2)):
            leaves.append(self.plan_leaf(self.pick(inner), root))
        # At least one class with subclasses needs the whole object; the others choose.
        for c in inner:
            c.whole = self.chance(0.5)
        if not any(c.whole for c in inner):
            self.pick(inner).whole = True
        for c in inner + leaves:
            c.throws = self.exceptions and self.chance(0.3)
            first = ['p'] + (['prev'] if self.chance(0.5) else []) + (['s'] if self.chance(0.6) else [])
            if root.generic:
                first.append('item')
            self.r.shuffle(first)
            c.ctors = [first]
            if self.chance(0.4):
                c.ctors.append([x for x in first if x != 's'] if 's' in first else first + ['s'])
        # Which classes implement `weight()`: an abstract root may leave it abstract, a class
        # with subclasses below it may override it, and every class without subclasses does.
        root.weight = not root.abstract or self.chance(0.3)
        for mid in inner[1:]:
            mid.weight = self.chance(0.6) or (not mid.abstract and not self.weight_above(mid))
        for leaf in leaves:
            leaf.weight = True
        return root

    def plan_leaf(self, sup, root):
        leaf = Part(self.name('Leaf'), sup)
        sup.subs.append(leaf)
        if root.generic:
            leaf.arg = self.pick(['String', 'Integer', None])  # None: the leaf is generic too
        return leaf

    # Types.

    def part_root(self, c):
        while c.sup:
            c = c.sup
        return c

    def item_type(self, c):
        """The type of the item in class `c`'s body."""
        return c.arg or 'T'

    def root_type(self, c):
        """The type of a reference to the root in class `c`'s body."""
        root = self.part_root(c)
        if not root.generic:
            return root.name
        return f'{root.name}<{self.item_type(c)}>'

    def param_decl(self, c, p):
        return {'prev': f'{self.root_type(c)} prev', 'p': 'int p', 's': 'String s',
                'item': f'{self.item_type(c)} item'}[p]

    # Emitting.

    def emit_construction(self, root):
        for c in [root] + self.part_descendants(root):
            self.emit_part(c, root)

    def part_descendants(self, c):
        out = []
        for s in c.subs:
            out += [s] + self.part_descendants(s)
        return out

    def emit_part(self, c, root):
        head = 'static ' + ('abstract ' if c.abstract else '') + 'class '
        if c.sup is None:
            head += c.name + ('<T>' if root.generic else '')
        else:
            generic_here = root.generic and c.arg is None
            head += c.name + ('<T>' if generic_here else '')
            sup_args = f'<{self.item_type(c)}>' if root.generic else ''
            head += f' extends {c.sup.name}{sup_args}'
        self.emit(head + ' {')
        self.indent += 1
        if c.sup is None:
            self.emit('int id;')
            self.emit('String label;')
            self.emit(f'{self.root_type(c)} prev;')
            self.emit(f'{self.root_type(c)} next;')
            if root.generic:
                self.emit('T item;')
            if root.statics:
                self.emit('static int made;')
                self.emit(f'static {root.name} last;')
        elif c.subs:
            self.emit(f'int extra{self.field_init("int")};')
        else:
            self.emit(f'int bonus{self.field_init("int")};')
            if self.chance(0.6):
                self.emit(f'String tag{self.field_init("String")};')
            else:
                self.emit('String tag;')
            if self.chance(0.4):
                self.emit(f'boolean seen{self.field_init("boolean")};')
            else:
                self.emit('boolean seen;')
        for i, sig in enumerate(c.ctors):
            self.emit_part_ctor(c, root, i, sig)
        self.emit_part_methods(c, root)
        self.indent -= 1
        self.emit('}')

    def field_init(self, t):
        if not self.chance(0.7):
            return ''
        if t == 'int':
            return ' = ' + self.pick(['1', '4', '7', '-2', '(sInt + 1)'])
        if t == 'String':
            return ' = ' + self.pick(['"t"', '"tag"', '"new " + sInt'])
        return ' = true'

    def part_args(self, sup, c, have):
        """The arguments of a `super(..)` or `this(..)` call of a constructor of `sup` with the
        parameters `have` in class `c`."""
        sig = self.pick(sup.ctors)
        args = []
        for p in sig:
            if p in have and self.chance(0.8):
                args.append({'prev': 'prev', 'p': self.pick(['p', 'p + 1', '-p']), 's': 's',
                             'item': 'item'}[p])
            elif p == 'prev':
                args.append('null')
            elif p == 'p':
                args.append(self.pick(['1', '2', '-3']))
            elif p == 's':
                args.append(self.pick(WORDS))
            elif self.item_type(c) == 'String':
                args.append(self.pick(WORDS))
            elif self.item_type(c) == 'Integer':
                args.append(self.pick(['1', '9']))
            else:
                args.append('null')
        return sig, args

    def emit_part_ctor(self, c, root, index, sig):
        params = ', '.join(self.param_decl(c, p) for p in sig)
        self.emit(f'{c.name}({params}) {{')
        self.indent += 1
        delegated = index > 0 and self.chance(0.7)
        if delegated:
            first = [p for p in c.ctors[0]]
            args = []
            for p in first:
                if p in sig:
                    args.append({'prev': 'prev', 'p': 'p', 's': 's', 'item': 'item'}[p])
                else:
                    args.append({'prev': 'null', 'p': '0', 's': self.pick(WORDS),
                                 'item': 'null'}[p])
            self.emit(f'this({", ".join(args)});')
        elif c.sup:
            _, args = self.part_args(c.sup, c, sig)
            self.emit(f'super({", ".join(args)});')
        if c.throws and 'p' in sig and self.chance(0.7):
            self.guarded(self.pick(['p < 0', 'p == 3', 'p < -2']),
                         f'throw new Fail("{c.name} " + p, p);')
        if not delegated:
            self.part_ctor_body(c, root, sig)
        elif self.chance(0.5):
            self.emit(f'System.out.println("{c.name} again " + {self.part_text(c, root)});')
        self.indent -= 1
        self.emit('}')

    def part_ctor_body(self, c, root, sig):
        lines = []
        if c.sup is None:
            if 'p' in sig:
                lines.append('id = p;' if not root.statics or self.chance(0.5) else 'id = p + made;')
            if 's' in sig:
                lines.append('label = s;')
            if 'item' in sig:
                lines.append('this.item = item;')
            if root.statics:
                lines.append('made++;')
        elif c.subs:
            if 'p' in sig and self.chance(0.6):
                lines.append('extra += p;')
        else:
            if 'p' in sig and self.chance(0.6):
                lines.append(self.pick(['bonus += p;', 'bonus = bonus * 2 + p;']))
            if 's' in sig and self.chance(0.5):
                lines.append('tag = s + tag;')
            if self.chance(0.4):
                lines.append('seen = !seen;')
        if c.whole or (not c.subs and self.chance(0.3)):
            lines += self.whole_uses(c, root, sig)
        self.r.shuffle(lines)
        for line in [x for group in lines for x in (group if isinstance(group, list) else [group])]:
            if isinstance(line, tuple):
                self.guarded(*line)
            else:
                self.emit(line)
        if c.throws and self.chance(0.4):
            self.guarded(f'weight() > {self.r.randint(5, 30)}',
                         f'throw new Fail("heavy {c.name} " + weight(), 7);')

    def whole_uses(self, c, root, sig):
        """Statements that need the whole object: at least one of them."""
        options = []
        if root.statics:
            options.append(f'{root.name}.last = this;')
            # Through a local: a null test of a static does not prove it not null afterwards.
            options.append([f'{root.name} before = last;', ('before != null', 'before.next = this;')])
            options.append(('this != last', f'acc += "fresh{c.name.lower()} ";'))
        if 'prev' in sig:
            options.append('this.prev = prev;')
            options.append(('prev != null', 'prev.next = this;'))
            options.append(('prev == this', 'acc += "self ";'))
        options.append('acc += weight() + " ";')
        options.append(f'System.out.println("built {c.name} " + {self.part_text(c, root)});')
        options.append(f'note{self.part_root(c).name}(this);')
        options.append('acc += show() + " ";')
        return self.r.sample(options, self.r.randint(1, min(3, len(options))))

    def part_text(self, c, root):
        return self.pick(['weight()', 'show()', 'this', 'id', 'label'])

    def emit_part_methods(self, c, root):
        if c.sup is None:
            if not c.weight:
                self.emit('abstract int weight();')
            else:
                self.emit('int weight() {')
                self.emit(f'    return id * {self.r.randint(2, 4)};')
                self.emit('}')
            self.emit('String show() {')
            item = ' + "/" + item' if root.generic else ''
            self.emit(f'    return label + "#" + id{item};')
            self.emit('}')
            self.emit('void bump(int k) {')
            self.emit('    id += k;')
            self.emit('}')
            self.emit('@Override')
            self.emit('public String toString() {')
            self.emit(f'    return "{c.name}(" + show() + " w" + weight() + ")";')
            self.emit('}')
            generic = '<U> ' if root.generic else ''
            rt = f'{c.name}<U>' if root.generic else c.name
            self.emit(f'static {generic}void note{c.name}({rt} x) {{')
            self.emit('    acc += "note " + x.weight() + " ";')
            self.emit('}')
            return
        if c.subs:
            if c.weight:
                self.emit('@Override')
                self.emit('int weight() {')
                if self.weight_above(c):
                    self.emit('    return super.weight() + extra;')
                else:
                    self.emit('    return extra + id;')
                self.emit('}')
            return
        self.emit('@Override')
        self.emit('int weight() {')
        base = 'super.weight() + ' if self.weight_above(c) and self.chance(0.6) else ''
        self.emit(f'    return {base}bonus + (seen ? 10 : 0);')
        self.emit('}')
        if self.chance(0.6):
            self.emit('@Override')
            self.emit('String show() {')
            self.emit('    return tag + ":" + super.show();')
            self.emit('}')
        if self.chance(0.4):
            self.emit('@Override')
            self.emit('void bump(int k) {')
            self.emit('    super.bump(k + bonus);')
            self.emit('    bonus--;')
            self.emit('}')

    def weight_above(self, c):
        """Whether a superclass of `c` implements `weight()`, so `super.weight()` compiles."""
        c = c.sup
        while c is not None:
            if c.weight:
                return True
            c = c.sup
        return False

    # The statement of main.

    def concrete_parts(self):
        out = []
        for root in self.parts:
            out += [c for c in [root] + self.part_descendants(root) if not c.abstract]
        return out

    def chain_throws(self, c):
        while c is not None:
            if c.throws:
                return True
            c = c.sup
        return False

    def st_obj_build(self, depth):
        options = self.concrete_parts()
        if not options:
            return self.st_obj_print(depth)
        c = self.pick(options)
        root = self.part_root(c)
        x = self.name('w')
        caught = self.chain_throws(c) and self.chance(0.97)
        if caught:
            self.emit('try {')
            self.indent += 1
        arg = c.arg or (self.pick(['String', 'Integer']) if root.generic else None)
        declared = c.name + (f'<{arg}>' if root.generic and c.arg is None else '')
        made = c.name + ('<>' if root.generic and c.arg is None else '')
        sig = self.pick(c.ctors)
        args = []
        for p in sig:
            if p == 'prev':
                args.append(self.pick([f'{root.name}.last', 'null'] if root.statics else ['null']))
            elif p == 'p':
                args.append(self.pick(INTS))
            elif p == 's':
                args.append(self.pick(WORDS))
            elif arg == 'String':
                args.append(self.pick(WORDS))
            else:
                args.append(self.pick(['1', '9', '-2']))
        if root.generic and 'prev' in sig and self.chance(0.6):
            # The previous object of the same item type, built here.
            prev = self.name('w')
            self.emit(f'{declared} {prev} = new {made}({self.prev_args(sig, arg)});')
            args[sig.index('prev')] = prev
        self.emit(f'{declared} {x} = new {made}({", ".join(args)});')
        for _ in range(self.r.randint(1, 3)):
            self.emit(self.part_print(c, root, x))
        if self.chance(0.4):
            self.emit(f'{x}.bump({self.pick(INTS)});')
            self.emit(f'System.out.println("bumped " + {x});')
        if caught:
            self.indent -= 1
            e = self.name('ex')
            self.emit(f'}} catch (Fail {e}) {{')
            self.emit(f'    System.out.println("Fail " + {e}.getMessage() + " " + {e}.code);')
            self.emit('}')
        if root.statics and self.chance(0.5):
            self.guarded(f'{root.name}.last != null',
                         f'System.out.println("last " + {root.name}.last + " " + {root.name}.made);')

    def prev_args(self, sig, arg):
        out = []
        for p in sig:
            if p == 'prev':
                out.append('null')
            elif p == 'p':
                out.append(self.pick(['1', '2']))
            elif p == 's':
                out.append(self.pick(WORDS))
            elif arg == 'String':
                out.append(self.pick(WORDS))
            else:
                out.append('4')
        return ', '.join(out)

    def part_print(self, c, root, x):
        options = [f'System.out.println({x});',
                   f'System.out.println({x}.weight() + " " + {x}.show());',
                   f'System.out.println("next " + ({x}.next == null) + " prev " + ({x}.prev == null));']
        if root.statics:
            options.append(f'System.out.println({root.name}.last == {x});')
        if not c.subs and c.sup is not None:
            options.append(f'System.out.println({x}.bonus + " " + {x}.tag + " " + {x}.seen);')
        if c.sup is not None:
            options.append(f'System.out.println({x}.prev != null ? "" + {x}.prev : "none");')
        return self.pick(options)
