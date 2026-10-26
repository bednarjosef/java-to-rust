import java.util.function.UnaryOperator;

public class AssignedReceiverMessages {
    String name = "start";
    static String shared;

    void update(UnaryOperator<String> f) {
        (this.name = f.apply(this.name)).length();
    }

    public static void main(String[] args) {
        AssignedReceiverMessages a = new AssignedReceiverMessages();
        try {
            a.update(s -> null);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        String local;
        try {
            (local = shared).length();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            (shared = a.name).isEmpty();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
