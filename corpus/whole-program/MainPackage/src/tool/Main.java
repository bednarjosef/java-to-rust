package tool;

import tool.main.Option;

// A package named `main`, as javac's com.sun.tools.javac.main, beside the program's own `main`.
public class Main {
    public static void main(String[] args) {
        Object[] all = {Option.VERBOSE, "and", Option.NOWARN};
        for (Object o : all) {
            System.out.println(o instanceof Option option ? option.flag() : o);
        }
    }
}
