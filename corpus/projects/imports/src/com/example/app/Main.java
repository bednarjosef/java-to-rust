package com.example.app;

import com.example.util.Counter;
import com.example.util.Strings;

import static com.example.util.Numbers.LIMIT;
import static com.example.util.Numbers.square;

/** Two packages: imports, static imports, a same-package class and a fully qualified name. */
public class Main {
    public static void main(String[] args) {
        System.out.println(Strings.repeat("ab", 3));
        System.out.println(Strings.reverse("hello"));
        System.out.println(square(7));
        System.out.println(LIMIT - square(3));
        System.out.println(com.example.util.Numbers.cube(3));
        Counter clicks = new Counter("clicks");
        clicks.add(2);
        clicks.add(5);
        System.out.println(clicks);
        System.out.println(Report.line("total", clicks.total()));
        System.out.println(Report.describe());
        System.out.println(Strings.describe());
    }
}
