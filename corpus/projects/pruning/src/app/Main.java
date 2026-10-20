package app;

import lib.Greeter;

/** Only what `main` reaches is translated: not the package `extra`, nor unused members of `lib`. */
public class Main {
    public static void main(String[] args) {
        Greeter hello = new Greeter("Hello");
        System.out.println(hello.greet("Ada"));
        System.out.println(new Greeter("Hi").greet("Grace"));
        System.out.println(Greeter.count());
    }
}
