// A static read while its own class's initialization is under way, before the initializer sets
// it, gives null, wherever that value goes: into a field (the first node of `java.util.regex.Pattern`
// is built while `Pattern.accept` is still null), a return value, or another class's static.
public class StaticReadsDuringInitialization {
    static class Node {
        Node next;
        String name;

        Node(String name) {
            this.name = name;
            next = StaticReadsDuringInitialization.ACCEPT;
        }
    }

    static class Registry {
        static final Node FIRST = StaticReadsDuringInitialization.last();
    }

    static Node last() {
        return LAST;
    }

    static final Node ACCEPT = new Node("accept");
    static final Node SEEN = Registry.FIRST;
    static final Node LAST = new Node("last");

    public static void main(String[] args) {
        System.out.println(ACCEPT.name + " -> " + (ACCEPT.next == null ? "null" : ACCEPT.next.name));
        System.out.println(LAST.name + " -> " + (LAST.next == null ? "null" : LAST.next.name));
        System.out.println("seen " + (SEEN == null) + ", first " + (Registry.FIRST == null));
        Node later = new Node("later");
        System.out.println(later.name + " -> " + later.next.name);
        System.out.println("last " + last().name);
    }
}
