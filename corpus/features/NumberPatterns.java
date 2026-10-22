/** A `Number` tested for the box it holds, by `instanceof` and by a pattern switch. */
public class NumberPatterns {
    static String kind(Number number) {
        return switch (number) {
            case Double d -> "double " + (d * 2);
            case Long l -> "long " + (l + 1);
            case Integer i -> "int " + (i - 1);
            default -> "other " + number.intValue();
        };
    }

    public static void main(String[] args) {
        Number[] numbers = {1.5, 7L, 42, (short) 3, 2.5f};
        for (Number n : numbers) {
            System.out.println(kind(n));
            if (n instanceof Integer i) {
                System.out.println("an integer " + i);
            }
            System.out.println((n instanceof Float) + " " + (n instanceof Short s && s > 2));
        }
    }
}
