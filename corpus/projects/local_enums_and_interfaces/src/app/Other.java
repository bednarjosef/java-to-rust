package app;

// A second unit with a local enum of the same simple name as another unit's local type.
class Other {
    static String coin() {
        enum Kind {
            HEADS,
            TAILS { String mark() { return "~"; } };
            String mark() { return "<"; }
        }
        StringBuilder out = new StringBuilder();
        for (Kind c : Kind.values()) {
            String side = switch (c) { case HEADS -> "h"; case TAILS -> "t"; };
            out.append(c).append(side).append(c.mark());
        }
        return out.toString();
    }
}
