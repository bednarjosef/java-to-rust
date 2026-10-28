// A class with subclasses that waits and notifies on its own monitor, as a promise does: the
// methods that only read its fields still hold the monitor of `this` around `wait`.
public class SubclassedMonitorWait {
    static class Promise {
        private volatile Object result;
        private int waiters;

        boolean isDone() {
            return result != null;
        }

        Promise await() throws InterruptedException {
            synchronized (this) {
                while (!isDone()) {
                    waiters++;
                    try {
                        wait();
                    } finally {
                        waiters--;
                    }
                }
            }
            return this;
        }

        void complete(Object value) {
            synchronized (this) {
                result = value;
                if (waiters > 0) {
                    notifyAll();
                }
            }
        }

        Object get() {
            return result;
        }
    }

    static class NamedPromise extends Promise {
        final String name;

        NamedPromise(String name) {
            this.name = name;
        }
    }

    static void later(Promise p, Object value) {
        new Thread(() -> {
            try {
                Thread.sleep(50);
            } catch (InterruptedException e) {
                return;
            }
            p.complete(value);
        }).start();
    }

    public static void main(String[] args) throws InterruptedException {
        Promise plain = new Promise();
        NamedPromise named = new NamedPromise("named");
        later(plain, "plain done");
        later(named, 42);
        System.out.println(plain.await().get());
        System.out.println(named.name + " " + named.await().get());
    }
}
