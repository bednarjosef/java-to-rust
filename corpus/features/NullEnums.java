// Enum values that may be null: passed as arguments, held in fields that a constructor sets
// after a check or after `super()`, returned, switched on, compared and dereferenced.
public class NullEnums {
    enum Style {
        SHORT,
        LONG;

        String describe() {
            return name().toLowerCase();
        }
    }

    enum Rounding {
        UP,
        DOWN
    }

    // An enum whose constants hold another enum or null, and nothing else changes.
    enum Tone {
        LOW(Style.SHORT),
        NONE(null);

        final Style style;

        Tone(Style style) {
            this.style = style;
        }
    }

    // An enum whose constants hold another enum, or null, which may change.
    enum Size {
        SMALL(Style.SHORT),
        PLAIN(null);

        final Style style;
        Rounding rounding;

        Size(Style style) {
            this.style = style;
        }
    }

    // As `NumberFormat.getInstance(locale, null, NUMBERSTYLE)` passes no style.
    static String format(int value, Style style) {
        if (style == null) {
            return "plain " + value;
        }
        return style.describe() + " " + value;
    }

    static String named(Style style) {
        return "style " + style;
    }

    // As `MathContext` checks its arguments before it sets its rounding mode.
    static class Context {
        final int precision;
        final Rounding rounding;

        Context(int precision, Rounding rounding) {
            if (precision < 0) {
                throw new IllegalArgumentException("Digits < 0");
            }
            if (rounding == null) {
                throw new NullPointerException("null RoundingMode");
            }
            this.precision = precision;
            this.rounding = rounding;
        }

        @Override
        public String toString() {
            return "precision=" + precision + " roundingMode=" + rounding;
        }
    }

    static class Node {
        Node() {}
    }

    // As `Pattern.Curly` sets its type after the implicit `super()`.
    static class Curly extends Node {
        final Style type;

        Curly(Style type) {
            this.type = type;
        }

        int weight() {
            switch (type) {
                case SHORT:
                    return 1;
                default:
                    return 2;
            }
        }
    }

    static Style pick(int i) {
        if (i == 0) {
            return null;
        }
        return i == 1 ? Style.SHORT : Style.LONG;
    }

    static Style chosen;
    static Rounding unset;

    public static void main(String[] args) {
        System.out.println(format(3, null));
        System.out.println(format(4, Style.LONG));
        System.out.println(named(null));
        System.out.println(named(Style.SHORT));
        System.out.println(new Context(5, Rounding.UP));
        try {
            new Context(5, null);
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        System.out.println(new Curly(Style.LONG).weight() + new Curly(Style.SHORT).weight());
        for (int i = 0; i < 3; i++) {
            Style s = pick(i);
            System.out.println(i + ": " + s + " " + (s == Style.SHORT) + " " + (s != null ? s.ordinal() : -1));
        }
        System.out.println(chosen == null);
        chosen = pick(2);
        System.out.println(chosen.describe() + " " + chosen.compareTo(Style.SHORT));
        Style[] styles = {pick(0), pick(1)};
        System.out.println(styles[0] + " " + styles[1].name());
        more();
    }

    static class Holder {
        Style style;
        Rounding rounding = null;

        Holder() {}

        Holder(Style style) {
            this.style = style;
        }

        @Override
        public String toString() {
            return style + "/" + rounding;
        }
    }

    static Style either(boolean b) {
        return b ? null : Style.LONG;
    }

    static int ordinalOf(Style s) {
        switch (s) {
            case SHORT:
                return 10;
            case LONG:
                return 20;
        }
        return 0;
    }

    // The messages of the `NullPointerException`s that a null enum throws.
    static void messages(Style missing) {
        try {
            System.out.println(missing.describe());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(missing.ordinal());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(missing.name());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(Style.LONG.compareTo(missing));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(ordinalOf(missing));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void more() {
        Holder empty = new Holder();
        Holder full = new Holder(Style.SHORT);
        System.out.println(empty + " " + full);
        empty.style = full.style;
        full.style = null;
        System.out.println(empty + " " + full + " " + (empty.style == full.style));
        Object o = either(false);
        Object none = either(true);
        System.out.println(o + " " + none + " " + String.valueOf(either(true)));
        Style cast = (Style) null;
        System.out.println(cast == null);
        Style a = either(true);
        Style b = either(false);
        System.out.println(Style.LONG.equals(b) + " " + Style.LONG.equals(a) + " " + b.equals(a));
        System.out.println(b.toString() + " " + (b.hashCode() == Style.LONG.hashCode()));
        Style[] fresh = new Style[2];
        fresh[1] = Style.SHORT;
        for (Style s : fresh) {
            try {
                System.out.println(ordinalOf(s));
            } catch (NullPointerException e) {
                System.out.println("no style to switch on");
            }
        }
        try {
            System.out.println(a.ordinal());
        } catch (NullPointerException e) {
            System.out.println("no ordinal");
        }
        try {
            System.out.println(a.describe());
        } catch (NullPointerException e) {
            System.out.println("nothing to describe");
        }
        try {
            System.out.println(b.compareTo(a));
        } catch (NullPointerException e) {
            System.out.println("nothing to compare with");
        }
        messages(a);
        for (Size size : Size.values()) {
            System.out.println(size + " " + size.style + " " + size.rounding + " " + (size.style == null));
        }
        for (Tone tone : Tone.values()) {
            System.out.println(tone + " " + tone.style + " " + (tone.style == null));
        }
        Size.SMALL.rounding = Rounding.DOWN;
        System.out.println(Size.SMALL.rounding + " " + Size.PLAIN.rounding);
        System.out.println(unset + " " + (unset == null));
        Runnable r = () -> System.out.println("captured " + a + " " + b);
        r.run();
        System.out.println(new Holder(either(true)).style == null);
    }
}
