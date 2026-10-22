// Monitors that main holds when it starts its first thread: two nested ones, one of them entered
// twice, taken while main was the only thread. The worker contends on both, and on the fields of
// the objects they guard, and must wait until main has left them; the counts it reads are those
// main left.
public class MonitorsHeldAcrossStart {
    static class Account {
        int balance;
        int moves;
    }

    static final Account first = new Account();
    static final Account second = new Account();
    static final StringBuilder log = new StringBuilder();

    static synchronized void note(String s) {
        log.append(s).append('\n');
    }

    static void move(Account from, Account to, int amount) {
        synchronized (from) {
            synchronized (to) {
                from.balance -= amount;
                to.balance += amount;
                from.moves++;
                to.moves++;
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        first.balance = 100;
        Thread worker;
        synchronized (first) {
            synchronized (second) {
                synchronized (first) {
                    worker = new Thread(() -> {
                        move(first, second, 30);
                        note("worker moved after main: " + first.balance + " " + second.balance + ", moves " + first.moves);
                    });
                    worker.start();
                    // The worker cannot get past `first` while main is in here.
                    for (int i = 0; i < 5; i++) {
                        first.balance -= 1;
                        second.balance += 1;
                        Thread.sleep(10);
                    }
                    first.moves += 5;
                    second.moves += 5;
                    note("main done: " + first.balance + " " + second.balance + ", moves " + first.moves);
                }
            }
        }
        worker.join();
        System.out.print(log);
        System.out.println("total " + (first.balance + second.balance));
    }
}
