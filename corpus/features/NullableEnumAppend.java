// Appending an enum that may be null to a `StringBuilder`, as javac's `TypeAnnotationPosition`
// does with its `type` field: Java appends the enum's text, or "null" for a null, through
// `append(Object)`. Here the enum is a field, a local, an array element and a nullable return.
public class NullableEnumAppend {
    enum Target { FIELD, METHOD, PARAMETER }

    static class Position {
        Target type;
        int index;

        Position(Target type, int index) {
            this.type = type;
            this.index = index;
        }

        @Override
        public String toString() {
            StringBuilder sb = new StringBuilder();
            sb.append('[');
            sb.append(type);
            sb.append(' ');
            sb.append(index);
            sb.append(']');
            return sb.toString();
        }
    }

    static Target find(String name) {
        return name.startsWith("m") ? Target.METHOD : null;
    }

    public static void main(String[] args) {
        System.out.println(new Position(Target.FIELD, 1));
        System.out.println(new Position(null, 2));
        Target[] slots = new Target[3];
        slots[1] = Target.PARAMETER;
        StringBuilder sb = new StringBuilder();
        for (Target slot : slots) {
            sb.append(slot).append(';');
        }
        System.out.println(sb);
        Target found = find("main");
        sb.setLength(0);
        sb.append(found);
        sb.append(',');
        sb.append(find("field"));
        System.out.println(sb.toString());
        System.out.println(new StringBuilder().append("x=").append((Target) null).toString());
        System.out.println("found " + find("main") + " and " + find("none"));
    }
}
