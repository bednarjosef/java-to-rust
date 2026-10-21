// wait with a timeout returns when nothing notifies, notify wakes a waiter, and wait or notify
// without holding the monitor throws IllegalMonitorStateException.
public class WaitTimeout {
    static class Gate {
        private boolean open;

        synchronized boolean awaitOpen(long millis) throws InterruptedException {
            long deadline = System.currentTimeMillis() + millis;
            while (!open) {
                long left = deadline - System.currentTimeMillis();
                if (left <= 0) {
                    return false;
                }
                wait(left);
            }
            return true;
        }

        synchronized void open() {
            open = true;
            notify();
        }
    }

    static class Opener extends Thread {
        private final Gate gate;

        Opener(Gate gate) {
            this.gate = gate;
        }

        @Override
        public void run() {
            try {
                Thread.sleep(50);
            } catch (InterruptedException e) {
                return;
            }
            gate.open();
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Gate shut = new Gate();
        System.out.println("shut gate opened: " + shut.awaitOpen(30));
        Gate gate = new Gate();
        Opener opener = new Opener(gate);
        opener.start();
        System.out.println("gate opened: " + gate.awaitOpen(10000));
        opener.join();
        try {
            gate.notify();
        } catch (IllegalMonitorStateException e) {
            System.out.println("notify without the monitor: " + e.getClass().getName());
        }
        try {
            gate.wait(1);
        } catch (IllegalMonitorStateException e) {
            System.out.println("wait without the monitor: " + e.getClass().getName());
        }
    }
}
