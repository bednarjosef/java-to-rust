import java.util.ArrayList;
import java.util.List;

// The null receiver of a bound method reference, which the reference checks where it is made,
// with no message, and which the class of the reference so never finds null, also where its
// call casts the argument first, as javac's `Attr.bindingEnv` does with
// `bindings.forEach(env1.info.scope::enter)`.
public class NullMessagesOfBoundReferences {
    static class Symbol {
        final String name;

        Symbol(String name) {
            this.name = name;
        }
    }

    static class BindingSymbol extends Symbol {
        BindingSymbol(String name) {
            super(name);
        }
    }

    abstract static class Scope {
        abstract List<String> names();
    }

    abstract static class WriteableScope extends Scope {
        abstract void enter(Symbol s);

        abstract WriteableScope dup();
    }

    static class ScopeImpl extends WriteableScope {
        final List<String> entered = new ArrayList<>();

        void enter(Symbol s) {
            entered.add(s.name);
        }

        WriteableScope dup() {
            return new ScopeImpl();
        }

        List<String> names() {
            return entered;
        }
    }

    static class AttrContext {
        WriteableScope scope;

        AttrContext(WriteableScope scope) {
            this.scope = scope;
        }

        AttrContext dup(WriteableScope scope) {
            return new AttrContext(scope);
        }
    }

    static class Env<A> {
        A info;

        Env(A info) {
            this.info = info;
        }

        Env<A> dup(A info) {
            return new Env<>(info);
        }
    }

    static Env<AttrContext> bindingEnv(Env<AttrContext> env, List<BindingSymbol> bindings) {
        WriteableScope scope = env.info.scope == null ? null : env.info.scope.dup();
        Env<AttrContext> env1 = env.dup(env.info.dup(scope));
        bindings.forEach(env1.info.scope::enter);
        return env1;
    }

    static void run(Runnable r) {
        try {
            r.run();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        List<BindingSymbol> bindings = List.of(new BindingSymbol("x"), new BindingSymbol("y"));
        run(() -> System.out.println(bindingEnv(new Env<>(new AttrContext(new ScopeImpl())), bindings).info.scope.names()));
        run(() -> bindingEnv(new Env<>(new AttrContext(null)), bindings));
        run(() -> bindingEnv(new Env<>(null), bindings));
    }
}
