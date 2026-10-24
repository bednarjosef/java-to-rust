package app;

import util.Counter;

public class Main {
    public static void main(String[] args) {
        Counter<String> c = Counter.create("words");
        c.add("a").add("b");
        c.add(null);
        System.out.println(c.count());
        System.out.println(c.label());
        Counter.Visitor v = o -> o != null;
        System.out.println(v.visit("x") + " " + v.visit(null));
        Counter.Pair p = new Counter.Pair("one", 1);
        System.out.println(p.first() + p.second() + " " + Counter.Mode.FAST);
    }
}
