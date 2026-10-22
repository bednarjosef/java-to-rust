package app;

import app.api.Greeter;
import app.data.Point;
import java.util.ServiceLoader;

public class Main {
    interface Unused {
    }

    public static void main(String[] args) throws Exception {
        Module app = Main.class.getModule();
        System.out.println(app.isNamed() + " " + app.getName() + " " + app);
        System.out.println(app.getClassLoader() == ClassLoader.getSystemClassLoader());
        System.out.println(Main.class.getClassLoader() == ClassLoader.getSystemClassLoader());
        System.out.println(String.class.getClassLoader() == null);
        Module base = String.class.getModule();
        System.out.println(base + " " + base.isNamed() + " " + (base.getClassLoader() == null));
        System.out.println((int.class.getModule() == base) + " " + (String[].class.getModule() == base)
                + " " + (Main[].class.getModule() == app) + " " + (Point.class.getModule() == app));
        System.out.println(app.canRead(base) + " " + base.canRead(app) + " " + app.canRead(app));
        System.out.println(base.isExported("java.lang") + " " + base.isExported("jdk.internal.misc") + " "
                + base.isExported("jdk.internal.misc", app) + " " + base.isOpen("java.lang"));
        System.out.println(app.isExported("app.api") + " " + app.isExported("app") + " "
                + app.isOpen("app.data", base) + " " + app.isOpen("app.data") + " " + app.isExported("app.data", base));
        System.out.println(new java.util.TreeSet<>(app.getPackages()));
        System.out.println(app.canUse(Greeter.class) + " " + app.canUse(Unused.class));
        app.addExports("app", base);
        System.out.println(app.isExported("app", base) + " " + app.isExported("app"));
        try {
            app.addExports("nowhere", base);
        } catch (IllegalArgumentException e) {
            System.out.println("IllegalArgumentException: " + e.getMessage());
        }
        app.addUses(Unused.class);
        System.out.println(app.canUse(Unused.class));
        System.out.println(ModuleLayer.boot().findModule("app").get() == app);
        System.out.println(ModuleLayer.boot().findModule("java.base").get() == base);
        System.out.println(ModuleLayer.boot().findModule("no.such.module").isPresent());
        System.out.println(ModuleLayer.boot().modules().contains(app) + " " + (app.getLayer() == ModuleLayer.boot()));
        System.out.println(ModuleLayer.boot().parents().size() + " " + ModuleLayer.empty().modules().isEmpty());
        System.out.println(Class.forName(app, "app.data.Point") == Point.class);
        System.out.println(Class.forName(base, "app.data.Point") == null);
        System.out.println(Class.forName(app, "java.lang.String") == null);
        Module unnamed = ClassLoader.getSystemClassLoader().getUnnamedModule();
        System.out.println(unnamed.isNamed() + " " + unnamed.getName() + " " + (unnamed == app) + " "
                + unnamed.toString().startsWith("unnamed module @") + " " + unnamed.isExported("anything"));
        System.out.println(app.getResourceAsStream("app/none.txt"));
        for (Greeter g : ServiceLoader.load(Greeter.class)) {
            System.out.println(g.greet("module"));
        }
        try {
            ServiceLoader.load(Runnable.class);
        } catch (java.util.ServiceConfigurationError e) {
            System.out.println("ServiceConfigurationError: " + e.getMessage());
        }
    }
}
