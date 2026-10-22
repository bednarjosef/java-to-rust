public class UnsignedParseNull {
    static String text(int i) {
        return i % 2 == 0 ? null : "ff";
    }

    public static void main(String[] args) {
        for (int i = 0; i < 2; i++) {
            try {
                System.out.println(Integer.parseUnsignedInt(text(i), 16));
                System.out.println(Long.parseUnsignedLong(text(i)));
            } catch (NullPointerException e) {
                System.out.println("NPE " + e.getMessage());
            } catch (NumberFormatException e) {
                System.out.println("NFE " + e.getMessage());
            }
        }
    }
}
