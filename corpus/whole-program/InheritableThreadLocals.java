// The JDK's InheritableThreadLocal: a new thread starts with its parent's values as they are when
// the thread is made, each through `childValue`, which runs in the parent. A later `set` in either
// thread is not seen by the other, a plain ThreadLocal is not inherited, and a thread made by a
// thread inherits from that one.
public class InheritableThreadLocals {
    static final InheritableThreadLocal<String> name = new InheritableThreadLocal<>();
    static final InheritableThreadLocal<Integer> depth = new InheritableThreadLocal<>() {
        @Override
        protected Integer initialValue() {
            return 0;
        }

        @Override
        protected Integer childValue(Integer parent) {
            System.out.println("childValue " + parent + " in " + Thread.currentThread().getName());
            return parent + 1;
        }
    };
    static final ThreadLocal<String> plain = new ThreadLocal<>();

    public static void main(String[] args) throws InterruptedException {
        Thread before = new Thread(() -> System.out.println("before: " + name.get() + " " + depth.get()));
        name.set("main");
        plain.set("plain");
        depth.set(depth.get());
        String[] inner = new String[1];
        Thread child = new Thread(() -> {
            System.out.println("child: " + name.get() + " " + depth.get() + " " + plain.get());
            name.set("child");
            Thread grandchild = new Thread(() -> inner[0] = name.get() + " " + depth.get(), "grandchild");
            name.set("changed");
            grandchild.start();
            try {
                grandchild.join();
            } catch (InterruptedException e) {
                throw new RuntimeException(e);
            }
        }, "child");
        name.set("main again");
        before.start();
        before.join();
        child.start();
        child.join();
        System.out.println("grandchild: " + inner[0]);
        System.out.println("main: " + name.get() + " " + depth.get());
    }
}
