public class StaticInitializerThrows {
    static class Config {
        static final int LIMIT = 3;
        static int[] table = build();

        static int[] build() {
            System.out.println("building table");
            return new int[LIMIT];
        }
    }

    static class Broken {
        static int value = compute(4);

        static int compute(int n) {
            System.out.println("computing " + n);
            if (n > 3) {
                throw new IllegalStateException("no value for " + n);
            }
            return n;
        }

        static int twice() {
            return value * 2;
        }
    }

    public static void main(String[] args) {
        System.out.println(Config.LIMIT);
        System.out.println(Config.table.length);
        System.out.println("before");
        System.out.println(Broken.twice());
        System.out.println("unreachable");
    }
}
