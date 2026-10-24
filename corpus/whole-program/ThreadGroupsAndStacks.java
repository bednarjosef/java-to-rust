// `ThreadGroup.parentOf` and `isDestroyed`, and a thread's stack: `Thread.dumpStack` prints the
// exception without frames on standard error, and `getStackTrace` is empty, a new array for the
// current thread and the shared empty one for a thread that is not alive.
public class ThreadGroupsAndStacks {
    @SuppressWarnings("removal")
    public static void main(String[] args) throws Exception {
        ThreadGroup child = new ThreadGroup("child");
        ThreadGroup grandchild = new ThreadGroup(child, "grandchild");
        ThreadGroup main = Thread.currentThread().getThreadGroup();
        System.out.println(main.parentOf(child) + " " + main.parentOf(grandchild) + " " + child.parentOf(main));
        System.out.println(child.parentOf(child) + " " + grandchild.parentOf(null) + " " + child.isDestroyed());
        Thread.dumpStack();
        Thread t = new Thread(() -> {});
        StackTraceElement[] unstarted = t.getStackTrace();
        System.out.println(unstarted.length + " " + (unstarted == t.getStackTrace()));
        System.out.println(Thread.currentThread().getStackTrace() == Thread.currentThread().getStackTrace());
        t.start();
        t.join();
        System.out.println(t.getStackTrace().length + " " + (t.getStackTrace() == unstarted));
    }
}
