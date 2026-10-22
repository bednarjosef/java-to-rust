// Locals declared without a value and assigned in a `try` block that only the block reads, as in
// javac's ClassReader and the JDK's CharPredicates: of an enum and of a class, also in a loop;
// and arrays read after the `try`, as in the JDK's BreakIteratorResourceBundle.
public class TryAssignedLocals {
    enum Script {
        LATIN,
        GREEK;

        static Script named(String name) {
            for (Script s : values()) {
                if (s.name().equalsIgnoreCase(name)) {
                    return s;
                }
            }
            throw new IllegalArgumentException("no script " + name);
        }
    }

    static class Reader {
        final String source;

        Reader(String source) {
            if (source.isEmpty()) {
                throw new IllegalStateException("nothing to read");
            }
            this.source = source;
        }

        String read() {
            return "read " + source;
        }
    }

    static String forScript(String name) {
        final Script script;
        try {
            script = Script.named(name);
            return "script " + script + " " + script.ordinal();
        } catch (IllegalArgumentException e) {
            return "unknown: " + e.getMessage();
        }
    }

    static String readBoth(String first, String second) {
        String a = null;
        String b = null;
        Reader reader;
        try {
            if (first != null) {
                reader = new Reader(first);
                a = reader.read();
            }
            if (second != null) {
                reader = new Reader(second);
                b = reader.read();
            }
        } catch (IllegalStateException e) {
            return "failed: " + e.getMessage();
        }
        return a + " / " + b;
    }

    static String counted(String[] names) {
        StringBuilder out = new StringBuilder();
        for (String name : names) {
            Script script;
            int length;
            try {
                length = name.length();
                script = Script.named(name);
                out.append(script).append(length);
            } catch (IllegalArgumentException e) {
                out.append('?');
            }
        }
        return out.toString();
    }

    static byte[] bytesOf(String text) {
        if (text.isEmpty()) {
            throw new IllegalArgumentException("no bytes");
        }
        return text.getBytes();
    }

    static int sum(String text) {
        byte[] data;
        try {
            data = bytesOf(text);
        } catch (IllegalArgumentException e) {
            throw new IllegalStateException("can't load " + text, e);
        }
        int total = 0;
        for (byte b : data) {
            total += b;
        }
        return total;
    }

    static byte[] kept;

    static int keep(String text) {
        byte[] data;
        try {
            data = bytesOf(text);
        } catch (IllegalArgumentException e) {
            data = new byte[] {1, 2};
        }
        kept = data;
        return kept.length;
    }

    public static void main(String[] args) {
        System.out.println(forScript("greek"));
        System.out.println(forScript("runic"));
        System.out.println(readBoth("x", null));
        System.out.println(readBoth("x", "y"));
        System.out.println(readBoth("x", ""));
        System.out.println(counted(new String[] {"latin", "cyrillic", "Greek"}));
        System.out.println(sum("abc"));
        try {
            System.out.println(sum(""));
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage() + " (" + e.getCause().getMessage() + ")");
        }
        System.out.println(keep("hello"));
        System.out.println(keep(""));
    }
}
