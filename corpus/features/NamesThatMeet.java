// Names that meet in Rust though Java's differ, as in javac: enum constants `JDK1_2` and `JDK12`,
// fields `module` and `Module`, a class `Reader.ProxyType` beside an enum `Proxy.Type`, and an
// enum's own `toString(String)` called through an interface.
public class NamesThatMeet {
    enum Source {
        JDK1_2("1.2"),
        JDK12("12");

        final String name;

        Source(String name) {
            this.name = name;
        }
    }

    enum Category {
        CLASSFILE("classfile", false),
        CAST("cast", true, "casts", "casting");

        final String option;
        final int aliases;

        Category(String option, boolean enabled, String... aliases) {
            this.option = option;
            int n = 0;
            for (String alias : aliases) {
                n += alias.length();
            }
            this.aliases = n;
        }
    }

    String module = "module";
    String Module = "Module";

    interface Formattable {
        String toString(String locale);
    }

    enum KindName implements Formattable {
        CLASS;

        public String toString(String locale) {
            return "kind in " + locale;
        }
    }

    static class Text implements Formattable {
        public String toString(String locale) {
            return "text in " + locale;
        }
    }

    public static void main(String[] args) {
        for (Source s : Source.values()) {
            System.out.println(s + " " + s.name);
        }
        for (Category c : Category.values()) {
            System.out.println(c + " " + c.option + " " + c.aliases);
        }
        NamesThatMeet n = new NamesThatMeet();
        n.Module = n.Module + "!";
        System.out.println(n.module + " " + n.Module);
        Object[] objects = {new Reader.ProxyType(), Proxy.Type.HTTP, new Type()};
        for (Object o : objects) {
            System.out.println(o);
        }
        Formattable[] fs = {KindName.CLASS, new Text()};
        for (Formattable f : fs) {
            System.out.println(f.toString("cs"));
        }
    }
}

class Reader {
    static class ProxyType {
        public String toString() {
            return "reader proxy";
        }
    }
}

class Proxy {
    enum Type {
        DIRECT,
        HTTP
    }
}

class Type {
    public String toString() {
        return "type";
    }
}
