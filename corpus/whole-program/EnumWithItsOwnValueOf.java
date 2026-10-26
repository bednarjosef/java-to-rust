// An enum that declares static valueOf methods of its own (netty's HttpStatusClass.valueOf(int)
// and valueOf(CharSequence)) beside the implicit valueOf(String).
public class EnumWithItsOwnValueOf {
    enum StatusClass {
        SUCCESS(200, 300),
        CLIENT_ERROR(400, 500),
        UNKNOWN(0, 0);

        private final int min;
        private final int max;

        StatusClass(int min, int max) {
            this.min = min;
            this.max = max;
        }

        public static StatusClass valueOf(int code) {
            for (StatusClass c : values()) {
                if (code >= c.min && code < c.max) {
                    return c;
                }
            }
            return UNKNOWN;
        }

        public static StatusClass valueOf(CharSequence code) {
            return valueOf(Integer.parseInt(code.toString()));
        }
    }

    public static void main(String[] args) {
        System.out.println(StatusClass.valueOf(204) + " " + StatusClass.valueOf(404) + " " + StatusClass.valueOf(99));
        System.out.println(StatusClass.valueOf("SUCCESS") + " " + StatusClass.valueOf((CharSequence) "450"));
        try {
            StatusClass.valueOf("NOPE");
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
    }
}
