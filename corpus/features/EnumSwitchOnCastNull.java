public class EnumSwitchOnCastNull {
    enum Color { RED, GREEN }

    static Object pick(int i) {
        return i == 0 ? null : Color.GREEN;
    }

    public static void main(String[] args) {
        for (int i = 0; i < 2; i++) {
            Object value = pick(i);
            try {
                switch ((Color) value) {
                    case RED:
                        System.out.println("red");
                        break;
                    default:
                        System.out.println("other");
                }
            } catch (NullPointerException e) {
                System.out.println(e.getMessage());
            }
        }
    }
}
