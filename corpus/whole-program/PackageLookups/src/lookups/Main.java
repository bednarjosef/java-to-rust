package lookups;

import lookups.api.Named;
import lookups.base.Shape;

// `Package.getPackage(name)` of the packages that are loaded wherever it is called: the caller's
// own, its supertypes' (loaded before it) and `java.lang`, each the package that
// `Class.getPackage()` gives. (Any other package may or may not have been loaded yet, so asking
// for it is rejected.)
public class Main extends Shape implements Named {
    static final String HOME = "lookups";

    @Override
    public String name() {
        return "main";
    }

    @SuppressWarnings("deprecation")
    static Package home() {
        return Package.getPackage(HOME);
    }

    @SuppressWarnings("deprecation")
    public static void main(String[] args) {
        Package mine = Package.getPackage("lookups");
        System.out.println(mine.getName() + " " + (mine == Main.class.getPackage()) + " " + (mine == home()));
        Package base = Package.getPackage("lookups.base");
        System.out.println(base.getName() + " " + (base == Shape.class.getPackage()));
        Package api = Package.getPackage("lookups.api");
        System.out.println(api.getName() + " " + (api == Named.class.getPackage()) + " " + api.equals(base));
        Package lang = Package.getPackage("java.lang");
        System.out.println(lang + " " + (lang == Object.class.getPackage()) + " " + lang.getImplementationVersion());

        // A lambda's caller is the class that holds it; an anonymous class's is itself, whose
        // superclass is `Thread`.
        Runnable lambda = () -> System.out.println(Package.getPackage("lookups.api") == api);
        lambda.run();
        Thread thread = new Thread() {
            @Override
            public void run() {
                System.out.println(Package.getPackage("lookups") == mine);
                System.out.println(Package.getPackage("java.lang").getName());
            }
        };
        thread.run();
        System.out.println(new Main().name());
    }
}
