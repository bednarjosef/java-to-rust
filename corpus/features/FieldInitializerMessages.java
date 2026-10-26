public class FieldInitializerMessages {
    static String shared;
    static FieldInitializerMessages registry;

    String owner = "x";
    final int length = shared.length();

    static class Holder {
        final String name = registry.owner.trim();
    }

    public static void main(String[] args) {
        try {
            new FieldInitializerMessages();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            new Holder();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
