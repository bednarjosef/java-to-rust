// `Integer.decode` and the other boxed classes' `decode`, which read a sign and a prefix of the
// radix, with Java's messages for what they reject, as `InetAddressCachePolicy` reads the
// lifetimes of its cache.
public class DecodedNumbers {
    public static void main(String[] args) {
        String[] inputs = {
            "10", "-10", "0x1F", "-0X1f", "#ff", "010", "0", "-0", "+7", "", "0x", "-", "+", "--1",
            "0x-1", "-2147483648", "2147483648", "0x80000000", "-0x80000000", "08", "9z", "#",
        };
        for (String s : inputs) {
            try {
                System.out.println("int " + s + " -> " + Integer.decode(s));
            } catch (NumberFormatException e) {
                System.out.println("int " + s + " fails: " + e.getMessage());
            }
            try {
                Long value = Long.decode(s);
                System.out.println("long " + s + " -> " + value);
            } catch (NumberFormatException e) {
                System.out.println("long " + s + " fails: " + e.getMessage());
            }
        }
        for (String s : new String[] {"40000", "-0x8000", "0x7fff"}) {
            try {
                System.out.println("short " + s + " -> " + Short.decode(s));
            } catch (NumberFormatException e) {
                System.out.println("short " + s + " fails: " + e.getMessage());
            }
        }
        for (String s : new String[] {"-0x81", "0177", "#7F"}) {
            try {
                System.out.println("byte " + s + " -> " + Byte.decode(s));
            } catch (NumberFormatException e) {
                System.out.println("byte " + s + " fails: " + e.getMessage());
            }
        }
    }
}
