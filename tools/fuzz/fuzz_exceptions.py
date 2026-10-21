"""Exceptions for the `--objects` programs: the program's own exception classes, methods that
throw them, `try` with `catch` clauses and `finally`, and failures of the runtime that a `catch`
sees (division by zero; and, rarely, a failing cast and a null dereference, which the
translator rejects for now when they are caught).

Every `throw` is behind a condition, so no statement after it is unreachable, and every
`catch` of the checked exception has a call that throws it in its `try`, as javac requires.
"""

# The exception classes, their order from the most specific: a catch of a superclass after a
# catch of its subclass compiles, the other order does not.
EXCEPTION_CLASSES = [
    'static class Fail extends RuntimeException {',
    '    int code;',
    '    Fail(String message, int code) {',
    '        super(message);',
    '        this.code = code;',
    '    }',
    '}',
    'static class Worse extends Fail {',
    '    Worse(String message) {',
    '        super(message + "!", 2);',
    '    }',
    '}',
    'static class Checked extends Exception {',
    '    Checked(String message) {',
    '        super(message);',
    '    }',
    '}',
    'static int risky(int k) throws Checked {',
    '    if (k % 3 == 0) {',
    '        throw new Checked("risky " + k);',
    '    }',
    '    return k + 1;',
    '}',
    'static int shaky(int k) {',
    '    if (Math.floorMod(k, 4) == 1) {',
    '        throw new Worse("shaky " + k);',
    '    }',
    '    if (Math.floorMod(k, 4) == 2) {',
    '        throw new Fail("shaky " + k, k);',
    '    }',
    '    return k * 2;',
    '}',
]


class ExceptionGen:
    def exception_decls(self):
        """Emits the exception classes and the methods that throw, when the program has them."""
        self.exceptions = self.chance(0.6)
        if self.exceptions:
            for line in EXCEPTION_CLASSES:
                self.emit(line)

    def shown(self, e):
        """A text that shows the caught exception `e`."""
        return self.pick([f'{e}.getMessage()', f'"" + {e}', f'"caught"'])

    def st_obj_try(self, depth):
        if not self.exceptions:
            return self.st_obj_print(depth)
        r = self.r.random()
        if r < 0.08:
            return self.caught_cast(depth)
        if r < 0.14:
            return self.caught_null(depth)
        checked = self.chance(0.3)
        self.emit('try {')
        self.frames.append([])
        self.indent += 1
        if checked:
            self.emit(f'acc += risky({self.expr("int", 2)});')
        for _ in range(self.r.randint(1, 2)):
            self.stmt(depth - 1)
        kind = self.pick(['throw', 'shaky', 'divide', 'none'])
        if kind == 'throw':
            which = self.pick(['Fail', 'Worse'])
            args = f'"thrown {self.count}", {self.expr("int", 1)}' if which == 'Fail' \
                else f'"thrown {self.count}"'
            self.guarded(self.bool_expr(2), f'throw new {which}({args});')
        elif kind == 'shaky':
            self.emit(f'System.out.println("shaky " + shaky({self.expr("int", 2)}));')
        elif kind == 'divide':
            self.emit(f'System.out.println("divide " + ({self.expr("int", 2)} / zero0));')
        self.indent -= 1
        self.frames.pop()
        clauses = []
        if self.chance(0.5):
            clauses.append('Worse')
        clauses.append('Fail')
        if checked:
            clauses.append('Checked')
        if kind == 'divide' or self.chance(0.3):
            clauses.append('ArithmeticException')
        if self.chance(0.3):
            clauses.append('RuntimeException')
        for c in clauses:
            e = self.name('ex')
            self.emit(f'}} catch ({c} {e}) {{')
            self.indent += 1
            detail = f' + " " + {e}.code' if c in ('Fail', 'Worse') and self.chance(0.5) else ''
            self.emit(f'System.out.println("{c} " + {self.shown(e)}{detail});')
            self.indent -= 1
            if self.chance(0.3):
                self.block(depth)
        if self.chance(0.3):
            self.emit('} finally {')
            self.indent += 1
            self.emit(f'System.out.println("finally {self.count}");')
            self.indent -= 1
        self.emit('}')

    def caught_cast(self, depth):
        """A failing downcast in a `try` that catches it."""
        pairs = [(a, b) for a in self.concretes() for b in self.classes()
                 if not self.is_sub(a.name, b.name)]
        if not pairs:
            return self.st_obj_print(depth)
        a, b = self.pick(pairs)
        o, c, e = self.name('o'), self.name('c'), self.name('ex')
        self.emit('try {')
        self.indent += 1
        self.emit(f'Object {o} = {self.new_object(a.name, 1)};')
        self.emit(f'{b.name} {c} = ({b.name}) {o};')
        self.emit(f'System.out.println("cast " + ({c} == null));')
        self.indent -= 1
        self.emit(f'}} catch (ClassCastException {e}) {{')
        self.emit('    System.out.println("ClassCastException");')
        self.emit('}')

    def caught_null(self, depth):
        """A null dereference in a `try` that catches it."""
        options = [c for c in self.classes() if self.visible_fields(c)]
        if not options:
            return self.st_obj_print(depth)
        c = self.pick(options)
        n, e = self.name('n'), self.name('ex')
        self.emit('try {')
        self.indent += 1
        self.emit(f'{c.name} {n} = null;')
        self.emit(f'System.out.println({n}.{self.visible_fields(c)[0][0]});')
        self.indent -= 1
        self.emit(f'}} catch (NullPointerException {e}) {{')
        self.emit('    System.out.println("NullPointerException");')
        self.emit('}')

    def throw_uncaught(self):
        """A program exception that nothing catches, where Java exits with code 1."""
        if not self.exceptions:
            return False
        self.guarded('zero0 == 0', f'throw new Fail("uncaught", {self.expr("int", 1)});')
        return True
