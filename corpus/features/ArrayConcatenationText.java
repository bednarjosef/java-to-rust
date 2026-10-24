// An array added to a string shows as its type and identity hash, as `Object.toString` makes it:
// in `+`, in `+=` and in the `toString` of a record with an array component.
public class ArrayConcatenationText {
    record Message(String text, int[] parts) {}

    public static void main(String[] args) {
        int[] numbers = {1, 2};
        Object object = numbers;
        System.out.println(("a=" + numbers).startsWith("a=[I@"));
        System.out.println(("" + numbers).equals(object.toString()));
        System.out.println(("" + numbers).equals("" + numbers));
        System.out.println(("" + numbers).equals("" + new int[] {1, 2}));
        Message message = new Message("hi", numbers);
        System.out.println(message.toString().equals("Message[text=hi, parts=" + object + "]"));
        String[] words = {"x"};
        System.out.println(("c" + words).startsWith("c[Ljava.lang.String;@"));
        int[][] grid = new int[1][];
        System.out.println((grid + "!").startsWith("[[I@"));
        char[] letters = {'h', 'i'};
        System.out.println(("" + letters).startsWith("[C@"));
        String text = "u";
        text += numbers;
        System.out.println(text.startsWith("u[I@"));
        int[] none = null;
        System.out.println("none=" + none);
        text = "n";
        text += none;
        System.out.println(text);
    }
}
