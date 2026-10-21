// Class.forName with a class name spelled out: the class's static initializer runs, as the JDK's
// SharedSecrets runs a class's to have it register itself, and the call gives the class.
public class ClassForName {
    static class Holder {
        static String value;
    }

    static class Registry {
        static {
            System.out.println("Registry initialized");
            Holder.value = "set by Registry";
        }
    }

    static class Plain {
        static int count = 1;
    }

    static String lookUp() {
        String value = Holder.value;
        if (value == null) {
            try {
                Class.forName("ClassForName$Registry");
                value = Holder.value;
            } catch (ClassNotFoundException e) {
                value = "not found";
            }
        }
        return value;
    }

    public static void main(String[] args) throws Exception {
        System.out.println("start");
        System.out.println(lookUp());
        System.out.println(lookUp());
        Class<?> plain = Class.forName("ClassForName$Plain");
        System.out.println(plain.getName());
        System.out.println(plain == Plain.class);
        System.out.println(Plain.count);
    }
}
