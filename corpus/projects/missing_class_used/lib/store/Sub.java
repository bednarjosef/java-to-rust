package store;

import engine.Engine;

/** A class whose superclass is missing: the JVM cannot load it. */
public class Sub extends Engine.Base {
    static int made = 0;

    public Sub() {
        made++;
    }

    public String base() {
        return "sub";
    }

    public static String describe() {
        return "sub " + made;
    }
}
