public class StaticInitializerErrorCaught {
    static int zero = 0;

    static class Table {
        static final int SIZE = 100 / zero;
        static int count = 3;

        static int size() {
            return SIZE;
        }
    }

    static class Strict {
        static int level = 1;
        static {
            if (zero == 0) {
                throw new IllegalStateException("no level");
            }
        }

        static void touch() {
            System.out.println("touched " + level);
        }
    }

    static class Fatal {
        static {
            if (zero == 0) {
                throw new AssertionError();
            }
        }

        static void touch() {
            System.out.println("fatal touched");
        }
    }

    public static void main(String[] args) {
        try {
            System.out.println("size " + Table.size());
        } catch (ExceptionInInitializerError e) {
            System.out.println("eiie " + e.getMessage() + " cause " + e.getCause());
        }
        try {
            System.out.println("count " + Table.count);
        } catch (NoClassDefFoundError e) {
            System.out.println("ncdfe " + e.getMessage() + " cause " + e.getCause());
        }
        try {
            Strict.touch();
        } catch (Error e) {
            System.out.println("error " + e);
        }
        try {
            Strict.touch();
        } catch (Throwable e) {
            System.out.println("again " + e.getClass().getName() + " " + e.getMessage());
        }
        try {
            Fatal.touch();
        } catch (ExceptionInInitializerError e) {
            System.out.println("wrong " + e);
        } catch (AssertionError e) {
            System.out.println("assertion " + e.getMessage());
        }
        try {
            Fatal.touch();
        } catch (Throwable e) {
            System.out.println("fatal again " + e.getClass().getName());
        }
        System.out.println("end");
        System.out.println(Table.size());
    }
}
