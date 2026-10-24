package app;

import store.Holder;
import store.Probe;
import store.Sub;
import store.Uses;

/**
 * A program whose library refers to a class that is not on its class path, and runs the code
 * that uses it: each use throws NoClassDefFoundError naming the class, as on the JVM, and the
 * last one is not caught.
 */
public class Main {
    interface Step {
        Object run();
    }

    static void step(String name, Step step) {
        try {
            System.out.println(name + ": " + step.run());
        } catch (Throwable t) {
            System.out.println(name + " threw " + t);
        }
    }

    public static void main(String[] args) {
        System.out.println("available: " + Probe.available());
        System.out.println("cause: " + Probe.cause());
        System.out.println(Probe.describe(false));
        System.out.println(Probe.count(null));
        Runnable later = Probe.later();
        System.out.println("made a runnable");
        try {
            later.run();
        } catch (NoClassDefFoundError e) {
            System.out.println("run: " + e.getMessage());
        }
        step("describe", () -> Probe.describe(true));
        step("event", Probe::event);

        Uses uses = new Uses();
        step("engineName", uses::engineName);
        step("qualified", Uses::qualified);
        step("nested false", () -> Uses.nested(false));
        step("nested true", () -> Uses.nested(true));
        step("caught", () -> store.Catcher.caught());
        step("caught again", () -> store.Catcher.caught());
        step("count", () -> store.Catcher.count());
        step("test", () -> Uses.test("x"));
        step("select 2", () -> Uses.select(2));
        step("select 1", () -> Uses.select(1));
        step("listener", () -> Uses.listener());
        step("sizeOf", () -> Uses.sizeOf(null));
        step("width", () -> Holder.width());
        step("width again", () -> Holder.width());
        step("sub", () -> Sub.describe());
        step("new sub", () -> new Sub().base());
        System.out.println("last");
        System.out.println(Uses.nested(true));
    }
}
