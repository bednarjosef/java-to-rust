public class StaticInitializerErrorAcrossThreads {
    static int zero = 0;

    static class Table {
        static final int SIZE = 100 / zero;

        static int size() {
            return SIZE;
        }
    }

    static class Worker extends Thread {
        public void run() {
            try {
                System.out.println("size " + Table.size());
            } catch (Throwable e) {
                System.out.println("worker: " + e);
            }
        }
    }

    public static void main(String[] args) throws Exception {
        try {
            System.out.println("size " + Table.size());
        } catch (ExceptionInInitializerError e) {
            System.out.println("first: " + e.getCause());
        }
        Worker w = new Worker();
        w.start();
        w.join();
    }
}
