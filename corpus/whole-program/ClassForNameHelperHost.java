// `Class.forName` with a computed name becomes one method that lowering adds to the class whose
// code it first meets (here `Host`), and every other call of `Class.forName` calls it too. That
// method stands for the code at its call sites: calling it from `Other` must not initialize
// `Host`, whose static initializer prints, as calling a method of `Host` itself would.
class Host {
    static {
        System.out.println("Host initialized");
    }

    static Class<?> find(String name) throws ClassNotFoundException {
        return Class.forName(name);
    }
}

class Other {
    static Class<?> find(String name) throws ClassNotFoundException {
        return Class.forName(name);
    }
}

class Target {
    static {
        System.out.println("Target initialized");
    }
}

public class ClassForNameHelperHost {
    public static void main(String[] args) throws Exception {
        String target = args.length > 0 ? args[0] : "Target";
        System.out.println(Other.find(target).getName());
        try {
            Other.find(target + "Missing");
        } catch (ClassNotFoundException e) {
            System.out.println("not found");
        }
        System.out.println("before Host");
        System.out.println(Host.find(target).getName());
    }
}
