public class StaticInitOrder {
    static int calls = 0;
    static final String TAG = "main";
    static int base = log("base", 10);
    static Settings settings = new Settings(base * 2);

    static int log(String what, int v) {
        calls++;
        System.out.println("init " + what + " " + v + " calls=" + calls);
        return v;
    }

    static class Settings {
        static int created;
        static int seed = log("Settings.seed", 3);
        final int limit;
        Settings(int limit) {
            this.limit = limit + seed;
            created++;
        }
    }

    static class Registry {
        static final int[] TABLE = { 1, 2, 3 };
        static int lookups;
        static int find(int v) {
            lookups++;
            for (int i = 0; i < TABLE.length; i++) {
                if (TABLE[i] == v) return i;
            }
            return -1;
        }
    }

    public static void main(String[] args) {
        System.out.println("main starts " + TAG);
        System.out.println("limit " + settings.limit + " created " + Settings.created);
        Settings second = new Settings(1);
        System.out.println("limit " + second.limit + " created " + Settings.created);
        System.out.println(Registry.find(3) + " " + Registry.find(9) + " lookups " + Registry.lookups);
        Registry.TABLE[0] = 7;
        System.out.println(Registry.find(7) + " " + Registry.find(1));
        System.out.println("calls " + calls);
    }
}
